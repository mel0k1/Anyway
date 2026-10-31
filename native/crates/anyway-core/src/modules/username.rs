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

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub status: SiteStatus,
    pub http: u16,
    pub url: String,
    pub body_len: usize,
}

#[async_trait::async_trait]
impl Module for UsernameModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "username.enumerate",
            name: "Username Enumeration",
            description: "Probes 700+ social, developer, forum and gaming platforms with marker, status, redirect heuristics and nonce calibration against false positives",
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
                    let probe = probe(&client, &site, &username).await;
                    let i = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if i.is_multiple_of(25) {
                        ctx.progress(i as u64, total as u64).await;
                    }
                    if probe.status != SiteStatus::Claimed {
                        return;
                    }
                    let base = signal_confidence(&site);
                    let (confidence, calibration) =
                        calibrate(&client, &site, &username, base, &probe).await;
                    if calibration.downgraded {
                        ctx.note(&format!(
                            "{}: nonce probe {}, confidence reduced to {:?}",
                            site.name, calibration.reason, confidence
                        ))
                        .await;
                    }
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
                            "httpStatus": probe.http,
                            "probeUrl": probe.url,
                            "signal": signal_name(&site),
                            "verified": confidence == Confidence::High,
                        })),
                    })
                    .await;
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

fn signal_confidence(site: &Site) -> Confidence {
    match &site.kind {
        DetectKind::Message { present, .. } if !present.is_empty() => Confidence::High,
        _ => Confidence::Medium,
    }
}

fn signal_name(site: &Site) -> &'static str {
    match &site.kind {
        DetectKind::Message { present, .. } if !present.is_empty() => "marker",
        DetectKind::Message { .. } => "absence",
        DetectKind::Redirect { .. } => "redirect",
        DetectKind::Status { .. } => "status",
    }
}

pub struct Calibration {
    pub downgraded: bool,
    pub reason: String,
}

static NONCE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn nonce_for(site_name: &str, target: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    site_name.hash(&mut h);
    target.hash(&mut h);
    NONCE_SEQ
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .hash(&mut h);
    let a = h.finish();
    let b = a.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
    format!("nx{a:016x}{b:016x}")
}

fn nonce_allowed(site: &Site, nonce: &str) -> bool {
    match &site.regex {
        Some(rx) => rx.is_match(nonce),
        None => true,
    }
}

pub async fn calibrate(
    client: &reqwest::Client,
    site: &Site,
    username: &str,
    base: Confidence,
    hit: &ProbeResult,
) -> (Confidence, Calibration) {
    if base == Confidence::High {
        return (
            base,
            Calibration {
                downgraded: false,
                reason: "explicit marker matched".into(),
            },
        );
    }
    let nonce = nonce_for(&site.name, username);
    if !nonce_allowed(site, &nonce) {
        return (
            base,
            Calibration {
                downgraded: false,
                reason: "site username rules, calibration skipped".into(),
            },
        );
    }
    let nonce_probe = probe(client, site, &nonce).await;
    let same_body = nonce_probe.body_len == hit.body_len && hit.body_len > 0;
    calibration_decision(base, nonce_probe.status, same_body)
}

pub fn calibration_decision(
    base: Confidence,
    nonce_status: SiteStatus,
    same_body: bool,
) -> (Confidence, Calibration) {
    match nonce_status {
        SiteStatus::Claimed => (
            Confidence::Low,
            Calibration {
                downgraded: true,
                reason: "reports random names as taken".into(),
            },
        ),
        SiteStatus::Available => {
            if same_body {
                (
                    Confidence::Low,
                    Calibration {
                        downgraded: true,
                        reason: "identical body for taken and free names".into(),
                    },
                )
            } else {
                (
                    base,
                    Calibration {
                        downgraded: false,
                        reason: "nonce correctly reported free".into(),
                    },
                )
            }
        }
        SiteStatus::Waf => (
            Confidence::Low,
            Calibration {
                downgraded: true,
                reason: "answers behind bot protection".into(),
            },
        ),
        SiteStatus::Unknown => (
            Confidence::Medium,
            Calibration {
                downgraded: base == Confidence::High,
                reason: "calibration inconclusive".into(),
            },
        ),
        SiteStatus::Illegal => (
            base,
            Calibration {
                downgraded: false,
                reason: "calibration skipped".into(),
            },
        ),
    }
}

async fn probe(client: &reqwest::Client, site: &Site, username: &str) -> ProbeResult {
    if let Some(rx) = &site.regex {
        if !rx.is_match(username) {
            return ProbeResult {
                status: SiteStatus::Illegal,
                http: 0,
                url: String::new(),
                body_len: 0,
            };
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
        return ProbeResult {
            status: SiteStatus::Unknown,
            http: 0,
            url,
            body_len: 0,
        };
    };
    let http = resp.status().as_u16();
    if resp
        .headers()
        .get("cf-mitigated")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("challenge"))
        .unwrap_or(false)
    {
        return ProbeResult {
            status: SiteStatus::Waf,
            http,
            url,
            body_len: 0,
        };
    }
    let final_url = resp.url().to_string();
    let body = resp.text().await.unwrap_or_default();
    let body_len = body.len();

    if looks_like_waf(&body) {
        return ProbeResult {
            status: SiteStatus::Waf,
            http,
            url,
            body_len,
        };
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
    ProbeResult {
        status,
        http,
        url,
        body_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::sites::Site;

    fn message_site(present: Vec<&str>, absent: Vec<&str>) -> Site {
        Site {
            name: "Test".into(),
            category: "test".into(),
            url: "https://test.example/{}".into(),
            probe: String::new(),
            post: false,
            body: String::new(),
            headers: Vec::new(),
            kind: DetectKind::Message {
                absent: absent.into_iter().map(String::from).collect(),
                present: present.into_iter().map(String::from).collect(),
            },
            regex: None,
            unknown_codes: vec![403, 429, 503],
        }
    }

    #[test]
    fn marker_sites_signal_high() {
        let site = message_site(vec!["profile-found"], vec!["not found"]);
        assert_eq!(signal_confidence(&site), Confidence::High);
        assert_eq!(signal_name(&site), "marker");
        let plain = message_site(vec![], vec!["not found"]);
        assert_eq!(signal_confidence(&plain), Confidence::Medium);
        assert_eq!(signal_name(&plain), "absence");
    }

    #[test]
    fn always_positive_sites_downgraded() {
        let (conf, cal) = calibration_decision(Confidence::Medium, SiteStatus::Claimed, false);
        assert_eq!(conf, Confidence::Low);
        assert!(cal.downgraded);
    }

    #[test]
    fn generic_body_downgraded() {
        let (conf, cal) = calibration_decision(Confidence::Medium, SiteStatus::Available, true);
        assert_eq!(conf, Confidence::Low);
        assert!(cal.downgraded);
    }

    #[test]
    fn clean_nonce_keeps_base_confidence() {
        let (conf, cal) = calibration_decision(Confidence::Medium, SiteStatus::Available, false);
        assert_eq!(conf, Confidence::Medium);
        assert!(!cal.downgraded);
    }

    #[test]
    fn waf_nonce_downgrades_to_unverified() {
        let (conf, cal) = calibration_decision(Confidence::Medium, SiteStatus::Waf, false);
        assert_eq!(conf, Confidence::Low);
        assert!(cal.downgraded);
    }

    #[test]
    fn inconclusive_nonce_caps_at_questionable() {
        let (conf, _) = calibration_decision(Confidence::High, SiteStatus::Unknown, false);
        assert_eq!(conf, Confidence::Medium);
        let (conf, _) = calibration_decision(Confidence::Medium, SiteStatus::Unknown, false);
        assert_eq!(conf, Confidence::Medium);
    }

    #[test]
    fn nonce_is_alphanumeric_and_unique() {
        let a = nonce_for("GitHub", "mel0k1");
        let b = nonce_for("GitHub", "mel0k1");
        assert_ne!(a, b);
        assert!(a.starts_with("nx"));
        assert!(a.chars().skip(2).all(|c| c.is_ascii_hexdigit()));
        assert!(a.len() > 20);
    }

    #[test]
    fn nonce_respects_site_regex() {
        let mut site = message_site(vec![], vec![]);
        site.regex = Some(regex::Regex::new("^[a-z]{1,10}$").unwrap());
        assert!(!nonce_allowed(&site, &nonce_for("T", "x")));
        site.regex = None;
        assert!(nonce_allowed(&site, &nonce_for("T", "x")));
    }
}
