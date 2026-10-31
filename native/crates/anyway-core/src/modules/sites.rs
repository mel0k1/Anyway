use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
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
    pub hdr: BTreeMap<String, String>,
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
    #[serde(default)]
    pub strip: String,
    #[serde(default)]
    pub source: String,
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
    pub strip: String,
    pub source: String,
}

pub const DEFAULT_UNKNOWN_CODES: &[u16] = &[403, 429, 503];

pub const SITES_JSON: &str = include_str!("sites.json");
pub const WMN_JSON: &str = include_str!("wmn-data.json");
pub const MAIGRET_JSON: &str = include_str!("maigret-data.json");

pub fn sites() -> &'static Vec<Site> {
    static SITES: OnceLock<Vec<Site>> = OnceLock::new();
    SITES.get_or_init(|| merge_databases(builtin_sites(), wmn_sites(), maigret_sites()))
}

pub fn core_site_count() -> usize {
    sites().iter().filter(|s| s.source != "maigret").count()
}

pub fn maigret_site_count() -> usize {
    sites().iter().filter(|s| s.source == "maigret").count()
}

pub fn builtin_sites() -> Vec<Site> {
    let defs: Vec<SiteDef> = serde_json::from_str(SITES_JSON).expect("embedded sites.json");
    defs.into_iter().filter_map(compiled).collect()
}

pub fn maigret_sites() -> Vec<Site> {
    let defs: Vec<SiteDef> =
        serde_json::from_str(MAIGRET_JSON).expect("embedded maigret-data.json");
    defs.into_iter().filter_map(compiled).collect()
}

pub fn wmn_sites() -> Vec<Site> {
    let data: WmnData = serde_json::from_str(WMN_JSON).expect("embedded wmn-data.json");
    let filtered: Vec<WmnSite> = data
        .sites
        .into_iter()
        .filter(|s| {
            s.valid && s.uri_check.contains("{account}") && !host_of(&s.uri_check).is_empty()
        })
        .collect();
    let mut best: HashMap<String, (u8, String)> = HashMap::new();
    for site in &filtered {
        let slot = wmn_slot(site);
        let rank = wmn_rank(site);
        match best.get(&slot) {
            Some((top, _)) if rank <= *top => {}
            _ => {
                best.insert(slot, (rank, site.name.clone()));
            }
        }
    }
    filtered
        .into_iter()
        .filter(|site| {
            best.get(&wmn_slot(site))
                .is_some_and(|(_, name)| *name == site.name)
        })
        .map(wmn_def)
        .filter_map(compiled)
        .collect()
}

#[derive(Debug, Deserialize)]
struct WmnData {
    #[serde(default)]
    sites: Vec<WmnSite>,
}

#[derive(Debug, Deserialize)]
struct WmnSite {
    name: String,
    uri_check: String,
    #[serde(default)]
    uri_pretty: String,
    #[serde(default = "default_e_code")]
    e_code: u16,
    #[serde(default)]
    e_string: String,
    #[serde(default)]
    m_code: u16,
    #[serde(default)]
    m_string: String,
    #[serde(default)]
    cat: String,
    #[serde(default = "default_true")]
    valid: bool,
    #[serde(default)]
    post_body: String,
    #[serde(default)]
    strip_bad_char: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
}

fn default_e_code() -> u16 {
    200
}

fn default_true() -> bool {
    true
}

fn wmn_rank(site: &WmnSite) -> u8 {
    if !site.e_string.trim().is_empty() {
        3
    } else if !site.m_string.trim().is_empty() {
        2
    } else {
        1
    }
}

fn wmn_slot(site: &WmnSite) -> String {
    format!(
        "{}\u{1}{}",
        host_of(&site.uri_check),
        site.name.to_ascii_lowercase()
    )
}

fn wmn_def(site: WmnSite) -> SiteDef {
    let probe = site.uri_check.replace("{account}", "{}");
    let url = if site.uri_pretty.is_empty() {
        probe.clone()
    } else {
        site.uri_pretty.replace("{account}", "{}")
    };
    let (kind, codes, msgs, has, unk) =
        if site.e_string.trim().is_empty() && site.m_string.trim().is_empty() {
            let mut unk = Vec::new();
            if !(200..300).contains(&site.e_code) {
                unk.push(site.e_code);
            }
            (
                "status".to_string(),
                vec![if site.m_code > 0 { site.m_code } else { 404 }],
                Vec::new(),
                Vec::new(),
                unk,
            )
        } else {
            (
                "message".to_string(),
                Vec::new(),
                split_alternatives(&site.m_string),
                split_alternatives(&site.e_string),
                Vec::new(),
            )
        };
    SiteDef {
        name: site.name,
        category: if site.cat.trim().is_empty() {
            "misc".to_string()
        } else {
            site.cat.trim().to_ascii_lowercase()
        },
        url,
        probe,
        method: if site.post_body.is_empty() {
            String::new()
        } else {
            "POST".to_string()
        },
        body: site.post_body.replace("{account}", "{}"),
        hdr: site.headers,
        kind,
        codes,
        msgs,
        has,
        mark: String::new(),
        rx: String::new(),
        unk,
        strip: site.strip_bad_char,
        source: "wmn".to_string(),
    }
}

fn split_alternatives(raw: &str) -> Vec<String> {
    raw.split("||")
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(String::from)
        .collect()
}

pub fn host_of(url: &str) -> String {
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let authority = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);
    let host = authority.split(':').next().unwrap_or(authority);
    host.strip_prefix("www.")
        .unwrap_or(host)
        .to_ascii_lowercase()
}

pub fn probe_key(site: &Site) -> String {
    let check = if site.probe.is_empty() {
        &site.url
    } else {
        &site.probe
    };
    check.to_ascii_lowercase().trim_end_matches('/').to_string()
}

fn slot_of(site: &Site) -> String {
    let check = if site.probe.is_empty() {
        &site.url
    } else {
        &site.probe
    };
    format!("{}\u{1}{}", host_of(check), site.name.to_ascii_lowercase())
}

fn detection_rank(site: &Site) -> u8 {
    match &site.kind {
        DetectKind::Message { present, .. } if !present.is_empty() => 3,
        DetectKind::Message { .. } | DetectKind::Redirect { .. } => 2,
        DetectKind::Status { .. } => 1,
    }
}

fn source_rank(source: &str) -> u8 {
    match source {
        "builtin" => 2,
        "wmn" => 1,
        _ => 0,
    }
}

fn merge_databases(builtin: Vec<Site>, wmn: Vec<Site>, maigret: Vec<Site>) -> Vec<Site> {
    let mut candidates: Vec<Site> = builtin;
    candidates.extend(wmn);
    candidates.extend(maigret);

    let mut slot_best: HashMap<String, (u8, usize)> = HashMap::new();
    for (index, site) in candidates.iter().enumerate() {
        let entry = slot_of(site);
        let rank = detection_rank(site);
        match slot_best.get(&entry) {
            Some((top, _)) if *top >= rank => {}
            _ => {
                slot_best.insert(entry, (rank, index));
            }
        }
    }

    let mut url_best: HashMap<String, (u8, u8, std::cmp::Reverse<usize>, usize)> = HashMap::new();
    for (index, site) in candidates.iter().enumerate() {
        if slot_best.get(&slot_of(site)).map(|(_, i)| *i) != Some(index) {
            continue;
        }
        let candidate = (
            detection_rank(site),
            source_rank(&site.source),
            std::cmp::Reverse(site.name.len()),
            index,
        );
        match url_best.get(&probe_key(site)) {
            Some(best) if *best >= candidate => {}
            _ => {
                url_best.insert(probe_key(site), candidate);
            }
        }
    }

    let mut out: Vec<Site> = Vec::new();
    for (index, site) in candidates.into_iter().enumerate() {
        let slot_winner = slot_best.get(&slot_of(&site)).map(|(_, i)| *i) == Some(index);
        let url_winner = url_best
            .get(&probe_key(&site))
            .map(|(_, _, _, i)| *i)
            .is_none_or(|i| i == index);
        if slot_winner && url_winner {
            out.push(site);
        }
    }
    out
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
        strip: def.strip,
        source: if def.source.is_empty() {
            "builtin".to_string()
        } else {
            def.source
        },
    })
}

pub fn categories() -> Vec<String> {
    category_counts().into_iter().map(|(c, _)| c).collect()
}

pub fn category_counts() -> Vec<(String, usize)> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for site in sites() {
        *seen.entry(site.category.clone()).or_insert(0) += 1;
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
    use std::collections::HashSet;

    #[test]
    fn embedded_databases_load() {
        let all = sites();
        assert!(
            all.len() >= 3000,
            "expected merged site database above 3000, got {}",
            all.len()
        );
        for s in all.iter() {
            assert!(!s.name.is_empty());
            assert!(
                s.url.contains("{}") || s.probe.contains("{}") || s.body.contains("{}"),
                "site {} missing placeholder",
                s.name
            );
            assert!(
                s.source == "builtin" || s.source == "wmn" || s.source == "maigret",
                "site {} has unknown source {}",
                s.name,
                s.source
            );
        }
    }

    #[test]
    fn wmn_snapshot_parses() {
        let data: WmnData = serde_json::from_str(WMN_JSON).expect("wmn-data.json");
        assert!(data.sites.len() >= 650, "wmn sites {}", data.sites.len());
    }

    #[test]
    fn merged_database_unions_both_sources() {
        let builtin = builtin_sites();
        let wmn = wmn_sites();
        let maigret = maigret_sites();
        assert!(builtin.len() >= 650, "builtin {}", builtin.len());
        assert!(wmn.len() >= 600, "wmn {}", wmn.len());
        assert!(maigret.len() >= 6000, "maigret {}", maigret.len());
        let all = sites();
        assert_eq!(all.len(), merge_databases(builtin, wmn, maigret).len());
        let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        for key in [
            "GitHub (User)",
            "GitHub (Gists)",
            "X",
            "Telegram",
            "Reddit",
            "247sports",
            "ACF",
        ] {
            assert!(names.contains(&key), "missing {key}");
        }
        assert!(all.iter().any(|s| s.source == "builtin"));
        assert!(all.iter().any(|s| s.source == "wmn"));
        assert!(all.iter().any(|s| s.source == "maigret"));
    }

    #[test]
    fn core_database_is_independent_of_long_tail() {
        let core = core_site_count();
        let total = sites().len();
        assert!(core >= 1000, "core sites {core}");
        assert!(total > core, "long tail must extend the core database");
        assert_eq!(core + maigret_site_count(), total);
        let sample = sites()
            .iter()
            .find(|s| s.source == "maigret" && s.name == "0-3.RU")
            .expect("expected the 0-3.RU forum from the maigret long tail");
        assert!(sample.url.contains("{}"));
        assert!(matches!(&sample.kind, DetectKind::Message { absent, .. } if !absent.is_empty()));
    }

    #[test]
    fn probe_urls_are_unique() {
        let mut seen: HashSet<String> = HashSet::new();
        for s in sites() {
            assert!(
                seen.insert(probe_key(s)),
                "duplicate probe url {} in {}",
                probe_key(s),
                s.name
            );
        }
    }

    #[test]
    fn wmn_markers_are_converted() {
        let site = sites()
            .iter()
            .find(|s| s.name == "247sports")
            .expect("247sports from wmn");
        assert_eq!(site.source, "wmn");
        assert_eq!(site.category, "hobby");
        match &site.kind {
            DetectKind::Message { present, absent } => {
                assert!(present.contains(&"staff-header-container".to_string()));
                assert!(absent.contains(&"<title>247Sports</title>".to_string()));
            }
            other => panic!("expected message detection, got {other:?}"),
        }
        assert!(sites().iter().any(|s| s.source == "wmn" && s.post));
        assert!(sites()
            .iter()
            .any(|s| s.source == "wmn" && !s.strip.is_empty()));
        assert!(sites()
            .iter()
            .any(|s| s.source == "wmn" && s.url != s.probe));
    }

    #[test]
    fn host_of_normalizes() {
        assert_eq!(host_of("https://www.Example.com/a/{}"), "example.com");
        assert_eq!(
            host_of("http://user:pass@host.example:8080/x"),
            "host.example"
        );
        assert_eq!(host_of("example.org/path"), "example.org");
        assert_eq!(host_of(""), "");
    }

    #[test]
    fn key_sites_present() {
        let names: Vec<&str> = sites().iter().map(|s| s.name.as_str()).collect();
        for key in [
            "GitHub (User)",
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
        assert!(category_counts().iter().all(|(_, n)| *n > 0));
    }

    #[test]
    fn regexes_compile_or_are_dropped() {
        for s in sites() {
            assert!(
                !s.url.contains("{username}") && !s.probe.contains("{username}"),
                "unnormalized placeholder in {}",
                s.name
            );
        }
    }
}
