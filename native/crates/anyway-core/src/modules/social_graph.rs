use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Value};

pub const MAX_LOGINS: usize = 24;

pub struct SocialGraphModule;

#[async_trait::async_trait]
impl Module for SocialGraphModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "social.graph",
            name: "Social Graph",
            description: "Pulls public follower and follow lists (GitHub) and links every account into the relationship graph as a high-confidence connection",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let user = ctx.target.normalized.clone();
        let steps: Vec<(&str, String)> = vec![
            (
                "followers",
                format!("https://api.github.com/users/{user}/followers?per_page=30"),
            ),
            (
                "following",
                format!("https://api.github.com/users/{user}/following?per_page=30"),
            ),
        ];
        ctx.progress(0, steps.len() as u64).await;
        for (i, (direction, url)) in steps.iter().enumerate() {
            if ctx.cancelled() {
                return Ok(());
            }
            if let Ok(value) = crate::http::fetch_json(&ctx.client, url).await {
                let logins = logins_from_json(&value);
                if !logins.is_empty() {
                    ctx.emit(Finding {
                        module_id: "social.graph".into(),
                        category: "social_connection".into(),
                        label: format!("GitHub {direction}"),
                        value: format!("{} accounts", logins.len()),
                        url: Some(format!("https://github.com/{user}?tab={direction}")),
                        confidence: Confidence::High,
                        target: None,
                        detail: Some(json!({
                            "platform": "github",
                            "direction": direction,
                            "logins": logins,
                        })),
                    })
                    .await;
                }
            }
            ctx.progress(i as u64 + 1, steps.len() as u64).await;
        }
        Ok(())
    }
}

pub fn logins_from_json(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(items) = value.as_array() else {
        return out;
    };
    for item in items {
        let Some(login) = item.get("login").and_then(|l| l.as_str()) else {
            continue;
        };
        let login = login.trim().trim_start_matches('@').to_lowercase();
        if login.is_empty() || login.len() > 64 {
            continue;
        }
        if !login
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        if out.contains(&login) {
            continue;
        }
        out.push(login);
        if out.len() >= MAX_LOGINS {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_followers() {
        let v: Value = serde_json::from_str(
            r#"[{"login":"alice","id":1},{"login":"Bob","id":2},{"login":"@carol","id":3},{"login":"","id":4},{"login":"alice","id":1},{"bad":true}]"#,
        )
        .unwrap();
        let logins = logins_from_json(&v);
        assert_eq!(logins, vec!["alice", "bob", "carol"]);
    }

    #[test]
    fn rejects_non_array_payloads() {
        assert!(logins_from_json(&json!({"message": "Not Found"})).is_empty());
        assert!(logins_from_json(&json!(null)).is_empty());
    }

    #[test]
    fn caps_at_max_logins() {
        let items: Vec<Value> = (0..100)
            .map(|i| json!({"login": format!("user{i}")}))
            .collect();
        let logins = logins_from_json(&Value::Array(items));
        assert_eq!(logins.len(), MAX_LOGINS);
    }
}
