use anyway_core::modules::sites::{builtin_sites, host_of, sites, wmn_sites, Site};
use std::collections::HashMap;

fn key(site: &Site) -> String {
    let check = if site.probe.is_empty() {
        &site.url
    } else {
        &site.probe
    };
    host_of(check)
}

fn main() {
    let builtin = builtin_sites();
    let wmn = wmn_sites();
    let all = sites();
    println!(
        "builtin {} wmn {} merged {}",
        builtin.len(),
        wmn.len(),
        all.len()
    );
    let mut by_source: HashMap<&str, usize> = HashMap::new();
    for s in all.iter() {
        *by_source.entry(s.source.as_str()).or_default() += 1;
    }
    println!("by source: {by_source:?}");
    let mut cats: HashMap<&str, usize> = HashMap::new();
    for s in all.iter() {
        *cats.entry(s.category.as_str()).or_default() += 1;
    }
    let mut cats: Vec<_> = cats.into_iter().collect();
    cats.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!("top categories: {cats:?}");
    let hits: Vec<String> = all
        .iter()
        .filter(|s| s.name.to_lowercase().contains("git"))
        .map(|s| format!("{} [{}] {} probe={}", s.name, s.source, s.url, key(s)))
        .collect();
    println!("name~git: {hits:?}");
    let wmn_names: Vec<&str> = wmn.iter().map(|s| s.name.as_str()).collect();
    for wanted in ["GitHub", "X", "Telegram", "Reddit", "Chess.com", "Discord"] {
        let hit = wmn_names.contains(&wanted);
        println!("wmn has {wanted}: {hit}");
    }
}
