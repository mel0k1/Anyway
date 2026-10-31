use crate::models::{Confidence, Finding, PivotLink, Target, TargetKind};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_SUBDOMAIN_NODES: usize = 48;
const MAX_RESOLVE_NODES: usize = 16;
const MAX_NS_NODES: usize = 4;
const MAX_MX_NODES: usize = 6;
const MAX_SOCIAL_NODES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Username,
    Email,
    Ip,
    Domain,
    Phone,
    File,
    Platform,
    Org,
    Country,
    Location,
    Device,
    Person,
    Provider,
    Infra,
    Lead,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub confidence: Confidence,
    pub sources: Vec<String>,
    pub root: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub confidence: Confidence,
    pub via: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OsintGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl OsintGraph {
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

pub fn confidence_rank(c: Confidence) -> u8 {
    match c {
        Confidence::High => 2,
        Confidence::Medium => 1,
        Confidence::Low => 0,
    }
}

pub fn max_confidence(a: Confidence, b: Confidence) -> Confidence {
    if confidence_rank(a) >= confidence_rank(b) {
        a
    } else {
        b
    }
}

pub fn entity_id(kind: NodeKind, value: &str) -> String {
    format!("{kind:?}:{}", value.trim().to_lowercase())
}

pub(crate) fn target_node_kind(kind: TargetKind) -> NodeKind {
    match kind {
        TargetKind::Username => NodeKind::Username,
        TargetKind::Email => NodeKind::Email,
        TargetKind::IpAddress => NodeKind::Ip,
        TargetKind::Domain => NodeKind::Domain,
        TargetKind::PhoneNumber => NodeKind::Phone,
        TargetKind::File => NodeKind::File,
        TargetKind::Unknown => NodeKind::Other,
    }
}

fn target_id(t: &Target) -> String {
    entity_id(target_node_kind(t.kind), &t.normalized)
}

fn is_email_like(s: &str) -> bool {
    let s = s.trim();
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !s.contains(' ')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || ".@_-+".contains(c))
}

pub fn domain_from_url(input: &str) -> Option<String> {
    let s = input.trim();
    let host = if let Some((_, rest)) = s.split_once("://") {
        rest.split('/').next().unwrap_or("")
    } else if s.contains('/') {
        s.split('/').next().unwrap_or("")
    } else {
        s
    };
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    let host = host.trim_end_matches('.').to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    Some(host)
}

fn strip_list_marker(s: &str) -> &str {
    let s = s.trim();
    match s.find(char::is_whitespace) {
        Some(i) if s[..i].chars().all(|c| c.is_ascii_digit() || c == '.') => s[i..].trim(),
        _ => s,
    }
}

struct Builder {
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeMap<String, GraphEdge>,
    target_ids: Vec<String>,
    target_raws: Vec<String>,
}

impl Builder {
    fn new(targets: &[Target]) -> Self {
        let mut b = Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            target_ids: Vec::new(),
            target_raws: Vec::new(),
        };
        for t in targets {
            let id = target_id(t);
            b.target_ids.push(id.clone());
            b.target_raws.push(t.raw.clone());
            b.node(
                &id,
                target_node_kind(t.kind),
                if t.raw.is_empty() {
                    t.normalized.clone()
                } else {
                    t.raw.clone()
                },
                Confidence::High,
                "scan_target",
                true,
            );
        }
        b
    }

    fn node(
        &mut self,
        id: &str,
        kind: NodeKind,
        label: String,
        confidence: Confidence,
        source: &str,
        root: bool,
    ) {
        let entry = self
            .nodes
            .entry(id.to_string())
            .or_insert_with(|| GraphNode {
                id: id.to_string(),
                kind,
                label,
                confidence,
                sources: Vec::new(),
                root,
            });
        entry.confidence = max_confidence(entry.confidence, confidence);
        if root {
            entry.root = true;
        }
        if !entry.sources.iter().any(|s| s == source) {
            entry.sources.push(source.to_string());
        }
    }

    fn edge(&mut self, from: &str, to: &str, relation: &str, confidence: Confidence, via: &str) {
        if from == to {
            return;
        }
        let key = format!("{from}\u{1}{to}\u{1}{relation}");
        let entry = self.edges.entry(key).or_insert_with(|| GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            relation: relation.to_string(),
            confidence,
            via: via.to_string(),
        });
        entry.confidence = max_confidence(entry.confidence, confidence);
    }

    fn root_for(&self, finding: &Finding) -> &str {
        if let Some(raw) = finding.target.as_deref() {
            if let Some(i) = self.target_raws.iter().position(|r| r == raw) {
                return &self.target_ids[i];
            }
        }
        self.target_ids.first().map(|s| s.as_str()).unwrap_or("")
    }

    fn finish(self) -> OsintGraph {
        let mut nodes: Vec<GraphNode> = self.nodes.into_values().collect();
        for n in &mut nodes {
            n.sources.sort();
        }
        OsintGraph {
            nodes,
            edges: self.edges.into_values().collect(),
        }
    }
}

fn platform_id(label: &str) -> String {
    entity_id(NodeKind::Platform, label)
}

fn extract_domain_entities(
    b: &mut Builder,
    root: &str,
    platform: Option<&str>,
    raw: &str,
    relation: &str,
    confidence: Confidence,
    via: &str,
) {
    if let Some(domain) = domain_from_url(raw) {
        let id = entity_id(NodeKind::Domain, &domain);
        b.node(&id, NodeKind::Domain, domain, confidence, via, false);
        let from = platform
            .map(str::to_string)
            .unwrap_or_else(|| root.to_string());
        b.edge(&from, &id, relation, confidence, via);
    }
}

fn extract_email_entity(
    b: &mut Builder,
    from: &str,
    raw: &str,
    relation: &str,
    confidence: Confidence,
    via: &str,
) {
    let email = raw.trim().to_lowercase();
    if !is_email_like(&email) {
        return;
    }
    let id = entity_id(NodeKind::Email, &email);
    b.node(&id, NodeKind::Email, email, confidence, via, false);
    b.edge(from, &id, relation, confidence, via);
}

fn extract_username_entity(
    b: &mut Builder,
    from: &str,
    raw: &str,
    relation: &str,
    confidence: Confidence,
    via: &str,
) {
    let handle = raw.trim().trim_start_matches('@').to_lowercase();
    if handle.is_empty()
        || handle.len() > 64
        || !handle
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-')
        || !handle.chars().next().is_some_and(|c| c.is_alphanumeric())
    {
        return;
    }
    let id = entity_id(NodeKind::Username, &handle);
    b.node(&id, NodeKind::Username, handle, confidence, via, false);
    b.edge(from, &id, relation, confidence, via);
}

fn extract_text_location(b: &mut Builder, from: &str, raw: &str, via: &str) {
    let loc = raw.trim();
    if loc.is_empty() || loc.len() > 96 || loc.eq_ignore_ascii_case("unknown") || loc == "-" {
        return;
    }
    let id = entity_id(NodeKind::Location, loc);
    b.node(
        &id,
        NodeKind::Location,
        loc.to_string(),
        Confidence::Low,
        via,
        false,
    );
    b.edge(from, &id, "located_at", Confidence::Low, via);
}

fn add_subdomain_entities(b: &mut Builder, root: &str, subs: &[String], via: &str) {
    for sub in subs.iter().take(MAX_SUBDOMAIN_NODES) {
        let Some(host) = domain_from_url(sub) else {
            continue;
        };
        if host.split('.').count() < 3 {
            continue;
        }
        let id = entity_id(NodeKind::Domain, &host);
        b.node(
            &id,
            NodeKind::Domain,
            host.clone(),
            Confidence::High,
            via,
            false,
        );
        b.edge(&id, root, "subdomain_of", Confidence::High, via);
    }
}

fn add_mx_entities(b: &mut Builder, root: &str, records: &[String], via: &str) {
    for rec in records.iter().take(MAX_MX_NODES) {
        let Some(host) = domain_from_url(strip_list_marker(rec)) else {
            continue;
        };
        let id = entity_id(NodeKind::Provider, &host);
        b.node(&id, NodeKind::Provider, host, Confidence::High, via, false);
        b.edge(root, &id, "mail_via", Confidence::High, via);
    }
}

fn add_resolve_entities(b: &mut Builder, root: &str, records: &[String], via: &str) {
    for rec in records.iter().take(MAX_RESOLVE_NODES) {
        let trimmed = rec.trim();
        if crate::detect::is_ip_address(trimmed) {
            let id = entity_id(NodeKind::Ip, trimmed);
            b.node(
                &id,
                NodeKind::Ip,
                trimmed.to_string(),
                Confidence::High,
                via,
                false,
            );
            b.edge(root, &id, "resolves_to", Confidence::High, via);
        }
    }
}

fn add_ns_entities(b: &mut Builder, root: &str, records: &[String], via: &str) {
    for rec in records.iter().take(MAX_NS_NODES) {
        let Some(host) = domain_from_url(strip_list_marker(rec)) else {
            continue;
        };
        let id = entity_id(NodeKind::Infra, &host);
        b.node(&id, NodeKind::Infra, host, Confidence::Low, via, false);
        b.edge(root, &id, "served_by", Confidence::Low, via);
    }
}

fn add_ptr_entities(b: &mut Builder, root: &str, records: &[String], via: &str) {
    for rec in records.iter().take(MAX_RESOLVE_NODES) {
        let Some(host) = domain_from_url(rec) else {
            continue;
        };
        let id = entity_id(NodeKind::Domain, &host);
        b.node(&id, NodeKind::Domain, host, Confidence::Medium, via, false);
        b.edge(root, &id, "ptr_record", Confidence::Medium, via);
    }
}

fn add_org_entity(
    b: &mut Builder,
    root: &str,
    name: &str,
    relation: &str,
    confidence: Confidence,
    via: &str,
) {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("unknown") || name == "-" {
        return;
    }
    let id = entity_id(NodeKind::Org, name);
    b.node(&id, NodeKind::Org, name.to_string(), confidence, via, false);
    b.edge(root, &id, relation, confidence, via);
}

fn add_country_entity(
    b: &mut Builder,
    root: &str,
    code: &str,
    relation: &str,
    confidence: Confidence,
    via: &str,
) {
    let code = code.trim();
    if code.is_empty() || code.eq_ignore_ascii_case("unknown") || code == "-" {
        return;
    }
    let id = entity_id(NodeKind::Country, code);
    b.node(
        &id,
        NodeKind::Country,
        code.to_string(),
        confidence,
        via,
        false,
    );
    b.edge(root, &id, relation, confidence, via);
}

fn add_person_entity(b: &mut Builder, from: &str, name: &str, via: &str, confidence: Confidence) {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return;
    }
    let id = entity_id(NodeKind::Person, name);
    b.node(
        &id,
        NodeKind::Person,
        name.to_string(),
        confidence,
        via,
        false,
    );
    b.edge(from, &id, "named_as", confidence, via);
}

fn detail_str(detail: &serde_json::Value, key: &str) -> Option<String> {
    detail
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn detail_list(detail: &serde_json::Value, key: &str) -> Vec<String> {
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

fn apply_finding(b: &mut Builder, f: &Finding) {
    let root = b.root_for(f).to_string();
    let via = &f.module_id;
    let detail = f.detail.clone().unwrap_or(serde_json::Value::Null);

    match f.category.as_str() {
        "social_profile" => {
            let id = platform_id(&f.label);
            b.node(
                &id,
                NodeKind::Platform,
                f.label.clone(),
                f.confidence,
                via,
                false,
            );
            b.edge(&root, &id, "profile_on", f.confidence, via);
        }
        "api_profile" => {
            let id = platform_id(&f.label);
            b.node(
                &id,
                NodeKind::Platform,
                f.label.clone(),
                Confidence::High,
                via,
                false,
            );
            b.edge(&root, &id, "profile_on", Confidence::High, via);
            if let Some(email) = detail_str(&detail, "email") {
                extract_email_entity(b, &id, &email, "exposes_email", Confidence::Medium, via);
            }
            for key in ["blog", "website", "website_url"] {
                if let Some(site) = detail_str(&detail, key) {
                    extract_domain_entities(
                        b,
                        &root,
                        Some(&id),
                        &site,
                        "links_to",
                        Confidence::Medium,
                        via,
                    );
                }
            }
            for key in ["twitter", "twitter_username", "github", "github_username"] {
                if let Some(handle) = detail_str(&detail, key) {
                    extract_username_entity(b, &id, &handle, "alt_handle", Confidence::Medium, via);
                }
            }
            if let Some(company) = detail_str(&detail, "company") {
                add_org_entity(b, &id, &company, "member_of", Confidence::Medium, via);
            }
            if let Some(location) = detail_str(&detail, "location") {
                extract_text_location(b, &id, &location, via);
            }
            if let Some(name) = detail_str(&detail, "realName") {
                add_person_entity(b, &id, &name, via, Confidence::Low);
            }
            if let Some(name) = detail_str(&detail, "name") {
                add_person_entity(b, &id, &name, via, Confidence::Low);
            }
        }
        "search_dork" => {
            let id = entity_id(NodeKind::Lead, &f.label);
            b.node(
                &id,
                NodeKind::Lead,
                f.label.clone(),
                Confidence::Medium,
                via,
                false,
            );
            b.edge(&root, &id, "search_lead", Confidence::Medium, via);
        }
        "social_connection" => {
            let direction = detail_str(&detail, "direction").unwrap_or_default();
            let logins = detail_list(&detail, "logins");
            for login in logins.iter().take(MAX_SOCIAL_NODES) {
                let handle = login.trim().trim_start_matches('@').to_lowercase();
                if handle.is_empty() || handle.len() > 64 {
                    continue;
                }
                let id = entity_id(NodeKind::Username, &handle);
                b.node(
                    &id,
                    NodeKind::Username,
                    handle,
                    Confidence::High,
                    via,
                    false,
                );
                let relation = if direction == "following" {
                    "follows"
                } else {
                    "followed_by"
                };
                b.edge(&root, &id, relation, Confidence::High, via);
            }
        }
        "email_info" => match f.label.as_str() {
            "Local part" => {
                extract_username_entity(
                    b,
                    &root,
                    &f.value,
                    "local_part_suggests",
                    Confidence::Low,
                    via,
                );
                if let Some(domain) = detail_str(&detail, "domain") {
                    let id = entity_id(NodeKind::Domain, &domain);
                    b.node(&id, NodeKind::Domain, domain, Confidence::High, via, false);
                    b.edge(&root, &id, "uses_domain", Confidence::High, via);
                }
            }
            "Mail servers" => {
                add_mx_entities(b, &root, &detail_list(&detail, "records"), via);
            }
            "Disposable provider" | "Provider" => {
                let raw = detail_str(&detail, "domain").unwrap_or_else(|| f.value.clone());
                if let Some(host) = domain_from_url(&raw) {
                    let id = entity_id(NodeKind::Provider, &host);
                    b.node(&id, NodeKind::Provider, host, f.confidence, via, false);
                    b.edge(&root, &id, "mail_via", f.confidence, via);
                }
            }
            _ => {}
        },
        "gravatar" => {
            let id = platform_id("Gravatar");
            b.node(
                &id,
                NodeKind::Platform,
                "Gravatar".into(),
                f.confidence,
                via,
                false,
            );
            b.edge(&root, &id, "avatar_on", f.confidence, via);
            if let Some(name) = detail_str(&detail, "display_name") {
                add_person_entity(b, &id, &name, via, Confidence::Medium);
            }
        }
        "ip_geo" => {
            if let Some(as_name) = detail_str(&detail, "as") {
                add_org_entity(b, &root, &as_name, "announced_by", Confidence::High, via);
            } else if let Some(org) = detail_str(&detail, "org") {
                add_org_entity(b, &root, &org, "announced_by", Confidence::High, via);
            }
            if let Some(country) = detail_str(&detail, "country") {
                add_country_entity(b, &root, &country, "located_in", Confidence::Medium, via);
            }
            if let Some(city) = detail_str(&detail, "city") {
                let country = detail_str(&detail, "country").unwrap_or_default();
                let label = if city.eq_ignore_ascii_case("unknown") || city == "-" {
                    String::new()
                } else if country.is_empty() {
                    city
                } else {
                    format!("{city}, {country}")
                };
                if !label.is_empty() {
                    let id = entity_id(NodeKind::Location, &label);
                    b.node(
                        &id,
                        NodeKind::Location,
                        label,
                        Confidence::Medium,
                        via,
                        false,
                    );
                    b.edge(&root, &id, "located_in", Confidence::Medium, via);
                }
            }
            if let Some(reverse) = detail_str(&detail, "reverse") {
                if let Some(host) = domain_from_url(&reverse) {
                    let id = entity_id(NodeKind::Domain, &host);
                    b.node(&id, NodeKind::Domain, host, Confidence::Low, via, false);
                    b.edge(&root, &id, "ptr_hint", Confidence::Low, via);
                }
            }
        }
        "ip_network" => {
            if let Some(org) = detail_str(&detail, "org") {
                add_org_entity(b, &root, &org, "announced_by", Confidence::Medium, via);
            } else if let Some(asname) = detail_str(&detail, "asname") {
                add_org_entity(b, &root, &asname, "announced_by", Confidence::Medium, via);
            }
        }
        "ip_rdap" => {
            if let Some(netname) = detail_str(&detail, "netname") {
                add_org_entity(b, &root, &netname, "registered_as", Confidence::Medium, via);
            }
            if let Some(country) = detail_str(&detail, "country") {
                add_country_entity(b, &root, &country, "located_in", Confidence::Medium, via);
            }
            if let Some(abuse) = detail_str(&detail, "abuse") {
                extract_email_entity(b, &root, &abuse, "abuse_contact", Confidence::Medium, via);
            }
        }
        "ip_ptr" => {
            add_ptr_entities(b, &root, &detail_list(&detail, "records"), via);
        }
        "domain_rdap" => {
            if let Some(registrar) = detail_str(&detail, "registrar") {
                add_org_entity(
                    b,
                    &root,
                    &registrar,
                    "registered_via",
                    Confidence::High,
                    via,
                );
            }
            let ns = detail_list(&detail, "nameservers");
            if !ns.is_empty() {
                add_ns_entities(b, &root, &ns, via);
            }
        }
        "dns_records" => {
            if let Some(map) = detail.as_object() {
                for rrtype in ["A", "AAAA"] {
                    if let Some(vals) = map.get(rrtype).and_then(|v| v.as_array()) {
                        let recs: Vec<String> = vals
                            .iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect();
                        add_resolve_entities(b, &root, &recs, via);
                    }
                }
                if let Some(vals) = map.get("MX").and_then(|v| v.as_array()) {
                    let recs: Vec<String> = vals
                        .iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect();
                    add_mx_entities(b, &root, &recs, via);
                }
                if let Some(vals) = map.get("NS").and_then(|v| v.as_array()) {
                    let recs: Vec<String> = vals
                        .iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect();
                    add_ns_entities(b, &root, &recs, via);
                }
            }
        }
        "subdomains" => {
            let subs = detail_list(&detail, "list");
            add_subdomain_entities(b, &root, &subs, via);
        }
        "phone_info" => {
            if let Some(country) = detail_str(&detail, "country") {
                add_country_entity(b, &root, &country, "registered_in", Confidence::High, via);
            }
        }
        "metadata" => {
            if let Some(gps) = detail.get("gps") {
                if let (Some(lat), Some(lon)) = (
                    gps.get("lat").and_then(|v| v.as_f64()),
                    gps.get("lon").and_then(|v| v.as_f64()),
                ) {
                    let label = format!("{lat:.5}, {lon:.5}");
                    let id = entity_id(NodeKind::Location, &label);
                    b.node(&id, NodeKind::Location, label, Confidence::High, via, false);
                    b.edge(&root, &id, "captured_at", Confidence::High, via);
                }
            }
            let make = detail_str(&detail, "make");
            let model = detail_str(&detail, "model");
            let combined = match (&make, &model) {
                (Some(mk), Some(md)) if md.contains(mk.as_str()) => Some(md.clone()),
                (Some(mk), Some(md)) => Some(format!("{mk} {md}")),
                (Some(mk), None) => Some(mk.clone()),
                (None, Some(md)) => Some(md.clone()),
                _ => None,
            };
            if let Some(device) = combined {
                let id = entity_id(NodeKind::Device, &device);
                b.node(&id, NodeKind::Device, device, Confidence::High, via, false);
                b.edge(&root, &id, "shot_on", Confidence::High, via);
            }
            if let Some(artist) =
                detail_str(&detail, "artist").or_else(|| detail_str(&detail, "author"))
            {
                add_person_entity(b, &root, &artist, via, Confidence::Medium);
            }
        }
        _ => {}
    }
}

pub fn build_graph(targets: &[Target], findings: &[Finding]) -> OsintGraph {
    build_graph_with_pivots(targets, findings, &[])
}

pub fn build_graph_with_pivots(
    targets: &[Target],
    findings: &[Finding],
    pivots: &[PivotLink],
) -> OsintGraph {
    let mut b = Builder::new(targets);
    for f in findings {
        apply_finding(&mut b, f);
    }
    for p in pivots {
        let child = p.child.clone();
        let id = target_id(&child);
        b.node(
            &id,
            target_node_kind(child.kind),
            if child.raw.is_empty() {
                child.normalized.clone()
            } else {
                child.raw.clone()
            },
            Confidence::Medium,
            "pivot",
            true,
        );
        let parent = entity_id(target_node_kind(p.parent_kind), &p.parent);
        b.edge(&parent, &id, "pivot", Confidence::Medium, &p.via);
    }
    b.finish()
}

pub fn unique_sources(graph: &OsintGraph) -> BTreeSet<String> {
    graph
        .nodes
        .iter()
        .flat_map(|n| n.sources.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Target;

    fn target(kind: TargetKind, raw: &str) -> Target {
        Target {
            kind,
            raw: raw.to_string(),
            normalized: raw.trim_start_matches('@').to_lowercase(),
        }
    }

    fn finding(category: &str, label: &str, value: &str, conf: Confidence) -> Finding {
        Finding {
            module_id: "test.module".into(),
            category: category.into(),
            label: label.into(),
            value: value.into(),
            url: None,
            confidence: conf,
            target: None,
            detail: None,
        }
    }

    #[test]
    fn roots_created_from_targets() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let g = build_graph(&targets, &[]);
        assert_eq!(g.node_count(), 1);
        assert_eq!(g.nodes[0].id, "Username:mel0k1");
        assert!(g.nodes[0].root);
    }

    #[test]
    fn social_profiles_link_to_target() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let f1 = finding(
            "social_profile",
            "GitHub",
            "https://github.com/mel0k1",
            Confidence::High,
        );
        let f2 = finding(
            "social_profile",
            "VK",
            "https://vk.com/mel0k1",
            Confidence::Medium,
        );
        let g = build_graph(&targets, &[f1, f2]);
        assert_eq!(g.node_count(), 3);
        assert_eq!(g.edge_count(), 2);
        assert!(g
            .edges
            .iter()
            .all(|e| e.relation == "profile_on" && e.from == "Username:mel0k1"));
    }

    #[test]
    fn duplicate_platform_nodes_merge_with_max_confidence() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let f1 = finding(
            "social_profile",
            "GitHub",
            "https://github.com/mel0k1",
            Confidence::Medium,
        );
        let f2 = finding(
            "api_profile",
            "GitHub",
            "https://github.com/mel0k1",
            Confidence::High,
        );
        let g = build_graph(&targets, &[f1, f2]);
        assert_eq!(g.node_count(), 2);
        let platform = g.nodes.iter().find(|n| n.id == "Platform:github").unwrap();
        assert_eq!(platform.confidence, Confidence::High);
        assert_eq!(platform.sources.len(), 1);
    }

    #[test]
    fn enrich_extracts_email_and_website() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let mut f = finding(
            "api_profile",
            "GitHub",
            "https://github.com/mel0k1",
            Confidence::High,
        );
        f.detail = Some(serde_json::json!({
            "email": "me@corp.io",
            "blog": "https://blog.corp.io/about",
            "twitter": "mel0k1_dev",
            "company": "Acme Corp",
            "location": "Berlin, Germany",
        }));
        let g = build_graph(&targets, &[f]);
        let kinds: Vec<String> = g.nodes.iter().map(|n| format!("{:?}", n.kind)).collect();
        assert!(kinds.contains(&"Email".to_string()));
        assert!(kinds.contains(&"Domain".to_string()));
        assert!(kinds.contains(&"Org".to_string()));
        assert!(kinds.contains(&"Location".to_string()));
        assert!(g.edges.iter().any(|e| e.relation == "exposes_email"));
        assert!(g.edges.iter().any(|e| e.relation == "links_to"));
        assert!(g.edges.iter().any(|e| e.relation == "alt_handle"));
        assert!(g.edges.iter().any(|e| e.relation == "member_of"));
    }

    #[test]
    fn pivots_link_parent_entity_to_new_root() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let child = target(TargetKind::Email, "jane@corp.io");
        let pivots = vec![PivotLink {
            parent: "mel0k1".into(),
            parent_kind: TargetKind::Username,
            child: child.clone(),
            depth: 1,
            via: "enrich.api".into(),
        }];
        let g = build_graph_with_pivots(&targets, &[], &pivots);
        assert!(g
            .nodes
            .iter()
            .any(|n| n.id == "Email:jane@corp.io" && n.root));
        let edge = g
            .edges
            .iter()
            .find(|e| e.relation == "pivot")
            .expect("pivot edge");
        assert_eq!(edge.from, "Username:mel0k1");
        assert_eq!(edge.to, "Email:jane@corp.io");
        assert_eq!(edge.via, "enrich.api");
    }

    #[test]
    fn social_connections_link_followers_and_following() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let mut f = finding(
            "social_connection",
            "GitHub followers",
            "12 accounts",
            Confidence::High,
        );
        f.detail = Some(serde_json::json!({
            "platform": "github",
            "direction": "followers",
            "logins": ["alice", "@Bob", "", "carol"],
        }));
        let mut f2 = finding(
            "social_connection",
            "GitHub following",
            "1 accounts",
            Confidence::High,
        );
        f2.detail = Some(serde_json::json!({
            "platform": "github",
            "direction": "following",
            "logins": ["dave"],
        }));
        let g = build_graph(&targets, &[f, f2]);
        for handle in ["alice", "bob", "carol", "dave"] {
            let node = g
                .nodes
                .iter()
                .find(|n| n.id == format!("Username:{handle}"))
                .unwrap_or_else(|| panic!("missing node {handle}"));
            assert_eq!(node.confidence, Confidence::High);
        }
        assert!(g
            .edges
            .iter()
            .any(|e| e.relation == "followed_by" && e.to == "Username:alice"));
        assert!(g
            .edges
            .iter()
            .any(|e| e.relation == "followed_by" && e.to == "Username:bob"));
        assert!(g
            .edges
            .iter()
            .any(|e| e.relation == "follows" && e.to == "Username:dave"));
        assert!(!g.edges.iter().any(|e| e.to == "Username:"));
    }

    #[test]
    fn pivot_self_edges_are_dropped() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let child = target(TargetKind::Username, "mel0k1");
        let pivots = vec![PivotLink {
            parent: "mel0k1".into(),
            parent_kind: TargetKind::Username,
            child,
            depth: 1,
            via: "m".into(),
        }];
        let g = build_graph_with_pivots(&targets, &[], &pivots);
        assert!(!g.edges.iter().any(|e| e.relation == "pivot"));
    }

    #[test]
    fn email_local_part_suggests_username_and_domain() {
        let targets = vec![target(TargetKind::Email, "mel0k1@corp.io")];
        let mut f = finding("email_info", "Local part", "mel0k1", Confidence::High);
        f.detail = Some(serde_json::json!({"domain": "corp.io"}));
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.id == "Username:mel0k1"));
        assert!(g.nodes.iter().any(|n| n.id == "Domain:corp.io"));
        assert!(g.edges.iter().any(|e| e.relation == "local_part_suggests"));
        assert!(g.edges.iter().any(|e| e.relation == "uses_domain"));
    }

    #[test]
    fn dns_records_create_ip_nodes() {
        let targets = vec![target(TargetKind::Domain, "corp.io")];
        let mut f = finding("dns_records", "DNS records", "A", Confidence::High);
        f.detail = Some(serde_json::json!({
            "A": ["93.184.216.34", "93.184.216.35"],
            "MX": ["10 mx.corp.io.", "20 mx2.corp.io."],
            "NS": ["a.iana-servers.net", "b.iana-servers.net"],
        }));
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.id == "Ip:93.184.216.34"));
        assert!(g.edges.iter().any(|e| e.relation == "resolves_to"));
        assert!(g.edges.iter().any(|e| e.relation == "mail_via"));
        assert!(g.edges.iter().any(|e| e.relation == "served_by"));
    }

    #[test]
    fn subdomains_point_at_domain() {
        let targets = vec![target(TargetKind::Domain, "corp.io")];
        let mut f = finding(
            "subdomains",
            "Certificate subdomains",
            "48 found",
            Confidence::High,
        );
        f.detail = Some(serde_json::json!({"list": ["api.corp.io", "dev.corp.io", "notadomain"]}));
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.id == "Domain:api.corp.io"));
        assert!(g
            .edges
            .iter()
            .any(|e| e.relation == "subdomain_of" && e.to == "Domain:corp.io"));
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn exif_metadata_creates_geo_and_device() {
        let targets = vec![target(TargetKind::File, "/tmp/pic.jpg")];
        let mut f = finding(
            "metadata",
            "EXIF metadata",
            "/tmp/pic.jpg",
            Confidence::High,
        );
        f.detail = Some(serde_json::json!({
            "make": "Canon", "model": "Canon EOS R5",
            "gps": {"lat": 52.52, "lon": 13.405, "map": "https://maps.example"},
        }));
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Location));
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Device));
        assert!(g.edges.iter().any(|e| e.relation == "captured_at"));
        assert!(g.edges.iter().any(|e| e.relation == "shot_on"));
    }

    #[test]
    fn phone_country_registered() {
        let targets = vec![target(TargetKind::PhoneNumber, "+79261234567")];
        let mut f = finding(
            "phone_info",
            "Number details",
            "+7 926 123-45-67",
            Confidence::High,
        );
        f.detail = Some(serde_json::json!({"country": "RU", "lineType": "Mobile"}));
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.id == "Country:ru"));
        assert!(g.edges.iter().any(|e| e.relation == "registered_in"));
    }

    #[test]
    fn dorks_become_leads() {
        let targets = vec![target(TargetKind::Username, "mel0k1")];
        let f = finding(
            "search_dork",
            "GitHub code search",
            "https://github.com/search?q=x",
            Confidence::Medium,
        );
        let g = build_graph(&targets, &[f]);
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Lead));
        assert!(g.edges.iter().any(|e| e.relation == "search_lead"));
    }

    #[test]
    fn self_edges_are_dropped() {
        let targets = vec![target(TargetKind::Domain, "corp.io")];
        let mut f = finding("dns_records", "DNS records", "A", Confidence::High);
        f.detail = Some(serde_json::json!({"A": ["93.184.216.34"], "NS": ["ns1.corp.io"]}));
        let g = build_graph(&targets, &[f]);
        assert!(g.edges.iter().all(|e| e.from != e.to));
    }

    #[test]
    fn subdomain_nodes_capped() {
        let targets = vec![target(TargetKind::Domain, "corp.io")];
        let mut f = finding(
            "subdomains",
            "Certificate subdomains",
            "many",
            Confidence::High,
        );
        let list: Vec<String> = (0..300).map(|i| format!("host{i}.corp.io")).collect();
        f.detail = Some(serde_json::json!({"list": list}));
        let g = build_graph(&targets, &[f]);
        let sub_nodes = g
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Domain && n.id != "Domain:corp.io")
            .count();
        assert_eq!(sub_nodes, MAX_SUBDOMAIN_NODES);
    }

    #[test]
    fn domain_from_url_handles_variants() {
        assert_eq!(
            domain_from_url("https://blog.corp.io/about").as_deref(),
            Some("blog.corp.io")
        );
        assert_eq!(
            domain_from_url("http://WWW.Corp.io:8080/x").as_deref(),
            Some("corp.io")
        );
        assert_eq!(
            domain_from_url("mx1.mail.ru.").as_deref(),
            Some("mx1.mail.ru")
        );
        assert!(domain_from_url("10 mx1.mail.ru.").is_none());
        assert!(domain_from_url("nohost").is_none());
        assert_eq!(strip_list_marker("10 mx1.mail.ru."), "mx1.mail.ru.");
        assert_eq!(strip_list_marker("20 mx2.mail.ru"), "mx2.mail.ru");
        assert_eq!(strip_list_marker("plain.host"), "plain.host");
    }
}
