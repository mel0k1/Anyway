use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Username,
    Email,
    IpAddress,
    Domain,
    PhoneNumber,
    File,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub kind: TargetKind,
    pub raw: String,
    pub normalized: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub module_id: String,
    pub category: String,
    pub label: String,
    pub value: String,
    pub url: Option<String>,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub query: String,
    #[serde(default)]
    pub options: ScanOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    pub concurrency: u32,
    pub timeout_secs: u64,
    pub user_agent: String,
    pub proxy: String,
    pub hibp_api_key: String,
    pub enabled_modules: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            concurrency: 24,
            timeout_secs: 12,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_string(),
            proxy: String::new(),
            hibp_api_key: String::new(),
            enabled_modules: Vec::new(),
        }
    }
}

impl ScanOptions {
    pub fn proxy_scheme(&self) -> Option<String> {
        let p = self.proxy.trim();
        let (scheme, _) = p.split_once("://")?;
        Some(scheme.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleProgress {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(skip)]
    pub key: String,
    pub done: u64,
    pub total: u64,
    pub status: String,
    pub found: u64,
}
