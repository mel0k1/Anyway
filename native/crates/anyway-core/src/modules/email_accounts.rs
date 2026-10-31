use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use futures::stream::{self, StreamExt};
use rand::Rng;
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

pub struct EmailAccountsModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Exists,
    Free,
    Unknown,
}

pub struct ProbeResponse {
    pub status: u16,
    pub body: String,
}

#[derive(Clone, Copy)]
pub enum HttpVerb {
    Get,
    Post,
}

#[derive(Clone)]
pub struct EmailSite {
    pub name: &'static str,
    pub domain: &'static str,
    pub verb: HttpVerb,
    pub url: &'static str,
    pub body: &'static str,
    pub content_type: &'static str,
    pub header: Option<(&'static str, &'static str)>,
    pub evidence: &'static str,
    pub confidence: Confidence,
    pub check: Option<fn(&ProbeResponse) -> Verdict>,
}

pub fn json_lookup<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for key in path {
        cur = match cur {
            Value::Array(arr) => {
                let idx = key.parse::<usize>().ok()?;
                arr.get(idx)?
            }
            _ => cur.get(key)?,
        };
    }
    Some(cur)
}

pub fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn probe_json(resp: &ProbeResponse) -> Option<Value> {
    serde_json::from_str(&resp.body).ok()
}

fn twitter_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("taken")) {
        Some(Value::Bool(true)) => Verdict::Exists,
        Some(Value::Bool(false)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn spotify_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("status")) {
        Some(Value::Number(n)) if n.as_i64() == Some(20) => Verdict::Exists,
        Some(Value::Number(n)) if n.as_i64() == Some(1) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn firefox_check(resp: &ProbeResponse) -> Verdict {
    if resp.body.contains(r#""exists":true"#) {
        Verdict::Exists
    } else if resp.body.contains(r#""exists":false"#) {
        Verdict::Free
    } else {
        Verdict::Unknown
    }
}

fn lastpass_check(resp: &ProbeResponse) -> Verdict {
    match resp.body.trim() {
        "no" => Verdict::Exists,
        "ok" | "emailinvalid" => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn patreon_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp)
        .as_ref()
        .and_then(|v| json_lookup(v, &["data", "is_available"]))
    {
        Some(Value::Bool(true)) => Verdict::Free,
        Some(Value::Bool(false)) => Verdict::Exists,
        _ => Verdict::Unknown,
    }
}

fn tellonym_check(resp: &ProbeResponse) -> Verdict {
    if resp.body.contains("EMAIL_ALREADY_IN_USE") {
        Verdict::Exists
    } else if resp.status == 200 {
        Verdict::Free
    } else {
        Verdict::Unknown
    }
}

fn duolingo_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("users")) {
        Some(Value::Array(users)) => {
            if users.is_empty() {
                Verdict::Free
            } else {
                Verdict::Exists
            }
        }
        _ => Verdict::Unknown,
    }
}

fn nike_check(resp: &ProbeResponse) -> Verdict {
    match resp.status {
        409 => Verdict::Exists,
        200 => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn deliveroo_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("registered")) {
        Some(Value::Bool(true)) => Verdict::Exists,
        Some(Value::Bool(false)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn flickr_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("state_code")) {
        Some(Value::Number(n)) if n.as_i64() == Some(5) => Verdict::Exists,
        Some(Value::Number(_)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn replit_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("exists")) {
        Some(Value::Bool(true)) => Verdict::Exists,
        Some(Value::Bool(false)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn vsco_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp)
        .as_ref()
        .and_then(|v| v.get("email_status"))
    {
        Some(Value::String(s)) if s == "has_account" => Verdict::Exists,
        Some(Value::String(s)) if s == "no_account" => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn anydo_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("user_exists")) {
        Some(Value::Bool(true)) => Verdict::Exists,
        Some(Value::Bool(false)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn archive_check(resp: &ProbeResponse) -> Verdict {
    if resp.body.contains("is already taken.") {
        Verdict::Exists
    } else if resp.status == 200 {
        Verdict::Free
    } else {
        Verdict::Unknown
    }
}

fn proton_check(resp: &ProbeResponse) -> Verdict {
    if resp.body.contains("info:1:1") {
        Verdict::Exists
    } else if resp.body.contains("info:1:0") {
        Verdict::Free
    } else {
        Verdict::Unknown
    }
}

fn mailru_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp).as_ref().and_then(|v| v.get("status")) {
        Some(Value::Number(n)) if n.as_i64() == Some(200) => Verdict::Exists,
        Some(Value::Number(_)) => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

fn imgur_check(resp: &ProbeResponse) -> Verdict {
    match probe_json(resp)
        .as_ref()
        .and_then(|v| json_lookup(v, &["data", "available"]))
    {
        Some(Value::Bool(true)) => Verdict::Free,
        Some(Value::Bool(false)) => Verdict::Exists,
        _ => Verdict::Unknown,
    }
}

fn pinterest_check(resp: &ProbeResponse) -> Verdict {
    let Some(parsed) = probe_json(resp) else {
        return Verdict::Unknown;
    };
    let Some(data) = json_lookup(&parsed, &["resource_response", "data"]) else {
        return Verdict::Unknown;
    };
    if data.get("source_field").is_some_and(|v| !v.is_null())
        || data
            .as_object()
            .is_some_and(|o| o.contains_key("message") || o.contains_key("error"))
    {
        return Verdict::Unknown;
    }
    if data.is_null() || matches!(data, Value::Bool(false)) {
        Verdict::Free
    } else {
        Verdict::Exists
    }
}

fn discord_check(resp: &ProbeResponse) -> Verdict {
    let Some(parsed) = probe_json(resp) else {
        return Verdict::Unknown;
    };
    if let Some(code) = json_lookup(&parsed, &["errors", "email", "_errors", "0", "code"]) {
        if code.as_str() == Some("EMAIL_ALREADY_REGISTERED") {
            return Verdict::Exists;
        }
        return Verdict::Free;
    }
    if json_lookup(&parsed, &["captcha_key"])
        .is_some_and(|v| v.as_str() == Some("captcha-required"))
    {
        return Verdict::Unknown;
    }
    Verdict::Unknown
}

pub const SITES: &[EmailSite] = &[
    EmailSite {
        name: "Twitter / X",
        domain: "twitter.com",
        verb: HttpVerb::Get,
        url: "https://api.twitter.com/i/users/email_available.json?email={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "registration availability API reports the email as taken",
        confidence: Confidence::High,
        check: Some(twitter_check),
    },
    EmailSite {
        name: "Spotify",
        domain: "spotify.com",
        verb: HttpVerb::Get,
        url: "https://spclient.wg.spotify.com/signup/public/v1/account?validate=1&email={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "signup validation answers status 20 for an already registered address",
        confidence: Confidence::High,
        check: Some(spotify_check),
    },
    EmailSite {
        name: "Firefox",
        domain: "mozilla.org",
        verb: HttpVerb::Post,
        url: "https://api.accounts.firefox.com/v1/account/status",
        body: r#"{"email":"{email}"}"#,
        content_type: "application/json",
        header: None,
        evidence: "Firefox Accounts status endpoint confirms the address exists",
        confidence: Confidence::High,
        check: Some(firefox_check),
    },
    EmailSite {
        name: "LastPass",
        domain: "lastpass.com",
        verb: HttpVerb::Get,
        url: "https://lastpass.com/create_account.php?check=avail&skipcontent=1&mistype=1&username={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "account availability endpoint answers no",
        confidence: Confidence::High,
        check: Some(lastpass_check),
    },
    EmailSite {
        name: "Patreon",
        domain: "patreon.com",
        verb: HttpVerb::Post,
        url: "https://www.patreon.com/api/email/available?json-api-version=1.0&include=%5B%5D",
        body: r#"{"data":{"attributes":{"email":"{email}"}}}"#,
        content_type: "application/vnd.api+json",
        header: None,
        evidence: "email availability endpoint reports the address as unavailable",
        confidence: Confidence::Medium,
        check: Some(patreon_check),
    },
    EmailSite {
        name: "Tellonym",
        domain: "tellonym.me",
        verb: HttpVerb::Get,
        url: "https://api.tellonym.me/accounts/check?email={email_q}&errorMessage=&limit=25",
        body: "",
        content_type: "",
        header: None,
        evidence: "registration check returns EMAIL_ALREADY_IN_USE",
        confidence: Confidence::High,
        check: Some(tellonym_check),
    },
    EmailSite {
        name: "Duolingo",
        domain: "duolingo.com",
        verb: HttpVerb::Get,
        url: "https://www.duolingo.com/2017-06-30/users?email={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "user lookup by email returns a profile",
        confidence: Confidence::High,
        check: Some(duolingo_check),
    },
    EmailSite {
        name: "Nike",
        domain: "nike.com",
        verb: HttpVerb::Post,
        url: "https://unite.nike.com/account/email/v1?appVersion=831&experienceVersion=831&uxid=com.nike.commerce.nikedotcom.web&locale=fr_FR&backendEnvironment=identity&browser=&mobile=false&native=false&visit=1",
        body: r#"{"emailAddress":"{email}"}"#,
        content_type: "text/plain;charset=UTF-8",
        header: None,
        evidence: "email check answers HTTP 409 conflict",
        confidence: Confidence::High,
        check: Some(nike_check),
    },
    EmailSite {
        name: "Deliveroo",
        domain: "deliveroo.com",
        verb: HttpVerb::Post,
        url: "https://consumer-ow-api.deliveroo.com/orderapp/v1/check-email",
        body: r#"{"email_address":"{email}"}"#,
        content_type: "application/json;charset=UTF-8",
        header: None,
        evidence: "check-email reports the account as registered",
        confidence: Confidence::High,
        check: Some(deliveroo_check),
    },
    EmailSite {
        name: "Flickr",
        domain: "flickr.com",
        verb: HttpVerb::Get,
        url: "https://identity-api.flickr.com/migration?email={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "identity migration endpoint returns state 5 for an existing account",
        confidence: Confidence::High,
        check: Some(flickr_check),
    },
    EmailSite {
        name: "Replit",
        domain: "replit.com",
        verb: HttpVerb::Post,
        url: "https://replit.com/data/user/exists",
        body: r#"{"email":"{email}"}"#,
        content_type: "application/json",
        header: None,
        evidence: "user existence API reports the email as registered",
        confidence: Confidence::High,
        check: Some(replit_check),
    },
    EmailSite {
        name: "VSCO",
        domain: "vsco.co",
        verb: HttpVerb::Get,
        url: "https://api.vsco.co/2.0/users/email?email={email_q}",
        body: "",
        content_type: "",
        header: Some((
            "Authorization",
            "Bearer 7356455548d0a1d886db010883388d08be84d0c9",
        )),
        evidence: "email lookup returns has_account",
        confidence: Confidence::High,
        check: Some(vsco_check),
    },
    EmailSite {
        name: "Any.do",
        domain: "any.do",
        verb: HttpVerb::Post,
        url: "https://sm-prod2.any.do/check_email",
        body: r#"{"email":"{email}"}"#,
        content_type: "application/json; charset=UTF-8",
        header: None,
        evidence: "login probe reports an existing user",
        confidence: Confidence::High,
        check: Some(anydo_check),
    },
    EmailSite {
        name: "Internet Archive",
        domain: "archive.org",
        verb: HttpVerb::Post,
        url: "https://archive.org/account/signup",
        body: "-----------------------------\r\nContent-Disposition: form-data; name=\"input_name\"\r\n\r\nusername\r\n-----------------------------\r\nContent-Disposition: form-data; name=\"input_value\"\r\n\r\n{email}\r\n-----------------------------\r\nContent-Disposition: form-data; name=\"input_validator\"\r\n\r\ntrue\r\n-----------------------------\r\nContent-Disposition: form-data; name=\"submit_by_js\"\r\n\r\ntrue\r\n-------------------------------\r\n",
        content_type: "multipart/form-data; boundary=---------------------------",
        header: None,
        evidence: "signup validation reports the address as already taken",
        confidence: Confidence::High,
        check: Some(archive_check),
    },
    EmailSite {
        name: "Proton Mail",
        domain: "proton.me",
        verb: HttpVerb::Get,
        url: "https://api.protonmail.ch/pks/lookup?op=index&search={email_q}",
        body: "",
        content_type: "",
        header: None,
        evidence: "HKP keyserver indexes a public key created for the address",
        confidence: Confidence::High,
        check: Some(proton_check),
    },
    EmailSite {
        name: "Mail.ru",
        domain: "mail.ru",
        verb: HttpVerb::Post,
        url: "https://account.mail.ru/api/v1/user/password/restore",
        body: "email={email_q}&htmlencoded=false",
        content_type: "application/x-www-form-urlencoded; charset=UTF-8",
        header: Some(("x-requested-with", "XMLHttpRequest")),
        evidence: "password recovery endpoint accepts the address",
        confidence: Confidence::High,
        check: Some(mailru_check),
    },
    EmailSite {
        name: "Imgur",
        domain: "imgur.com",
        verb: HttpVerb::Post,
        url: "https://imgur.com/signin/ajax_email_available",
        body: "email={email_q}",
        content_type: "application/x-www-form-urlencoded; charset=UTF-8",
        header: Some(("x-requested-with", "XMLHttpRequest")),
        evidence: "email availability endpoint reports the address as unavailable",
        confidence: Confidence::Medium,
        check: Some(imgur_check),
    },
    EmailSite {
        name: "Pinterest",
        domain: "pinterest.com",
        verb: HttpVerb::Get,
        url: "https://www.pinterest.com/_ngjs/resource/EmailExistsResource/get/?source_url=/&data=%7B%22options%22:%7B%22email%22:%22{email_q}%22%7D,%22context%22:%7B%7D%7D",
        body: "",
        content_type: "",
        header: None,
        evidence: "EmailExistsResource returns an existing account",
        confidence: Confidence::High,
        check: Some(pinterest_check),
    },
    EmailSite {
        name: "Discord",
        domain: "discord.com",
        verb: HttpVerb::Post,
        url: "https://discord.com/api/v9/auth/register",
        body: r#"{"fingerprint":"","email":"{email}","username":"{rand20}","password":"{rand20}","invite":null,"consent":true,"date_of_birth":"","gift_code_sku_id":null,"captcha_key":null}"#,
        content_type: "application/json",
        header: None,
        evidence: "registration validation returns EMAIL_ALREADY_REGISTERED",
        confidence: Confidence::High,
        check: Some(discord_check),
    },
    EmailSite {
        name: "GitHub",
        domain: "github.com",
        verb: HttpVerb::Post,
        url: "https://github.com/join",
        body: "",
        content_type: "",
        header: None,
        evidence: "signup email check answers HTTP 422 for an already used address",
        confidence: Confidence::High,
        check: None,
    },
];

fn random_string(len: usize) -> String {
    let letters = b"abcdefghijklmnopqrstuvwxyz";
    (0..len)
        .map(|_| letters[rand::rng().random_range(0..letters.len())] as char)
        .collect()
}

fn render_body(site: &EmailSite, email: &str) -> String {
    let filled = site
        .body
        .replace("{email_q}", &urlencode(email))
        .replace("{email}", email);
    if filled.contains("{rand20}") {
        filled.replace("{rand20}", &random_string(20))
    } else {
        filled
    }
}

pub fn github_token(html: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r#"(?s)<auto-check src="/signup_check/username.*?value="([^"]+)".*<auto-check src="/signup_check/email.*?value="([^"]+)""#,
        )
        .expect("github token regex")
    });
    re.captures(html)
        .and_then(|c| c.get(2))
        .map(|m| m.as_str().to_string())
}

async fn probe_github(client: &Client, email: &str) -> Verdict {
    let join = match client.get("https://github.com/join").send().await {
        Ok(r) => match r.error_for_status() {
            Ok(r) => r,
            Err(_) => return Verdict::Unknown,
        },
        Err(_) => return Verdict::Unknown,
    };
    let Ok(html) = join.text().await else {
        return Verdict::Unknown;
    };
    let Some(token) = github_token(&html) else {
        return Verdict::Unknown;
    };
    let body = format!(
        "value={}&authenticity_token={}",
        urlencode(email),
        urlencode(&token)
    );
    let Ok(resp) = client
        .post("https://github.com/signup_check/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
    else {
        return Verdict::Unknown;
    };
    match resp.status().as_u16() {
        422 => Verdict::Exists,
        200 => Verdict::Free,
        _ => Verdict::Unknown,
    }
}

async fn probe(client: &Client, site: &EmailSite, email: &str) -> Verdict {
    if site.name == "GitHub" {
        return probe_github(client, email).await;
    }
    let url = site
        .url
        .replace("{email_q}", &urlencode(email))
        .replace("{email}", email);
    let mut request = match site.verb {
        HttpVerb::Get => client.get(&url),
        HttpVerb::Post => {
            let mut req = client.post(&url);
            if !site.body.is_empty() {
                req = req.body(render_body(site, email));
            }
            req
        }
    };
    if !site.content_type.is_empty() {
        request = request.header("content-type", site.content_type);
    }
    if let Some((key, value)) = site.header {
        request = request.header(key, value);
    }
    let Ok(resp) = request.send().await else {
        return Verdict::Unknown;
    };
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let Some(check) = site.check else {
        return Verdict::Unknown;
    };
    check(&ProbeResponse { status, body })
}

#[async_trait::async_trait]
impl Module for EmailAccountsModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "email.accounts",
            name: "Email Accounts",
            description: "Registration, login and password-recovery probes across popular services in the holehe style that reveal which accounts hang off an email address",
            accepts: &[crate::models::TargetKind::Email],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let email = ctx.target.normalized.clone();
        let conc = ctx.options.concurrency.clamp(4, 64) as usize;
        let total = SITES.len();
        let done = Arc::new(AtomicUsize::new(0));
        let client = Arc::new(ctx.client.clone());

        ctx.note(&format!("probing {total} services for linked accounts"))
            .await;

        stream::iter(SITES.iter().cloned())
            .map(|site: EmailSite| {
                let client = client.clone();
                let email = email.clone();
                let done = done.clone();
                let ctx = ctx.clone();
                async move {
                    if ctx.cancelled() {
                        return;
                    }
                    let verdict = probe(&client, &site, &email).await;
                    let i = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if i.is_multiple_of(10) {
                        ctx.progress(i as u64, total as u64).await;
                    }
                    if verdict != Verdict::Exists {
                        return;
                    }
                    let probe_url = site
                        .url
                        .replace("{email_q}", &urlencode(&email))
                        .replace("{email}", email.as_str());
                    ctx.emit(Finding {
                        module_id: "email.accounts".into(),
                        category: "email_account".into(),
                        label: site.name.into(),
                        value: "Registered account".into(),
                        url: Some(format!("https://{}", site.domain)),
                        confidence: site.confidence,
                        target: None,
                        detail: Some(serde_json::json!({
                            "site": site.domain,
                            "evidence": site.evidence,
                            "probe": probe_url,
                        })),
                    })
                    .await;
                }
            })
            .buffer_unordered(conc)
            .count()
            .await;
        ctx.progress(total as u64, total as u64).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict_of(name: &str, status: u16, body: &str) -> Verdict {
        let site = SITES
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("{name} in table"));
        (site.check.expect("check fn"))(&ProbeResponse {
            status,
            body: body.to_string(),
        })
    }

    #[test]
    fn sites_table_is_sane() {
        let mut names: Vec<_> = SITES.iter().map(|s| s.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
        assert_eq!(SITES.len(), 20);
        for site in SITES {
            assert!(!site.domain.is_empty());
            assert!(!site.evidence.is_empty());
            if site.name == "GitHub" {
                assert!(site.check.is_none());
            } else {
                assert!(site.check.is_some());
                assert!(
                    site.url.contains("{email_q}")
                        || site.url.contains("{email}")
                        || site.body.contains("{email_q}")
                        || site.body.contains("{email}"),
                    "{} misses the email placeholder",
                    site.name
                );
            }
        }
    }

    #[test]
    fn urlencode_escapes_email() {
        assert_eq!(
            urlencode("jane.doe.1990@proton.me"),
            "jane.doe.1990%40proton.me"
        );
        assert_eq!(urlencode("a+b@c.co"), "a%2Bb%40c.co");
    }

    #[test]
    fn twitter_taken_flag() {
        assert_eq!(
            verdict_of("Twitter / X", 200, r#"{"taken":true,"reason":"taken"}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Twitter / X", 200, r#"{"taken":false}"#),
            Verdict::Free
        );
        assert_eq!(verdict_of("Twitter / X", 404, "blocked"), Verdict::Unknown);
    }

    #[test]
    fn spotify_status_codes() {
        assert_eq!(
            verdict_of("Spotify", 200, r#"{"status":20}"#),
            Verdict::Exists
        );
        assert_eq!(verdict_of("Spotify", 200, r#"{"status":1}"#), Verdict::Free);
        assert_eq!(
            verdict_of("Spotify", 200, r#"{"status":400}"#),
            Verdict::Unknown
        );
    }

    #[test]
    fn firefox_exists_flag() {
        assert_eq!(
            verdict_of("Firefox", 200, r#"{"exists":true}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Firefox", 200, r#"{"exists":false}"#),
            Verdict::Free
        );
    }

    #[test]
    fn lastpass_availability() {
        assert_eq!(verdict_of("LastPass", 200, "no"), Verdict::Exists);
        assert_eq!(verdict_of("LastPass", 200, "ok"), Verdict::Free);
        assert_eq!(verdict_of("LastPass", 200, "emailinvalid"), Verdict::Free);
    }

    #[test]
    fn patreon_inverted_availability() {
        assert_eq!(
            verdict_of("Patreon", 200, r#"{"data":{"is_available":false}}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Patreon", 200, r#"{"data":{"is_available":true}}"#),
            Verdict::Free
        );
    }

    #[test]
    fn tellonym_marker() {
        assert_eq!(
            verdict_of("Tellonym", 200, r#"{"errors":"EMAIL_ALREADY_IN_USE"}"#),
            Verdict::Exists
        );
        assert_eq!(verdict_of("Tellonym", 200, r#"{"ok":true}"#), Verdict::Free);
    }

    #[test]
    fn duolingo_users_array() {
        assert_eq!(
            verdict_of("Duolingo", 200, r#"{"users":[{"id":1}]}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Duolingo", 200, r#"{"users":[]}"#),
            Verdict::Free
        );
    }

    #[test]
    fn nike_status_conflict() {
        assert_eq!(verdict_of("Nike", 409, "{}"), Verdict::Exists);
        assert_eq!(verdict_of("Nike", 200, "{}"), Verdict::Free);
    }

    #[test]
    fn deliveroo_registered_flag() {
        assert_eq!(
            verdict_of("Deliveroo", 200, r#"{"registered":true}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Deliveroo", 200, r#"{"registered":false}"#),
            Verdict::Free
        );
    }

    #[test]
    fn flickr_state_code() {
        assert_eq!(
            verdict_of("Flickr", 200, r#"{"state_code":5}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Flickr", 200, r#"{"state_code":0}"#),
            Verdict::Free
        );
    }

    #[test]
    fn replit_exists_flag() {
        assert_eq!(
            verdict_of("Replit", 200, r#"{"exists":true}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Replit", 200, r#"{"exists":false}"#),
            Verdict::Free
        );
    }

    #[test]
    fn vsco_email_status() {
        assert_eq!(
            verdict_of("VSCO", 200, r#"{"email_status":"has_account"}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("VSCO", 200, r#"{"email_status":"no_account"}"#),
            Verdict::Free
        );
    }

    #[test]
    fn anydo_user_exists() {
        assert_eq!(
            verdict_of("Any.do", 200, r#"{"user_exists":true}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Any.do", 200, r#"{"user_exists":false}"#),
            Verdict::Free
        );
    }

    #[test]
    fn archive_taken_marker() {
        assert_eq!(
            verdict_of("Internet Archive", 200, "That email is already taken."),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Internet Archive", 200, "welcome"),
            Verdict::Free
        );
    }

    #[test]
    fn proton_hkp_index() {
        assert_eq!(
            verdict_of("Proton Mail", 200, "info:1:1\n"),
            Verdict::Exists
        );
        assert_eq!(verdict_of("Proton Mail", 200, "info:1:0\n"), Verdict::Free);
    }

    #[test]
    fn mailru_recovery_status() {
        assert_eq!(
            verdict_of("Mail.ru", 200, r#"{"status":200,"body":{}}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Mail.ru", 200, r#"{"status":404}"#),
            Verdict::Free
        );
    }

    #[test]
    fn imgur_available_flag() {
        assert_eq!(
            verdict_of("Imgur", 200, r#"{"data":{"available":false}}"#),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Imgur", 200, r#"{"data":{"available":true}}"#),
            Verdict::Free
        );
    }

    #[test]
    fn pinterest_email_exists() {
        assert_eq!(
            verdict_of(
                "Pinterest",
                200,
                r#"{"resource_response":{"data":{"email":"x"}}}"#
            ),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of("Pinterest", 200, r#"{"resource_response":{"data":null}}"#),
            Verdict::Free
        );
        assert_eq!(
            verdict_of(
                "Pinterest",
                200,
                r#"{"resource_response":{"data":{"source_field":"email"}}}"#
            ),
            Verdict::Unknown
        );
    }

    #[test]
    fn discord_registration_errors() {
        assert_eq!(
            verdict_of(
                "Discord",
                400,
                r#"{"errors":{"email":{"_errors":[{"code":"EMAIL_ALREADY_REGISTERED"}]}}}"#
            ),
            Verdict::Exists
        );
        assert_eq!(
            verdict_of(
                "Discord",
                400,
                r#"{"errors":{"email":{"_errors":[{"code":"EMAIL_INVALID"}]}}}"#
            ),
            Verdict::Free
        );
        assert_eq!(
            verdict_of("Discord", 400, r#"{"captcha_key":["captcha-required"]}"#),
            Verdict::Unknown
        );
    }

    #[test]
    fn discord_body_renders_random_credentials() {
        let site = SITES.iter().find(|s| s.name == "Discord").unwrap();
        let body = render_body(site, "jane@proton.me");
        assert!(body.contains(r#""email":"jane@proton.me""#));
        assert!(!body.contains("{rand20}"));
        assert_ne!(body, render_body(site, "jane@proton.me"));
    }

    #[test]
    fn github_token_extraction() {
        let html = r#"<form><auto-check src="/signup_check/username" csrf="a">
<input value="tok1">
</auto-check>
<auto-check src="/signup_check/email" csrf="b">
<input value="tok2">
</auto-check></form>"#;
        assert_eq!(github_token(html).as_deref(), Some("tok2"));
        assert_eq!(github_token("<html>nothing</html>"), None);
    }

    #[test]
    fn json_lookup_walks_arrays_and_objects() {
        let v: Value = serde_json::from_str(r#"{"a":[{"b":{"c":["hit"]}}]}"#).unwrap();
        assert_eq!(
            json_lookup(&v, &["a", "0", "b", "c", "0"]).and_then(|s| s.as_str()),
            Some("hit")
        );
    }
}
