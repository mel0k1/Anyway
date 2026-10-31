use anyway_core::{engine, ScanOptions};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let opts = ScanOptions {
        concurrency: 16,
        timeout_secs: 8,
        ..Default::default()
    };
    let handle = engine::launch("smoke".into(), std::env::args().nth(1).unwrap_or_else(|| "mel0k1".into()), opts);
    loop {
        sleep(Duration::from_millis(400)).await;
        let snap = handle.snapshot().await;
        let status = snap["status"].as_str().unwrap_or("").to_string();
        println!(
            "status={} findings={} log_tail={:?}",
            status,
            snap["findings"].as_array().map(|a| a.len()).unwrap_or(0),
            snap["log"].as_array().map(|a| a.iter().rev().take(3).cloned().collect::<Vec<_>>()).unwrap_or_default()
        );
        if status != "running" {
            println!("{}", serde_json::to_string_pretty(&snap).unwrap());
            break;
        }
    }
}
