use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Value};

pub struct EnrichModule;

enum Source {
    Json,
    Text,
}

struct Check {
    label: &'static str,
    url: String,
    kind: Source,
}

#[async_trait::async_trait]
impl Module for EnrichModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "username.enrich",
            name: "Profile Enrichment",
            description: "Official API lookups: GitHub, Reddit, Hacker News, Dev.to, Keybase, Telegram, Bluesky, Mastodon and Steam",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let u = ctx.target.normalized.clone();
        let checks: Vec<Check> = vec![
            Check {
                label: "GitHub",
                url: format!("https://api.github.com/users/{u}"),
                kind: Source::Json,
            },
            Check {
                label: "Reddit",
                url: format!("https://www.reddit.com/user/{u}/about.json"),
                kind: Source::Json,
            },
            Check {
                label: "Hacker News",
                url: format!("https://hacker-news.firebaseio.com/v0/user/{u}.json"),
                kind: Source::Json,
            },
            Check {
                label: "Dev.to",
                url: format!("https://dev.to/api/users/by_username?url={u}"),
                kind: Source::Json,
            },
            Check {
                label: "Keybase",
                url: format!("https://keybase.io/_/api/1.0/user/lookup.json?username={u}"),
                kind: Source::Json,
            },
            Check {
                label: "Bluesky",
                url: format!(
                    "https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile?actor={u}"
                ),
                kind: Source::Json,
            },
            Check {
                label: "Mastodon",
                url: format!("https://mastodon.social/api/v1/accounts/lookup?acct={u}"),
                kind: Source::Json,
            },
            Check {
                label: "Telegram",
                url: format!("https://t.me/s/{u}"),
                kind: Source::Text,
            },
            Check {
                label: "Steam",
                url: format!("https://steamcommunity.com/id/{u}/?xml=1"),
                kind: Source::Text,
            },
        ];
        let total = checks.len() as u64;
        ctx.progress(0, total).await;
        for (i, check) in checks.into_iter().enumerate() {
            if ctx.cancelled() {
                return Ok(());
            }
            match check.kind {
                Source::Json => {
                    if let Ok(v) = crate::http::fetch_json(&ctx.client, &check.url).await {
                        if let Some(detail) = extract_json(check.label, &v) {
                            emit(&ctx, check.label, &u, detail).await;
                        }
                    }
                }
                Source::Text => {
                    if let Ok((status, body, _)) =
                        crate::http::fetch_text(&ctx.client, reqwest::Method::GET, &check.url).await
                    {
                        if status == 200 {
                            if let Some(detail) = extract_text(check.label, &body) {
                                emit(&ctx, check.label, &u, detail).await;
                            }
                        }
                    }
                }
            }
            ctx.progress(i as u64 + 1, total).await;
        }
        Ok(())
    }
}

async fn emit(ctx: &ModuleCtx, label: &str, u: &str, detail: Value) {
    ctx.emit(Finding {
        module_id: "username.enrich".into(),
        category: "api_profile".into(),
        label: label.to_string(),
        value: profile_url(label, u),
        url: Some(profile_url(label, u)),
        confidence: Confidence::High,
        target: None,
        detail: Some(detail),
    })
    .await;
}

fn profile_url(label: &str, u: &str) -> String {
    match label {
        "GitHub" => format!("https://github.com/{u}"),
        "Reddit" => format!("https://www.reddit.com/user/{u}"),
        "Hacker News" => format!("https://news.ycombinator.com/user?id={u}"),
        "Dev.to" => format!("https://dev.to/{u}"),
        "Bluesky" => format!("https://bsky.app/profile/{u}"),
        "Mastodon" => format!("https://mastodon.social/@{u}"),
        "Telegram" => format!("https://t.me/{u}"),
        _ => format!("https://keybase.io/{u}"),
    }
}

fn strip_tags(s: &str) -> String {
    regex::Regex::new(r"<[^>]*>")
        .map(|r| r.replace_all(s, "").trim().to_string())
        .unwrap_or_default()
}

fn first_match(body: &str, pattern: &str) -> Option<String> {
    regex::Regex::new(pattern)
        .ok()?
        .captures(body)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

fn extract_json(label: &str, v: &Value) -> Option<Value> {
    match label {
        "GitHub" => {
            v.get("login")?;
            Some(json!({
                "login": v["login"], "name": v["name"], "company": v["company"],
                "blog": v["blog"], "location": v["location"], "email": v["email"],
                "bio": v["bio"], "twitter": v["twitter_username"],
                "publicRepos": v["public_repos"], "followers": v["followers"],
                "following": v["following"], "createdAt": v["created_at"],
            }))
        }
        "Reddit" => {
            let d = &v["data"];
            d.get("name")?;
            Some(json!({
                "name": d["name"], "created": d["created_utc"],
                "linkKarma": d["link_karma"], "commentKarma": d["comment_karma"],
                "totalKarma": d["total_karma"], "verified": d["verified"],
                "hasVerifiedEmail": d["has_verified_email"], "icon": d["icon_img"],
            }))
        }
        "Hacker News" => {
            v.get("id")?;
            Some(json!({
                "id": v["id"], "karma": v["karma"], "created": v["created"], "about": v["about"],
            }))
        }
        "Dev.to" => {
            v.get("username")?;
            Some(json!({
                "username": v["username"], "name": v["name"], "summary": v["summary"],
                "joined": v["joined_at"], "location": v["location"], "website": v["website_url"],
                "twitter": v["twitter_username"], "github": v["github_username"],
            }))
        }
        "Bluesky" => {
            v.get("did")?;
            Some(json!({
                "handle": v["handle"], "did": v["did"], "name": v["displayName"],
                "description": v["description"], "followers": v["followersCount"],
                "following": v["followsCount"], "posts": v["postsCount"],
                "createdAt": v["createdAt"], "avatar": v["avatar"],
            }))
        }
        "Mastodon" => {
            v.get("id")?;
            Some(json!({
                "id": v["id"], "username": v["username"], "name": v["display_name"],
                "note": strip_tags(v["note"].as_str().unwrap_or("")),
                "createdAt": v["created_at"], "followers": v["followers_count"],
                "following": v["following_count"], "posts": v["statuses_count"],
                "url": v["url"], "fields": v["fields"],
            }))
        }
        _ => {
            let them = &v["them"];
            if them.is_null() || them["username"].is_null() {
                return None;
            }
            Some(json!({
                "username": them["username"], "name": them["profile"]["full_name"],
                "location": them["profile"]["location"], "bio": them["profile"]["bio"],
                "verified": them["is_verified"],
            }))
        }
    }
}

fn extract_text(label: &str, body: &str) -> Option<Value> {
    match label {
        "Telegram" => {
            let title = first_match(body, r#"<div class="tgme_page_title[^>]*>(.*?)</div>"#)?;
            let title = strip_tags(&title);
            if title.is_empty() {
                return None;
            }
            let desc = first_match(
                body,
                r#"<div class="tgme_page_description[^"]*">(.*?)</div>"#,
            )
            .map(|d| strip_tags(&d))
            .unwrap_or_default();
            let extra = first_match(body, r#"<div class="tgme_page_extra">([^<]*)</div>"#)
                .unwrap_or_default();
            Some(json!({
                "title": title,
                "description": desc,
                "extra": strip_tags(&extra),
            }))
        }
        "Steam" => {
            if body.contains("The specified profile could not be found") || body.contains("<error>")
            {
                return None;
            }
            let name = first_match(body, r#"<steamID><!\[CDATA\[(.*?)\]\]></steamID>"#)
                .or_else(|| first_match(body, r#"<steamID>(.*?)</steamID>"#))?;
            let online = first_match(body, r#"<onlineState><!\[CDATA\[(.*?)\]\]></onlineState>"#)
                .or_else(|| first_match(body, r#"<onlineState>(.*?)</onlineState>"#))
                .unwrap_or_default();
            let member_since =
                first_match(body, r#"<memberSince><!\[CDATA\[(.*?)\]\]></memberSince>"#)
                    .or_else(|| first_match(body, r#"<memberSince>(.*?)</memberSince>"#))
                    .unwrap_or_default();
            let location = first_match(body, r#"<location><!\[CDATA\[(.*?)\]\]></location>"#)
                .or_else(|| first_match(body, r#"<location>(.*?)</location>"#))
                .unwrap_or_default();
            let real_name = first_match(body, r#"<realname><!\[CDATA\[(.*?)\]\]></realname>"#)
                .or_else(|| first_match(body, r#"<realname>(.*?)</realname>"#))
                .unwrap_or_default();
            Some(json!({
                "name": name, "onlineState": online, "memberSince": member_since,
                "location": location, "realName": real_name,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_html() {
        assert_eq!(strip_tags("<b>hey</b> you"), "hey you");
    }

    #[test]
    fn finds_telegram_title() {
        let body = r#"<div class="tgme_page_title" dir="auto">Durov's Chat</div>"#;
        let v = extract_text("Telegram", body).expect("title");
        assert_eq!(v["title"], "Durov's Chat");
    }

    #[test]
    fn finds_steam_fields() {
        let body = "<profile><steamID><![CDATA[Guide]]></steamID><onlineState><![CDATA[online]]></onlineState></profile>";
        let v = extract_text("Steam", body).expect("steam");
        assert_eq!(v["name"], "Guide");
        assert_eq!(v["onlineState"], "online");
    }

    #[test]
    fn rejects_missing_steam() {
        assert!(extract_text(
            "Steam",
            "<error>The specified profile could not be found.</error>"
        )
        .is_none());
    }
}
