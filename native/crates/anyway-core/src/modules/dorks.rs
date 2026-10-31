use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding, TargetKind};
use serde_json::json;

pub struct DorksModule;

struct Dork {
    engine: &'static str,
    url: String,
    query: String,
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn quoted(s: &str) -> String {
    format!("%22{}%22", enc(s))
}

fn web_dorks(query: &str) -> Vec<Dork> {
    let q = quoted(query);
    vec![
        Dork {
            engine: "Google",
            url: format!("https://www.google.com/search?q={q}"),
            query: query.into(),
        },
        Dork {
            engine: "Bing",
            url: format!("https://www.bing.com/search?q={q}"),
            query: query.into(),
        },
        Dork {
            engine: "DuckDuckGo",
            url: format!("https://duckduckgo.com/?q={q}"),
            query: query.into(),
        },
        Dork {
            engine: "Yandex",
            url: format!("https://yandex.com/search/?text={q}"),
            query: query.into(),
        },
    ]
}

fn username_dorks(u: &str) -> Vec<Dork> {
    let mut out = web_dorks(u);
    out.push(Dork {
        engine: "GitHub users",
        url: format!("https://github.com/search?q={}&type=users", enc(u)),
        query: u.into(),
    });
    out.push(Dork {
        engine: "GitLab users",
        url: format!("https://gitlab.com/search?q={}&scope=users", enc(u)),
        query: u.into(),
    });
    out.push(Dork {
        engine: "Pastebin",
        url: format!("https://pastebin.com/u/{}", enc(u)),
        query: u.into(),
    });
    out.push(Dork {
        engine: "X search",
        url: format!("https://x.com/search?q={}", enc(u)),
        query: u.into(),
    });
    out.push(Dork {
        engine: "Reddit search",
        url: format!("https://www.reddit.com/search/?q={}", enc(u)),
        query: u.into(),
    });
    out.push(Dork {
        engine: "YouTube",
        url: format!("https://www.youtube.com/results?search_query={}", enc(u)),
        query: u.into(),
    });
    out
}

fn email_dorks(e: &str) -> Vec<Dork> {
    let mut out = web_dorks(e);
    out.push(Dork {
        engine: "GitHub code",
        url: format!("https://github.com/search?q={}&type=code", quoted(e)),
        query: e.into(),
    });
    out.push(Dork {
        engine: "Pastebin",
        url: format!("https://pastebin.com/search?q={}", enc(e)),
        query: e.into(),
    });
    out
}

fn phone_dorks(raw: &str) -> Vec<Dork> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let mut out = web_dorks(raw);
    out.push(Dork {
        engine: "Truecaller",
        url: format!("https://www.truecaller.com/search/global/{digits}"),
        query: raw.into(),
    });
    out.push(Dork {
        engine: "Sync.me",
        url: format!("https://sync.me/search/?number=%2B{digits}"),
        query: raw.into(),
    });
    out
}

fn domain_dorks(d: &str) -> Vec<Dork> {
    let mut out = web_dorks(d);
    out.push(Dork {
        engine: "crt.sh",
        url: format!("https://crt.sh/?q={}", enc(d)),
        query: d.into(),
    });
    out.push(Dork {
        engine: "urlscan.io",
        url: format!("https://urlscan.io/domain/{}", enc(d)),
        query: d.into(),
    });
    out.push(Dork {
        engine: "Shodan",
        url: format!("https://www.shodan.io/search?query=hostname%3A{}", enc(d)),
        query: format!("hostname:{d}"),
    });
    out.push(Dork {
        engine: "VirusTotal",
        url: format!("https://www.virustotal.com/gui/domain/{}", enc(d)),
        query: d.into(),
    });
    out.push(Dork {
        engine: "Wayback Machine",
        url: format!("https://web.archive.org/web/*/{}*", enc(d)),
        query: d.into(),
    });
    out
}

fn ip_dorks(ip: &str) -> Vec<Dork> {
    let i = enc(ip);
    vec![
        Dork {
            engine: "Shodan",
            url: format!("https://www.shodan.io/host/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "Censys",
            url: format!("https://search.censys.io/hosts/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "VirusTotal",
            url: format!("https://www.virustotal.com/gui/ip-address/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "AbuseIPDB",
            url: format!("https://www.abuseipdb.com/check/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "urlscan.io",
            url: format!("https://urlscan.io/ip/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "bgp.he.net",
            url: format!("https://bgp.he.net/ip/{i}"),
            query: ip.into(),
        },
        Dork {
            engine: "ipinfo.io",
            url: format!("https://ipinfo.io/{i}"),
            query: ip.into(),
        },
    ]
}

#[async_trait::async_trait]
impl Module for DorksModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "search.dorks",
            name: "Search Dorks",
            description: "One-click deep links: search engines, code hosts, paste sites and intel portals preloaded with the target",
            accepts: &[TargetKind::Username, TargetKind::Email, TargetKind::IpAddress, TargetKind::Domain, TargetKind::PhoneNumber],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let t = ctx.target.clone();
        let dorks = match t.kind {
            TargetKind::Username => username_dorks(&t.normalized),
            TargetKind::Email => email_dorks(&t.normalized),
            TargetKind::PhoneNumber => phone_dorks(&t.raw),
            TargetKind::Domain => domain_dorks(&t.normalized),
            TargetKind::IpAddress => ip_dorks(&t.normalized),
            _ => Vec::new(),
        };
        let total = dorks.len() as u64;
        ctx.progress(0, total).await;
        for (i, d) in dorks.into_iter().enumerate() {
            if ctx.cancelled() {
                return Ok(());
            }
            ctx.emit(Finding {
                module_id: "search.dorks".into(),
                category: "search_dork".into(),
                label: format!("Dork on {}", d.engine),
                value: d.url.clone(),
                url: Some(d.url),
                confidence: Confidence::Low,
                target: None,
                detail: Some(json!({"engine": d.engine, "query": d.query})),
            })
            .await;
            ctx.progress(i as u64 + 1, total).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_query_values() {
        assert_eq!(enc("abc123-_.~"), "abc123-_.~");
        assert_eq!(enc("a b"), "a%20b");
        assert_eq!(enc("a@b.co"), "a%40b.co");
        assert_eq!(enc("+7926"), "%2B7926");
    }

    #[test]
    fn quoted_wraps() {
        assert_eq!(quoted("john"), "%22john%22");
    }

    #[test]
    fn username_dorks_are_urls() {
        for d in username_dorks("torvalds") {
            assert!(d.url.starts_with("https://"), "{}", d.url);
            assert!(!d.url.contains(' '), "{}", d.url);
        }
        assert_eq!(username_dorks("torvalds").len(), 10);
    }

    #[test]
    fn per_kind_dorks() {
        assert!(!email_dorks("a@b.co").is_empty());
        assert!(!phone_dorks("+7 926 123-45-67").is_empty());
        assert!(!domain_dorks("example.org").is_empty());
        assert!(!ip_dorks("8.8.8.8").is_empty());
    }
}
