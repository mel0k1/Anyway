use super::sites::{looks_like_waf, sites, DetectKind, Site};
use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use futures::stream::{self, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct UsernameModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteStatus {
    Claimed,
    Available,
    Unknown,
    Illegal,
    Waf,
}

#[async_trait::async_trait]
impl Module for UsernameModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "username.enumerate",
            name: "Username Enumeration",
            description: "Probes 700+ social, developer, forum and gaming platforms with status, message, presence and redirect heuristics",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let username = ctx.target.normalized.clone();
        let conc = ctx.options.concurrency.clamp(4, 64) as usize;
        let all = sites();
        let total = all.len();
        let done = Arc::new(AtomicUsize::new(0));
        let client = Arc::new(ctx.client.clone());

        ctx.note(&format!("probing {total} sites")).await;

        stream::iter(all.iter().cloned())
            .map(|site| {
                let client = client.clone();
                let username = username.clone();
                let done = done.clone();
                let ctx = ctx.clone();
                async move {
                    if ctx.cancelled() {
                        return;
                    }
                    let (status, http, probe_url) = probe(&client, &site, &username).await;
                    let i = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if i.is_multiple_of(25) {
                        ctx.progress(i as u64, total as u64).await;
                    }
                    if status == SiteStatus::Claimed {
                        let confidence = match &site.kind {
                            DetectKind::Message { present, .. } if !present.is_empty() => {
                                Confidence::High
                            }
                            DetectKind::Redirect { .. } => Confidence::Medium,
                            _ => Confidence::High,
                        };
                        let link = profile_url(&site, &username);
                        ctx.emit(Finding {
                            module_id: "username.enumerate".into(),
                            category: "social_profile".into(),
                            label: site.name.clone(),
                            value: link.clone(),
                            url: Some(link),
                            confidence,
                            target: None,
                            detail: Some(serde_json::json!({
                                "siteCategory": site.category,
                                "httpStatus": http,
                                "probeUrl": probe_url,
                            })),
                        })
                        .await;
                    }
                }
            })
            .buffer_unordered(conc)
            .count()
            .await;
        ctx.progress(total as u64, total as u64).await;
        Ok(())
    }
}

fn profile_url(site: &Site, username: &str) -> String {
    site.url.replace("{}", username)
}

fn fill(pattern: &str, username: &str) -> String {
    pattern.replace("{}", username)
}

async fn probe(client: &reqwest::Client, site: &Site, username: &str) -> (SiteStatus, u16, String) {
    if let Some(rx) = &site.regex {
        if !rx.is_match(username) {
            return (SiteStatus::Illegal, 0, String::new());
        }
    }

    let target_url = if site.probe.is_empty() {
        &site.url
    } else {
        &site.probe
    };
    let url = fill(target_url, username);

    let mut request = match site.post {
        true => client
            .post(&url)
            .header(
                "Content-Type",
                site.headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_else(|| "application/json".into()),
            )
            .body(fill(&site.body, username)),
        false => client.get(&url),
    };
    for (k, v) in &site.headers {
        if site.post && k.eq_ignore_ascii_case("content-type") {
            continue;
        }
        request = request.header(k, v);
    }

    let Ok(resp) = request.send().await else {
        return (SiteStatus::Unknown, 0, url);
    };
    let http = resp.status().as_u16();
    if resp
        .headers()
        .get("cf-mitigated")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("challenge"))
        .unwrap_or(false)
    {
        return (SiteStatus::Waf, http, url);
    }
    let final_url = resp.url().to_string();
    let body = resp.text().await.unwrap_or_default();

    if looks_like_waf(&body) {
        return (SiteStatus::Waf, http, url);
    }

    let status = match &site.kind {
        DetectKind::Status { codes } => {
            if codes.contains(&http) {
                SiteStatus::Available
            } else if site.unknown_codes.contains(&http) {
                SiteStatus::Unknown
            } else if (200..300).contains(&http) {
                SiteStatus::Claimed
            } else {
                SiteStatus::Unknown
            }
        }
        DetectKind::Message { absent, present } => {
            if !(200..300).contains(&http) || body.is_empty() {
                SiteStatus::Unknown
            } else if absent.iter().any(|m| body.contains(m)) {
                SiteStatus::Available
            } else if !present.is_empty() {
                if present.iter().any(|m| body.contains(m)) {
                    SiteStatus::Claimed
                } else {
                    SiteStatus::Available
                }
            } else {
                SiteStatus::Claimed
            }
        }
        DetectKind::Redirect { marker } => {
            if !(200..400).contains(&http) {
                SiteStatus::Unknown
            } else if final_url.contains(marker) || body.contains(marker) {
                SiteStatus::Available
            } else {
                SiteStatus::Claimed
            }
        }
    };
    (status, http, url)
}
