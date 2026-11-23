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
pub struct PivotLink {
    pub parent: String,
    pub parent_kind: TargetKind,
    pub child: Target,
    pub depth: u32,
    pub via: String,
}

/// How the scan should route network traffic.
/// - `Auto`: use a local Tor SOCKS port when one is listening, otherwise go direct.
/// - `On`: always route through Tor; the scan fails closed when Tor is unreachable.
/// - `Off`: never touch Tor, use the explicit proxy (or direct) as configured.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TorMode {
    #[default]
    Auto,
    On,
    Off,
}

impl TorMode {
    pub fn from_wire(s: &str) -> Self {
        match s {
            "on" => TorMode::On,
            "off" => TorMode::Off,
            _ => TorMode::Auto,
        }
    }

    pub fn wire(&self) -> &'static str {
        match self {
            TorMode::Auto => "auto",
            TorMode::On => "on",
            TorMode::Off => "off",
        }
    }
}

pub const TOR_DEFAULT_PORT: u16 = 9050;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanOptions {
    pub concurrency: u32,
    pub timeout_secs: u64,
    pub user_agent: String,
    pub proxy: String,
    pub tor_mode: TorMode,
    pub tor_socks_port: u16,
    pub hibp_api_key: String,
    pub vt_api_key: String,
    pub embedded_tor: bool,
    pub enabled_modules: Vec<String>,
    pub pivot: bool,
    pub pivot_depth: u32,
    pub pivot_budget: u32,
    pub long_tail: bool,
    pub browser_checks: bool,
    pub allowed_categories: Vec<String>,
    pub blocked_categories: Vec<String>,
    pub countries: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            concurrency: 24,
            timeout_secs: 12,
            user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_string(),
            proxy: String::new(),
            tor_mode: TorMode::Auto,
            tor_socks_port: TOR_DEFAULT_PORT,
            hibp_api_key: String::new(),
            vt_api_key: String::new(),
            embedded_tor: true,
            enabled_modules: Vec::new(),
            pivot: false,
            pivot_depth: 2,
            pivot_budget: 8,
            long_tail: false,
            browser_checks: false,
            allowed_categories: Vec::new(),
            blocked_categories: Vec::new(),
            countries: Vec::new(),
        }
    }
}

impl ScanOptions {
    pub fn proxy_scheme(&self) -> Option<String> {
        let p = self.proxy.trim();
        let (scheme, _) = p.split_once("://")?;
        Some(scheme.to_ascii_lowercase())
    }

    pub fn tor_proxy_url(&self) -> String {
        format!("socks5h://127.0.0.1:{}", self.tor_socks_port)
    }

    /// Resolve which proxy URL the scan should use.
    ///
    /// `tor_listening` reflects whether the local Tor SOCKS port accepted a TCP
    /// connection. An explicit user proxy always wins; Tor applies otherwise.
    /// `On` keeps routing through Tor even when unreachable, so the scan fails
    /// closed instead of leaking traffic.
    pub fn effective_proxy(&self, tor_listening: bool) -> String {
        self.effective_proxy_with(tor_listening, false)
    }

    /// Same, but also considers the embedded (arti) Tor client as a fallback
    /// route when the external daemon is absent.
    pub fn effective_proxy_with(&self, tor_listening: bool, arti_ready: bool) -> String {
        let explicit = self.proxy.trim();
        if !explicit.is_empty() {
            return explicit.to_string();
        }
        let arti_url = format!("socks5h://127.0.0.1:{}", crate::arti::ARTI_SOCKS_PORT);
        match self.tor_mode {
            TorMode::Off => String::new(),
            TorMode::On => {
                if tor_listening || !self.embedded_tor {
                    self.tor_proxy_url()
                } else if arti_ready {
                    arti_url
                } else {
                    self.tor_proxy_url()
                }
            }
            TorMode::Auto => {
                if tor_listening {
                    self.tor_proxy_url()
                } else if arti_ready {
                    arti_url
                } else {
                    String::new()
                }
            }
        }
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
