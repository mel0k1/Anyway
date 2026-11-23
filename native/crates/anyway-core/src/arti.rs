use arti_client::{TorClient, TorClientConfig};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tor_rtcompat::PreferredRuntime;

pub const ARTI_SOCKS_PORT: u16 = 9051;
const MAX_HANDSHAKE: usize = 512;

pub struct ArtiShared {
    status: &'static str,
    port: u16,
    detail: String,
    started_at: u64,
    ready_at: Option<u64>,
    connections: u64,
}

fn shared() -> &'static Mutex<ArtiShared> {
    static ARTI: OnceLock<Mutex<ArtiShared>> = OnceLock::new();
    ARTI.get_or_init(|| {
        Mutex::new(ArtiShared {
            status: "off",
            port: ARTI_SOCKS_PORT,
            detail: String::new(),
            started_at: 0,
            ready_at: None,
            connections: 0,
        })
    })
}

fn client_cell() -> &'static Mutex<Option<Arc<TorClient<PreferredRuntime>>>> {
    static CLIENT: OnceLock<Mutex<Option<Arc<TorClient<PreferredRuntime>>>>> = OnceLock::new();
    CLIENT.get_or_init(|| Mutex::new(None))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn ready() -> bool {
    shared().lock().expect("arti shared").status == "ready"
}

pub fn proxy_url() -> Option<String> {
    if ready() {
        Some(format!("socks5h://127.0.0.1:{}", ARTI_SOCKS_PORT))
    } else {
        None
    }
}

pub fn status_lite() -> Value {
    let s = shared().lock().expect("arti shared");
    json!({
        "status": s.status,
        "port": s.port,
        "ready": s.status == "ready",
    })
}

pub fn status() -> Value {
    let s = shared().lock().expect("arti shared");
    status_locked(&s)
}

fn status_locked(s: &ArtiShared) -> Value {
    json!({
        "ok": true,
        "running": s.status == "ready",
        "status": s.status,
        "port": s.port,
        "detail": s.detail,
        "started_at": s.started_at,
        "ready_at": s.ready_at,
        "connections": s.connections,
    })
}

pub fn start(port: u16, data_dir: PathBuf) -> Value {
    let current;
    {
        let mut s = shared().lock().expect("arti shared");
        if s.status == "starting" || s.status == "bootstrapping" || s.status == "ready" {
            current = status_locked(&s);
        } else {
            s.status = "starting";
            s.port = port;
            s.detail.clear();
            s.started_at = now_unix();
            s.ready_at = None;
            current = status_locked(&s);
        }
    }
    if current["status"] == "starting" {
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let mut s = shared().lock().expect("arti shared");
                    s.status = "failed";
                    s.detail = format!("arti runtime: {e}");
                    return;
                }
            };
            rt.block_on(run(port, data_dir));
        });
    }
    current
}

pub fn kick_start() {
    let dir = std::env::temp_dir().join("anyway-arti");
    start(ARTI_SOCKS_PORT, dir);
}

async fn run(port: u16, data_dir: PathBuf) {
    let boot = async {
        let mut builder = TorClientConfig::builder();
        builder
            .storage()
            .state_dir(arti_client::config::CfgPath::new_literal(
                data_dir.join("state"),
            ))
            .cache_dir(arti_client::config::CfgPath::new_literal(
                data_dir.join("cache"),
            ));
        let config = builder.build().map_err(|e| format!("arti config: {e}"))?;
        let client = TorClient::builder()
            .config(config)
            .create_unbootstrapped()
            .map_err(|e| format!("arti client: {e}"))?;
        set_status("bootstrapping");
        client
            .bootstrap()
            .await
            .map_err(|e| format!("arti bootstrap: {e}"))?;
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("arti socks bind: {e}"))?;
        Ok::<(TcpListener, Arc<TorClient<PreferredRuntime>>), String>((listener, client))
    };
    match boot.await {
        Ok((listener, client)) => {
            *client_cell().lock().expect("arti client") = Some(client.clone());
            {
                let mut s = shared().lock().expect("arti shared");
                s.status = "ready";
                s.ready_at = Some(now_unix());
                s.detail.clear();
            }
            accept_loop(listener, client).await;
        }
        Err(e) => {
            let mut s = shared().lock().expect("arti shared");
            s.status = "failed";
            s.detail = e;
        }
    }
}

async fn accept_loop(listener: TcpListener, client: Arc<TorClient<PreferredRuntime>>) {
    loop {
        match listener.accept().await {
            Ok((sock, _peer)) => {
                let client = client.clone();
                tokio::spawn(async move {
                    let n = serve_conn(client, sock).await;
                    if n > 0 {
                        shared().lock().expect("arti shared").connections += 1;
                    }
                });
            }
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(200)).await,
        }
    }
}

fn set_status(status: &'static str) {
    shared().lock().expect("arti shared").status = status;
}

async fn serve_conn(client: Arc<TorClient<PreferredRuntime>>, mut sock: TcpStream) -> u8 {
    let Ok((host, port)) = handshake(&mut sock).await else {
        return 0;
    };
    let request = build_success_reply();
    if sock.write_all(&request).await.is_err() {
        return 0;
    }
    match client.connect((host.as_str(), port)).await {
        Ok(mut stream) => {
            let _ = copy_bidirectional(&mut sock, &mut stream).await;
            1
        }
        Err(_) => 0,
    }
}

async fn handshake(sock: &mut TcpStream) -> Result<(String, u16), ()> {
    let mut head = [0u8; 2];
    sock.read_exact(&mut head).await.map_err(|_| ())?;
    let (ver, nmethods) = (head[0], head[1] as usize);
    if ver != 0x05 || nmethods == 0 || nmethods > 16 {
        return Err(());
    }
    let mut methods = vec![0u8; nmethods];
    sock.read_exact(&mut methods).await.map_err(|_| ())?;
    if !methods.contains(&0x00) {
        let _ = sock.write_all(&[0x05, 0xFF]).await;
        return Err(());
    }
    sock.write_all(&[0x05, 0x00]).await.map_err(|_| ())?;

    let mut req = [0u8; 4];
    sock.read_exact(&mut req).await.map_err(|_| ())?;
    if req[0] != 0x05 || req[1] != 0x01 {
        let _ = sock
            .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
            .await;
        return Err(());
    }
    let atyp = req[3];
    let body: Vec<u8> = match atyp {
        0x01 => {
            let mut b = vec![0u8; 6];
            sock.read_exact(&mut b).await.map_err(|_| ())?;
            b
        }
        0x03 => {
            let mut lb = [0u8; 1];
            sock.read_exact(&mut lb).await.map_err(|_| ())?;
            let n = lb[0] as usize;
            if n == 0 || 1 + n + 2 > MAX_HANDSHAKE {
                let _ = sock
                    .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await;
                return Err(());
            }
            let mut b = Vec::with_capacity(1 + n + 2);
            b.push(lb[0]);
            let mut host = vec![0u8; n];
            sock.read_exact(&mut host).await.map_err(|_| ())?;
            b.extend_from_slice(&host);
            let mut tail = [0u8; 2];
            sock.read_exact(&mut tail).await.map_err(|_| ())?;
            b.extend_from_slice(&tail);
            b
        }
        0x04 => {
            let mut b = vec![0u8; 18];
            sock.read_exact(&mut b).await.map_err(|_| ())?;
            b
        }
        _ => {
            let _ = sock
                .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            return Err(());
        }
    };
    let mut full = Vec::with_capacity(4 + body.len());
    full.extend_from_slice(&req);
    full.extend_from_slice(&body);
    match parse_socks5_request(&full) {
        Ok((host, port)) => Ok((host, port)),
        Err(code) => {
            let _ = sock
                .write_all(&[0x05, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await;
            Err(())
        }
    }
}

fn parse_socks5_request(buf: &[u8]) -> Result<(String, u16), u8> {
    if buf.len() < 5 || buf[0] != 0x05 {
        return Err(0x01);
    }
    if buf[1] != 0x01 {
        return Err(0x07);
    }
    let port = u16::from_be_bytes([buf[buf.len() - 2], buf[buf.len() - 1]]);
    match buf[3] {
        0x01 => {
            if buf.len() != 4 + 4 + 2 {
                return Err(0x01);
            }
            let ip = std::net::Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]);
            Ok((ip.to_string(), port))
        }
        0x03 => {
            let n = buf[4] as usize;
            if buf.len() != 4 + 1 + n + 2 || n == 0 || n > 255 {
                return Err(0x01);
            }
            let host = String::from_utf8_lossy(&buf[5..5 + n]).to_string();
            if host.is_empty() {
                return Err(0x01);
            }
            Ok((host, port))
        }
        0x04 => {
            if buf.len() != 4 + 16 + 2 {
                return Err(0x01);
            }
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&buf[4..20]);
            Ok((std::net::Ipv6Addr::from(octets).to_string(), port))
        }
        _ => Err(0x08),
    }
}

fn build_success_reply() -> [u8; 10] {
    [0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(atyp: u8, addr: &[u8], port: u16) -> Vec<u8> {
        let mut v = vec![0x05, 0x01, 0x00, atyp];
        v.extend_from_slice(addr);
        v.extend_from_slice(&port.to_be_bytes());
        v
    }

    #[test]
    fn parses_domain_ipv4_ipv6() {
        let domain = req(0x03, b"\x0bexample.org", 443);
        let (host, port) = parse_socks5_request(&domain).unwrap();
        assert_eq!(host, "example.org");
        assert_eq!(port, 443);

        let v4 = req(0x01, &[1, 2, 3, 4], 9050);
        let (host, port) = parse_socks5_request(&v4).unwrap();
        assert_eq!(host, "1.2.3.4");
        assert_eq!(port, 9050);

        let mut v6 = req(
            0x04,
            &[0x2a, 6, 0x98, 0xc0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            80,
        );
        v6.truncate(v6.len());
        let (host, port) = parse_socks5_request(&v6).unwrap();
        assert!(host.starts_with("2a06:"));
        assert_eq!(port, 80);
    }

    #[test]
    fn rejects_malformed_requests() {
        let mut bad = req(0x03, b"\x04mail", 25);
        bad[0] = 0x04;
        assert_eq!(parse_socks5_request(&bad), Err(0x01));

        let mut bind = req(0x03, b"\x04mail", 25);
        bind[1] = 0x02;
        assert_eq!(parse_socks5_request(&bind), Err(0x07));

        assert_eq!(
            parse_socks5_request(&[0x05, 0x01, 0x00, 0x01, 1, 2, 3]),
            Err(0x01)
        );

        assert_eq!(parse_socks5_request(&req(0x03, b"", 80)), Err(0x01));

        assert_eq!(parse_socks5_request(&req(0x09, &[1, 2], 80)), Err(0x08));
    }

    #[test]
    fn status_shape_before_start() {
        let s = status();
        assert_eq!(s["ok"], true);
        assert!(s["port"].is_u64());
        assert_eq!(s["running"], false);
        assert!(status_lite()["ready"].is_boolean());
    }

    #[tokio::test]
    async fn handshake_reads_domain_requests() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            handshake(&mut sock).await
        });
        let mut c = TcpStream::connect(addr).await.unwrap();
        c.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut sel = [0u8; 2];
        c.read_exact(&mut sel).await.unwrap();
        assert_eq!(sel, [0x05, 0x00]);
        let mut msg = vec![0x05, 0x01, 0x00, 0x03, 0x0b];
        msg.extend_from_slice(b"example.org");
        msg.extend_from_slice(&443u16.to_be_bytes());
        c.write_all(&msg).await.unwrap();
        let (host, port) = server.await.unwrap().unwrap();
        assert_eq!(host, "example.org");
        assert_eq!(port, 443);
    }

    #[tokio::test]
    async fn start_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("arti-test-{}", std::process::id()));
        let first = start(ARTI_SOCKS_PORT, dir.clone());
        let second = start(ARTI_SOCKS_PORT, dir);

        assert_eq!(first["status"], second["status"]);
        assert!(
            first["status"] == "starting"
                || first["status"] == "bootstrapping"
                || first["status"] == "ready"
        );
    }
}
