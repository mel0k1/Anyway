use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::{json, Map, Value};

pub const MAX_FRIENDS: usize = 60;
pub const MAX_PAGES: usize = 3;

pub struct SteamFriendsModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamFriend {
    pub login: String,
    pub name: String,
}

#[async_trait::async_trait]
impl Module for SteamFriendsModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "social.steam",
            name: "Steam Friends",
            description: "Reads the public Steam Community friends list page by page and links every friend account into the relationship graph ready for manual re-pivoting",
            accepts: &[crate::models::TargetKind::Username],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let user = ctx.target.normalized.clone();
        if user.contains('@') {
            ctx.note("steam: handle with @ is not a vanity id, skipped")
                .await;
            return Ok(());
        }
        ctx.progress(0, MAX_PAGES as u64).await;
        let mut logins: Vec<String> = Vec::new();
        let mut names: Map<String, Value> = Map::new();
        for page in 1..=MAX_PAGES {
            if ctx.cancelled() {
                return Ok(());
            }
            let url = format!("https://steamcommunity.com/id/{user}/friends?l=english&p={page}");
            let Ok((status, body, _)) =
                crate::http::fetch_text(&ctx.client, reqwest::Method::GET, &url).await
            else {
                break;
            };
            if status != 200 {
                break;
            }
            let before = logins.len();
            for friend in parse_friends(&body) {
                if logins.len() >= MAX_FRIENDS {
                    break;
                }
                if friend.login == user || logins.contains(&friend.login) {
                    continue;
                }
                logins.push(friend.login.clone());
                if !friend.name.is_empty() {
                    names.insert(friend.login, json!(friend.name));
                }
            }
            ctx.progress(page as u64, MAX_PAGES as u64).await;
            if logins.len() >= MAX_FRIENDS || logins.len() == before {
                break;
            }
        }
        if ctx.cancelled() || logins.is_empty() {
            return Ok(());
        }
        ctx.note(&format!("steam: {} public friends parsed", logins.len()))
            .await;
        ctx.emit(Finding {
            module_id: "social.steam".into(),
            category: "social_connection".into(),
            label: "Steam friends".into(),
            value: format!("{} friends", logins.len()),
            url: Some(format!("https://steamcommunity.com/id/{user}/friends")),
            confidence: Confidence::High,
            target: None,
            detail: Some(json!({
                "platform": "steam",
                "direction": "friends",
                "logins": logins,
                "names": names,
            })),
        })
        .await;
        Ok(())
    }
}

fn first_match(chunk: &str, pattern: &str) -> Option<String> {
    regex::Regex::new(pattern)
        .ok()?
        .captures(chunk)?
        .get(1)
        .map(|m| m.as_str().to_string())
}

fn decode_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
}

fn valid_slug(slug: &str) -> bool {
    slug.len() >= 2
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

pub fn parse_friends(body: &str) -> Vec<SteamFriend> {
    let mut out = Vec::new();
    for chunk in body.split("friend_block_v2").skip(1) {
        let Some(steamid) = first_match(chunk, r#"data-steamid="(\d{10,20})""#) else {
            continue;
        };
        let overlay = first_match(
            chunk,
            r#"selectable_overlay"[^>]*href="https://steamcommunity\.com/(id/[A-Za-z0-9_.\-]+|profiles/\d+)""#,
        );
        let login = match overlay {
            Some(path) => match path.strip_prefix("id/") {
                Some(slug) if valid_slug(slug) => slug.to_ascii_lowercase(),
                _ => path
                    .strip_prefix("profiles/")
                    .unwrap_or(&steamid)
                    .to_string(),
            },
            None => steamid,
        };
        let name = first_match(chunk, r#"data-search="([^"]*)""#)
            .map(|s| {
                decode_entities(&s)
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        out.push(SteamFriend { login, name });
        if out.len() >= MAX_FRIENDS {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: &str = r##"<div data-panel="{&quot;flow-children&quot;:&quot;row&quot;}" class="selectable friend_block_v2 persona offline  "
                id="fr_83528534"
                data-steamid="76561198043794262"
                                    data-miniprofile="83528534"
                    data-search="xx_tokyoghoul_xx ;  ; "
                                                >
            <div class="indicator select_friend">
                <input class="select_friend_checkbox" type="checkbox">
            </div>
                            <a class="selectable_overlay" data-container="#fr_83528534" href="https://steamcommunity.com/id/gamingfrancisfan"></a>
            <div class="player_avatar friend_block_link_overlay">
                <img src="https://avatars.steamstatic.com/x.jpg">
            </div>
            <div class="friend_block_content">Tokyo<span class="friend_small_text">Offline</span></div>
        </div>"##;

    #[test]
    fn parses_friend_blocks() {
        let body = format!(
            "onkeyup=\"ShowMatching('search_results', 'friend_block_v2', 'data-search');\" {}",
            BLOCK
        );
        let friends = parse_friends(&body);
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].login, "gamingfrancisfan");
        assert_eq!(friends[0].name, "xx_tokyoghoul_xx");
    }

    #[test]
    fn falls_back_to_steamid_when_no_vanity() {
        let block = BLOCK
            .replace("https://steamcommunity.com/id/gamingfrancisfan", "")
            .replace("xx_tokyoghoul_xx", "Plain &amp; Simple");
        let friends = parse_friends(&block);
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].login, "76561198043794262");
        assert_eq!(friends[0].name, "Plain & Simple");
    }

    #[test]
    fn prefers_numeric_profile_links() {
        let block = BLOCK.replace(
            "https://steamcommunity.com/id/gamingfrancisfan",
            "https://steamcommunity.com/profiles/76561198999999999",
        );
        let friends = parse_friends(&block);
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].login, "76561198999999999");
    }

    #[test]
    fn skips_chunks_without_steamid() {
        let friends = parse_friends("header friend_block_v2 search box only");
        assert!(friends.is_empty());
    }

    #[test]
    fn caps_at_max_friends() {
        let many: String = (0..100)
            .map(|i| {
                BLOCK
                    .replace("xx_tokyoghoul_xx", &format!("user{i}"))
                    .replace("76561198043794262", &format!("7656119804379{i:05}"))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let friends = parse_friends(&many);
        assert_eq!(friends.len(), MAX_FRIENDS);
    }
}
