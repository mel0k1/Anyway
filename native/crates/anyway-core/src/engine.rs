use crate::detect::detect_target;
use crate::models::{Finding, ModuleProgress, ScanOptions, Target, TargetKind};
use crate::modules::{registry, Module};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub const MAX_TARGETS: usize = 12;

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
        json!({
            "scanId": s.scan_id,
            "target": s.target,
            "targets": s.targets,
            "status": s.status,
            "startedAt": s.started_at,
            "finishedAt": s.finished_at,
            "modules": s.modules,
            "findings": s.findings,
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

    let all = registry();
    let enabled: Vec<&str> = if options.enabled_modules.is_empty() {
        all.iter().map(|m| m.id()).collect()
    } else {
        options.enabled_modules.iter().map(|s| s.as_str()).collect()
    };

    let mut rows: Vec<ModuleProgress> = Vec::new();
    let mut plan: Vec<(Arc<dyn Module>, Target)> = Vec::new();
    for module in &all {
        if !enabled.contains(&module.id()) {
            continue;
        }
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
                label,
                done: 0,
                total: 1,
                status: "queued".into(),
                found: 0,
            });
            plan.push((module.clone(), t));
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
        log,
    }));
    let cancel = CancellationToken::new();
    let handle = ScanHandle {
        state: state.clone(),
        cancel: cancel.clone(),
    };

    if let Ok(client) = client {
        let sem = Arc::new(tokio::sync::Semaphore::new(
            options.concurrency.max(8) as usize
        ));
        let mut tasks = Vec::new();

        for (module, target) in plan {
            let label = if multi {
                target.raw.clone()
            } else {
                String::new()
            };
            let ctx = ModuleCtx {
                row: row_key(module.id(), &label),
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
            let row = ctx.row.clone();
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

        tokio::spawn(async move {
            for t in tasks {
                let _ = t.await;
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
        });
    }

    handle
}
