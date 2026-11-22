use crate::detect::detect_target;
use crate::models::{Finding, ModuleProgress, PivotLink, ScanOptions, Target, TargetKind, TorMode};
use crate::modules::{registry, Module};
use serde_json::json;
use std::collections::HashSet;
use std::net::IpAddr;
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
    pub options: ScanOptions,
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

    if options.pivot {
        pivot_loop(&state, &client, &modules, &options, &cancel, &sem).await;
    }

    finalize(&state, &cancel).await;
}

fn enabled_modules(options: &ScanOptions) -> Vec<Arc<dyn Module>> {
    let all = registry();
    let enabled: Vec<&str> = if options.enabled_modules.is_empty() {
        all.iter().map(|m| m.id()).collect()
    } else {
        options.enabled_modules.iter().map(|s| s.as_str()).collect()
    };
    all.into_iter()
        .filter(|m| enabled.contains(&m.id()))
        .collect()
}

async fn pivot_loop(
    state: &Arc<RwLock<ScanState>>,
    client: &reqwest::Client,
    modules: &[Arc<dyn Module>],
    options: &ScanOptions,
    cancel: &CancellationToken,
    sem: &Arc<tokio::sync::Semaphore>,
) {
    let mut budget = options.pivot_budget.min(MAX_PIVOT_BUDGET) as usize;
    let max_depth = options.pivot_depth.clamp(1, MAX_PIVOT_DEPTH);

    while !cancel.is_cancelled() && budget > 0 {
        let (findings, scanned, depth) = {
            let s = state.read().await;
            let depth = s.pivots.iter().map(|p| p.depth).max().unwrap_or(0);
            (
                s.findings.clone(),
                s.targets
                    .iter()
                    .map(|t| (t.kind, t.normalized.clone()))
                    .collect::<HashSet<_>>(),
                depth,
            )
        };
        if depth >= max_depth {
            break;
        }
        let candidates = crate::pivot::plan(&findings, &scanned, budget);
        if candidates.is_empty() {
            break;
        }
        budget -= candidates.len();
        let depth = depth + 1;

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
                for m in modules {
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
        run_wave(state, client, options, cancel, sem, wave).await;
    }
}

async fn finalize(state: &Arc<RwLock<ScanState>>, cancel: &CancellationToken) {
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

pub fn parse_pivot_node(node_id: &str) -> Result<Target, String> {
    let Some((kind_str, value)) = node_id.split_once(':') else {
        return Err("malformed node id".into());
    };
    let value = value.trim();
    if value.is_empty() || value.len() > 253 {
        return Err("empty node value".into());
    }
    let kind = match kind_str {
        "Username" => TargetKind::Username,
        "Email" => TargetKind::Email,
        "Domain" => TargetKind::Domain,
        "Ip" => TargetKind::IpAddress,
        "Phone" => TargetKind::PhoneNumber,
        _ => return Err("this node kind cannot be pivoted".into()),
    };
    let valid = match kind {
        TargetKind::Username => match value.split_once('@') {
            Some((local, instance)) => {
                valid_handle_local(local) && crate::graph::domain_from_url(instance).is_some()
            }
            None => valid_handle_local(value),
        },
        TargetKind::Email => match value.split_once('@') {
            Some((local, domain)) => {
                local.len() >= 2
                    && domain.contains('.')
                    && value
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || ".@_-+".contains(c))
            }
            None => false,
        },
        TargetKind::Domain => crate::graph::domain_from_url(value).is_some_and(|h| h == value),
        TargetKind::IpAddress => value
            .parse::<IpAddr>()
            .map(crate::pivot::routable_ip)
            .unwrap_or(false),
        TargetKind::PhoneNumber => value.chars().filter(|c| c.is_ascii_digit()).count() >= 7,
        _ => false,
    };
    if !valid {
        return Err("node value is not a scannable entity".into());
    }
    Ok(Target {
        kind,
        raw: value.to_string(),
        normalized: value.to_lowercase(),
    })
}

fn valid_handle_local(local: &str) -> bool {
    local.len() >= 2
        && local.chars().next().is_some_and(|c| c.is_alphanumeric())
        && local
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
}

async fn continue_scan(state: Arc<RwLock<ScanState>>, cancel: CancellationToken, target: Target) {
    let (options, modules) = {
        let s = state.read().await;
        (s.options.clone(), enabled_modules(&s.options))
    };
    if cancel.is_cancelled() {
        finalize(&state, &cancel).await;
        return;
    }
    let client = match crate::http::build_client(&options) {
        Ok(c) => c,
        Err(e) => {
            let mut s = state.write().await;
            s.status = "failed".into();
            s.finished_at = Some(unix_now());
            s.log.push(format!("network error: {e}"));
            return;
        }
    };
    let sem = Arc::new(tokio::sync::Semaphore::new(
        options.concurrency.max(8) as usize
    ));
    let label = pivot_label(1, &target.raw);
    let mut wave: Vec<(Arc<dyn Module>, Target, String)> = Vec::new();
    {
        let mut s = state.write().await;
        for m in &modules {
            if !m.accepts(target.kind) {
                continue;
            }
            let key = row_key(m.id(), &label);
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
            wave.push((m.clone(), target.clone(), label.clone()));
        }
    }
    run_wave(&state, &client, &options, &cancel, &sem, wave).await;
    pivot_loop(&state, &client, &modules, &options, &cancel, &sem).await;
    finalize(&state, &cancel).await;
}

pub async fn manual_pivot(handle: &ScanHandle, node_id: &str) -> Result<Target, String> {
    let target = parse_pivot_node(node_id)?;
    {
        let mut s = handle.state.write().await;
        if s.status == "running" {
            return Err("scan is still running".into());
        }
        if s.status == "cancelled" {
            return Err("scan was cancelled, start a new scan".into());
        }
        if s.targets
            .iter()
            .any(|t| t.kind == target.kind && t.normalized == target.normalized)
        {
            return Err("already scanned".into());
        }
        let depth = s.pivots.iter().map(|p| p.depth).max().unwrap_or(0) + 1;
        s.pivots.push(PivotLink {
            parent: target.normalized.clone(),
            parent_kind: target.kind,
            child: target.clone(),
            depth,
            via: "manual".into(),
        });
        s.targets.push(target.clone());
        s.log.push(format!(
            "manual pivot: + {} ({:?})",
            target.raw, target.kind
        ));
        s.status = "running".into();
        s.finished_at = None;
    }
    let state = handle.state.clone();
    let cancel = handle.cancel.clone();
    let seed = target.clone();
    tokio::spawn(continue_scan(state, cancel, seed));
    Ok(target)
}

pub fn launch(scan_id: String, query: String, mut options: ScanOptions) -> ScanHandle {
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

    let listening = crate::tor::tor_listening(&options);
    let requested_proxy = options.proxy.clone();
    let effective = options.effective_proxy(listening);
    let explicit_proxy = !requested_proxy.trim().is_empty();
    if !explicit_proxy {
        options.proxy = effective;
    }
    let network_note = if !explicit_proxy && options.tor_mode != TorMode::Off {
        if options.proxy == options.tor_proxy_url() {
            format!(
                "network: Tor (mode {}, port {}) via {}",
                options.tor_mode.wire(),
                options.tor_socks_port,
                options.tor_proxy_url()
            )
        } else if options.tor_mode == TorMode::On {
            format!(
                "network: Tor (mode on) requested but port {} is not listening - requests will fail closed",
                options.tor_socks_port
            )
        } else {
            "network: direct (Tor unreachable in auto mode)".to_string()
        }
    } else if options.proxy.trim().is_empty() {
        "network: direct".to_string()
    } else {
        "network: via proxy".to_string()
    };
    let client = crate::http::build_client(&options);
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

    let modules = enabled_modules(&options);

    let mut rows: Vec<ModuleProgress> = Vec::new();
    let mut plan: Vec<(Arc<dyn Module>, Target, String)> = Vec::new();
    for module in &modules {
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
        options: options.clone(),
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

    #[test]
    fn parses_pivot_node_ids() {
        let t = parse_pivot_node("Username:alice").unwrap();
        assert_eq!(t.kind, TargetKind::Username);
        assert_eq!(t.normalized, "alice");
        let t = parse_pivot_node("Username:user@fosstodon.org").unwrap();
        assert_eq!(t.kind, TargetKind::Username);
        assert_eq!(t.normalized, "user@fosstodon.org");
        assert!(parse_pivot_node("Email:jane@corp.io").is_ok());
        assert!(parse_pivot_node("Domain:corp.io").is_ok());
        assert!(parse_pivot_node("Ip:8.8.8.8").is_ok());
        assert!(parse_pivot_node("Phone:+79261234567").is_ok());
    }

    #[test]
    fn rejects_unscannable_pivot_nodes() {
        assert!(parse_pivot_node("no-separator").is_err());
        assert!(parse_pivot_node("Platform:github").is_err());
        assert!(parse_pivot_node("Lead:GitHub code search").is_err());
        assert!(parse_pivot_node("Username:a").is_err());
        assert!(parse_pivot_node("Username:").is_err());
        assert!(parse_pivot_node("Email:notanemail").is_err());
        assert!(parse_pivot_node("Ip:192.168.1.4").is_err());
        assert!(parse_pivot_node("Domain:notadomain").is_err());
    }

    #[tokio::test]
    async fn manual_pivot_states_and_duplicates() {
        let options = ScanOptions {
            proxy: "bad://x".into(),
            ..Default::default()
        };
        let handle = launch("scan-test".into(), "mel0k1".into(), options);
        assert!(manual_pivot(&handle, "Platform:github").await.is_err());
        assert!(manual_pivot(&handle, "Username:mel0k1").await.is_err());
        let pivots_before = handle.state.read().await.pivots.len();
        let t = manual_pivot(&handle, "Username:alice").await.unwrap();
        assert_eq!(t.normalized, "alice");
        {
            let s = handle.state.read().await;
            assert!(s.status == "running" || s.status == "failed");
            assert_eq!(s.pivots.len(), pivots_before + 1);
            assert!(s
                .targets
                .iter()
                .any(|x| x.kind == TargetKind::Username && x.normalized == "alice"));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(manual_pivot(&handle, "Username:alice").await.is_err());
    }
}
