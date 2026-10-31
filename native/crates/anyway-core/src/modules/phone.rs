use super::Module;
use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::{Confidence, Finding};
use serde_json::json;

pub struct PhoneModule;

#[async_trait::async_trait]
impl Module for PhoneModule {
    fn descriptor(&self) -> &'static super::ModuleDescriptor {
        &super::ModuleDescriptor {
            id: "phone.analysis",
            name: "Phone Analysis",
            description: "Parses the number with libphonenumber: country, line type and validity",
            accepts: &[crate::models::TargetKind::PhoneNumber],
        }
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError> {
        let raw = ctx.target.raw.clone();
        ctx.progress(0, 1).await;

        match phonenumber::parse(None, &raw) {
            Ok(number) => {
                let e164 = number.format().mode(phonenumber::Mode::E164).to_string();
                let intl = number
                    .format()
                    .mode(phonenumber::Mode::International)
                    .to_string();
                let country = number
                    .country()
                    .id()
                    .map(|id| format!("{id:?}"))
                    .unwrap_or_else(|| "unknown".into());
                let line_type =
                    format!("{:?}", number.number_type(&phonenumber::metadata::DATABASE));
                let valid = phonenumber::is_valid(&number);

                ctx.emit(Finding {
                    module_id: "phone.analysis".into(),
                    category: "phone_info".into(),
                    label: "Number details".into(),
                    value: intl.clone(),
                    url: None,
                    confidence: Confidence::High,
                    target: None,
                    detail: Some(json!({
                        "e164": e164,
                        "international": intl,
                        "country": country,
                        "lineType": line_type,
                        "valid": valid,
                    })),
                })
                .await;
            }
            Err(e) => {
                ctx.note(&format!("parse failed: {e}")).await;
            }
        }
        ctx.progress(1, 1).await;
        Ok(())
    }
}
