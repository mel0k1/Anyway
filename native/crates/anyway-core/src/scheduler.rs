use crate::engine::{self, ScanHandle};
use crate::models::ScanOptions;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Mutex as AsyncMutex;

pub const TICK_INTERVAL_SECS: u64 = 15;
pub const MIN_INTERVAL_SECS: u64 = 60;
const POLL_INTERVAL_MS: u64 = 500;
const MAX_STORED_LABELS: usize = 4000;

pub struct SchedulerEntry {
    pub id: String,
    pub query: String,
    pub options: ScanOptions,
    pub interval_secs: u64,
    pub enabled: bool,
    pub runs: u64,
    pub last_run: Option<u64>,
    pub next_run: u64,
    pub running: bool,
    pub last_status: Option<String>,
    pub new_findings: Option<u64>,
    pub last_labels: Vec<String>,
    pub last_snapshot: Option<Value>,
}

impl SchedulerEntry {
    pub fn from_json(v: &Value, now: u64) -> Result<SchedulerEntry, String> {
        let query = v["query"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("query is required")?
            .to_string();
        let interval_secs = (v["interval_secs"].as_u64().unwrap_or(3600)).max(MIN_INTERVAL_SECS);
        let options: ScanOptions = match v.get("options") {
            Some(o) if o.is_object() => {
                serde_json::from_value(o.clone()).map_err(|e| format!("bad options: {e}"))?
            }
            _ => ScanOptions::default(),
        };
        let id = v["id"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map_or_else(|| format!("sched-{}-{}", now, rand_suffix()), String::from);
        let enabled = v["enabled"].as_bool().unwrap_or(true);
        Ok(SchedulerEntry {
            id,
            query,
            options,
            interval_secs,
            enabled,
            runs: 0,
            last_run: None,
            next_run: now + interval_secs,
            running: false,
            last_status: None,
            new_findings: None,
            last_labels: Vec::new(),
            last_snapshot: None,
        })
    }

    pub fn to_json(&self, now: u64) -> Value {
        json!({
            "id": self.id,
            "query": self.query,
            "interval_secs": self.interval_secs,
            "enabled": self.enabled,
            "runs": self.runs,
            "last_run": self.last_run,
            "next_run": self.next_run,
            "running": self.running,
            "last_status": self.last_status,
            "new_findings": self.new_findings,
            "due_in": self.next_run.saturating_sub(now),
            "findings_total": self.last_labels.len(),
        })
    }

    fn labels_from_snapshot(snapshot: &Value) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(findings) = snapshot["findings"].as_array() {
            for f in findings {
                let label = format!(
                    "{}|{}",
                    f["category"].as_str().unwrap_or("?"),
                    f["value"].as_str().unwrap_or("?")
                );
                if !out.contains(&label) {
                    out.push(label);
                }
            }
        }
        out.truncate(MAX_STORED_LABELS);
        out
    }
}

fn rand_suffix() -> u64 {
    use rand::Rng;
    rand::rng().random_range(0..1_000_000)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Count labels present now but absent in the previous run.
pub fn diff_labels(previous: &[String], current: &[String]) -> u64 {
    current.iter().filter(|l| !previous.contains(l)).count() as u64
}

struct Scheduler {
    entries: Mutex<HashMap<String, Arc<AsyncMutex<SchedulerEntry>>>>,
    ticker_started: AtomicBool,
}

fn scheduler() -> &'static Scheduler {
    static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();
    SCHEDULER.get_or_init(|| Scheduler {
        entries: Mutex::new(HashMap::new()),
        ticker_started: AtomicBool::new(false),
    })
}

fn insert(entry: SchedulerEntry) -> Arc<AsyncMutex<SchedulerEntry>> {
    let id = entry.id.clone();
    let arc = Arc::new(AsyncMutex::new(entry));
    scheduler()
        .entries
        .lock()
        .expect("scheduler entries")
        .insert(id, arc.clone());
    arc
}

fn ensure_ticker() {
    let s = scheduler();
    if s.ticker_started.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(TICK_INTERVAL_SECS)).await;
            tick().await;
        }
    });
}

async fn tick() {
    let now = unix_now();
    let due: Vec<Arc<AsyncMutex<SchedulerEntry>>> = {
        let map = scheduler().entries.lock().expect("scheduler entries");
        map.values()
            .filter(|arc| match arc.try_lock() {
                Ok(guard) => guard.enabled && !guard.running && guard.next_run <= now,
                Err(_) => false,
            })
            .cloned()
            .collect()
    };
    for entry in due {
        spawn_run(entry);
    }
}

/// Launch one scheduled scan for `entry` in the background and watch it.
pub fn spawn_run(entry: Arc<AsyncMutex<SchedulerEntry>>) {
    tokio::spawn(run_entry(entry));
}

async fn run_entry(entry: Arc<AsyncMutex<SchedulerEntry>>) {
    let (query, options, interval) = {
        let mut e = entry.lock().await;
        if e.running {
            return;
        }
        e.running = true;
        (e.query.clone(), e.options.clone(), e.interval_secs)
    };
    let scan_id = format!("sched-{}-{}", entry.lock().await.id, unix_now());
    let handle: ScanHandle = engine::launch(scan_id.clone(), query, options);
    let final_snapshot = watch(handle).await;
    let mut e = entry.lock().await;
    e.running = false;
    e.runs += 1;
    e.last_run = Some(unix_now());
    e.next_run = unix_now() + interval;
    if let Some(snap) = &final_snapshot {
        e.last_status = snap["status"].as_str().map(String::from);
        e.last_snapshot = Some(snap.clone());
        if e.last_status.as_deref() == Some("done") {
            let current = SchedulerEntry::labels_from_snapshot(snap);
            e.new_findings = Some(diff_labels(&e.last_labels, &current));
            e.last_labels = current;
        } else {
            // failed or cancelled runs keep the previous baseline
            e.new_findings = None;
        }
    } else {
        e.last_status = Some("failed".into());
        e.new_findings = None;
    }
}

async fn watch(handle: ScanHandle) -> Option<Value> {
    // up to 24h of polling at 500ms
    for _ in 0..172_800u32 {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        {
            let s = handle.state.read().await;
            if s.status == "running" {
                continue;
            }
        }
        return Some(handle.snapshot().await);
    }
    None
}

pub fn restore(entries_json: &Value, now: u64) -> Result<usize, String> {
    let arr = entries_json
        .as_array()
        .ok_or("expected a JSON array of schedule entries")?;
    let mut map = scheduler().entries.lock().expect("scheduler entries");
    map.clear();
    let mut n = 0;
    for v in arr {
        match SchedulerEntry::from_json(v, now) {
            Ok(e) => {
                let id = e.id.clone();
                let arc = Arc::new(AsyncMutex::new(e));
                map.insert(id, arc);
                n += 1;
            }
            Err(err) => return Err(format!("entry {n}: {err}")),
        }
    }
    drop(map);
    ensure_ticker();
    Ok(n)
}

pub fn upsert(v: &Value) -> Result<Value, String> {
    let now = unix_now();
    let entry = SchedulerEntry::from_json(v, now)?;
    let id = entry.id.clone();
    // Preserve run history when the caller re-saves an existing schedule.
    let existing = scheduler()
        .entries
        .lock()
        .expect("scheduler entries")
        .get(&id)
        .cloned();
    if let Some(prev) = existing {
        let prev = prev.blocking_lock();
        let mut e = entry;
        e.runs = prev.runs;
        e.last_run = prev.last_run;
        e.next_run = prev.next_run.max(now + e.interval_secs);
        e.last_status = prev.last_status.clone();
        e.new_findings = prev.new_findings;
        e.last_labels = prev.last_labels.clone();
        e.last_snapshot = prev.last_snapshot.clone();
        insert(e);
    } else {
        insert(entry);
        ensure_ticker();
    }
    Ok(json!({"ok": true, "id": id}))
}

pub fn remove(id: &str) -> bool {
    scheduler()
        .entries
        .lock()
        .expect("scheduler entries")
        .remove(id)
        .is_some()
}

pub fn list() -> Value {
    let now = unix_now();
    let map = scheduler().entries.lock().expect("scheduler entries");
    let entries: Vec<Value> = map
        .iter()
        .map(|(id, arc)| match arc.try_lock() {
            Ok(e) => e.to_json(now),
            Err(_) => json!({"id": id, "running": true}),
        })
        .collect();
    json!({"ok": true, "entries": entries, "now": now})
}

pub fn run_now(id: &str) -> Result<Value, String> {
    let arc = scheduler()
        .entries
        .lock()
        .expect("scheduler entries")
        .get(id)
        .cloned()
        .ok_or("unknown schedule entry")?;
    spawn_run(arc);
    Ok(json!({"ok": true}))
}

pub fn snapshot(id: &str) -> Option<Value> {
    let arc = scheduler()
        .entries
        .lock()
        .expect("scheduler entries")
        .get(id)
        .cloned()?;
    let e = arc.blocking_lock();
    e.last_snapshot.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_json(query: &str, interval: u64) -> Value {
        json!({"query": query, "interval_secs": interval, "enabled": true})
    }

    #[test]
    fn parses_entry_and_defaults() {
        let e = SchedulerEntry::from_json(&entry_json("mel0k1", 0), 1000).unwrap();
        assert_eq!(e.query, "mel0k1");
        assert_eq!(e.interval_secs, MIN_INTERVAL_SECS); // clamped
        assert_eq!(e.next_run, 1000 + MIN_INTERVAL_SECS);
        assert!(e.enabled);
        assert_eq!(e.options.tor_mode.wire(), "auto");
        assert!(e.id.starts_with("sched-"));
        assert!(SchedulerEntry::from_json(&json!({"interval_secs": 60}), 1000).is_err());
    }

    #[test]
    fn keeps_existing_id() {
        let v = json!({"id": "sched-fixed", "query": "x", "interval_secs": 600});
        let e = SchedulerEntry::from_json(&v, 5).unwrap();
        assert_eq!(e.id, "sched-fixed");
    }

    #[test]
    fn diffs_labels() {
        let prev = vec!["a|1".into(), "b|2".into()];
        let cur = vec!["a|1".into(), "c|3".into(), "d|4".into()];
        assert_eq!(diff_labels(&prev, &cur), 2);
        assert_eq!(diff_labels(&[], &cur), 3);
        assert_eq!(diff_labels(&cur, &cur), 0);
    }

    #[test]
    fn extracts_labels_from_snapshot() {
        let snap = json!({"findings": [
            {"category": "social_profile", "value": "https://github.com/x"},
            {"category": "social_profile", "value": "https://github.com/x"},
            {"category": "breach", "value": "Adobe"}
        ]});
        let labels = SchedulerEntry::labels_from_snapshot(&snap);
        assert_eq!(labels.len(), 2);
        assert!(labels.contains(&"breach|Adobe".to_string()));
    }

    #[test]
    fn to_json_shape() {
        let e = SchedulerEntry::from_json(&entry_json("a@b.co", 3600), 100).unwrap();
        let v = e.to_json(150);
        assert_eq!(v["query"], "a@b.co");
        assert_eq!(v["due_in"], 3550);
        assert_eq!(v["runs"], 0);
    }

    #[tokio::test]
    async fn run_entry_records_failed_scan() {
        let opts = ScanOptions {
            proxy: "bad://x".into(), // client build fails -> scan failed
            ..Default::default()
        };
        let e = SchedulerEntry {
            id: "sched-test".into(),
            query: "mel0k1".into(),
            options: opts,
            interval_secs: 3600,
            enabled: true,
            runs: 0,
            last_run: None,
            next_run: 0,
            running: false,
            last_status: None,
            new_findings: None,
            last_labels: vec!["old|label".into()],
            last_snapshot: None,
        };
        let arc = Arc::new(AsyncMutex::new(e));
        run_entry(arc.clone()).await;
        let e = arc.lock().await;
        assert_eq!(e.runs, 1);
        assert!(e.last_run.is_some());
        assert!(e.next_run > 0);
        assert!(!e.running);
        // failed scans keep the previous label baseline and report no new findings
        assert_eq!(e.last_labels, vec!["old|label".to_string()]);
        assert_eq!(e.new_findings, None);
    }
}
