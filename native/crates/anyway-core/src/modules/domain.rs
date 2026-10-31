use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::json;
use std::time::Duration;

pub struct DomainModule;

#[async_trait::async_trait]
impl Module for DomainModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "domain.intel",
            name: "Domain Intelligence",
            description: "RDAP whois, DNS records over DoH and subdomains from crt.sh",
            accepts: &[crate::models::TargetKind::Domain],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let domain = ctx.target.normalized.clone();
        ctx.progress(0, 3).await;

        if let Ok(v) =
            crate::http::fetch_json(&ctx.client, &format!("https://rdap.org/domain/{domain}")).await
        {
            let registrar = v["entities"]
                .as_array()
                .and_then(|es| {
                    es.iter()
                        .find(|e| {
                            e["roles"]
                                .as_array()
                                .map(|r| r.iter().any(|x| x.as_str() == Some("registrar")))
                                .unwrap_or(false)
                        })
                        .and_then(|e| {
                            e["vcardArray"][1]
                                .as_array()
                                .and_then(|props| {
                                    props
                                        .iter()
                                        .find(|p| p.get(0).and_then(|x| x.as_str()) == Some("fn"))
                                })
                                .and_then(|p| p.get(3).and_then(|x| x.as_str()))
                                .map(|s| s.to_string())
                        })
                })
                .unwrap_or_default();
            let events: Vec<(String, String)> = v["events"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|e| {
                            (
                                e["eventAction"].as_str().unwrap_or("?").to_string(),
                                e["eventDate"].as_str().unwrap_or("?").to_string(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let statuses: Vec<String> = v["status"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let nameservers: Vec<String> = v["nameservers"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|n| {
                            n["ldhName"]
                                .as_str()
                                .map(|s| s.trim_end_matches('.').to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            ctx.emit(Finding {
                module_id: "domain.intel".into(),
                category: "domain_rdap".into(),
                label: "RDAP registration".into(),
                value: registrar.clone(),
                url: None,
                confidence: Confidence::High,
                target: None,
                detail: Some(json!({
                    "registrar": registrar, "handle": v["handle"],
                    "statuses": statuses, "nameservers": nameservers,
                    "dnssec": v["secureDNS"]["delegationSigned"], "events": events,
                })),
            })
            .await;
        }
        ctx.progress(1, 3).await;

        let mut records = serde_json::Map::new();
        for rrtype in ["A", "AAAA", "MX", "NS", "TXT"] {
            if ctx.cancelled() {
                return Ok(());
            }
            let vals = crate::http::dns_lookup(&ctx.client, &domain, rrtype)
                .await
                .unwrap_or_default();
            if !vals.is_empty() {
                records.insert(rrtype.to_string(), json!(vals));
            }
        }
        if !records.is_empty() {
            ctx.emit(Finding {
                module_id: "domain.intel".into(),
                category: "dns_records".into(),
                label: "DNS records".into(),
                value: records
                    .iter()
                    .map(|(k, v)| format!("{k}:{}", v.as_array().map(|a| a.len()).unwrap_or(0)))
                    .collect::<Vec<_>>()
                    .join(", "),
                url: None,
                confidence: Confidence::High,
                target: None,
                detail: Some(json!(records)),
            })
            .await;
        }
        ctx.progress(2, 3).await;

        let crt_client = reqwest::Client::builder()
            .user_agent(ctx.options.user_agent.clone())
            .timeout(Duration::from_secs(30))
            .build()
            .expect("crt client");
        if let Ok(v) = crt_client
            .get(format!("https://crt.sh/?q=%.{domain}&output=json"))
            .send()
            .await
        {
            if let Ok(entries) = v.json::<serde_json::Value>().await {
                if let Some(arr) = entries.as_array() {
                    let mut subs: Vec<String> = Vec::new();
                    for e in arr {
                        if let Some(names) = e["name_value"].as_str() {
                            for n in names.split('\n') {
                                let n = n.trim().trim_start_matches("*.").to_lowercase();
                                if n.ends_with(&domain) && n != domain && !subs.contains(&n) {
                                    subs.push(n);
                                }
                            }
                        }
                    }
                    subs.truncate(80);
                    if !subs.is_empty() {
                        ctx.emit(Finding {
                            module_id: "domain.intel".into(),
                            category: "subdomains".into(),
                            label: format!("Subdomains (crt.sh, {})", subs.len()),
                            value: subs.join(", "),
                            url: None,
                            confidence: Confidence::Medium,
                            target: None,
                            detail: Some(json!({"list": subs})),
                        })
                        .await;
                    }
                }
            }
        }
        ctx.progress(3, 3).await;
        Ok(())
    }
}
