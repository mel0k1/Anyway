pub mod breaches;
pub mod domain;
pub mod dorks;
pub mod email;
pub mod email_accounts;
pub mod enrich;
pub mod exif;
pub mod ipaddr;
pub mod phone;
pub mod sites;
pub mod social_graph;
pub mod username;

use crate::engine::ModuleCtx;
use crate::error::CoreError;
use crate::models::TargetKind;
use std::sync::Arc;

pub struct ModuleDescriptor {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub accepts: &'static [TargetKind],
}

#[async_trait::async_trait]
pub trait Module: Send + Sync {
    fn descriptor(&self) -> &'static ModuleDescriptor;

    fn id(&self) -> &'static str {
        self.descriptor().id
    }

    fn accepts(&self, kind: TargetKind) -> bool {
        self.descriptor().accepts.contains(&kind)
    }

    async fn run(&self, ctx: ModuleCtx) -> Result<(), CoreError>;
}

pub fn registry() -> Vec<Arc<dyn Module>> {
    vec![
        Arc::new(username::UsernameModule),
        Arc::new(enrich::EnrichModule),
        Arc::new(social_graph::SocialGraphModule),
        Arc::new(dorks::DorksModule),
        Arc::new(email::EmailModule),
        Arc::new(email_accounts::EmailAccountsModule),
        Arc::new(breaches::BreachModule),
        Arc::new(ipaddr::IpModule),
        Arc::new(domain::DomainModule),
        Arc::new(phone::PhoneModule),
        Arc::new(exif::ExifModule),
    ]
}
