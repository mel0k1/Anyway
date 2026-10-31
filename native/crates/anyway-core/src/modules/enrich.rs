use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Value};

pub struct EnrichModule;

#[async_trait::async_trait]
impl Module for EnrichModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "username.enrich",
            name: "Profile Enrichment",
            description: "Official API lookups on GitHub, Reddit, Hacker News, Dev.to and Keybase",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let u = ctx.target.normalized.clone();
        let checks: [(&str, String); 5] = [
            (
                "GitHub",
                format!("https://api.github.com/users/{u}"),
            ),
            (
                "Reddit",
                format!("https://www.reddit.com/user/{u}/about.json"),
            ),
            (
                "Hacker News",
                format!("https://hacker-news.firebaseio.com/v0/user/{u}.json"),
            ),
            (
                "Dev.to",
                format!("https://dev.to/api/users/by_username?url={u}"),
            ),
            (
                "Keybase",
                format!("https://keybase.io/_/api/1.0/user/lookup.json?username={u}"),
            ),
        ];
        ctx.progress(0, checks.len() as u64).await;
        for (i, (label, url)) in checks.iter().enumerate() {
            if ctx.cancelled() {
                return Ok(());
            }
            if let Ok(v) = crate::http::fetch_json(&ctx.client, url).await {
                if let Some(detail) = extract(label, &v) {
                    ctx.emit(Finding {
                        module_id: "username.enrich".into(),
                        category: "api_profile".into(),
                        label: label.to_string(),
                        value: profile_url(label, &u),
                        url: Some(profile_url(label, &u)),
                        confidence: Confidence::High,
                        detail: Some(detail),
                    })
                    .await;
                }
            }
            ctx.progress(i as u64 + 1, checks.len() as u64).await;
        }
        Ok(())
    }
}

fn profile_url(label: &str, u: &str) -> String {
    match label {
        "GitHub" => format!("https://github.com/{u}"),
        "Reddit" => format!("https://www.reddit.com/user/{u}"),
        "Hacker News" => format!("https://news.ycombinator.com/user?id={u}"),
        "Dev.to" => format!("https://dev.to/{u}"),
        _ => format!("https://keybase.io/{u}"),
    }
}

fn extract(label: &str, v: &Value) -> Option<Value> {
    match label {
        "GitHub" => {
            if v.get("login").is_none() {
                return None;
            }
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
            if d.get("name").is_none() {
                return None;
            }
            Some(json!({
                "name": d["name"], "created": d["created_utc"],
                "linkKarma": d["link_karma"], "commentKarma": d["comment_karma"],
                "totalKarma": d["total_karma"], "verified": d["verified"],
                "hasVerifiedEmail": d["has_verified_email"], "icon": d["icon_img"],
            }))
        }
        "Hacker News" => {
            if v.get("id").is_none() {
                return None;
            }
            Some(json!({
                "id": v["id"], "karma": v["karma"], "created": v["created"], "about": v["about"],
            }))
        }
        "Dev.to" => {
            if v.get("username").is_none() {
                return None;
            }
            Some(json!({
                "username": v["username"], "name": v["name"], "summary": v["summary"],
                "joined": v["joined_at"], "location": v["location"], "website": v["website_url"],
                "twitter": v["twitter_username"], "github": v["github_username"],
            }))
        }
        _ => {
            let them = &v["them"];
            if them.is_null() {
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
