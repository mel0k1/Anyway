use serde_json::{json, Value};
use std::collections::HashMap;

/// Entity resolution over a scan snapshot: extract identities from findings,
/// score cross-platform correlations and cluster them, plus build a unified
/// activity timeline from every date-bearing detail field.
pub const DEFAULT_LINK_THRESHOLD: u32 = 45;

#[derive(Debug, Clone, Default)]
pub struct Identity {
    pub idx: usize,
    pub platform: String,
    pub handle: String,
    pub display_name: String,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub url: String,
    pub bio: String,
    pub avatar_hash: Option<u64>,
    pub module_id: String,
    pub category: String,
}

impl Identity {
    fn to_json(&self) -> Value {
        json!({
            "idx": self.idx,
            "platform": self.platform,
            "handle": self.handle,
            "displayName": self.display_name,
            "emails": self.emails,
            "phones": self.phones,
            "url": self.url,
            "bio": self.bio,
            "module": self.module_id,
            "category": self.category,
        })
    }
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

fn norm_handle(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn handle_from_url(url: &str) -> Option<String> {
    let rest = url.trim().split("://").nth(1)?;
    // drop query string and fragment before splitting the path
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return None; // host only, no profile path
    }
    // profile handle is conventionally the last non-empty path segment
    segs.last()
        .map(|s| s.trim_start_matches('@').trim_end_matches('.').to_string())
        .filter(|s| !s.is_empty())
}

fn extract(detail: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = detail.get(*k).and_then(Value::as_str) {
            let s = s.trim();
            if !s.is_empty() && s != "null" {
                return s.to_string();
            }
        }
    }
    String::new()
}

fn is_email(s: &str) -> bool {
    let s = s.trim().to_lowercase();
    s.contains('@')
        && s.split_once('@')
            .map(|(l, d)| !l.is_empty() && d.contains('.'))
            .unwrap_or(false)
}

fn identity_from_finding(f: &Value, idx: usize) -> Option<Identity> {
    let category = f["category"].as_str().unwrap_or("").to_string();
    let module_id = f["module_id"].as_str().unwrap_or("").to_string();
    let value = f["value"].as_str().unwrap_or("").to_string();
    let label = f["label"].as_str().unwrap_or("").to_string();
    let url = f["url"].as_str().unwrap_or("").to_string();
    let detail = f.get("detail").cloned().unwrap_or(Value::Null);

    let mut id = Identity {
        idx,
        platform: label.clone(),
        url: url.clone(),
        module_id,
        category,
        ..Default::default()
    };
    match id.category.as_str() {
        "social_profile" => {
            id.handle = norm(&handle_from_url(&value).unwrap_or(value.clone()));
            if id.handle.is_empty() {
                return None;
            }
            id.platform = label;
            Some(id)
        }
        "api_profile" => {
            let d = if detail.is_object() {
                detail.clone()
            } else {
                Value::Null
            };
            let handle = extract(&d, &["username", "login", "handle"]);
            if handle.is_empty() {
                return None;
            }
            id.handle = norm(&handle);
            id.display_name = extract(&d, &["name", "displayName", "display_name"]);
            id.bio = extract(&d, &["bio", "note", "summary", "description"]);
            if id.platform.is_empty() {
                id.platform = extract(&d, &["platform", "instance"]);
            }
            Some(id)
        }
        "email_account" | "email_info" | "breach" | "breach_info" | "gravatar" => {
            let email = if is_email(&value) {
                value.clone()
            } else {
                extract(&detail, &["email", "address", "emailAddress"])
            };
            if email.is_empty() || !is_email(&email) {
                return None;
            }
            id.emails = vec![email.to_lowercase()];
            id.handle = email.split('@').next().unwrap_or("").to_lowercase();
            if id.handle.is_empty() {
                return None;
            }
            Some(id)
        }
        _ => None,
    }
}

pub fn extract_identities(snapshot: &Value) -> Vec<Identity> {
    let mut out = Vec::new();
    for (idx, f) in snapshot["findings"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        if let Some(id) = identity_from_finding(f, idx) {
            out.push(id);
        }
    }
    out
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1)
                .min(cur[j] + 1)
                .min(prev[j] + usize::from(ca != cb));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn bio_jaccard(a: &str, b: &str) -> f64 {
    let toks = |s: &str| -> std::collections::HashSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() >= 4)
            .map(String::from)
            .collect()
    };
    let (ta, tb) = (toks(a), toks(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let inter = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    inter as f64 / union as f64
}

#[derive(Debug, Clone)]
pub struct ScoredPair {
    pub a: usize,
    pub b: usize,
    pub score: u32,
    pub reasons: Vec<(String, u32)>,
}

impl ScoredPair {
    fn to_json(&self, conf: &'static str) -> Value {
        json!({
            "a": self.a,
            "b": self.b,
            "score": self.score,
            "confidence": conf,
            "reasons": self.reasons.iter().map(|(r, p)| json!({"reason": r, "points": p})).collect::<Vec<_>>(),
        })
    }
}

/// Cross-platform correlation score between two identities (0..=100).
pub fn score_pair(a: &Identity, b: &Identity) -> ScoredPair {
    let mut reasons: Vec<(String, u32)> = Vec::new();
    let mut total: u32 = 0;
    let mut add = |reason: &str, points: u32| {
        reasons.push((reason.to_string(), points));
        total += points;
    };

    let (ha, hb) = (&a.handle, &b.handle);
    if !ha.is_empty() && !hb.is_empty() {
        if ha == hb {
            add("identical handle", 45);
        } else if a.platform.to_lowercase() == b.platform.to_lowercase()
            && norm_handle(ha) == norm_handle(hb)
        {
            add("same platform, same handle modulo separators", 40);
        } else if ha.len() >= 5 && hb.len() >= 5 && norm_handle(ha) == norm_handle(hb) {
            add("handles match ignoring separators", 35);
        } else if ha.len() >= 6 && hb.len() >= 6 && levenshtein(ha, hb) <= 2 {
            add("handles within edit distance 2", 18);
        } else if (ha.len() >= 4 && hb.contains(ha.as_str()))
            || (hb.len() >= 4 && ha.contains(hb.as_str()))
        {
            add("one handle contains the other", 14);
        }
    }

    let (da, db) = (norm(&a.display_name), norm(&b.display_name));
    if !da.is_empty() && !db.is_empty() {
        if da == db {
            add("identical display name", 15);
        } else if (da.len() >= 4 && db.contains(da.as_str()))
            || (db.len() >= 4 && da.contains(db.as_str()))
        {
            add("display names overlap", 8);
        }
    }

    let shared_email = a.emails.iter().any(|e| b.emails.iter().any(|x| x == e));
    if shared_email {
        add("shared email address", 25);
    }

    let email_local_matches = |id_a: &Identity, id_b: &Identity| {
        id_a.emails.iter().any(|e| {
            e.split('@').next().map(norm_handle).as_deref()
                == Some(norm_handle(&id_b.handle).as_str())
                && !norm_handle(&id_b.handle).is_empty()
        })
    };
    if email_local_matches(a, b) || email_local_matches(b, a) {
        add("email local part matches handle", 15);
    }

    if a.phones.iter().any(|p| b.phones.contains(p)) {
        add("shared phone number", 25);
    }

    if let (Some(ha), Some(hb)) = (a.avatar_hash, b.avatar_hash) {
        let d = crate::imaging::hamming(ha, hb);
        if d <= 8 {
            add("avatar images look alike (pHash)", 20);
        }
    }

    if bio_jaccard(&a.bio, &b.bio) >= 0.5 {
        add("similar bio text", 8);
    }

    let score = total.min(100);
    ScoredPair {
        a: a.idx,
        b: b.idx,
        score,
        reasons,
    }
}

fn confidence_of(score: u32) -> &'static str {
    if score >= 70 {
        "high"
    } else if score >= DEFAULT_LINK_THRESHOLD {
        "medium"
    } else {
        "low"
    }
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let p = self.parent[x];
            self.parent[x] = self.find(p);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

pub struct ResolutionReport {
    pub identities: Vec<Identity>,
    pub pairs: Vec<ScoredPair>,
    pub clusters: Vec<Vec<usize>>,
}

impl ResolutionReport {
    pub fn to_json(&self, threshold: u32) -> Value {
        let identities_json: Vec<Value> = self.identities.iter().map(|i| i.to_json()).collect();
        let pairs_json: Vec<Value> = self
            .pairs
            .iter()
            .filter(|p| p.score >= threshold)
            .map(|p| p.to_json(confidence_of(p.score)))
            .collect();
        let clusters: Vec<Value> = self
            .clusters
            .iter()
            .filter(|c| c.len() > 1 || self.identities.len() == 1)
            .map(|c| {
                let members: Vec<Value> = c
                    .iter()
                    .filter_map(|i| self.identities.get(*i).map(|id| id.to_json()))
                    .collect();
                let best = c
                    .iter()
                    .filter_map(|i| self.identities.get(*i))
                    .filter(|id| !id.handle.is_empty())
                    .map(|id| id.handle.clone())
                    .next()
                    .unwrap_or_default();
                json!({
                    "label": best,
                    "size": c.len(),
                    "members": members,
                    "memberIdx": c.clone(),
                })
            })
            .collect();
        let linked = clusters
            .iter()
            .filter(|c| c["size"].as_u64().unwrap_or(0) > 1)
            .count();
        json!({
            "ok": true,
            "threshold": threshold,
            "stats": {
                "identities": self.identities.len(),
                "linkedPairs": pairs_json.len(),
                "clusters": linked,
            },
            "identities": identities_json,
            "pairs": pairs_json,
            "clusters": clusters,
        })
    }
}

pub fn resolve(snapshot: &Value, threshold: u32) -> ResolutionReport {
    let identities = extract_identities(snapshot);
    let mut pairs = Vec::new();
    for i in 0..identities.len() {
        for j in (i + 1)..identities.len() {
            let p = score_pair(&identities[i], &identities[j]);
            if p.score > 0 {
                pairs.push(p);
            }
        }
    }
    let pos: HashMap<usize, usize> = identities
        .iter()
        .enumerate()
        .map(|(i, id)| (id.idx, i))
        .collect();
    let mut uf = UnionFind::new(identities.len());
    for p in &pairs {
        if p.score >= threshold {
            if let (Some(&ia), Some(&ib)) = (pos.get(&p.a), pos.get(&p.b)) {
                uf.union(ia, ib);
            }
        }
    }
    let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..identities.len() {
        clusters.entry(uf.find(i)).or_default().push(i);
    }
    let mut list: Vec<Vec<usize>> = clusters.into_values().collect();
    list.sort_by_key(|c| std::cmp::Reverse(c.len()));
    ResolutionReport {
        identities,
        pairs,
        clusters: list,
    }
}

// ---------------- Timeline ----------------

#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub ts: Option<i64>,
    pub ts_raw: String,
    pub label: String,
    pub kind: String,
    pub source: String,
    pub url: String,
}

impl TimelineEvent {
    fn to_json(&self) -> Value {
        json!({
            "ts": self.ts,
            "date": self.ts_raw,
            "label": self.label,
            "kind": self.kind,
            "source": self.source,
            "url": self.url,
        })
    }
}

const DATE_KEYS: &[&str] = &[
    "createdAt",
    "created_at",
    "joined",
    "joined_at",
    "breachDate",
    "addedDate",
    "AddedDate",
    "not_before",
    "firstSeen",
    "registered_at",
    "published",
    "postedAt",
];

/// Parse the date formats that actually occur in module payloads.
pub fn parse_date(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if s.is_empty() || s == "null" || s == "?" {
        return None;
    }
    // ISO forms: 2013-10-04[ T00:00:00Z...]
    let bytes = s.as_bytes();
    if bytes.len() >= 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && s[..4].chars().all(|c| c.is_ascii_digit())
    {
        let y: i64 = s[..4].parse().ok()?;
        let m: i64 = s[5..7].parse().ok()?;
        let d: i64 = s[8..10].parse().ok()?;
        if (1..=12).contains(&m) && (1..=31).contains(&d) && (1900..=2100).contains(&y) {
            let days = days_from_civil(y, m, d);
            let mut secs = days * 86400;
            if bytes.len() >= 19 && bytes[10] == b'T' || (bytes.len() >= 19 && bytes[10] == b' ') {
                let hh: i64 = s[11..13].parse().unwrap_or(0);
                let mm: i64 = s[14..16].parse().unwrap_or(0);
                let ss: i64 = s[17..19].parse().unwrap_or(0);
                secs += hh * 3600 + mm * 60 + ss;
            }
            return Some(secs);
        }
        return None;
    }
    // RFC1123-ish: 04 Oct 2013
    let months = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let low = s.to_lowercase();
    if let Some(mon) = months.iter().position(|m| low.contains(m)) {
        let toks: Vec<&str> = low.split_whitespace().collect();
        for t in &toks {
            let digits = t.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
            if digits.len() == 4 {
                if let Ok(y) = digits.parse::<i64>() {
                    if (1900..=2100).contains(&y) {
                        let day = toks
                            .iter()
                            .find_map(|t| t.trim_end_matches(',').parse::<i64>().ok())
                            .unwrap_or(1);
                        return Some(days_from_civil(y, mon as i64 + 1, day.clamp(1, 31)) * 86400);
                    }
                }
            }
        }
    }
    None
}

/// Howard Hinnant's days_from_civil algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn push_detail_dates(detail: &Value, keys: &[&str], out: &mut Vec<(String, String)>) {
    for k in keys {
        if let Some(s) = detail.get(*k).and_then(Value::as_str) {
            if parse_date(s).is_some() {
                out.push((k.to_string(), s.trim().to_string()));
            }
        }
    }
}

pub fn build_timeline(snapshot: &Value) -> Value {
    let mut events: Vec<TimelineEvent> = Vec::new();
    for f in snapshot["findings"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or(&[])
    {
        let module_id = f["module_id"].as_str().unwrap_or("").to_string();
        let category = f["category"].as_str().unwrap_or("").to_string();
        let label = f["label"].as_str().unwrap_or("").to_string();
        let url = f["url"].as_str().unwrap_or("").to_string();
        let detail = f.get("detail").cloned().unwrap_or(Value::Null);
        if !detail.is_object() {
            continue;
        }

        // flat date keys
        let mut found: Vec<(String, String)> = Vec::new();
        push_detail_dates(&detail, DATE_KEYS, &mut found);
        for (key, raw) in found {
            events.push(TimelineEvent {
                ts: parse_date(&raw),
                ts_raw: raw.clone(),
                label: format!("{label} · {key}"),
                kind: category.clone(),
                source: module_id.clone(),
                url: url.clone(),
            });
        }

        // rdap-style event list
        if let Some(evts) = detail["events"].as_array() {
            for e in evts {
                let action = e["eventAction"].as_str().unwrap_or("event").to_string();
                let date = e["eventDate"].as_str().unwrap_or("");
                if let Some(ts) = parse_date(date) {
                    events.push(TimelineEvent {
                        ts: Some(ts),
                        ts_raw: date.trim().to_string(),
                        label: format!("{label} · {action}"),
                        kind: category.clone(),
                        source: module_id.clone(),
                        url: url.clone(),
                    });
                }
            }
        }

        // certificate lists
        if let Some(certs) = detail["certs"].as_array() {
            for c in certs {
                for key in ["not_before", "not_after"] {
                    if let Some(raw) = c[key].as_str() {
                        if let Some(ts) = parse_date(raw) {
                            events.push(TimelineEvent {
                                ts: Some(ts),
                                ts_raw: raw.trim().to_string(),
                                label: format!(
                                    "certificate {} ({})",
                                    if key == "not_before" {
                                        "issued"
                                    } else {
                                        "expires"
                                    },
                                    c["issuer"].as_str().unwrap_or("unknown")
                                ),
                                kind: "certificates".into(),
                                source: module_id.clone(),
                                url: url.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    events.sort_by_key(|e| e.ts.unwrap_or(i64::MAX));
    json!({
        "ok": true,
        "events": events.iter().map(|e| e.to_json()).collect::<Vec<_>>(),
        "total": events.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(findings: Value) -> Value {
        json!({"findings": findings})
    }

    #[test]
    fn extracts_social_and_api_identities() {
        let s = snap(json!([
            {"module_id": "username.enumerate", "category": "social_profile", "label": "GitHub",
             "value": "https://github.com/mel0k1", "url": "https://github.com/mel0k1"},
            {"module_id": "enrich", "category": "api_profile", "label": "GitHub API",
             "value": "https://api.github.com/users/mel0k1", "detail": {"username": "Mel0K1", "name": "Mel", "bio": "osint researcher"}},
            {"module_id": "breaches", "category": "breach", "label": "Adobe", "value": "victim@example.org"},
            {"module_id": "domain.intel", "category": "dns_records", "label": "DNS records", "value": "A:2"}
        ]));
        let ids = extract_identities(&s);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].handle, "mel0k1");
        assert_eq!(ids[0].platform, "GitHub");
        assert_eq!(ids[1].handle, "mel0k1"); // case-normalized
        assert_eq!(ids[1].display_name, "Mel");
        assert_eq!(ids[2].emails, vec!["victim@example.org"]);
        assert_eq!(ids[2].handle, "victim");
    }

    #[test]
    fn scores_handle_and_email_correlation() {
        let a = Identity {
            idx: 0,
            platform: "GitHub".into(),
            handle: "mel0k1".into(),
            emails: vec!["me@corp.io".into()],
            ..Default::default()
        };
        let b = Identity {
            idx: 1,
            platform: "GitLab".into(),
            handle: "mel0k1".into(),
            emails: vec!["me@corp.io".into()],
            ..Default::default()
        };
        let p = score_pair(&a, &b);
        // identical handle (45) + shared email (25)
        assert!(p.score >= 70, "score {}", p.score);
        assert_eq!(confidence_of(p.score), "high");

        let c = Identity {
            idx: 2,
            platform: "Steam".into(),
            handle: "totallydifferent".into(),
            ..Default::default()
        };
        assert_eq!(score_pair(&a, &c).score, 0);
    }

    #[test]
    fn email_local_matches_handle() {
        let a = Identity {
            idx: 0,
            platform: "Breaches".into(),
            handle: "victim".into(),
            emails: vec!["victim@example.org".into()],
            ..Default::default()
        };
        let b = Identity {
            idx: 1,
            platform: "SomeForum".into(),
            handle: "victim".into(),
            ..Default::default()
        };
        let p = score_pair(&a, &b);
        assert!(
            p.score >= 45 + 15,
            "handle 45 + email-local 15, got {}",
            p.score
        );
    }

    #[test]
    fn clusters_linked_identities() {
        let s = snap(json!([
            {"module_id": "username.enumerate", "category": "social_profile", "label": "GitHub",
             "value": "https://github.com/mel0k1", "url": "https://github.com/mel0k1"},
            {"module_id": "username.enumerate", "category": "social_profile", "label": "GitLab",
             "value": "https://gitlab.com/mel0k1", "url": "https://gitlab.com/mel0k1"},
            {"module_id": "username.enumerate", "category": "social_profile", "label": "X",
             "value": "https://x.com/other_user", "url": "https://x.com/other_user"}
        ]));
        let report = resolve(&s, DEFAULT_LINK_THRESHOLD);
        assert_eq!(report.identities.len(), 3);
        assert_eq!(report.clusters[0].len(), 2);
        let j = report.to_json(DEFAULT_LINK_THRESHOLD);
        assert_eq!(j["stats"]["clusters"], 1);
        assert_eq!(j["stats"]["identities"], 3);
        assert!(!j["pairs"].as_array().unwrap().is_empty());
        assert_eq!(j["clusters"][0]["label"], "mel0k1");
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("", "ab"), 2);
    }

    #[test]
    fn parses_common_dates() {
        assert_eq!(parse_date("2013-10-04"), Some(1_380_844_800));
        assert_eq!(parse_date("2013-10-04T00:00:00Z"), Some(1_380_844_800));
        assert!(parse_date("04 Oct 2013").is_some());
        assert!(parse_date("2015-03-08T10:20:30Z").is_some());
        assert_eq!(parse_date("not a date"), None);
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("null"), None);
    }

    #[test]
    fn timeline_orders_events() {
        let s = snap(json!([
            {"module_id": "breaches", "category": "breach", "label": "Adobe",
             "value": "a@b.co", "detail": {"breachDate": "2013-10-04", "pwnCount": 152}},
            {"module_id": "enrich", "category": "api_profile", "label": "GitHub",
             "value": "u", "detail": {"createdAt": "2015-03-08T10:20:30Z", "username": "u"}},
            {"module_id": "domain.intel", "category": "domain_rdap", "label": "RDAP",
             "value": "corp.io", "detail": {"events": [
                {"eventAction": "registration", "eventDate": "2012-01-15T00:00:00Z"},
                {"eventAction": "expiration", "eventDate": "2027-01-15"}
             ]}},
            {"module_id": "domain.intel", "category": "certificates", "label": "crt.sh",
             "value": "corp.io", "detail": {"certs": [
                {"issuer": "Let's Encrypt", "not_before": "2024-02-01", "not_after": "2024-05-01"}
             ]}},
            {"module_id": "x", "category": "dns_records", "label": "no dates", "value": "x", "detail": {"foo": true}}
        ]));
        let t = build_timeline(&s);
        let evts = t["events"].as_array().unwrap();
        assert_eq!(t["total"], 6);
        assert_eq!(evts.len(), 6);
        // sorted ascending
        let tss: Vec<i64> = evts.iter().filter_map(|e| e["ts"].as_i64()).collect();
        let mut sorted = tss.clone();
        sorted.sort();
        assert_eq!(tss, sorted);
        assert_eq!(evts[0]["source"], "domain.intel"); // registration 2012
        assert!(evts.last().unwrap()["date"]
            .as_str()
            .unwrap()
            .starts_with("2027"));
    }

    #[test]
    fn identity_url_handle_edge_cases() {
        assert_eq!(
            handle_from_url("https://x.com/@user").as_deref(),
            Some("user")
        );
        assert_eq!(
            handle_from_url("https://forum.tld/u/name?tab=1").as_deref(),
            Some("name")
        );
        assert_eq!(
            handle_from_url("https://steamcommunity.com/id/mel0k1").as_deref(),
            Some("mel0k1")
        );
        assert_eq!(
            handle_from_url("https://github.com/mel0k1/").as_deref(),
            Some("mel0k1")
        );
        assert_eq!(handle_from_url("notaurl"), None);
        assert_eq!(handle_from_url("https://x.com"), None);
    }
}
