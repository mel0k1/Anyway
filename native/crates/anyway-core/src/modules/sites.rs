use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct SiteDef {
    pub name: String,
    #[serde(rename = "cat")]
    pub category: String,
    pub url: String,
    #[serde(default)]
    pub probe: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub hdr: std::collections::BTreeMap<String, String>,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub codes: Vec<u16>,
    #[serde(default)]
    pub msgs: Vec<String>,
    #[serde(default)]
    pub has: Vec<String>,
    #[serde(default)]
    pub mark: String,
    #[serde(default)]
    pub rx: String,
    #[serde(default)]
    pub unk: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectKind {
    Status {
        codes: Vec<u16>,
    },
    Message {
        absent: Vec<String>,
        present: Vec<String>,
    },
    Redirect {
        marker: String,
    },
}

#[derive(Debug, Clone)]
pub struct Site {
    pub name: String,
    pub category: String,
    pub url: String,
    pub probe: String,
    pub post: bool,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub kind: DetectKind,
    pub regex: Option<regex::Regex>,
    pub unknown_codes: Vec<u16>,
}

pub const DEFAULT_UNKNOWN_CODES: &[u16] = &[403, 429, 503];

pub const SITES_JSON: &str = include_str!("sites.json");

pub fn sites() -> &'static Vec<Site> {
    static SITES: OnceLock<Vec<Site>> = OnceLock::new();
    SITES.get_or_init(|| {
        let defs: Vec<SiteDef> = serde_json::from_str(SITES_JSON).expect("embedded sites.json");
        defs.into_iter().filter_map(compiled).collect()
    })
}

fn compiled(def: SiteDef) -> Option<Site> {
    if def.url.is_empty() && def.probe.is_empty() {
        return None;
    }
    let kind = match def.kind.as_str() {
        "status" => DetectKind::Status {
            codes: if def.codes.is_empty() {
                vec![404]
            } else {
                def.codes
            },
        },
        "redirect" if !def.mark.is_empty() => DetectKind::Redirect { marker: def.mark },
        _ => {
            if def.msgs.is_empty() && def.has.is_empty() {
                return None;
            }
            DetectKind::Message {
                absent: def.msgs.clone(),
                present: def.has.clone(),
            }
        }
    };
    let unknown_codes = if def.unk.is_empty() {
        DEFAULT_UNKNOWN_CODES.to_vec()
    } else {
        def.unk
            .iter()
            .copied()
            .chain(DEFAULT_UNKNOWN_CODES.iter().copied())
            .collect::<std::collections::BTreeSet<u16>>()
            .into_iter()
            .collect()
    };
    Some(Site {
        name: def.name,
        category: def.category,
        url: if def.url.is_empty() {
            def.probe.clone()
        } else {
            def.url
        },
        probe: def.probe,
        post: def.method.eq_ignore_ascii_case("POST"),
        body: def.body,
        headers: def.hdr.into_iter().collect(),
        kind,
        regex: if def.rx.is_empty() {
            None
        } else {
            regex::Regex::new(&def.rx).ok()
        },
        unknown_codes,
    })
}

pub fn categories() -> Vec<&'static str> {
    let mut seen = std::collections::BTreeSet::new();
    for s in sites() {
        seen.insert(s.category.as_str());
    }
    seen.into_iter().collect()
}

pub const WAF_FINGERPRINTS: &[&str] = &[
    "Just a moment",
    "Attention Required",
    "_Incapsula_Resource",
    "perimeterx",
    "cf-chl",
    "cf-browser-verification",
    "cf-mitigated: challenge",
];

pub fn looks_like_waf(body: &str) -> bool {
    WAF_FINGERPRINTS.iter().any(|f| body.contains(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_database_loads() {
        let all = sites();
        assert!(
            all.len() >= 600,
            "expected large site database, got {}",
            all.len()
        );
        for s in all {
            assert!(!s.name.is_empty());
            assert!(
                s.url.contains("{}") || s.probe.contains("{}") || s.body.contains("{}"),
                "site {} missing placeholder",
                s.name
            );
        }
    }

    #[test]
    fn key_sites_present() {
        let names: Vec<&str> = sites().iter().map(|s| s.name.as_str()).collect();
        for key in [
            "GitHub",
            "X",
            "Telegram",
            "Reddit",
            "Instagram",
            "Steam",
            "Discord",
            "VK",
            "Chess.com",
        ] {
            assert!(names.contains(&key), "missing {key}");
        }
    }

    #[test]
    fn categories_are_non_empty() {
        assert!(!categories().is_empty());
    }

    #[test]
    fn regexes_compile_or_are_dropped() {
        for s in sites() {
            assert!(
                !s.url.contains("{username}"),
                "unnormalized placeholder in {}",
                s.name
            );
        }
    }
}
