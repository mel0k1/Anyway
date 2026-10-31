use crate::detect::detect_target;
use crate::models::{Finding, ModuleProgress, ScanOptions, Target};
use crate::modules::registry;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ScanState {
    pub scan_id: String,
    pub target: Target,
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
    module_id: String,
}

impl ModuleCtx {
    pub async fn progress(&self, done: u64, total: u64) {
        let mut s = self.state.write().await;
        if let Some(m) = s.modules.iter_mut().find(|m| m.id == self.module_id) {
            m.done = done;
            m.total = total;
        }
    }

    pub async fn emit(&self, finding: Finding) {
        let mut s = self.state.write().await;
        if let Some(m) = s.modules.iter_mut().find(|m| m.id == self.module_id) {
            m.found += 1;
        }
        s.log.push(format!("[{}] + {}", self.module_id, finding.label));
        s.findings.push(finding);
    }

    pub async fn note(&self, line: &str) {
        let mut s = self.state.write().await;
        s.log.push(format!("[{}] {}", self.module_id, line));
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

pub fn launch(scan_id: String, query: String, options: ScanOptions) -> ScanHandle {
    let target = detect_target(&query);
    let state = Arc::new(RwLock::new(ScanState {
        scan_id: scan_id.clone(),
        target: target.clone(),
        status: "running".into(),
        started_at: unix_now(),
        finished_at: None,
        findings: Vec::new(),
        modules: Vec::new(),
        log: vec![format!("target detected: {:?}", target.kind)],
    }));
    let cancel = CancellationToken::new();
    let handle = ScanHandle {
        state: state.clone(),
        cancel: cancel.clone(),
    };

    let all = registry();
    let enabled: Vec<&str> = if options.enabled_modules.is_empty() {
        all.iter().map(|m| m.id()).collect()
    } else {
        options.enabled_modules.iter().map(|s| s.as_str()).collect()
    };
    let applicable: Vec<_> = all
        .into_iter()
        .filter(|m| enabled.contains(&m.id()))
        .filter(|m| m.accepts(target.kind))
        .collect();

    tokio::spawn(async move {
        let all_ids: Vec<String> = registry().iter().map(|m| m.id().to_string()).collect();
        {
            let mut s = state.write().await;
            s.modules = all_ids
                .iter()
                .map(|id| ModuleProgress {
                    id: id.clone(),
                    done: 0,
                    total: 1,
                    status: if applicable.iter().any(|m| m.id() == id) { "queued".into() } else { "skipped".into() },
                    found: 0,
                })
                .collect();
        }

        let client = crate::http::build_client(&options);
        let sem = Arc::new(tokio::sync::Semaphore::new(options.concurrency.max(8) as usize));
        let mut tasks = Vec::new();

        for module in applicable {
            let task_state = state.clone();
            let task_cancel = cancel.clone();
            let ctx = ModuleCtx {
                target: target.clone(),
                client: client.clone(),
                options: options.clone(),
                cancel: cancel.clone(),
                state: state.clone(),
                module_id: module.id().to_string(),
            };
            let sem = sem.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await;
                {
                    let mut s = task_state.write().await;
                    if let Some(m) = s.modules.iter_mut().find(|m| m.id == module.id()) {
                        m.status = "running".into();
                    }
                }
                if let Err(e) = module.run(ctx).await {
                    let mut s = task_state.write().await;
                    s.log.push(format!("[{}] error: {e}", module.id()));
                }
                let mut s = task_state.write().await;
                if let Some(m) = s.modules.iter_mut().find(|m| m.id == module.id()) {
                    m.status = if task_cancel.is_cancelled() { "cancelled".into() } else { "done".into() };
                }
            }));
        }

        for t in tasks {
            let _ = t.await;
        }

        let mut s = state.write().await;
        if s.status == "running" {
            s.status = if cancel.is_cancelled() { "cancelled".into() } else { "done".into() };
            s.finished_at = Some(unix_now());
        }
    });

    handle
}
