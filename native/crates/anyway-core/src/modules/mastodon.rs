use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Value};

pub const MAX_ACCOUNTS: usize = 60;
pub const MAX_PAGES: usize = 2;
pub const FALLBACK_INSTANCE: &str = "mastodon.social";

pub struct MastodonModule;

#[async_trait::async_trait]
impl Module for MastodonModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "social.mastodon",
            name: "Mastodon Connections",
            description: "Resolves the fediverse profile on mastodon.social, or on the instance of an email-style user@instance handle, then pulls the public follower and follow lists",
            accepts: &[crate::models::TargetKind::Username, crate::models::TargetKind::Email],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let raw = ctx.target.normalized.clone();
        let Some((instance, acct)) = split_handle(&raw) else {
            ctx.note("mastodon: handle does not resolve to an instance, skipped")
                .await;
            return Ok(());
        };
        let lookup = format!("https://{instance}/api/v1/accounts/lookup?acct={acct}");
        let Ok(profile) = crate::http::fetch_json(&ctx.client, &lookup).await else {
            ctx.note(&format!("mastodon: no profile {acct} on {instance}"))
                .await;
            return Ok(());
        };
        let id = match profile.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => return Ok(()),
        };
        ctx.progress(0, 2).await;
        if instance != FALLBACK_INSTANCE {
            let url = profile
                .get("url")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("https://{instance}/@{acct}"));
            let label = format!("Mastodon ({instance})");
            ctx.emit(Finding {
                module_id: "social.mastodon".into(),
                category: "api_profile".into(),
                label: label.clone(),
                value: url.clone(),
                url: Some(url),
                confidence: Confidence::High,
                target: None,
                detail: Some(json!({
                    "username": profile["username"], "name": profile["display_name"],
                    "followers": profile["followers_count"], "following": profile["following_count"],
                    "posts": profile["statuses_count"], "instance": instance,
                })),
            })
            .await;
        }
        for (i, direction) in ["followers", "following"].into_iter().enumerate() {
            if ctx.cancelled() {
                return Ok(());
            }
            let mut logins: Vec<String> = Vec::new();
            let mut next = Some(format!(
                "https://{instance}/api/v1/accounts/{id}/{direction}?limit=80"
            ));
            for _ in 0..MAX_PAGES {
                let Some(url) = next else { break };
                let Some((accounts, link)) = fetch_accounts(&ctx.client, &url).await else {
                    break;
                };
                next = link;
                for acct in accounts {
                    if logins.len() >= MAX_ACCOUNTS {
                        break;
                    }
                    if !logins.contains(&acct) {
                        logins.push(acct);
                    }
                }
                if logins.len() >= MAX_ACCOUNTS {
                    break;
                }
            }
            if !logins.is_empty() {
                ctx.emit(Finding {
                    module_id: "social.mastodon".into(),
                    category: "social_connection".into(),
                    label: format!("Mastodon {direction}"),
                    value: format!("{} accounts", logins.len()),
                    url: Some(format!("https://{instance}/@{acct}")),
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(json!({
                        "platform": "mastodon",
                        "direction": direction,
                        "instance": instance,
                        "logins": logins,
                    })),
                })
                .await;
            }
            ctx.progress(i as u64 + 1, 2).await;
        }
        Ok(())
    }
}

pub fn split_handle(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim().trim_start_matches('@').to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }
    match raw.split_once('@') {
        Some((local, instance)) => {
            if local.is_empty() || !valid_instance(instance) {
                return None;
            }
            Some((instance.to_string(), raw))
        }
        None => Some((FALLBACK_INSTANCE.to_string(), raw)),
    }
}

pub fn valid_instance(instance: &str) -> bool {
    crate::graph::domain_from_url(instance).is_some_and(|h| h == instance)
}

fn valid_acct(acct: &str) -> bool {
    let name_ok = |s: &str| {
        !s.is_empty()
            && s.len() <= 64
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
    };
    match acct.split_once('@') {
        Some((user, instance)) => name_ok(user) && valid_instance(instance) && acct.len() <= 80,
        None => name_ok(acct),
    }
}

async fn fetch_accounts(
    client: &reqwest::Client,
    url: &str,
) -> Option<(Vec<String>, Option<String>)> {
    let resp = client.get(url).send().await.ok()?;
    let next = resp
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .and_then(next_link);
    if !resp.status().is_success() {
        return None;
    }
    let value = resp.json::<Value>().await.ok()?;
    let items = value.as_array()?;
    let mut out = Vec::new();
    for item in items {
        let acct = item
            .get("acct")
            .and_then(|v| v.as_str())
            .or_else(|| item.get("username").and_then(|v| v.as_str()))?
            .trim()
            .trim_start_matches('@')
            .to_ascii_lowercase();
        if valid_acct(&acct) && !out.contains(&acct) {
            out.push(acct);
        }
    }
    Some((out, next))
}

pub fn next_link(header: &str) -> Option<String> {
    for part in header.split(',') {
        let mut segments = part.split(';');
        let url = segments.next()?.trim();
        let rel = segments
            .map(|s| s.trim())
            .any(|s| s == "rel=\"next\"" || s == "rel=next");
        if rel && url.len() > 2 && url.starts_with('<') && url.ends_with('>') {
            return Some(url[1..url.len() - 1].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_usernames_use_fallback_instance() {
        let (instance, acct) = split_handle("Mel0k1").unwrap();
        assert_eq!(instance, FALLBACK_INSTANCE);
        assert_eq!(acct, "mel0k1");
        assert!(split_handle("").is_none());
        assert!(split_handle("   ").is_none());
    }

    #[test]
    fn email_style_handles_resolve_their_instance() {
        let (instance, acct) = split_handle("@user@FossTodon.org").unwrap();
        assert_eq!(instance, "fosstodon.org");
        assert_eq!(acct, "user@fosstodon.org");
        let (instance, acct) = split_handle("jane@mastodon.social").unwrap();
        assert_eq!(instance, "mastodon.social");
        assert_eq!(acct, "jane@mastodon.social");
    }

    #[test]
    fn invalid_instance_domains_are_rejected() {
        assert!(split_handle("user@nodomain").is_none());
        assert!(split_handle("user@").is_none());
    }

    #[test]
    fn parses_next_link_header() {
        let header = r#"<https://mastodon.social/api/v1/accounts/1/followers?limit=80&page=2>; rel="next", <https://mastodon.social/api/v1/accounts/1/followers?limit=80>; rel="prev""#;
        assert_eq!(
            next_link(header).unwrap(),
            "https://mastodon.social/api/v1/accounts/1/followers?limit=80&page=2"
        );
        assert!(next_link(r#"<https://x.example>; rel="prev""#).is_none());
    }

    #[test]
    fn acct_validation_accepts_local_and_remote() {
        assert!(valid_acct("alice"));
        assert!(valid_acct("alice@fosstodon.org"));
        assert!(!valid_acct("alice@nodomain"));
        assert!(!valid_acct("ali ce"));
        assert!(!valid_acct(""));
    }
}
