use crate::models::ScanOptions;
use crate::CoreError;
use reqwest::redirect::Policy;
use reqwest::{Client, Method};
use serde_json::Value;
use std::time::Duration;

pub fn build_client(options: &ScanOptions) -> Result<Client, String> {
    let mut builder = Client::builder()
        .user_agent(options.user_agent.clone())
        .timeout(Duration::from_secs(options.timeout_secs))
        .connect_timeout(Duration::from_secs(options.timeout_secs.min(10)))
        .redirect(Policy::limited(6))
        .gzip(true);
    let proxy = options.proxy.trim().to_string();
    if !proxy.is_empty() {
        let scheme = options.proxy_scheme().unwrap_or_default();
        match scheme.as_str() {
            "http" | "https" | "socks5" | "socks5h" => {}
            other => {
                return Err(format!(
                    "unsupported proxy scheme '{other}' (use http, https, socks5 or socks5h)"
                ))
            }
        }
        let built = reqwest::Proxy::all(&proxy).map_err(|e| format!("bad proxy url: {e}"))?;
        builder = builder.proxy(built);
    }
    builder
        .build()
        .map_err(|e| format!("client build failed: {e}"))
}

pub async fn fetch_text(
    client: &Client,
    method: Method,
    url: &str,
) -> Result<(u16, String, String), CoreError> {
    let resp = client.request(method, url).send().await?;
    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let body = resp.text().await.unwrap_or_default();
    Ok((status, body, final_url))
}

pub async fn fetch_json(client: &Client, url: &str) -> Result<Value, CoreError> {
    let resp = client.get(url).send().await?.error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

pub async fn dns_lookup(
    client: &Client,
    name: &str,
    rrtype: &str,
) -> Result<Vec<String>, CoreError> {
    let url = format!("https://dns.google/resolve?name={name}&type={rrtype}");
    let value = fetch_json(client, &url).await?;
    let mut out = Vec::new();
    if let Some(answers) = value["Answer"].as_array() {
        for a in answers {
            if let Some(data) = a["data"].as_str() {
                out.push(data.trim_end_matches('.').to_string());
            }
        }
    }
    Ok(out)
}

pub async fn reverse_dns(client: &Client, ip: &str) -> Result<Vec<String>, CoreError> {
    let arpa = match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => {
            let octets: Vec<String> = v4.octets().iter().rev().map(|o| o.to_string()).collect();
            format!("{}.in-addr.arpa", octets.join("."))
        }
        Ok(std::net::IpAddr::V6(v6)) => {
            let hex: String = v6.octets().iter().map(|o| format!("{o:02x}")).collect();
            let nibbles: Vec<String> = hex.chars().rev().map(String::from).collect();
            format!("{}.ip6.arpa", nibbles.join("."))
        }
        Err(_) => return Ok(vec![]),
    };
    dns_lookup(client, &arpa, "PTR").await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(proxy: &str) -> ScanOptions {
        ScanOptions {
            proxy: proxy.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn direct_client_builds() {
        assert!(build_client(&opts("")).is_ok());
    }

    #[test]
    fn tor_proxy_builds() {
        assert!(build_client(&opts("socks5h://127.0.0.1:9050")).is_ok());
        assert!(build_client(&opts("socks5://127.0.0.1:9050")).is_ok());
        assert!(build_client(&opts("http://proxy.internal:3128")).is_ok());
    }

    #[test]
    fn bad_proxy_rejected() {
        assert!(build_client(&opts("ftp://127.0.0.1:1080")).is_err());
        assert!(build_client(&opts("not a url")).is_err());
    }

    #[test]
    fn proxy_scheme_detected() {
        assert_eq!(
            opts("socks5h://127.0.0.1:9050").proxy_scheme().as_deref(),
            Some("socks5h")
        );
        assert_eq!(opts("http://x:1").proxy_scheme().as_deref(), Some("http"));
        assert_eq!(opts("").proxy_scheme(), None);
    }
}
