#![allow(clippy::missing_safety_doc)]

use anyway_core::{registry, ScanHandle, ScanOptions, ScanRequest};
use serde_json::json;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static MANAGER: OnceLock<Mutex<HashMap<String, ScanHandle>>> = OnceLock::new();
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime")
    })
}

fn manager() -> &'static Mutex<HashMap<String, ScanHandle>> {
    MANAGER.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn read_str(ptr: *const c_char) -> String {
    CStr::from_ptr(ptr).to_string_lossy().into_owned()
}

fn to_ptr(value: serde_json::Value) -> *mut c_char {
    CString::new(value.to_string())
        .unwrap_or_else(|_| CString::new("{\"ok\":false,\"error\":\"encode\"}").expect("literal"))
        .into_raw()
}

fn err_ptr(msg: &str) -> *mut c_char {
    to_ptr(json!({"ok": false, "error": msg}))
}

#[no_mangle]
pub extern "C" fn anyway_version() -> *mut c_char {
    to_ptr(json!({"ok": true, "version": env!("CARGO_PKG_VERSION")}))
}

#[no_mangle]
pub extern "C" fn anyway_module_list() -> *mut c_char {
    let site_count = anyway_core::modules::sites::sites().len() as u64;
    let core_count = anyway_core::modules::sites::core_site_count() as u64;
    let account_site_count = anyway_core::modules::email_accounts::SITES.len() as u64;
    let modules: Vec<serde_json::Value> = registry()
        .iter()
        .map(|m| {
            let d = m.descriptor();
            let mut entry = json!({
                "id": d.id,
                "name": d.name,
                "description": d.description,
                "accepts": d.accepts.iter().map(|k| serde_json::to_value(k).unwrap()).collect::<Vec<_>>(),
            });
            if d.id == "username.enumerate" {
                entry["sites"] = json!(site_count);
                entry["coreSites"] = json!(core_count);
            }
            if d.id == "email.accounts" {
                entry["sites"] = json!(account_site_count);
            }
            entry
        })
        .collect();
    let categories: Vec<serde_json::Value> = anyway_core::modules::sites::category_counts()
        .into_iter()
        .map(|(name, count)| json!({"name": name, "count": count}))
        .collect();
    let countries: Vec<serde_json::Value> = anyway_core::modules::sites::country_counts()
        .into_iter()
        .map(|(name, count)| json!({"name": name, "count": count}))
        .collect();
    to_ptr(json!({
        "ok": true,
        "modules": modules,
        "sites": site_count,
        "coreSites": core_count,
        "categories": categories,
        "countries": countries,
    }))
}

#[no_mangle]
pub extern "C" fn anyway_browser_detect() -> *mut c_char {
    let browser = anyway_core::browser::find_browser();
    to_ptr(json!({
        "ok": true,
        "browser": browser.map(|p| p.display().to_string()),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scan_start(request: *const c_char) -> *mut c_char {
    let body = read_str(request);
    let req: ScanRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => return err_ptr(&format!("bad request: {e}")),
    };
    if req.query.trim().is_empty() {
        return err_ptr("empty query");
    }
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scan_id = format!(
        "scan-{}-{n}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let options: ScanOptions = req.options;
    let handle = {
        let _guard = runtime().enter();
        anyway_core::engine::launch(scan_id.clone(), req.query, options)
    };
    manager()
        .lock()
        .expect("manager")
        .insert(scan_id.clone(), handle);
    to_ptr(json!({"ok": true, "scanId": scan_id}))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scan_poll(scan_id: *const c_char) -> *mut c_char {
    let id = read_str(scan_id);
    let handle = manager().lock().expect("manager").get(&id).cloned();
    match handle {
        Some(h) => {
            let snapshot = runtime().block_on(h.snapshot());
            to_ptr(json!({"ok": true, "snapshot": snapshot}))
        }
        None => err_ptr("unknown scan"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scan_cancel(scan_id: *const c_char) -> *mut c_char {
    let id = read_str(scan_id);
    let handle = manager().lock().expect("manager").get(&id).cloned();
    match handle {
        Some(h) => {
            runtime().block_on(h.cancel());
            to_ptr(json!({"ok": true}))
        }
        None => err_ptr("unknown scan"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scan_pivot(
    scan_id: *const c_char,
    node: *const c_char,
) -> *mut c_char {
    let id = read_str(scan_id);
    let node_id = read_str(node);
    let handle = manager().lock().expect("manager").get(&id).cloned();
    match handle {
        Some(h) => match runtime().block_on(anyway_core::engine::manual_pivot(&h, &node_id)) {
            Ok(target) => to_ptr(json!({"ok": true, "target": target})),
            Err(e) => err_ptr(&e),
        },
        None => err_ptr("unknown scan"),
    }
}

// ---------- Tor ----------

#[no_mangle]
pub unsafe extern "C" fn anyway_tor_check(options_json: *const c_char) -> *mut c_char {
    let body = read_str(options_json);
    let options: ScanOptions = serde_json::from_str(&body).unwrap_or_default();
    let check = runtime().block_on(anyway_core::tor::check(&options));
    let mut v = check.to_json();
    v["ok"] = json!(true);
    to_ptr(v)
}

// ---------- Imaging ----------

#[no_mangle]
pub unsafe extern "C" fn anyway_image_intel(payload: *const c_char) -> *mut c_char {
    let body = read_str(payload);
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return err_ptr(&format!("bad payload: {e}")),
    };
    let Some(b64) = v["b64"].as_str() else {
        return err_ptr("payload needs 'b64' image bytes");
    };
    let ocr = v["ocr"].as_bool().unwrap_or(false);
    let langs = v["langs"].as_str().unwrap_or("eng");
    match runtime().block_on(anyway_core::imaging::intel_bytes_base64(b64, ocr, langs)) {
        Ok(out) => to_ptr(out),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_ocr_available() -> *mut c_char {
    let v = runtime().block_on(anyway_core::imaging::ocr_available());
    to_ptr(v)
}

#[no_mangle]
pub unsafe extern "C" fn anyway_reverse_links(url: *const c_char) -> *mut c_char {
    to_ptr(anyway_core::imaging::reverse_links(&read_str(url)))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_image_compare(
    hash_a: *const c_char,
    hash_b: *const c_char,
) -> *mut c_char {
    let (a, b) = (read_str(hash_a), read_str(hash_b));
    let (Ok(a), Ok(b)) = (
        serde_json::from_str::<serde_json::Value>(&a),
        serde_json::from_str::<serde_json::Value>(&b),
    ) else {
        return err_ptr("bad hash json");
    };
    match anyway_core::imaging::compare_hashes(&a, &b) {
        Ok(cmp) => {
            let mut v = cmp.to_json();
            v["ok"] = json!(true);
            to_ptr(v)
        }
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_image_index_add(
    index: *const c_char,
    entry: *const c_char,
) -> *mut c_char {
    let (Ok(idx), Ok(ent)) = (
        serde_json::from_str::<serde_json::Value>(&read_str(index)),
        serde_json::from_str::<serde_json::Value>(&read_str(entry)),
    ) else {
        return err_ptr("bad index or entry json");
    };
    match anyway_core::imaging::index_add(&idx, ent) {
        Ok(new_index) => to_ptr(json!({"ok": true, "index": new_index})),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_image_index_search(
    index: *const c_char,
    probe: *const c_char,
    threshold: u32,
    limit: u32,
) -> *mut c_char {
    let (Ok(idx), Ok(probe)) = (
        serde_json::from_str::<serde_json::Value>(&read_str(index)),
        serde_json::from_str::<serde_json::Value>(&read_str(probe)),
    ) else {
        return err_ptr("bad index or probe json");
    };
    let mut out = anyway_core::imaging::index_search(&idx, &probe, threshold, limit as usize)
        .unwrap_or_else(|e| json!({"ok": false, "error": e}));
    if out["ok"].is_null() {
        out["ok"] = json!(true);
    }
    to_ptr(out)
}

// ---------- Entity resolution ----------

#[no_mangle]
pub unsafe extern "C" fn anyway_resolve_entities(
    snapshot: *const c_char,
    threshold: u32,
) -> *mut c_char {
    let Ok(snap) = serde_json::from_str::<serde_json::Value>(&read_str(snapshot)) else {
        return err_ptr("bad snapshot json");
    };
    let report = anyway_core::resolve::resolve(&snap, threshold);
    to_ptr(report.to_json(threshold))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_build_timeline(snapshot: *const c_char) -> *mut c_char {
    let Ok(snap) = serde_json::from_str::<serde_json::Value>(&read_str(snapshot)) else {
        return err_ptr("bad snapshot json");
    };
    to_ptr(anyway_core::resolve::build_timeline(&snap))
}

// ---------- Leak dumps ----------

fn leaks_task(
    f: impl FnOnce() -> Result<serde_json::Value, String> + Send + 'static,
) -> *mut c_char {
    let res = runtime().block_on(async move {
        tokio::task::spawn_blocking(f)
            .await
            .unwrap_or_else(|e| Err(format!("task join failed: {e}")))
    });
    match res {
        Ok(v) => to_ptr(v),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_leaks_import(payload: *const c_char) -> *mut c_char {
    let body = read_str(payload);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        return err_ptr("bad payload json");
    };
    let db_path = v["db_path"].as_str().unwrap_or("").to_string();
    let source = v["source"].as_str().unwrap_or("").to_string();
    let text = v["text"].as_str().unwrap_or("").to_string();
    leaks_task(move || anyway_core::leaks::import_text(&db_path, &source, &text))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_leaks_search(payload: *const c_char) -> *mut c_char {
    let body = read_str(payload);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        return err_ptr("bad payload json");
    };
    let db_path = v["db_path"].as_str().unwrap_or("").to_string();
    let query = v["query"].as_str().unwrap_or("").to_string();
    let limit = v["limit"].as_u64().unwrap_or(200) as usize;
    let reveal = v["reveal"].as_bool().unwrap_or(false);
    leaks_task(move || anyway_core::leaks::search(&db_path, &query, limit, reveal))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_leaks_stats(db_path: *const c_char) -> *mut c_char {
    let path = read_str(db_path);
    leaks_task(move || anyway_core::leaks::stats(&path))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_leaks_clear(db_path: *const c_char) -> *mut c_char {
    let path = read_str(db_path);
    leaks_task(move || anyway_core::leaks::clear(&path))
}

// ---------- Scan scheduler ----------

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_set(entries_json: *const c_char) -> *mut c_char {
    let body = read_str(entries_json);
    let _guard = runtime().enter();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("bad json: {e}"))
        .and_then(|v| anyway_core::scheduler::restore(&v, now))
    {
        Ok(n) => to_ptr(json!({"ok": true, "restored": n})),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_list() -> *mut c_char {
    to_ptr(anyway_core::scheduler::list())
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_upsert(entry_json: *const c_char) -> *mut c_char {
    let body = read_str(entry_json);
    let _guard = runtime().enter();
    match serde_json::from_str::<serde_json::Value>(&body)
        .map_err(|e| format!("bad json: {e}"))
        .and_then(|v| anyway_core::scheduler::upsert(&v))
    {
        Ok(v) => to_ptr(v),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_remove(id: *const c_char) -> *mut c_char {
    let removed = anyway_core::scheduler::remove(&read_str(id));
    to_ptr(json!({"ok": removed}))
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_run_now(id: *const c_char) -> *mut c_char {
    let _guard = runtime().enter();
    match anyway_core::scheduler::run_now(&read_str(id)) {
        Ok(v) => to_ptr(v),
        Err(e) => err_ptr(&e),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_scheduler_snapshot(id: *const c_char) -> *mut c_char {
    let id = read_str(id);
    match anyway_core::scheduler::snapshot(&id) {
        Some(snap) => to_ptr(json!({"ok": true, "snapshot": snap})),
        None => err_ptr("no stored snapshot for this entry"),
    }
}

#[no_mangle]
pub unsafe extern "C" fn anyway_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}
