use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use md5::{Digest, Md5};
use serde_json::json;

const DISPOSABLE: &[&str] = &[
    "10minutemail.com", "guerrillamail.com", "mailinator.com", "tempmail.com",
    "throwawaymail.com", "yopmail.com", "getnada.com", "dispostable.com",
    "maildrop.cc", "sharklasers.com", "temp-mail.org", "fakeinbox.com",
    "mailnesia.com", "trashmail.com", "mytemp.email", "moakt.com",
];

const PROVIDERS: &[(&str, &str)] = &[
    ("gmail.com", "Google"), ("outlook.com", "Microsoft"), ("hotmail.com", "Microsoft"),
    ("live.com", "Microsoft"), ("yahoo.com", "Yahoo"), ("proton.me", "Proton"),
    ("protonmail.com", "Proton"), ("icloud.com", "Apple"), ("yandex.ru", "Yandex"),
    ("mail.ru", "VK"), ("zoho.com", "Zoho"), ("gmx.com", "GMX"), ("aol.com", "AOL"),
];

pub struct EmailModule;

#[async_trait::async_trait]
impl Module for EmailModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "email.recon",
            name: "Email Recon",
            description: "Syntax and MX validation, disposable detection, provider lookup and Gravatar resolution",
            accepts: &[crate::models::TargetKind::Email],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let email = ctx.target.normalized.clone();
        let Some((local, domain)) = email.split_once('@') else {
            return Ok(());
        };
        ctx.progress(0, 4).await;

        ctx.emit(Finding {
            module_id: "email.recon".into(),
            category: "email_info".into(),
            label: "Local part".into(),
            value: local.to_string(),
            url: None,
            confidence: Confidence::High,
            detail: Some(json!({"domain": domain})),
        })
        .await;
        ctx.progress(1, 4).await;

        let mx = crate::http::dns_lookup(&ctx.client, domain, "MX").await.unwrap_or_default();
        if mx.is_empty() {
            ctx.emit(Finding {
                module_id: "email.recon".into(),
                category: "email_info".into(),
                label: "No MX records".into(),
                value: domain.to_string(),
                url: None,
                confidence: Confidence::Medium,
                detail: None,
            })
            .await;
        } else {
            ctx.emit(Finding {
                module_id: "email.recon".into(),
                category: "email_info".into(),
                label: "Mail servers".into(),
                value: mx.join(", "),
                url: None,
                confidence: Confidence::High,
                detail: Some(json!({"records": mx})),
            })
            .await;
        }
        ctx.progress(2, 4).await;

        if DISPOSABLE.iter().any(|d| domain.eq_ignore_ascii_case(d)) {
            ctx.emit(Finding {
                module_id: "email.recon".into(),
                category: "email_info".into(),
                label: "Disposable provider".into(),
                value: domain.to_string(),
                url: None,
                confidence: Confidence::High,
                detail: None,
            })
            .await;
        }

        if let Some((_, provider)) = PROVIDERS.iter().find(|(d, _)| domain.eq_ignore_ascii_case(d)) {
            ctx.emit(Finding {
                module_id: "email.recon".into(),
                category: "email_info".into(),
                label: "Provider".into(),
                value: provider.to_string(),
                url: None,
                confidence: Confidence::High,
                detail: None,
            })
            .await;
        }
        ctx.progress(3, 4).await;

        let mut hasher = Md5::new();
        hasher.update(email.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let avatar_url = format!("https://gravatar.com/avatar/{hash}?d=404&s=200");
        if let Ok(resp) = ctx.client.head(&avatar_url).send().await {
            if resp.status().is_success() {
                ctx.emit(Finding {
                    module_id: "email.recon".into(),
                    category: "gravatar".into(),
                    label: "Gravatar avatar".into(),
                    value: avatar_url.clone(),
                    url: Some(avatar_url),
                    confidence: Confidence::High,
                    detail: Some(json!({"hash": hash})),
                })
                .await;
            }
        }

        let profile_url = format!("https://api.gravatar.com/v3/profiles/{hash}");
        if let Ok(resp) = ctx.client.get(&profile_url).send().await {
            if resp.status().is_success() {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    ctx.emit(Finding {
                        module_id: "email.recon".into(),
                        category: "gravatar".into(),
                        label: "Gravatar profile".into(),
                        value: v["display_name"].as_str().unwrap_or("profile").to_string(),
                        url: Some(format!("https://gravatar.com/{hash}")),
                        confidence: Confidence::High,
                        detail: Some(v),
                    })
                    .await;
                }
            }
        }
        ctx.progress(4, 4).await;
        Ok(())
    }
}
