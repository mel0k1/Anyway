use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::json;

pub struct IpModule;

#[async_trait::async_trait]
impl Module for IpModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "ip.intel",
            name: "IP Intelligence",
            description: "Geo and ASN via ip-api, RDAP registration data and reverse DNS",
            accepts: &[crate::models::TargetKind::IpAddress],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let ip = ctx.target.normalized.clone();
        ctx.progress(0, 3).await;

        let geo_url = format!(
            "http://ip-api.com/json/{ip}?fields=status,message,country,countryCode,regionName,city,zip,lat,lon,timezone,isp,org,as,asname,reverse,mobile,proxy,hosting,query"
        );
        if let Ok(v) = crate::http::fetch_json(&ctx.client, &geo_url).await {
            if v["status"].as_str() == Some("success") {
                ctx.emit(Finding {
                    module_id: "ip.intel".into(),
                    category: "ip_geo".into(),
                    label: "Geolocation".into(),
                    value: format!(
                        "{}, {} {} ({})",
                        v["city"].as_str().unwrap_or("?"),
                        v["country"].as_str().unwrap_or("?"),
                        v["zip"].as_str().unwrap_or(""),
                        v["timezone"].as_str().unwrap_or("?")
                    ),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(v.clone()),
                })
                .await;
                let flags: Vec<String> = [
                    ("hosting", v["hosting"].as_bool() == Some(true)),
                    ("proxy", v["proxy"].as_bool() == Some(true)),
                    ("mobile", v["mobile"].as_bool() == Some(true)),
                ]
                .iter()
                .filter(|(_, on)| *on)
                .map(|(k, _)| k.to_string())
                .collect();
                ctx.emit(Finding {
                    module_id: "ip.intel".into(),
                    category: "ip_network".into(),
                    label: "Network".into(),
                    value: format!("{} / {}", v["isp"].as_str().unwrap_or("?"), v["as"].as_str().unwrap_or("?")),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(json!({"org": v["org"], "asname": v["asname"], "flags": flags, "reverse": v["reverse"]})),
                })
                .await;
            }
        }
        ctx.progress(1, 3).await;

        if let Ok(v) =
            crate::http::fetch_json(&ctx.client, &format!("https://rdap.org/ip/{ip}")).await
        {
            let abuse = collect_abuse(&v);
            let events: Vec<String> = v["events"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|e| {
                            format!(
                                "{} {}",
                                e["eventAction"].as_str().unwrap_or("?"),
                                e["eventDate"].as_str().unwrap_or("?")
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            ctx.emit(Finding {
                module_id: "ip.intel".into(),
                category: "ip_rdap".into(),
                label: "RDAP registration".into(),
                value: v["name"].as_str().unwrap_or("network").to_string(),
                url: None,
                confidence: Confidence::High,
                target: None,
                detail: Some(json!({
                    "handle": v["handle"], "netname": v["name"], "type": v["type"],
                    "country": v["country"], "range": [v["startAddress"], v["endAddress"]],
                    "events": events, "abuse": abuse,
                })),
            })
            .await;
        }
        ctx.progress(2, 3).await;

        if let Ok(ptrs) = crate::http::reverse_dns(&ctx.client, &ip).await {
            if !ptrs.is_empty() {
                ctx.emit(Finding {
                    module_id: "ip.intel".into(),
                    category: "ip_ptr".into(),
                    label: "Reverse DNS".into(),
                    value: ptrs.join(", "),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(json!({"records": ptrs})),
                })
                .await;
            }
        }
        ctx.progress(3, 3).await;
        Ok(())
    }
}

fn collect_abuse(v: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(entities) = v["entities"].as_array() {
        for e in entities {
            let roles = e["roles"].as_array().cloned().unwrap_or_default();
            let is_abuse = roles.iter().any(|r| r.as_str() == Some("abuse"));
            if !is_abuse {
                continue;
            }
            if let Some(vcard) = e["vcardArray"].as_array() {
                if let Some(props) = vcard.get(1).and_then(|p| p.as_array()) {
                    for p in props {
                        if p.get(0).and_then(|x| x.as_str()) == Some("email") {
                            if let Some(mail) = p.get(3).and_then(|x| x.as_str()) {
                                out.push(mail.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    out
}
