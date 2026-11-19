use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::time::timeout;
use url::Url;

use crate::models::{normalize_timezone, ProxyCheckInfo, ProxyConfig};

trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}

fn looks_like_ipv4(s: &str) -> bool {
    s.parse::<Ipv4Addr>().is_ok()
}

fn split_host_port(s: &str) -> Option<(String, u16)> {
    let (h, p) = s.rsplit_once(':')?;
    let host = h.trim().trim_matches(['[', ']']);
    if host.is_empty() || host.contains(' ') {
        return None;
    }
    let port: u16 = p.trim().parse().ok()?;
    if port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

/// Parses proxy strings in any common format:
/// - `[scheme://]user:pass@host:port` (including usernames with dots, e.g. BrightData/SOAX)
/// - `[scheme://]host:port@user:pass`
/// - `[scheme://]host:port:user:pass`
/// - `[scheme://]user:pass:host:port`
/// - `[scheme://]host:port`
pub fn parse_proxy(raw_proxy: &str, default_scheme: &str) -> Option<ProxyConfig> {
    let mut clean = raw_proxy.trim();
    if clean.is_empty() {
        return None;
    }

    let mut scheme = default_scheme.trim().to_ascii_lowercase();
    if let Some((sch, rest)) = clean.split_once("://") {
        scheme = sch.trim().to_ascii_lowercase();
        clean = rest.trim();
    }

    if !matches!(scheme.as_str(), "http" | "https" | "socks5" | "socks4" | "socks4a") {
        return None;
    }
    if scheme == "socks4a" {
        scheme = "socks4".to_string();
    }

    if let Some((left, right)) = clean.rsplit_once('@') {
        // Check if `left` is explicitly `ip:port` and `right` is `user:pass`
        let left_hp = split_host_port(left);
        let right_hp = split_host_port(right);

        match (left_hp, right_hp) {
            // Standard URL format `user:pass@host:port` (preferred whenever right has valid host:port,
            // unless left is an IPv4 literal and right is NOT a plausible host:port)
            (Some((lh, lp)), None) => {
                let (u, p) = right.split_once(':')?;
                return Some(ProxyConfig {
                    scheme,
                    host: lh,
                    port: lp,
                    user: Some(u.to_string()),
                    password: Some(p.to_string()),
                });
            }
            (Some((lh, lp)), Some((rh, rp))) => {
                if looks_like_ipv4(&lh) && !looks_like_ipv4(&rh) && !rh.contains('.') {
                    let (u, p) = right.split_once(':')?;
                    return Some(ProxyConfig {
                        scheme,
                        host: lh,
                        port: lp,
                        user: Some(u.to_string()),
                        password: Some(p.to_string()),
                    });
                } else {
                    let (u, p) = left.split_once(':')?;
                    return Some(ProxyConfig {
                        scheme,
                        host: rh,
                        port: rp,
                        user: Some(u.to_string()),
                        password: Some(p.to_string()),
                    });
                }
            }
            (None, Some((rh, rp))) => {
                let (u, p) = left.split_once(':')?;
                return Some(ProxyConfig {
                    scheme,
                    host: rh,
                    port: rp,
                    user: Some(u.to_string()),
                    password: Some(p.to_string()),
                });
            }
            (None, None) => return None,
        }
    }

    let parts: Vec<&str> = clean.split(':').collect();
    match parts.len() {
        2 => {
            let (host, port) = split_host_port(clean)?;
            Some(ProxyConfig {
                scheme,
                host,
                port,
                user: None,
                password: None,
            })
        }
        4 => {
            // Distinguish `host:port:user:pass` vs `user:pass:host:port`
            let p1_port = parts[1].parse::<u16>().ok().filter(|&p| p > 0);
            let p3_port = parts[3].parse::<u16>().ok().filter(|&p| p > 0);

            if let Some(port) = p1_port {
                if looks_like_ipv4(parts[0]) || (parts[0].contains('.') && p3_port.is_none()) || p3_port.is_none() {
                    return Some(ProxyConfig {
                        scheme,
                        host: parts[0].to_string(),
                        port,
                        user: Some(parts[2].to_string()),
                        password: Some(parts[3].to_string()),
                    });
                }
            }
            if let Some(port) = p3_port {
                return Some(ProxyConfig {
                    scheme,
                    host: parts[2].to_string(),
                    port,
                    user: Some(parts[0].to_string()),
                    password: Some(parts[1].to_string()),
                });
            }
            None
        }
        _ => None,
    }
}

pub struct LocalProxyBridge {
    pub port: u16,
    shutdown_tx: watch::Sender<bool>,
}

impl LocalProxyBridge {
    pub async fn start(upstream: ProxyConfig) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Не удалось открыть локальный порт моста: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let upstream = Arc::new(upstream);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((client, _)) => {
                                let up = Arc::clone(&upstream);
                                tokio::spawn(async move {
                                    let _ = handle_client(client, up).await;
                                });
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Ok(Self { port, shutdown_tx })
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}

impl Drop for LocalProxyBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn read_http_headers(stream: &mut TcpStream, max_size: usize) -> Result<Vec<u8>, ()> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        if buf.len() >= max_size {
            return Err(());
        }
        let n = timeout(Duration::from_secs(20), stream.read(&mut chunk))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Ok(buf);
        }
    }
}

fn parse_target_host_port(method: &str, target: &str) -> Option<(String, u16)> {
    if method.eq_ignore_ascii_case("CONNECT") {
        if let Some((h, p)) = target.rsplit_once(':') {
            let port: u16 = p.parse().ok()?;
            return Some((h.trim_matches(['[', ']']).to_string(), port));
        }
        return Some((target.to_string(), 443));
    }

    if let Ok(parsed) = Url::parse(target) {
        let host = parsed.host_str()?.trim_matches(['[', ']']).to_string();
        let port = parsed.port_or_known_default().unwrap_or(80);
        return Some((host, port));
    }

    split_host_port(target).or_else(|| Some((target.to_string(), 80)))
}

async fn connect_upstream_stream(upstream: &ProxyConfig) -> Result<Box<dyn AsyncStream>, ()> {
    let raw = timeout(
        Duration::from_secs(15),
        TcpStream::connect((upstream.host.as_str(), upstream.port)),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    let _ = raw.set_nodelay(true);

    if upstream.scheme.eq_ignore_ascii_case("https") {
        let tls_cx = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|_| ())?;
        let connector = tokio_native_tls::TlsConnector::from(tls_cx);
        let tls_stream = timeout(
            Duration::from_secs(15),
            connector.connect(&upstream.host, raw),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
        Ok(Box::new(tls_stream))
    } else {
        Ok(Box::new(raw))
    }
}

fn rewrite_relative_http_request(req_data: &[u8], target: &str) -> Vec<u8> {
    if let Ok(parsed) = Url::parse(target) {
        let mut rel = parsed.path().to_string();
        if rel.is_empty() {
            rel.push('/');
        }
        if let Some(q) = parsed.query() {
            rel.push('?');
            rel.push_str(q);
        }
        if let Some(line_end) = req_data.windows(2).position(|w| w == b"\r\n") {
            let first_line = String::from_utf8_lossy(&req_data[..line_end]);
            let rewritten_line = first_line.replacen(target, &rel, 1);
            let mut out = Vec::with_capacity(req_data.len());
            out.extend_from_slice(rewritten_line.as_bytes());
            out.extend_from_slice(&req_data[line_end..]);
            return out;
        }
    }
    req_data.to_vec()
}

async fn handle_client(mut client: TcpStream, upstream: Arc<ProxyConfig>) -> Result<(), ()> {
    let _ = client.set_nodelay(true);
    let req_data = read_http_headers(&mut client, 32768).await?;

    let first_line_end = req_data.windows(2).position(|w| w == b"\r\n").ok_or(())?;
    let req_line = String::from_utf8_lossy(&req_data[..first_line_end]);
    let mut parts = req_line.split_whitespace();
    let method = parts.next().ok_or(())?.to_string();
    let target = parts.next().ok_or(())?.to_string();

    let mut upstream_stream = connect_upstream_stream(&upstream).await?;
    let scheme = upstream.scheme.as_str();

    if scheme == "http" || scheme == "https" {
        if method.eq_ignore_ascii_case("CONNECT") {
            let mut hdr = format!(
                "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Connection: Keep-Alive\r\n"
            );
            if let (Some(u), Some(p)) = (&upstream.user, &upstream.password) {
                let auth = BASE64.encode(format!("{u}:{p}"));
                hdr.push_str(&format!("Proxy-Authorization: Basic {auth}\r\n"));
            }
            hdr.push_str("\r\n");

            timeout(Duration::from_secs(15), upstream_stream.write_all(hdr.as_bytes()))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;

            let mut resp = Vec::with_capacity(512);
            let mut buf = [0u8; 1024];
            let hdr_end = loop {
                let n = timeout(Duration::from_secs(15), upstream_stream.read(&mut buf))
                    .await
                    .map_err(|_| ())?
                    .map_err(|_| ())?;
                if n == 0 {
                    return Err(());
                }
                resp.extend_from_slice(&buf[..n]);
                if let Some(pos) = resp.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                if resp.len() > 16384 {
                    return Err(());
                }
            };

            let status_line_end = resp.windows(2).position(|w| w == b"\r\n").unwrap_or(hdr_end);
            let status_line = String::from_utf8_lossy(&resp[..status_line_end]);
            if status_line.contains(" 200") {
                client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await
                    .map_err(|_| ())?;
                if hdr_end < resp.len() {
                    client.write_all(&resp[hdr_end..]).await.map_err(|_| ())?;
                }
            } else {
                let _ = client.write_all(&resp).await;
                return Err(());
            }
        } else {
            let mut out_req = Vec::with_capacity(req_data.len() + 128);
            out_req.extend_from_slice(&req_data[..first_line_end + 2]);
            if let (Some(u), Some(p)) = (&upstream.user, &upstream.password) {
                let auth = BASE64.encode(format!("{u}:{p}"));
                out_req.extend_from_slice(format!("Proxy-Authorization: Basic {auth}\r\n").as_bytes());
            }
            out_req.extend_from_slice(&req_data[first_line_end + 2..]);
            upstream_stream.write_all(&out_req).await.map_err(|_| ())?;
        }
    } else if scheme == "socks5" {
        if let (Some(u), Some(p)) = (&upstream.user, &upstream.password) {
            let u_bytes = u.as_bytes();
            let p_bytes = p.as_bytes();
            if u_bytes.len() > 255 || p_bytes.len() > 255 {
                return Err(());
            }
            upstream_stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await.map_err(|_| ())?;
            let mut greet = [0u8; 2];
            upstream_stream.read_exact(&mut greet).await.map_err(|_| ())?;
            if greet[0] != 0x05 {
                return Err(());
            }
            if greet[1] == 0x02 {
                let mut auth_pkt = Vec::with_capacity(3 + u_bytes.len() + p_bytes.len());
                auth_pkt.push(0x01);
                auth_pkt.push(u_bytes.len() as u8);
                auth_pkt.extend_from_slice(u_bytes);
                auth_pkt.push(p_bytes.len() as u8);
                auth_pkt.extend_from_slice(p_bytes);
                upstream_stream.write_all(&auth_pkt).await.map_err(|_| ())?;
                let mut auth_res = [0u8; 2];
                upstream_stream.read_exact(&mut auth_res).await.map_err(|_| ())?;
                if auth_res[1] != 0x00 {
                    return Err(());
                }
            } else if greet[1] != 0x00 {
                return Err(());
            }
        } else {
            upstream_stream.write_all(&[0x05, 0x01, 0x00]).await.map_err(|_| ())?;
            let mut greet = [0u8; 2];
            upstream_stream.read_exact(&mut greet).await.map_err(|_| ())?;
            if greet[0] != 0x05 || greet[1] != 0x00 {
                return Err(());
            }
        }

        let (thost, tport) = parse_target_host_port(&method, &target).ok_or(())?;
        let mut req_pkt = Vec::with_capacity(7 + thost.len());
        req_pkt.extend_from_slice(&[0x05, 0x01, 0x00]);
        if let Ok(ip4) = thost.parse::<Ipv4Addr>() {
            req_pkt.push(0x01);
            req_pkt.extend_from_slice(&ip4.octets());
        } else {
            let hb = thost.as_bytes();
            if hb.len() > 255 {
                return Err(());
            }
            req_pkt.push(0x03);
            req_pkt.push(hb.len() as u8);
            req_pkt.extend_from_slice(hb);
        }
        req_pkt.extend_from_slice(&tport.to_be_bytes());
        upstream_stream.write_all(&req_pkt).await.map_err(|_| ())?;

        let mut head = [0u8; 4];
        upstream_stream.read_exact(&mut head).await.map_err(|_| ())?;
        if head[1] != 0x00 {
            return Err(());
        }
        match head[3] {
            0x01 => {
                let mut skip = [0u8; 6];
                upstream_stream.read_exact(&mut skip).await.map_err(|_| ())?;
            }
            0x03 => {
                let mut len_b = [0u8; 1];
                upstream_stream.read_exact(&mut len_b).await.map_err(|_| ())?;
                let mut skip = vec![0u8; len_b[0] as usize + 2];
                upstream_stream.read_exact(&mut skip).await.map_err(|_| ())?;
            }
            0x04 => {
                let mut skip = [0u8; 18];
                upstream_stream.read_exact(&mut skip).await.map_err(|_| ())?;
            }
            _ => return Err(()),
        }

        if method.eq_ignore_ascii_case("CONNECT") {
            client
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .map_err(|_| ())?;
        } else {
            let rewritten = rewrite_relative_http_request(&req_data, &target);
            upstream_stream.write_all(&rewritten).await.map_err(|_| ())?;
        }
    } else if scheme == "socks4" {
        // SOCKS4 / SOCKS4a (remote DNS resolution via 0.0.0.1 to prevent local DNS leaks!)
        let (thost, tport) = parse_target_host_port(&method, &target).ok_or(())?;
        let user_bytes = upstream.user.as_deref().unwrap_or("").as_bytes();

        let mut pkt = Vec::with_capacity(16 + user_bytes.len() + thost.len());
        pkt.push(0x04);
        pkt.push(0x01);
        pkt.extend_from_slice(&tport.to_be_bytes());

        if let Ok(ip4) = thost.parse::<Ipv4Addr>() {
            pkt.extend_from_slice(&ip4.octets());
            pkt.extend_from_slice(user_bytes);
            pkt.push(0x00);
        } else {
            // SOCKS4a domain extension
            pkt.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
            pkt.extend_from_slice(user_bytes);
            pkt.push(0x00);
            pkt.extend_from_slice(thost.as_bytes());
            pkt.push(0x00);
        }

        upstream_stream.write_all(&pkt).await.map_err(|_| ())?;
        let mut resp = [0u8; 8];
        upstream_stream.read_exact(&mut resp).await.map_err(|_| ())?;
        if resp[1] != 0x5A {
            return Err(());
        }

        if method.eq_ignore_ascii_case("CONNECT") {
            client
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .map_err(|_| ())?;
        } else {
            let rewritten = rewrite_relative_http_request(&req_data, &target);
            upstream_stream.write_all(&rewritten).await.map_err(|_| ())?;
        }
    } else {
        return Err(());
    }

    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream_stream).await;
    Ok(())
}

/// Verifies the proxy end-to-end through our `LocalProxyBridge`, checking IP, Geo, ISP, Type, and Timezone.
/// Uses `https://ipwho.is/` (tests HTTPS CONNECT tunneling!) with fallback to `http://ip-api.com/json/...`.
pub async fn check_proxy(cfg: &ProxyConfig, timeout_secs: u64) -> Result<ProxyCheckInfo, String> {
    // Quick TCP reachability check first for clear error diagnostics
    match timeout(
        Duration::from_secs(timeout_secs),
        TcpStream::connect((cfg.host.as_str(), cfg.port)),
    )
    .await
    {
        Err(_) => {
            return Err("Таймаут подключения к хосту прокси (вероятно, мёртв или недоступен)".into())
        }
        Ok(Err(e)) => return Err(format!("Не удалось подключиться к прокси: {e}")),
        Ok(Ok(sock)) => drop(sock),
    }

    let bridge = LocalProxyBridge::start(cfg.clone()).await?;
    let proxy_url = format!("http://127.0.0.1:{}", bridge.port);

    let req_proxy = reqwest::Proxy::all(&proxy_url).map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .proxy(req_proxy)
        .timeout(Duration::from_secs(timeout_secs + 2))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| e.to_string())?;

    // 1. Primary GeoIP + Timezone via `https://ipwho.is/` (accurate BGP/MaxMind DB matching Pixelscan/Cloudflare + verifies HTTPS CONNECT)
    let mut resolved: Option<ProxyCheckInfo> = None;

    if let Ok(resp) = client.get("https://ipwho.is/").send().await {
        if resp.status().as_u16() == 407 {
            bridge.stop();
            return Err("Неверный логин или пароль прокси (407 Proxy Auth Required)".into());
        }
        if let Ok(json) = resp.json::<Value>().await {
            if json.get("success").and_then(|v| v.as_bool()) != Some(false) {
                if let Some(ip) = json.get("ip").and_then(|v| v.as_str()) {
                    let isp = json
                        .pointer("/connection/isp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let asn_num = json
                        .pointer("/connection/asn")
                        .and_then(|v| v.as_i64())
                        .map(|n| format!("AS{n}"))
                        .unwrap_or_default();
                    let tz = json
                        .pointer("/timezone/id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(normalize_timezone);
                    let lat = json.get("latitude").and_then(|v| v.as_f64());
                    let lon = json.get("longitude").and_then(|v| v.as_f64());

                    resolved = Some(ProxyCheckInfo {
                        ip: ip.to_string(),
                        country: json
                            .get("country")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string(),
                        city: json
                            .get("city")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string(),
                        timezone: tz,
                        isp,
                        asn: asn_num,
                        proxy_type: "Проверен (HTTPS)".to_string(),
                        latitude: lat,
                        longitude: lon,
                    });
                }
            }
        }
    }

    // 2. Second authoritative HTTPS GeoIP source: `https://ipinfo.io/json`
    if resolved.is_none() || resolved.as_ref().is_some_and(|r| r.timezone.is_none()) {
        if let Ok(resp) = client.get("https://ipinfo.io/json").send().await {
            if resp.status().as_u16() == 407 {
                bridge.stop();
                return Err("Неверный логин или пароль прокси (407 Proxy Auth Required)".into());
            }
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(ip) = json.get("ip").and_then(|v| v.as_str()) {
                    let org = json
                        .get("org")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let (asn, isp) = match org.split_once(' ') {
                        Some((a, rest)) if a.starts_with("AS") => (a.to_string(), rest.to_string()),
                        _ => (String::new(), org.to_string()),
                    };
                    let (lat, lon) = json
                        .get("loc")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.split_once(','))
                        .and_then(|(la, lo)| Some((la.parse::<f64>().ok()?, lo.parse::<f64>().ok()?)))
                        .map(|(a, b)| (Some(a), Some(b)))
                        .unwrap_or((None, None));
                    let tz = json
                        .get("timezone")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(normalize_timezone);

                    if let Some(ref mut existing) = resolved {
                        if existing.timezone.is_none() {
                            existing.timezone = tz;
                        }
                        if existing.latitude.is_none() {
                            existing.latitude = lat;
                            existing.longitude = lon;
                        }
                    } else {
                        resolved = Some(ProxyCheckInfo {
                            ip: ip.to_string(),
                            country: json
                                .get("country")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            city: json
                                .get("city")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            timezone: tz,
                            isp,
                            asn,
                            proxy_type: "Проверен (HTTPS)".to_string(),
                            latitude: lat,
                            longitude: lon,
                        });
                    }
                }
            }
        }
    }

    // 3. Enrich proxy type badge (Mobile / Datacenter / Residential) or fallback #3 via ip-api.com
    let ip_api_url = "http://ip-api.com/json/?fields=status,message,country,city,lat,lon,timezone,isp,org,as,mobile,proxy,hosting,query";
    if let Ok(resp) = client.get(ip_api_url).send().await {
        if resp.status().as_u16() == 407 && resolved.is_none() {
            bridge.stop();
            return Err("Неверный логин или пароль прокси (407 Proxy Auth Required)".into());
        }
        if let Ok(json) = resp.json::<Value>().await {
            if json.get("status").and_then(|v| v.as_str()) == Some("success") {
                let mobile = json.get("mobile").and_then(|v| v.as_bool()).unwrap_or(false);
                let hosting = json.get("hosting").and_then(|v| v.as_bool()).unwrap_or(false);
                let p_type = if mobile {
                    "Мобильный"
                } else if hosting {
                    "Датацентр"
                } else {
                    "Резидентский"
                }
                .to_string();
                let tz = json
                    .get("timezone")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(normalize_timezone);

                if let Some(ref mut info) = resolved {
                    // Enrich proxy type badge; only fill timezone/geo if earlier HTTPS sources lacked it
                    info.proxy_type = p_type;
                    if info.timezone.is_none() {
                        info.timezone = tz;
                    }
                    if info.latitude.is_none() {
                        info.latitude = json.get("lat").and_then(|v| v.as_f64());
                        info.longitude = json.get("lon").and_then(|v| v.as_f64());
                    }
                } else {
                    resolved = Some(ProxyCheckInfo {
                        ip: json.get("query").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                        country: json.get("country").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                        city: json.get("city").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                        timezone: tz,
                        isp: json.get("isp").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                        asn: json.get("as").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        proxy_type: p_type,
                        latitude: json.get("lat").and_then(|v| v.as_f64()),
                        longitude: json.get("lon").and_then(|v| v.as_f64()),
                    });
                }
            }
        }
    }

    bridge.stop();
    resolved.ok_or_else(|| "Прокси не пропускает трафик или неверна авторизация.".to_string())
}
