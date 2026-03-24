use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::UdpSocket;
use tracing::debug;
use url::Url;

use titan_config::ProxyConfig;

use crate::{BoxedDatagram, BoxedStream, Datagram, OutboundProxy, Target};

pub struct AnyTlsProxy {
    name: String,
    server: String,
    port: u16,
    password: String,
    sni: Option<String>,
    alpn: Vec<Vec<u8>>,
    skip_cert_verify: bool,
    udp: bool,
}

impl AnyTlsProxy {
    pub fn from_config(config: &ProxyConfig) -> Self {
        Self {
            name: config.name.clone(),
            server: config.server.clone(),
            port: config.port,
            password: config.password.clone().unwrap_or_default(),
            sni: config.sni.clone(),
            alpn: config
                .alpn
                .clone()
                .unwrap_or_else(|| vec!["h2".to_string(), "http/1.1".to_string()])
                .into_iter()
                .map(|value| value.into_bytes())
                .collect(),
            skip_cert_verify: config.skip_cert_verify,
            udp: config.udp,
        }
    }

    fn create_tls_connector(&self) -> tokio_rustls::TlsConnector {
        let mut config = if self.skip_cert_verify {
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(SkipCertVerifier))
                .with_no_client_auth()
        } else {
            let mut root_store = rustls::RootCertStore::empty();
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth()
        };
        config.alpn_protocols = self.alpn.clone();
        tokio_rustls::TlsConnector::from(Arc::new(config))
    }

    fn create_client(&self) -> anyhow::Result<anytls_rs::Client> {
        let server_addr = format!("{}:{}", self.server, self.port);
        let server_name_str = self.sni.as_deref().unwrap_or(&self.server).to_owned();
        let server_name = rustls::pki_types::ServerName::try_from(server_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("invalid server name: {}", server_name_str))?
            .to_owned();
        let tls_connector = self.create_tls_connector();
        let padding = anytls_rs::PaddingFactory::new(anytls_rs::DEFAULT_PADDING_SCHEME.as_bytes())
            .map_err(|err| anyhow::anyhow!("failed to create AnyTLS padding: {}", err))?;
        Ok(anytls_rs::Client::new(
            &self.password,
            server_addr,
            server_name,
            Arc::new(tls_connector),
            Arc::new(padding),
        ))
    }

    async fn probe_http_url(&self, url: &str) -> anyhow::Result<()> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a host"))?;
        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a port"))?;

        if !parsed.scheme().eq_ignore_ascii_case("http") {
            anyhow::bail!("AnyTLS delay probe currently only supports http URLs");
        }

        let target = Target::new(host.to_string(), port);
        let mut stream = self.connect_tcp(&target).await?;
        let path = match parsed.query() {
            Some(query) => format!("{}?{}", parsed.path(), query),
            None => parsed.path().to_string(),
        };
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: Titan/0.1\r\n\r\n",
            if path.is_empty() { "/" } else { &path },
            host
        );

        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;
        read_http_success_response(&mut stream).await
    }
}

#[async_trait]
impl OutboundProxy for AnyTlsProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        debug!("AnyTLS connecting: {} -> {}", self.name, target);
        let client = self.create_client()?;
        let (stream, _session) = client
            .create_proxy_stream((target.host.clone(), target.port))
            .await?;
        debug!("AnyTLS connected: {} -> {}", self.name, target);

        let (read_half, write_half) = tokio::io::split(AnyTlsStreamArc(stream));
        Ok(Box::new(SplitStream {
            read_half,
            write_half,
        }))
    }

    fn supports_udp(&self) -> bool {
        self.udp
    }

    async fn connect_udp(&self, target: &Target) -> anyhow::Result<BoxedDatagram> {
        let client = self.create_client()?;
        let target_addr = target.socket_addr()?;
        let local_bound = client
            .create_udp_proxy("127.0.0.1:0", target_addr)
            .await
            .map_err(|err| anyhow::anyhow!("failed to create AnyTLS UDP proxy: {}", err))?;
        let socket = UdpSocket::bind("127.0.0.1:0").await?;
        socket.connect(local_bound).await?;
        Ok(Box::new(AnyTlsDatagram { socket }))
    }

    async fn delay_test(&self, url: &str, timeout: Duration) -> anyhow::Result<Duration> {
        let start = Instant::now();
        tokio::time::timeout(timeout, self.probe_http_url(url)).await??;
        Ok(start.elapsed())
    }
}

pub(crate) async fn read_http_success_response<S>(stream: &mut S) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut response = Vec::with_capacity(1024);
    let mut buf = [0_u8; 512];

    loop {
        let read = stream.read(&mut buf).await?;
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buf[..read]);

        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }

        if response.len() > 16 * 1024 {
            anyhow::bail!("HTTP probe response header is too large");
        }
    }

    let response_text = String::from_utf8_lossy(&response);
    let status_line = response_text.lines().next().unwrap_or_default();
    if status_line.starts_with("HTTP/1.1 2") || status_line.starts_with("HTTP/1.0 2") {
        return Ok(());
    }

    anyhow::bail!("HTTP probe failed: {}", status_line)
}

struct AnyTlsStreamArc(Arc<anytls_rs::Stream>);

impl AsyncRead for AnyTlsStreamArc {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = unsafe { &mut *(Arc::as_ptr(&self.0) as *mut anytls_rs::Stream) };
        std::pin::Pin::new(stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for AnyTlsStreamArc {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        let stream = unsafe { &mut *(Arc::as_ptr(&self.0) as *mut anytls_rs::Stream) };
        std::pin::Pin::new(stream).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = unsafe { &mut *(Arc::as_ptr(&self.0) as *mut anytls_rs::Stream) };
        std::pin::Pin::new(stream).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let stream = unsafe { &mut *(Arc::as_ptr(&self.0) as *mut anytls_rs::Stream) };
        std::pin::Pin::new(stream).poll_shutdown(cx)
    }
}

struct SplitStream {
    read_half: tokio::io::ReadHalf<AnyTlsStreamArc>,
    write_half: tokio::io::WriteHalf<AnyTlsStreamArc>,
}

impl AsyncRead for SplitStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.read_half).poll_read(cx, buf)
    }
}

struct AnyTlsDatagram {
    socket: UdpSocket,
}

#[async_trait]
impl Datagram for AnyTlsDatagram {
    async fn send(&mut self, data: &[u8]) -> anyhow::Result<usize> {
        Ok(self.socket.send(data).await?)
    }

    async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        Ok(self.socket.recv(buf).await?)
    }
}

impl AsyncWrite for SplitStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.write_half).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.write_half).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.write_half).poll_shutdown(cx)
    }
}

#[derive(Debug)]
struct SkipCertVerifier;

impl rustls::client::danger::ServerCertVerifier for SkipCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}
