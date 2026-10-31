use crate::graph::domain_from_url;
use crate::models::{Confidence, Finding, Target, TargetKind};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct PivotCandidate {
    pub target: Target,
    pub via: String,
    pub confidence: Confidence,
    pub parent: String,
    pub parent_kind: TargetKind,
}

pub const ROLE_PREFIXES: &[&str] = &[
    "abuse",
    "admin",
    "administrator",
    "support",
    "contact",
    "info",
    "security",
    "webmaster",
    "hostmaster",
    "postmaster",
    "noreply",
    "no-reply",
    "privacy",
    "legal",
    "dmca",
    "sales",
    "marketing",
    "help",
    "office",
    "billing",
    "root",
    "sysadmin",
];

pub const PROVIDER_DOMAINS: &[&str] = &[
    "github.com",
    "gitlab.com",
    "twitter.com",
    "x.com",
    "instagram.com",
    "facebook.com",
    "tiktok.com",
    "youtube.com",
    "reddit.com",
    "linkedin.com",
    "pinterest.com",
    "medium.com",
    "tumblr.com",
    "blogspot.com",
    "wordpress.com",
    "gmail.com",
    "googlemail.com",
    "yahoo.com",
    "outlook.com",
    "hotmail.com",
    "live.com",
    "msn.com",
    "icloud.com",
    "me.com",
    "aol.com",
    "proton.me",
    "protonmail.com",
    "pm.me",
    "yandex.ru",
    "yandex.com",
    "mail.ru",
    "inbox.ru",
    "bk.ru",
    "list.ru",
    "gmx.com",
    "gmx.de",
    "gmx.net",
    "web.de",
    "qq.com",
    "163.com",
    "zoho.com",
    "fastmail.com",
    "tutanota.com",
    "discord.com",
    "discordapp.com",
    "telegram.me",
    "t.me",
    "whatsapp.com",
    "steamcommunity.com",
    "steampowered.com",
    "twitch.tv",
    "spotify.com",
    "vk.com",
    "ok.ru",
    "cloudflare.com",
    "amazonaws.com",
    "azurewebsites.net",
    "vercel.app",
    "netlify.app",
    "github.io",
    "gitlab.io",
    "pages.dev",
    "wixsite.com",
];

const MAX_RECORD_PIVOTS: usize = 4;

fn rank(c: Confidence) -> u8 {
    match c {
        Confidence::High => 2,
        Confidence::Medium => 1,
        Confidence::Low => 0,
    }
}

pub fn routable_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || o[0] == 0
                || o[0] == 100 && (o[1] & 0xc0) == 64
                || o[0] >= 224)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || (s[0] == 0x2001 && s[1] == 0x0db8)
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80
                || s[0] & 0xff00 == 0xff00)
        }
    }
}

fn candidate(
    kind: TargetKind,
    raw: &str,
    via: &str,
    confidence: Confidence,
    parent: &str,
    parent_kind: TargetKind,
) -> Option<PivotCandidate> {
    if confidence == Confidence::Low {
        return None;
    }
    let value = raw.trim().trim_start_matches('@').to_lowercase();
    if value.len() > 253 {
        return None;
    }
    match kind {
        TargetKind::Email => {
            let (local, domain) = value.split_once('@')?;
            if local.len() < 2
                || !domain.contains('.')
                || ROLE_PREFIXES.iter().any(|p| local.starts_with(p))
                || !value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".@_-+".contains(c))
            {
                return None;
            }
        }
        TargetKind::Username => {
            if value.len() < 2
                || value.len() > 64
                || !value.chars().next().is_some_and(|c| c.is_alphanumeric())
                || !value
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
            {
                return None;
            }
        }
        TargetKind::Domain => {
            let host = domain_from_url(&value)?;
            if host.len() < 4 || PROVIDER_DOMAINS.contains(&host.as_str()) {
                return None;
            }
            return Some(PivotCandidate {
                target: Target {
                    kind,
                    raw: host.clone(),
                    normalized: host,
                },
                via: via.to_string(),
                confidence,
                parent: parent.trim().to_lowercase(),
                parent_kind,
            });
        }
        TargetKind::IpAddress => {
            let ip: IpAddr = value.parse().ok()?;
            if !routable_ip(ip) {
                return None;
            }
        }
        _ => return None,
    }
    Some(PivotCandidate {
        target: Target {
            kind,
            raw: value.clone(),
            normalized: value,
        },
        via: via.to_string(),
        confidence,
        parent: parent.trim().to_lowercase(),
        parent_kind,
    })
}

fn detail_str(detail: &Value, key: &str) -> Option<String> {
    detail
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn detail_list(detail: &Value, key: &str) -> Vec<String> {
    detail
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn candidates_from(finding: &Finding) -> Vec<PivotCandidate> {
    let via = &finding.module_id;
    let detail = finding.detail.clone().unwrap_or(Value::Null);
    let mut out = Vec::new();
    let kind = finding
        .target
        .as_deref()
        .map(|raw| crate::detect::detect_target(raw).kind);

    match finding.category.as_str() {
        "api_profile" => {
            if let Some(email) = detail_str(&detail, "email") {
                if let Some(c) = candidate(
                    TargetKind::Email,
                    &email,
                    via,
                    Confidence::Medium,
                    &email,
                    TargetKind::Email,
                ) {
                    out.push(c);
                }
            }
            for key in ["blog", "website", "website_url"] {
                if let Some(site) = detail_str(&detail, key) {
                    if let Some(host) = domain_from_url(&site) {
                        if let Some(c) = candidate(
                            TargetKind::Domain,
                            &host,
                            via,
                            Confidence::Medium,
                            &host,
                            TargetKind::Domain,
                        ) {
                            out.push(c);
                        }
                    }
                }
            }
            for key in ["twitter", "twitter_username", "github", "github_username"] {
                if let Some(handle) = detail_str(&detail, key) {
                    let parent = kind.unwrap_or(TargetKind::Username);
                    if let Some(c) = candidate(
                        TargetKind::Username,
                        &handle,
                        via,
                        Confidence::Medium,
                        &finding.target.clone().unwrap_or_default(),
                        parent,
                    ) {
                        out.push(c);
                    }
                }
            }
        }
        "ip_rdap" => {
            if let Some(abuse) = detail_str(&detail, "abuse") {
                let parent = finding.target.clone().unwrap_or_default();
                if let Some(c) = candidate(
                    TargetKind::Email,
                    &abuse,
                    via,
                    Confidence::Medium,
                    &parent,
                    TargetKind::IpAddress,
                ) {
                    out.push(c);
                }
            }
        }
        "ip_ptr" => {
            let parent = finding.target.clone().unwrap_or_default();
            for rec in detail_list(&detail, "records")
                .into_iter()
                .take(MAX_RECORD_PIVOTS)
            {
                if let Some(host) = domain_from_url(&rec) {
                    if let Some(c) = candidate(
                        TargetKind::Domain,
                        &host,
                        via,
                        Confidence::Medium,
                        &parent,
                        TargetKind::IpAddress,
                    ) {
                        out.push(c);
                    }
                }
            }
        }
        "subdomains" => {
            let parent = finding.target.clone().unwrap_or_default();
            for sub in detail_list(&detail, "list")
                .into_iter()
                .take(MAX_RECORD_PIVOTS)
            {
                if let Some(host) = domain_from_url(&sub) {
                    if let Some(c) = candidate(
                        TargetKind::Domain,
                        &host,
                        via,
                        Confidence::High,
                        &parent,
                        TargetKind::Domain,
                    ) {
                        out.push(c);
                    }
                }
            }
        }
        "dns_records" => {
            let parent = finding.target.clone().unwrap_or_default();
            for rrtype in ["A", "AAAA"] {
                for rec in detail_list(&detail, rrtype)
                    .into_iter()
                    .take(MAX_RECORD_PIVOTS)
                {
                    if crate::detect::is_ip_address(rec.trim()) {
                        if let Some(c) = candidate(
                            TargetKind::IpAddress,
                            rec.trim(),
                            via,
                            Confidence::High,
                            &parent,
                            TargetKind::Domain,
                        ) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    out
}

pub fn plan(
    findings: &[Finding],
    scanned: &HashSet<(TargetKind, String)>,
    budget: usize,
) -> Vec<PivotCandidate> {
    if budget == 0 {
        return Vec::new();
    }
    let mut best: HashMap<(TargetKind, String), PivotCandidate> = HashMap::new();
    for f in findings {
        for c in candidates_from(f) {
            let key = (c.target.kind, c.target.normalized.clone());
            if scanned.contains(&key) {
                continue;
            }
            match best.get(&key) {
                Some(e) if rank(e.confidence) >= rank(c.confidence) => {}
                _ => {
                    best.insert(key, c);
                }
            }
        }
    }
    let mut out: Vec<PivotCandidate> = best.into_values().collect();
    out.sort_by(|a, b| {
        rank(b.confidence)
            .cmp(&rank(a.confidence))
            .then_with(|| a.target.normalized.cmp(&b.target.normalized))
    });
    out.truncate(budget);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Target;

    fn finding(
        category: &str,
        label: &str,
        value: &str,
        conf: Confidence,
        target: &str,
    ) -> Finding {
        Finding {
            module_id: "test.module".into(),
            category: category.into(),
            label: label.into(),
            value: value.into(),
            url: None,
            confidence: conf,
            target: Some(target.into()),
            detail: None,
        }
    }

    fn detail_finding(category: &str, target: &str, detail: serde_json::Value) -> Finding {
        Finding {
            module_id: "test.module".into(),
            category: category.into(),
            label: "x".into(),
            value: "x".into(),
            url: None,
            confidence: Confidence::Medium,
            target: Some(target.into()),
            detail: Some(detail),
        }
    }

    #[test]
    fn extracts_api_profile_pivots() {
        let f = detail_finding(
            "api_profile",
            "mel0k1",
            serde_json::json!({
                "email": "jane@example.org",
                "blog": "https://blog.jane.dev/about",
                "twitter": "janedoe"
            }),
        );
        let cands = candidates_from(&f);
        let kinds: Vec<(TargetKind, String)> = cands
            .iter()
            .map(|c| (c.target.kind, c.target.normalized.clone()))
            .collect();
        assert!(kinds.contains(&(TargetKind::Email, "jane@example.org".into())));
        assert!(kinds.contains(&(TargetKind::Domain, "blog.jane.dev".into())));
        assert!(kinds.contains(&(TargetKind::Username, "janedoe".into())));
    }

    #[test]
    fn extracts_dns_and_subdomain_pivots() {
        let f = detail_finding(
            "dns_records",
            "example.org",
            serde_json::json!({"A": ["8.8.8.8", "192.168.1.4"], "AAAA": ["2606:4700::1111"]}),
        );
        let cands = candidates_from(&f);
        let ips: Vec<&str> = cands
            .iter()
            .filter(|c| c.target.kind == TargetKind::IpAddress)
            .map(|c| c.target.normalized.as_str())
            .collect();
        assert_eq!(ips.len(), 2);
        assert!(ips.contains(&"8.8.8.8"));
        assert!(!ips.contains(&"192.168.1.4"));

        let f2 = detail_finding(
            "subdomains",
            "example.org",
            serde_json::json!({"list": ["mail.example.org", "dev.example.org"]}),
        );
        let cands2 = candidates_from(&f2);
        assert!(cands2
            .iter()
            .all(|c| c.target.kind == TargetKind::Domain && c.confidence == Confidence::High));
        assert_eq!(cands2.len(), 2);
    }

    #[test]
    fn role_and_provider_filters_apply() {
        let abuse = detail_finding(
            "ip_rdap",
            "8.8.8.8",
            serde_json::json!({"abuse": "abuse@isp.net"}),
        );
        assert!(candidates_from(&abuse).is_empty());

        let person = detail_finding(
            "ip_rdap",
            "8.8.8.8",
            serde_json::json!({"abuse": "ops@isp.net"}),
        );
        let cands = candidates_from(&person);
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].target.normalized, "ops@isp.net");
        assert_eq!(cands[0].parent, "8.8.8.8");

        let web = detail_finding(
            "api_profile",
            "mel0k1",
            serde_json::json!({"website": "https://github.com/mel0k1"}),
        );
        assert!(candidates_from(&web).is_empty());
    }

    #[test]
    fn low_confidence_never_pivots() {
        let f = finding("api_profile", "x", "y", Confidence::Low, "mel0k1");
        assert!(candidates_from(&f).is_empty());
    }

    #[test]
    fn plan_dedups_and_respects_scanned_and_budget() {
        let f1 = detail_finding(
            "api_profile",
            "mel0k1",
            serde_json::json!({"email": "jane@example.org"}),
        );
        let f2 = detail_finding(
            "ip_rdap",
            "8.8.8.8",
            serde_json::json!({"abuse": "ops@isp.net"}),
        );
        let mut scanned = HashSet::new();
        scanned.insert((TargetKind::Email, "jane@example.org".into()));

        let picked = plan(&[f1.clone(), f2.clone()], &scanned, 8);
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].target.normalized, "ops@isp.net");

        let picked2 = plan(&[f1, f2.clone(), f2.clone()], &HashSet::new(), 1);
        assert_eq!(picked2.len(), 1);

        assert!(plan(&[f2], &HashSet::new(), 0).is_empty());
    }

    #[test]
    fn private_and_reserved_ips_rejected() {
        for ip in [
            "127.0.0.1",
            "10.0.0.5",
            "192.168.0.1",
            "224.0.0.9",
            "169.254.1.1",
            "0.0.0.0",
        ] {
            assert!(!routable_ip(ip.parse().unwrap()), "{ip} must be rejected");
        }
        assert!(routable_ip("8.8.8.8".parse().unwrap()));
        assert!(routable_ip("2606:4700::1111".parse().unwrap()));
        assert!(!routable_ip("::1".parse().unwrap()));
        assert!(!routable_ip("fe80::1".parse().unwrap()));
        assert!(!routable_ip("fc00::1".parse().unwrap()));
    }

    #[test]
    fn target_normalization_matches_graph_ids() {
        let c = candidate(
            TargetKind::Domain,
            "https://Blog.Jane.dev/about",
            "m",
            Confidence::Medium,
            "x",
            TargetKind::Username,
        )
        .expect("candidate");
        assert_eq!(c.target.normalized, "blog.jane.dev");
        assert_eq!(c.parent, "x");
    }

    #[test]
    fn scan_state_pivot_defaults_serialize() {
        let opts: crate::models::ScanOptions = serde_json::from_str("{}").unwrap();
        assert!(!opts.pivot);
        assert_eq!(opts.pivot_depth, 2);
        assert_eq!(opts.pivot_budget, 8);
        assert!(!opts.long_tail);
        let _t = Target {
            kind: TargetKind::Username,
            raw: "x".into(),
            normalized: "x".into(),
        };
    }
}
