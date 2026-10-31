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
            }
            if d.id == "email.accounts" {
                entry["sites"] = json!(account_site_count);
            }
            entry
        })
        .collect();
    to_ptr(json!({"ok": true, "modules": modules, "sites": site_count}))
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
pub unsafe extern "C" fn anyway_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}
