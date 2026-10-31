use crate::detect::detect_target;
use crate::models::{Finding, ModuleProgress, PivotLink, ScanOptions, Target, TargetKind};
use crate::modules::{registry, Module};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub const MAX_TARGETS: usize = 12;
pub const MAX_PIVOT_DEPTH: u32 = 4;
pub const MAX_PIVOT_BUDGET: u32 = 32;

pub fn split_targets(query: &str) -> Vec<String> {
    let mut out: Vec<String> = query
        .split([',', ';', '\n'])
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if out.len() > MAX_TARGETS {
        out.truncate(MAX_TARGETS);
    }
    if out.is_empty() {
        out.push(query.trim().to_string());
    }
    out
}

#[derive(Debug, Clone)]
pub struct ScanState {
    pub scan_id: String,
    pub target: Target,
    pub targets: Vec<Target>,
    pub status: String,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub findings: Vec<Finding>,
    pub modules: Vec<ModuleProgress>,
    pub pivots: Vec<PivotLink>,
    pub log: Vec<String>,
}

#[derive(Clone)]
pub struct ScanHandle {
    pub state: Arc<RwLock<ScanState>>,
    pub cancel: CancellationToken,
}

impl ScanHandle {
    pub async fn snapshot(&self) -> serde_json::Value {
        let s = self.state.read().await;
        let graph = crate::graph::build_graph_with_pivots(&s.targets, &s.findings, &s.pivots);
        json!({
            "scanId": s.scan_id,
            "target": s.target,
            "targets": s.targets,
            "status": s.status,
            "startedAt": s.started_at,
            "finishedAt": s.finished_at,
            "modules": s.modules,
            "findings": s.findings,
            "graph": graph,
            "pivots": s.pivots,
            "log": s.log,
        })
    }

    pub async fn cancel(&self) {
        self.cancel.cancel();
        let mut s = self.state.write().await;
        if s.status == "running" {
            s.status = "cancelled".into();
            s.finished_at = Some(unix_now());
            s.log.push("cancelled by user".into());
        }
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone)]
pub struct ModuleCtx {
    pub target: Target,
    pub client: reqwest::Client,
    pub options: ScanOptions,
    pub cancel: CancellationToken,
    state: Arc<RwLock<ScanState>>,
    row: String,
    row_label: String,
}

fn row_key(id: &str, label: &str) -> String {
    if label.is_empty() {
        id.to_string()
    } else {
        format!("{id}\u{1}{label}")
    }
}

fn row_display(id: &str, label: &str) -> String {
    if label.is_empty() {
        id.to_string()
    } else {
        format!("{id} · {label}")
    }
}

impl ModuleCtx {
    pub async fn progress(&self, done: u64, total: u64) {
        let mut s = self.state.write().await;
        if let Some(m) = s.modules.iter_mut().find(|m| m.key == self.row) {
            m.done = done;
            m.total = total;
        }
    }

    pub async fn emit(&self, mut finding: Finding) {
        if finding.target.is_none() {
            finding.target = Some(self.target.raw.clone());
        }
        let mut s = self.state.write().await;
        if let Some(m) = s.modules.iter_mut().find(|m| m.key == self.row) {
            m.found += 1;
        }
        s.log.push(format!(
            "[{}] + {}",
            row_display(&self.row, &self.row_label),
            finding.label
        ));
        s.findings.push(finding);
    }

    pub async fn note(&self, line: &str) {
        let mut s = self.state.write().await;
        s.log.push(format!(
            "[{}] {}",
            row_display(&self.row, &self.row_label),
            line
        ));
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

fn pivot_label(depth: u32, raw: &str) -> String {
    format!("{}{}", "\u{2192} ".repeat(depth.clamp(1, 6) as usize), raw)
}

async fn run_wave(
    state: &Arc<RwLock<ScanState>>,
    client: &reqwest::Client,
    options: &ScanOptions,
    cancel: &CancellationToken,
    sem: &Arc<tokio::sync::Semaphore>,
    plan: Vec<(Arc<dyn Module>, Target, String)>,
) {
    let mut tasks = Vec::new();
    for (module, target, label) in plan {
        let row = row_key(module.id(), &label);
        let ctx = ModuleCtx {
            row: row.clone(),
            row_label: label,
            target,
            client: client.clone(),
            options: options.clone(),
            cancel: cancel.clone(),
            state: state.clone(),
        };
        let task_state = state.clone();
        let task_cancel = cancel.clone();
        let sem = sem.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await;
            {
                let mut s = task_state.write().await;
                if let Some(m) = s.modules.iter_mut().find(|m| m.key == row) {
                    m.status = "running".into();
                }
            }
            if let Err(e) = module.run(ctx).await {
                let mut s = task_state.write().await;
                s.log
                    .push(format!("[{}] error: {e}", row.replace('\u{1}', " · ")));
            }
            let mut s = task_state.write().await;
            if let Some(m) = s.modules.iter_mut().find(|m| m.key == row) {
                m.status = if task_cancel.is_cancelled() {
                    "cancelled".into()
                } else {
                    "done".into()
                };
            }
        }));
    }
    for t in tasks {
        let _ = t.await;
    }
}

async fn drive(
    state: Arc<RwLock<ScanState>>,
    client: reqwest::Client,
    modules: Vec<Arc<dyn Module>>,
    options: ScanOptions,
    cancel: CancellationToken,
    initial: Vec<(Arc<dyn Module>, Target, String)>,
) {
    let sem = Arc::new(tokio::sync::Semaphore::new(
        options.concurrency.max(8) as usize
    ));

    run_wave(&state, &client, &options, &cancel, &sem, initial).await;

    let mut budget = if options.pivot {
        options.pivot_budget.min(MAX_PIVOT_BUDGET) as usize
    } else {
        0
    };
    let max_depth = options.pivot_depth.clamp(1, MAX_PIVOT_DEPTH);
    let mut depth: u32 = 0;

    while options.pivot && !cancel.is_cancelled() && depth < max_depth && budget > 0 {
        let (findings, scanned) = {
            let s = state.read().await;
            (
                s.findings.clone(),
                s.targets
                    .iter()
                    .map(|t| (t.kind, t.normalized.clone()))
                    .collect::<HashSet<_>>(),
            )
        };
        let candidates = crate::pivot::plan(&findings, &scanned, budget);
        if candidates.is_empty() {
            break;
        }
        depth += 1;
        budget -= candidates.len();

        let mut wave: Vec<(Arc<dyn Module>, Target, String)> = Vec::new();
        {
            let mut s = state.write().await;
            for c in candidates {
                s.targets.push(c.target.clone());
                s.pivots.push(PivotLink {
                    parent: c.parent.clone(),
                    parent_kind: c.parent_kind,
                    child: c.target.clone(),
                    depth,
                    via: c.via.clone(),
                });
                s.log.push(format!(
                    "pivot wave {depth}: + {} ({}) via {}",
                    c.target.raw, c.via, c.parent
                ));
                for m in &modules {
                    if m.accepts(c.target.kind) {
                        let label = pivot_label(depth, &c.target.raw);
                        wave.push((m.clone(), c.target.clone(), label.clone()));
                    }
                }
            }
            for (m, _t, label) in &wave {
                let key = row_key(m.id(), label);
                if !s.modules.iter().any(|row| row.key == key) {
                    s.modules.push(ModuleProgress {
                        id: m.id().to_string(),
                        label: label.clone(),
                        key,
                        done: 0,
                        total: 1,
                        status: "queued".into(),
                        found: 0,
                    });
                }
            }
        }
        run_wave(&state, &client, &options, &cancel, &sem, wave).await;
    }

    let mut s = state.write().await;
    if s.status == "running" {
        s.status = if cancel.is_cancelled() {
            "cancelled".into()
        } else {
            "done".into()
        };
        s.finished_at = Some(unix_now());
    }
}

pub fn launch(scan_id: String, query: String, options: ScanOptions) -> ScanHandle {
    let raw = query.trim().to_string();
    let tokens = split_targets(&raw);
    let targets: Vec<Target> = tokens.iter().map(|t| detect_target(t)).collect();
    let multi = targets.len() > 1;
    let display = Target {
        kind: targets
            .first()
            .map(|t| t.kind)
            .unwrap_or(TargetKind::Unknown),
        raw: raw.clone(),
        normalized: raw.to_lowercase(),
    };

    let client = crate::http::build_client(&options);
    let network_note = if options.proxy.trim().is_empty() {
        "network: direct".to_string()
    } else {
        "network: via proxy".to_string()
    };
    let pivot_note = if options.pivot {
        format!(
            "pivot: enabled (depth {}, budget {})",
            options.pivot_depth.clamp(1, MAX_PIVOT_DEPTH),
            options.pivot_budget.min(MAX_PIVOT_BUDGET)
        )
    } else {
        "pivot: disabled".to_string()
    };
    let tail_note = if options.long_tail {
        "database: core + long tail".to_string()
    } else {
        "database: core".to_string()
    };

    let all = registry();
    let enabled: Vec<&str> = if options.enabled_modules.is_empty() {
        all.iter().map(|m| m.id()).collect()
    } else {
        options.enabled_modules.iter().map(|s| s.as_str()).collect()
    };

    let mut rows: Vec<ModuleProgress> = Vec::new();
    let mut plan: Vec<(Arc<dyn Module>, Target, String)> = Vec::new();
    let mut modules: Vec<Arc<dyn Module>> = Vec::new();
    for module in &all {
        if !enabled.contains(&module.id()) {
            continue;
        }
        modules.push(module.clone());
        let accepted: Vec<Target> = targets
            .iter()
            .filter(|t| module.accepts(t.kind))
            .cloned()
            .collect();
        if accepted.is_empty() {
            rows.push(ModuleProgress {
                id: module.id().to_string(),
                label: String::new(),
                key: module.id().to_string(),
                done: 0,
                total: 1,
                status: "skipped".into(),
                found: 0,
            });
            continue;
        }
        for t in accepted {
            let label = if multi { t.raw.clone() } else { String::new() };
            rows.push(ModuleProgress {
                id: module.id().to_string(),
                key: row_key(module.id(), &label),
                label: label.clone(),
                done: 0,
                total: 1,
                status: "queued".into(),
                found: 0,
            });
            plan.push((module.clone(), t, label));
        }
    }

    let failed = client.is_err();
    let status = if failed {
        "failed".into()
    } else {
        "running".into()
    };
    let mut log = vec![
        format!("target detected: {:?}", display.kind),
        format!("targets: {}", targets.len()),
        network_note,
        pivot_note,
        tail_note,
    ];
    if let Err(e) = &client {
        log.push(format!("network error: {e}"));
    }

    let state = Arc::new(RwLock::new(ScanState {
        scan_id: scan_id.clone(),
        target: display,
        targets,
        status,
        started_at: unix_now(),
        finished_at: if failed { Some(unix_now()) } else { None },
        findings: Vec::new(),
        modules: rows,
        pivots: Vec::new(),
        log,
    }));
    let cancel = CancellationToken::new();
    let handle = ScanHandle {
        state: state.clone(),
        cancel: cancel.clone(),
    };

    if let Ok(client) = client {
        tokio::spawn(drive(state.clone(), client, modules, options, cancel, plan));
    }

    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_truncates_targets() {
        let out = split_targets("a@b.co, example.org ; +7 900 000-00-00\nmel0k1");
        assert_eq!(out.len(), 4);
        let big = split_targets(&vec!["x"; MAX_TARGETS + 5].join(","));
        assert_eq!(big.len(), MAX_TARGETS);
    }

    #[test]
    fn pivot_label_marks_depth() {
        assert_eq!(pivot_label(1, "x@y.co"), "\u{2192} x@y.co");
        assert_eq!(pivot_label(2, "dev.org"), "\u{2192} \u{2192} dev.org");
        assert_eq!(pivot_label(0, "x"), "\u{2192} x");
    }

    #[test]
    fn row_keys_are_label_scoped() {
        assert_eq!(row_key("m", ""), "m");
        assert_ne!(row_key("m", "a"), row_key("m", "b"));
        assert_eq!(row_display("m", ""), "m");
        assert_eq!(row_display("m", "x"), "m \u{b7} x");
    }
}
