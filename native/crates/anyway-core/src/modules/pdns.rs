use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::http::fetch_json;
use crate::models::{Confidence, Finding, TargetKind};
use std::time::Duration;

pub struct PdnsModule;

const DESCRIPTOR: super::ModuleDescriptor = super::ModuleDescriptor {
    id: "pdns",
    name: "Passive DNS & history",
    description: "VirusTotal passive DNS (API key) and Wayback Machine captures for domains: historical IPs, subdomains and archived pages",
    accepts: &[TargetKind::Domain],
};

/// Parse a Wayback CDX JSON payload (first row is the header) into
/// (subdomain, archived URL) pairs.
pub fn parse_cdx(rows: &[serde_json::Value]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if rows.len() < 2 {
        return out;
    }
    for row in rows.iter().skip(1) {
        let url = row.get(1).and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            continue;
        }
        let host = url
            .split("://")
            .nth(1)
            .unwrap_or(url)
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if !host.is_empty() {
            out.push((host, url.to_string()));
        }
    }
    out
}

/// Extract (ip, hostname) pairs from a VirusTotal passive_dns response.
pub fn parse_vt(data: &[serde_json::Value]) -> Vec<(String, String)> {
    data.iter()
        .filter_map(|r| {
            let ip = r["ip_address"].as_str()?.to_string();
            let host = r["hostname"].as_str().unwrap_or("").to_lowercase();
            Some((ip, host))
        })
        .collect()
}

#[async_trait::async_trait]
impl Module for PdnsModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &DESCRIPTOR
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let domain = ctx.target.normalized.clone();
        ctx.progress(0, 2).await;

        // ---- Wayback Machine CDX (no key required) ----
        let wayback_url = format!(
            "http://web.archive.org/cdx/search/cdx?url={domain}/*&output=json&collapse=urlkey&fl=timestamp,original&limit=500"
        );
        let wayback_client = ctx.client.clone();
        if let Ok(v) = fetch_json(&wayback_client, &wayback_url).await {
            if !ctx.cancelled() {
                let rows = v.as_array().cloned().unwrap_or_default();
                let mut hosts: Vec<String> = Vec::new();
                let mut urls: Vec<String> = Vec::new();
                for (host, url) in parse_cdx(&rows) {
                    if host != domain && !hosts.contains(&host) && hosts.len() < 60 {
                        hosts.push(host);
                    }
                    if urls.len() < 40 && !urls.contains(&url) {
                        urls.push(url);
                    }
                }
                ctx.note(&format!(
                    "wayback: {} archived hosts, {} captures sampled",
                    hosts.len(),
                    urls.len()
                ))
                .await;
                for host in &hosts {
                    if ctx.cancelled() {
                        break;
                    }
                    ctx.emit(Finding {
                        module_id: "pdns".into(),
                        category: "pdns_subdomain".into(),
                        label: "Wayback".into(),
                        value: host.clone(),
                        url: None,
                        confidence: Confidence::Low,
                        target: Some(domain.clone()),
                        detail: None,
                    })
                    .await;
                }
                for url in &urls {
                    if ctx.cancelled() {
                        break;
                    }
                    ctx.emit(Finding {
                        module_id: "pdns".into(),
                        category: "archived_url".into(),
                        label: "Archive".into(),
                        value: url.clone(),
                        url: Some(url.clone()),
                        confidence: Confidence::Medium,
                        target: Some(domain.clone()),
                        detail: None,
                    })
                    .await;
                }
            }
        } else {
            ctx.note("wayback: cdx request failed").await;
        }
        ctx.progress(1, 2).await;

        // ---- VirusTotal passive DNS (needs an API key) ----
        let key = ctx.options.vt_api_key.trim().to_string();
        if key.is_empty() {
            ctx.note("virustotal: no API key, passive DNS skipped").await;
        } else if !ctx.cancelled() {
            let vt_client = ctx.client.clone();
            let vt_url = format!("https://www.virustotal.com/api/v3/domains/{domain}/passive_dns");
            let mut builder = vt_client
                .get(&vt_url)
                .header("x-apikey", key)
                .timeout(Duration::from_secs(20));
            builder = builder.header("accept", "application/json");
            match builder.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let v: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
                    let records = v["data"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    let pairs = parse_vt(&records);
                    let mut ips: Vec<String> = Vec::new();
                    for (ip, host) in pairs {
                        if !ips.contains(&ip) && ips.len() < 40 {
                            ips.push(ip.clone());
                        }
                        if host != domain && host.ends_with(&domain) && !ctx.cancelled() {
                            ctx.emit(Finding {
                                module_id: "pdns".into(),
                                category: "pdns_subdomain".into(),
                                label: "VirusTotal".into(),
                                value: host.clone(),
                                url: None,
                                confidence: Confidence::Medium,
                                target: Some(domain.clone()),
                                detail: None,
                            })
                            .await;
                        }
                    }
                    ctx.note(&format!("virustotal: {} passive DNS records", ips.len()))
                        .await;
                    for ip in &ips {
                        if ctx.cancelled() {
                            break;
                        }
                        ctx.emit(Finding {
                            module_id: "pdns".into(),
                            category: "pdns_ip".into(),
                            label: "VirusTotal".into(),
                            value: ip.clone(),
                            url: None,
                            confidence: Confidence::Medium,
                            target: Some(domain.clone()),
                            detail: None,
                        })
                        .await;
                    }
                }
                Ok(resp) => {
                    ctx.note(&format!(
                        "virustotal: api returned {} (check the key or quota)",
                        resp.status()
                    ))
                    .await;
                }
                Err(_) => ctx.note("virustotal: request failed").await,
            }
        }
        ctx.progress(2, 2).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cdx_rows() {
        let rows = serde_json::json!([
            ["timestamp", "original"],
            ["20210101", "http://old.example.org/page1"],
            ["20210201", "https://www.example.org/"],
            ["20210301", "http://example.org/"],
        ]);
        let pairs = parse_cdx(rows.as_array().unwrap());
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].0, "old.example.org");
        assert_eq!(pairs[2].0, "example.org");
        assert_eq!(parse_cdx(&[]).len(), 0);
        assert_eq!(parse_cdx(&[serde_json::json!(["a", "b"])]).len(), 0);
    }

    #[test]
    fn parses_vt_records() {
        let data = serde_json::json!([
            {"ip_address": "1.2.3.4", "hostname": "www.example.org"},
            {"ip_address": "1.2.3.4", "hostname": "example.org"},
            {"ip_address": "5.6.7.8", "hostname": "mail.example.org"},
        ]);
        let pairs = parse_vt(data.as_array().unwrap());
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].0, "1.2.3.4");
        assert_eq!(pairs[2].1, "mail.example.org");
    }
}
