use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::json;
use std::net::IpAddr;
use std::time::Duration;

pub struct DomainModule;

const CF_V4: &[(u32, u32)] = &[
    (0xADF53000, 20),
    (0x6715F400, 22),
    (0x6716C800, 22),
    (0x671F0400, 22),
    (0x8D654000, 18),
    (0x6CA2C000, 18),
    (0xBE5DF000, 20),
    (0xBC726000, 20),
    (0xC5EAF000, 22),
    (0xC6298000, 17),
    (0xA29E0000, 15),
    (0x68100000, 13),
    (0x68180000, 14),
    (0xAC400000, 13),
    (0x83004800, 22),
];

pub fn is_cloudflare_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let n = u32::from(*v4);
            CF_V4
                .iter()
                .any(|(base, bits)| (n >> (32 - bits)) == (base >> (32 - bits)))
        }
        IpAddr::V6(v6) => {
            let seg = v6.octets();
            let p0 = u16::from_be_bytes([seg[0], seg[1]]);
            let p1 = u16::from_be_bytes([seg[2], seg[3]]);
            (p0 == 0x2400 && p1 & 0xFF00 == 0xCB00)
                || (p0 == 0x2606 && p1 == 0x4700)
                || (p0 == 0x2803 && p1 == 0xF800)
                || (p0 == 0x2405 && (p1 == 0xB500 || p1 == 0x8100))
                || (p0 == 0x2A06 && p1 & 0xFFF8 == 0x98C0)
                || (p0 == 0x2C0F && p1 == 0xF248)
        }
    }
}

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
        ctx.progress(0, 5).await;

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
        ctx.progress(1, 5).await;

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
        ctx.progress(2, 5).await;

        let a_records: Vec<IpAddr> = records
            .get("A")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().and_then(|s| s.parse().ok()))
                    .collect()
            })
            .unwrap_or_default();
        if !a_records.is_empty() {
            let cf_hits: Vec<String> = a_records
                .iter()
                .filter(|ip| is_cloudflare_ip(ip))
                .map(|ip| ip.to_string())
                .collect();
            let ips_json: Vec<String> = a_records.iter().map(|ip| ip.to_string()).collect();
            if !cf_hits.is_empty() {
                ctx.emit(Finding {
                    module_id: "domain.intel".into(),
                    category: "cdn_detection".into(),
                    label: "Cloudflare in front".into(),
                    value: format!(
                        "{} of {} A records are Cloudflare ({})",
                        cf_hits.len(),
                        ips_json.len(),
                        cf_hits.join(", ")
                    ),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(json!({
                        "cloudflare": true,
                        "cfIps": cf_hits,
                        "allIps": ips_json,
                        "hint": "origin IP may surface in historic crt.sh certs, PTR records or DNS archives",
                    })),
                })
                .await;
            } else {
                ctx.emit(Finding {
                    module_id: "domain.intel".into(),
                    category: "cdn_detection".into(),
                    label: "No known CDN".into(),
                    value: format!("A records resolve directly: {}", ips_json.join(", ")),
                    url: None,
                    confidence: Confidence::Medium,
                    target: None,
                    detail: Some(json!({"cloudflare": false, "allIps": ips_json})),
                })
                .await;
            }

            let mut ptrs: Vec<(String, String)> = Vec::new();
            for ip in a_records.iter().take(6) {
                if ctx.cancelled() {
                    return Ok(());
                }
                if let Ok(names) = crate::http::reverse_dns(&ctx.client, &ip.to_string()).await {
                    if let Some(first) = names.first() {
                        ptrs.push((ip.to_string(), first.clone()));
                    }
                }
            }
            if !ptrs.is_empty() {
                ctx.emit(Finding {
                    module_id: "domain.intel".into(),
                    category: "ip_ptr".into(),
                    label: "Reverse DNS of A records".into(),
                    value: ptrs
                        .iter()
                        .map(|(ip, ptr)| format!("{ip} → {ptr}"))
                        .collect::<Vec<_>>()
                        .join("; "),
                    url: None,
                    confidence: Confidence::Medium,
                    target: None,
                    detail: Some(json!({"ptrs": ptrs.iter().map(|(ip, ptr)| json!({"ip": ip, "ptr": ptr})).collect::<Vec<_>>()})),
                })
                .await;
            }
        }
        ctx.progress(3, 5).await;

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
                    let mut certs: Vec<serde_json::Value> = Vec::new();
                    let mut seen_certs: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    for e in arr {
                        if let Some(names) = e["name_value"].as_str() {
                            for n in names.split('\n') {
                                let n = n.trim().trim_start_matches("*.").to_lowercase();
                                if n.ends_with(&domain) && n != domain && !subs.contains(&n) {
                                    subs.push(n);
                                }
                            }
                        }
                        let issuer = e["issuer_name"]
                            .as_str()
                            .unwrap_or(e["issuer"]["o"].as_str().unwrap_or("unknown"));
                        let not_before = e["not_before"].as_str().unwrap_or("");
                        let not_after = e["not_after"].as_str().unwrap_or("");
                        let common_name = e["common_name"].as_str().unwrap_or("").to_lowercase();
                        let key = format!("{common_name}|{not_before}|{issuer}");
                        if !not_before.is_empty()
                            && seen_certs.insert(key)
                            && (common_name.ends_with(&domain)
                                || common_name == format!("*.{domain}"))
                        {
                            certs.push(json!({
                                "commonName": common_name,
                                "issuer": issuer,
                                "notBefore": not_before,
                                "notAfter": not_after,
                            }));
                        }
                    }

                    certs.sort_by(|a, b| {
                        a["notBefore"]
                            .as_str()
                            .unwrap_or("")
                            .cmp(b["notBefore"].as_str().unwrap_or(""))
                    });
                    certs.truncate(50);
                    if !certs.is_empty() {
                        let first_issued = certs[0]["notBefore"].as_str().unwrap_or("?");
                        let issuers: std::collections::HashSet<String> = certs
                            .iter()
                            .filter_map(|c| c["issuer"].as_str().map(String::from))
                            .collect();
                        ctx.emit(Finding {
                            module_id: "domain.intel".into(),
                            category: "certificates".into(),
                            label: format!(
                                "Certificate history (crt.sh, {} certs, since {})",
                                certs.len(),
                                first_issued
                            ),
                            value: format!(
                                "issuers: {}",
                                issuers.into_iter().collect::<Vec<_>>().join(", ")
                            ),
                            url: Some(format!("https://crt.sh/?q=%.{domain}")),
                            confidence: Confidence::Medium,
                            target: None,
                            detail: Some(json!({"certs": certs, "firstSeen": first_issued})),
                        })
                        .await;
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
        ctx.progress(5, 5).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn cloudflare_ranges_detected() {
        assert!(is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(
            104, 16, 132, 229
        ))));
        assert!(is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(172, 64, 1, 1))));
        assert!(is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(
            103, 21, 244, 7
        ))));
        assert!(is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(
            188, 114, 96, 3
        ))));
        assert!(is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(
            198, 41, 200, 10
        ))));
        assert!(!is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 2))));
        assert!(!is_cloudflare_ip(&IpAddr::V4(Ipv4Addr::new(
            192, 168, 0, 1
        ))));
        assert!(is_cloudflare_ip(&"2606:4700::6810:84e5".parse().unwrap()));
        assert!(!is_cloudflare_ip(&"2001:db8::1".parse().unwrap()));
    }
}
