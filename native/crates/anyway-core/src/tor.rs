use crate::http::{build_client, fetch_json};
use crate::models::{ScanOptions, TorMode};
use serde_json::json;
use std::net::TcpStream;
use std::time::Duration;

/// Probe the local Tor SOCKS port with a short TCP connect.
pub fn tor_listening(options: &ScanOptions) -> bool {
    let addr = format!("127.0.0.1:{}", options.tor_socks_port);
    let Ok(addr) = addr.parse() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
}

#[derive(Debug, Clone)]
pub struct TorCheck {
    pub listening: bool,
    pub is_tor: Option<bool>,
    pub exit_ip: Option<String>,
    pub proxy: String,
    pub mode: TorMode,
}

impl TorCheck {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "listening": self.listening,
            "isTor": self.is_tor,
            "exitIp": self.exit_ip,
            "proxy": self.proxy,
            "mode": self.mode.wire(),
        })
    }
}

/// Verify the Tor route end to end: SOCKS port open and an exit node reachable.
pub async fn check(options: &ScanOptions) -> TorCheck {
    let listening = tor_listening(options);
    let check = TorCheck {
        listening,
        is_tor: None,
        exit_ip: None,
        proxy: options.effective_proxy(listening),
        mode: options.tor_mode,
    };
    if !listening {
        return check;
    }
    let mut probe_opts = options.clone();
    probe_opts.proxy = options.tor_proxy_url();
    let Ok(client) = build_client(&probe_opts) else {
        return check;
    };
    match fetch_json(&client, "https://check.torproject.org/api/ip").await {
        Ok(v) => TorCheck {
            listening,
            is_tor: v.get("IsTor").and_then(|b| b.as_bool()),
            exit_ip: v.get("IP").and_then(|s| s.as_str()).map(String::from),
            proxy: probe_opts.proxy,
            mode: options.tor_mode,
        },
        Err(_) => check,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: TorMode, proxy: &str, port: u16) -> ScanOptions {
        ScanOptions {
            tor_mode: mode,
            tor_socks_port: port,
            proxy: proxy.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn proxy_resolution_modes() {
        assert_eq!(opts(TorMode::Off, "", 9050).effective_proxy(true), "");
        assert_eq!(
            opts(TorMode::On, "", 9050).effective_proxy(false),
            "socks5h://127.0.0.1:9050"
        );
        assert_eq!(opts(TorMode::Auto, "", 9050).effective_proxy(false), "");
        assert_eq!(
            opts(TorMode::Auto, "", 9050).effective_proxy(true),
            "socks5h://127.0.0.1:9050"
        );
        // explicit proxy always wins
        assert_eq!(
            opts(TorMode::On, "http://corp:3128", 9050).effective_proxy(true),
            "http://corp:3128"
        );
        // custom port respected
        assert_eq!(
            opts(TorMode::On, "", 9051).effective_proxy(false),
            "socks5h://127.0.0.1:9051"
        );
    }

    #[test]
    fn tor_wire_roundtrip() {
        for m in [TorMode::Auto, TorMode::On, TorMode::Off] {
            assert_eq!(TorMode::from_wire(m.wire()), m);
        }
        assert_eq!(TorMode::from_wire("junk"), TorMode::Auto);
    }

    #[test]
    fn listening_false_without_tor() {
        // Nothing listens on 9905 in the test environment.
        let mut o = opts(TorMode::Auto, "", 9905);
        o.tor_socks_port = 9905;
        assert!(!tor_listening(&o));
    }

    #[test]
    fn check_json_shape() {
        let c = TorCheck {
            listening: false,
            is_tor: None,
            exit_ip: None,
            proxy: String::new(),
            mode: TorMode::Auto,
        };
        let v = c.to_json();
        assert_eq!(v["listening"], false);
        assert_eq!(v["mode"], "auto");
    }
}
