use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::Value;

pub struct BreachModule;

pub struct BreachEntry {
    pub name: String,
    pub title: String,
    pub domain: String,
    pub breach_date: String,
    pub pwn_count: u64,
    pub data_classes: Vec<String>,
    pub verified: bool,
}

pub fn parse_breaches(body: &str) -> Vec<BreachEntry> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|b| {
            let name = b.get("Name")?.as_str()?.to_string();
            Some(BreachEntry {
                title: b
                    .get("Title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&name)
                    .to_string(),
                name,
                domain: b
                    .get("Domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                breach_date: b
                    .get("BreachDate")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                pwn_count: b.get("PwnCount").and_then(|v| v.as_u64()).unwrap_or(0),
                data_classes: b
                    .get("DataClasses")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| c.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                verified: b
                    .get("IsVerified")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            })
        })
        .collect()
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[async_trait::async_trait]
impl Module for BreachModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "email.breaches",
            name: "Breach Lookup",
            description: "Have I Been Pwned lookup listing every breach that exposed the address with dates, record counts and data classes (bring your own free HIBP API key from Settings)",
            accepts: &[crate::models::TargetKind::Email],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let email = ctx.target.normalized.clone();
        ctx.progress(0, 2).await;

        let key = ctx.options.hibp_api_key.trim().to_string();
        if key.is_empty() {
            ctx.note("breach lookup skipped: no HIBP API key configured")
                .await;
            ctx.emit(Finding {
                module_id: "email.breaches".into(),
                category: "breach_info".into(),
                label: "Breach lookup skipped".into(),
                value: "Add a free Have I Been Pwned API key in Settings to enable breach intel"
                    .into(),
                url: Some("https://haveibeenpwned.com/API/Key".into()),
                confidence: Confidence::Low,
                target: None,
                detail: None,
            })
            .await;
            ctx.progress(1, 2).await;
            return Ok(());
        }

        let url = format!(
            "https://haveibeenpwned.com/api/v3/breachedaccount/{email}?truncateResponse=false&includeUnverified=false"
        );
        match ctx
            .client
            .get(&url)
            .header("hibp-api-key", key)
            .header("user-agent", "Anyway-OSINT")
            .send()
            .await
        {
            Err(e) => ctx.note(&format!("hibp request failed: {e}")).await,
            Ok(resp) => {
                let status = resp.status().as_u16();
                match status {
                    200 => {
                        let body = resp.text().await.unwrap_or_default();
                        let breaches = parse_breaches(&body);
                        ctx.note(&format!(
                            "{} breach{} found for {email}",
                            breaches.len(),
                            if breaches.len() == 1 { "" } else { "es" }
                        ))
                        .await;
                        for b in &breaches {
                            ctx.emit(Finding {
                                module_id: "email.breaches".into(),
                                category: "breach".into(),
                                label: b.title.clone(),
                                value: format!(
                                    "{} · {} accounts · {}",
                                    b.breach_date,
                                    thousands(b.pwn_count),
                                    b.data_classes.join(", ")
                                ),
                                url: Some(format!(
                                    "https://haveibeenpwned.com/PwnedWebsites#{}",
                                    b.name.replace(' ', "")
                                )),
                                confidence: Confidence::High,
                                target: None,
                                detail: Some(serde_json::json!({
                                    "name": b.name,
                                    "domain": b.domain,
                                    "breachDate": b.breach_date,
                                    "pwnCount": b.pwn_count,
                                    "dataClasses": b.data_classes,
                                    "verified": b.verified,
                                })),
                            })
                            .await;
                        }
                    }
                    404 => ctx.note(&format!("no known breaches for {email}")).await,
                    401 => {
                        ctx.note("hibp rejected the API key (HTTP 401)").await;
                        ctx.emit(Finding {
                            module_id: "email.breaches".into(),
                            category: "breach_info".into(),
                            label: "Breach lookup failed".into(),
                            value: "The Have I Been Pwned API key was rejected (HTTP 401)".into(),
                            url: Some("https://haveibeenpwned.com/API/Key".into()),
                            confidence: Confidence::Low,
                            target: None,
                            detail: None,
                        })
                        .await;
                    }
                    429 => ctx.note("hibp rate limit reached, retry later").await,
                    other => ctx.note(&format!("hibp returned HTTP {other}")).await,
                }
            }
        }
        ctx.progress(2, 2).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {
            "Name": "Adobe",
            "Title": "Adobe",
            "Domain": "adobe.com",
            "BreachDate": "2013-10-04",
            "PwnCount": 152445165,
            "DataClasses": ["Email addresses", "Password hints", "Passwords"],
            "IsVerified": true,
            "IsUnverified": false
        },
        {
            "Name": "Canva",
            "Title": "Canva",
            "Domain": "canva.com",
            "BreachDate": "2019-05-24",
            "PwnCount": 137272116,
            "DataClasses": ["Email addresses", "Geographic locations", "Passwords"],
            "IsVerified": true,
            "IsUnverified": false
        }
    ]"#;

    #[test]
    fn parses_hibp_array() {
        let breaches = parse_breaches(SAMPLE);
        assert_eq!(breaches.len(), 2);
        assert_eq!(breaches[0].name, "Adobe");
        assert_eq!(breaches[0].breach_date, "2013-10-04");
        assert_eq!(breaches[0].pwn_count, 152445165);
        assert_eq!(breaches[0].data_classes.len(), 3);
        assert!(breaches[0].verified);
    }

    #[test]
    fn tolerates_garbage_and_missing_fields() {
        assert!(parse_breaches("not json").is_empty());
        assert!(parse_breaches(r#"{"unexpected":1}"#).is_empty());
        assert!(parse_breaches(r#"[{"Title":"NoName"}]"#).is_empty());
    }

    #[test]
    fn thousands_formats_counts() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(152445165), "152,445,165");
    }
}
