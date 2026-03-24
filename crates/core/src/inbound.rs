use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};
use url::Url;

use crate::engine::ProxyEngine;

pub struct InboundServer {
    bind_addr: SocketAddr,
    engine: Arc<ProxyEngine>,
}

impl InboundServer {
    pub fn new(bind_addr: SocketAddr, engine: Arc<ProxyEngine>) -> Self {
        Self { bind_addr, engine }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("mixed inbound listening on {}", self.bind_addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let engine = self.engine.clone();

            tokio::spawn(async move {
                if let Err(err) = handle_client(stream, peer_addr, engine).await {
                    if is_benign_inbound_error(&err) {
                        debug!("inbound connection {} closed early: {}", peer_addr, err);
                    } else {
                        warn!("failed to handle inbound connection {}: {}", peer_addr, err);
                    }
                }
            });
        }
    }
}

fn is_benign_inbound_error(err: &anyhow::Error) -> bool {
    let message = err.to_string().to_ascii_lowercase();
    message.contains("client closed connection")
        || message.contains("unexpected eof")
        || message.contains("software caused connection abort")
        || message.contains("connection reset by peer")
        || message.contains("os error 10053")
        || message.contains("os error 10054")
}

enum InboundRequest {
    Socks5 {
        target: titan_protocols::Target,
    },
    HttpConnect {
        target: titan_protocols::Target,
    },
    HttpForward {
        target: titan_protocols::Target,
        initial_data: Vec<u8>,
    },
}

impl InboundRequest {
    fn target(&self) -> &titan_protocols::Target {
        match self {
            Self::Socks5 { target }
            | Self::HttpConnect { target }
            | Self::HttpForward { target, .. } => target,
        }
    }
}

async fn handle_client(
    mut client: TcpStream,
    peer_addr: SocketAddr,
    engine: Arc<ProxyEngine>,
) -> anyhow::Result<()> {
    let request = detect_and_parse_request(&mut client).await?;
    let target = request.target().clone();

    let route = engine
        .select_route(&target)
        .await
        .ok_or_else(|| anyhow::anyhow!("no matching outbound policy"))?;
    let proxy_name = route.policy.clone();
    let (resolved_proxy_name, mut outbound) =
        match engine.connect_outbound_with_failover(&proxy_name, &target, 2).await {
            Ok(result) => result,
            Err(err) => {
                respond_connect_failure(&mut client, &request).await?;
                return Err(err);
            }
        };

    match &request {
        InboundRequest::Socks5 { .. } => send_socks5_success(&mut client).await?,
        InboundRequest::HttpConnect { .. } => send_http_connect_success(&mut client).await?,
        InboundRequest::HttpForward { initial_data, .. } => {
            outbound.write_all(initial_data).await?;
        }
    }

    let session = engine.session_manager().create(
        peer_addr.to_string(),
        target.to_string(),
        resolved_proxy_name.clone(),
        Some(route.policy.clone()),
        Some(route.rule.clone()),
    );

    let relay_result = tokio::io::copy_bidirectional(&mut client, &mut outbound).await;
    match relay_result {
        Ok((upload, download)) => {
            engine
                .session_manager()
                .update_traffic(&session.id, upload, download);
            debug!(
                "relayed {} -> {} via {} rule={} (up={} down={})",
                peer_addr, target, resolved_proxy_name, route.rule, upload, download
            );
        }
        Err(err) => {
            engine.session_manager().close(&session.id);
            return Err(err.into());
        }
    }

    engine.session_manager().close(&session.id);
    Ok(())
}

async fn detect_and_parse_request(stream: &mut TcpStream) -> anyhow::Result<InboundRequest> {
    let mut probe = [0_u8; 1];
    let read = stream.peek(&mut probe).await?;
    if read == 0 {
        return Err(anyhow::anyhow!("client closed connection"));
    }

    if probe[0] == 0x05 {
        let target = handshake_socks5(stream).await?;
        return Ok(InboundRequest::Socks5 { target });
    }

    parse_http_proxy_request(stream).await
}

async fn respond_connect_failure(
    stream: &mut TcpStream,
    request: &InboundRequest,
) -> anyhow::Result<()> {
    match request {
        InboundRequest::Socks5 { .. } => {
            let _ = send_socks5_error(stream, 0x01).await;
        }
        InboundRequest::HttpConnect { .. } | InboundRequest::HttpForward { .. } => {
            let _ = send_http_error(stream, 502, "Bad Gateway").await;
        }
    }
    Ok(())
}

async fn handshake_socks5(stream: &mut TcpStream) -> anyhow::Result<titan_protocols::Target> {
    let version = stream.read_u8().await?;
    if version != 0x05 {
        return Err(anyhow::anyhow!("only SOCKS5 is supported on this path"));
    }

    let method_count = stream.read_u8().await? as usize;
    let mut methods = vec![0_u8; method_count];
    stream.read_exact(&mut methods).await?;

    if !methods.contains(&0x00) {
        stream.write_all(&[0x05, 0xFF]).await?;
        return Err(anyhow::anyhow!("client does not support no-auth SOCKS5"));
    }

    stream.write_all(&[0x05, 0x00]).await?;

    let request_version = stream.read_u8().await?;
    if request_version != 0x05 {
        let _ = send_socks5_error(stream, 0x01).await;
        return Err(anyhow::anyhow!("invalid SOCKS5 request version"));
    }

    let command = stream.read_u8().await?;
    let _reserved = stream.read_u8().await?;
    let atyp = stream.read_u8().await?;

    if command != 0x01 {
        let _ = send_socks5_error(stream, 0x07).await;
        return Err(anyhow::anyhow!("only CONNECT is supported"));
    }

    let host = match atyp {
        0x01 => {
            let mut octets = [0_u8; 4];
            stream.read_exact(&mut octets).await?;
            std::net::Ipv4Addr::from(octets).to_string()
        }
        0x03 => {
            let len = stream.read_u8().await? as usize;
            let mut domain = vec![0_u8; len];
            stream.read_exact(&mut domain).await?;
            String::from_utf8(domain)?
        }
        0x04 => {
            let mut octets = [0_u8; 16];
            stream.read_exact(&mut octets).await?;
            std::net::Ipv6Addr::from(octets).to_string()
        }
        _ => {
            let _ = send_socks5_error(stream, 0x08).await;
            return Err(anyhow::anyhow!("unsupported SOCKS5 address type"));
        }
    };

    let port = stream.read_u16().await?;
    Ok(titan_protocols::Target::new(host, port))
}

async fn parse_http_proxy_request(stream: &mut TcpStream) -> anyhow::Result<InboundRequest> {
    let raw_request = read_http_request(stream).await?;
    let header_end = find_header_end(&raw_request)
        .ok_or_else(|| anyhow::anyhow!("invalid HTTP proxy request"))?;
    let header_text = String::from_utf8_lossy(&raw_request[..header_end]);
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP method"))?;
    let uri = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP uri"))?;
    let version = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP version"))?;

    let headers: Vec<String> = lines.map(|line| line.to_string()).collect();

    if method.eq_ignore_ascii_case("CONNECT") {
        let target = parse_connect_target(uri)?;
        return Ok(InboundRequest::HttpConnect { target });
    }

    let (target, initial_data) = rewrite_http_forward_request(
        method,
        uri,
        version,
        &headers,
        &raw_request[header_end + 4..],
    )?;
    Ok(InboundRequest::HttpForward {
        target,
        initial_data,
    })
}

async fn read_http_request(stream: &mut TcpStream) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0_u8; 1024];

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Err(anyhow::anyhow!("unexpected EOF while reading HTTP request"));
        }
        buffer.extend_from_slice(&chunk[..read]);

        if find_header_end(&buffer).is_some() {
            return Ok(buffer);
        }

        if buffer.len() > 64 * 1024 {
            return Err(anyhow::anyhow!("HTTP request header is too large"));
        }
    }
}

fn rewrite_http_forward_request(
    method: &str,
    uri: &str,
    version: &str,
    headers: &[String],
    remaining_body: &[u8],
) -> anyhow::Result<(titan_protocols::Target, Vec<u8>)> {
    let (target, path, has_host_header) = if let Ok(url) = Url::parse(uri) {
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("absolute HTTP uri is missing host"))?;
        let port = url.port_or_known_default().unwrap_or(80);
        (
            titan_protocols::Target::new(host.to_string(), port),
            origin_form(&url),
            headers
                .iter()
                .any(|line| line.to_ascii_lowercase().starts_with("host:")),
        )
    } else {
        let host_header = headers
            .iter()
            .find_map(|line| line.split_once(':'))
            .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim().to_string())
            .ok_or_else(|| anyhow::anyhow!("HTTP proxy request is missing Host header"))?;
        let target = parse_host_header(&host_header)?;
        (target, uri.to_string(), true)
    };

    let mut rewritten = Vec::new();
    rewritten.extend_from_slice(format!("{} {} {}\r\n", method, path, version).as_bytes());

    if !has_host_header {
        rewritten.extend_from_slice(format!("Host: {}\r\n", target.addr()).as_bytes());
    }

    for header in headers {
        if header.is_empty() {
            continue;
        }

        let lower = header.to_ascii_lowercase();
        if lower.starts_with("proxy-connection:") || lower.starts_with("proxy-authorization:") {
            continue;
        }

        rewritten.extend_from_slice(header.as_bytes());
        rewritten.extend_from_slice(b"\r\n");
    }

    rewritten.extend_from_slice(b"\r\n");
    rewritten.extend_from_slice(remaining_body);

    Ok((target, rewritten))
}

fn parse_connect_target(authority: &str) -> anyhow::Result<titan_protocols::Target> {
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("CONNECT target must be host:port"))?;
    let port = port.parse::<u16>()?;
    Ok(titan_protocols::Target::new(host.to_string(), port))
}

fn parse_host_header(value: &str) -> anyhow::Result<titan_protocols::Target> {
    if let Some((host, port)) = value.rsplit_once(':') {
        if let Ok(port) = port.parse::<u16>() {
            return Ok(titan_protocols::Target::new(host.to_string(), port));
        }
    }

    Ok(titan_protocols::Target::new(value.to_string(), 80))
}

fn origin_form(url: &Url) -> String {
    let mut path = url.path().to_string();
    if path.is_empty() {
        path.push('/');
    }

    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }

    path
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

async fn send_socks5_success(stream: &mut TcpStream) -> io::Result<()> {
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
}

async fn send_socks5_error(stream: &mut TcpStream, code: u8) -> io::Result<()> {
    stream
        .write_all(&[0x05, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
}

async fn send_http_connect_success(stream: &mut TcpStream) -> io::Result<()> {
    stream
        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
        .await
}

async fn send_http_error(stream: &mut TcpStream, status: u16, reason: &str) -> io::Result<()> {
    let body = format!("{} {}\n", status, reason);
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        reason,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await
}
