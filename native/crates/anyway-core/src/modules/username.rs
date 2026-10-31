use super::sites::{Rule, Site, SiteMethod, SITES, WAF_FINGERPRINTS};
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
            description: "Probes 36 social and developer platforms for profiles matching the username",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let username = ctx.target.normalized.clone();
        let conc = ctx.options.concurrency.clamp(4, 32) as usize;
        let total = SITES.len();
        let done = Arc::new(AtomicUsize::new(0));
        let client = Arc::new(ctx.client.clone());

        stream::iter(SITES.iter().cloned())
            .map(|site| {
                let client = client.clone();
                let username = username.clone();
                let done = done.clone();
                let ctx = ctx.clone();
                async move {
                    if ctx.cancelled() {
                        return;
                    }
                    let (site_name, status, http, probe_url) = probe(&client, &site, &username).await;
                    let i = done.fetch_add(1, Ordering::Relaxed) + 1;
                    ctx.progress(i as u64, total as u64).await;
                    if status == SiteStatus::Claimed {
                        let confidence = match site.rule {
                            Rule::Status => Confidence::High,
                            _ => Confidence::Medium,
                        };
                        ctx.emit(Finding {
                            module_id: "username.enumerate".into(),
                            category: "social_profile".into(),
                            label: site_name.into(),
                            value: profile_url(&site, &username),
                            url: Some(profile_url(&site, &username)),
                            confidence,
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
        Ok(())
    }
}

fn profile_url(site: &Site, username: &str) -> String {
    let pattern = if site.link.is_empty() { site.url } else { site.link };
    pattern.replace("{}", username)
}

fn fill(pattern: &str, username: &str) -> String {
    pattern.replace("{}", username)
}

fn looks_like_waf(body: &str) -> bool {
    WAF_FINGERPRINTS.iter().any(|f| body.contains(f))
}

async fn probe(client: &reqwest::Client, site: &Site, username: &str) -> (&'static str, SiteStatus, u16, String) {
    if !site.regex.is_empty() {
        let legal = regex::Regex::new(site.regex)
            .map(|r| r.is_match(username))
            .unwrap_or(true);
        if !legal {
            return (site.name, SiteStatus::Illegal, 0, String::new());
        }
    }

    let target_url = if site.probe.is_empty() { site.url } else { site.probe };
    let url = fill(target_url, username);

    let request = match site.method {
        SiteMethod::Post => client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(fill(site.payload, username)),
        SiteMethod::Get => client.get(&url),
    };

    let Ok(resp) = request.send().await else {
        return (site.name, SiteStatus::Unknown, 0, url);
    };
    let http = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();

    if looks_like_waf(&body) {
        return (site.name, SiteStatus::Waf, http, url);
    }

    let status = match site.rule {
        Rule::Msg(msgs) => {
            if msgs.iter().any(|m| body.contains(m)) {
                SiteStatus::Available
            } else {
                SiteStatus::Claimed
            }
        }
        Rule::Redirect(_) => SiteStatus::Unknown,
        Rule::Status => {
            if site.unknown_codes.contains(&http) {
                SiteStatus::Unknown
            } else if !(200..300).contains(&http) {
                SiteStatus::Available
            } else {
                SiteStatus::Claimed
            }
        }
    };
    (site.name, status, http, url)
}
