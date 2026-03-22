use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use url::Url;

use titan_config::ProxyConfig;

use crate::{BoxedStream, OutboundProxy, Target};

pub struct VlessProxy {
    name: String,
    server: String,
    port: u16,
    uuid: String,
    tls: bool,
    sni: Option<String>,
    skip_cert_verify: bool,
    alpn: Vec<Vec<u8>>,
    flow: Option<String>,
    network: Option<String>,
}

impl VlessProxy {
    pub fn from_config(config: &ProxyConfig) -> Self {
        Self {
            name: config.name.clone(),
            server: config.server.clone(),
            port: config.port,
            uuid: config.uuid.clone().unwrap_or_default(),
            tls: config.tls,
            sni: config.sni.clone(),
            skip_cert_verify: config.skip_cert_verify,
            alpn: config
                .alpn
                .clone()
                .unwrap_or_else(|| vec!["h2".to_string(), "http/1.1".to_string()])
                .into_iter()
                .map(|value| value.into_bytes())
                .collect(),
            flow: config.flow.clone(),
            network: config.network.clone(),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if self.uuid.is_empty() {
            anyhow::bail!("VLESS proxy {} is missing uuid", self.name);
        }

        if let Some(network) = &self.network {
            if !network.is_empty() && !network.eq_ignore_ascii_case("tcp") {
                anyhow::bail!(
                    "VLESS proxy {} only supports tcp network right now, got {}",
                    self.name,
                    network
                );
            }
        }

        if let Some(flow) = &self.flow {
            anyhow::bail!(
                "VLESS proxy {} does not support flow {} yet",
                self.name,
                flow
            );
        }

        Ok(())
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

    async fn connect_transport(&self) -> anyhow::Result<WrappedVlessTransport> {
        let addr = format!("{}:{}", self.server, self.port);
        let stream = TcpStream::connect(&addr).await?;

        if !self.tls {
            return Ok(WrappedVlessTransport::Plain(stream));
        }

        let server_name_str = self.sni.as_deref().unwrap_or(&self.server).to_owned();
        let server_name = rustls::pki_types::ServerName::try_from(server_name_str.as_str())
            .map_err(|_| anyhow::anyhow!("invalid VLESS server name: {}", server_name_str))?
            .to_owned();
        let connector = self.create_tls_connector();
        let stream = connector.connect(server_name, stream).await?;
        Ok(WrappedVlessTransport::Tls(stream))
    }

    async fn write_request(
        &self,
        transport: &mut WrappedVlessTransport,
        target: &Target,
    ) -> anyhow::Result<()> {
        let uuid = uuid::Uuid::parse_str(&self.uuid)?;
        let mut request = Vec::with_capacity(64 + target.host.len());

        request.push(0x00);
        request.extend_from_slice(uuid.as_bytes());
        request.push(0x00);
        request.push(0x01);
        request.extend_from_slice(&target.port.to_be_bytes());

        if let Ok(ipv4) = target.host.parse::<std::net::Ipv4Addr>() {
            request.push(0x01);
            request.extend_from_slice(&ipv4.octets());
        } else if let Ok(ipv6) = target.host.parse::<std::net::Ipv6Addr>() {
            request.push(0x03);
            request.extend_from_slice(&ipv6.octets());
        } else {
            if target.host.len() > u8::MAX as usize {
                anyhow::bail!("VLESS target hostname is too long");
            }
            request.push(0x02);
            request.push(target.host.len() as u8);
            request.extend_from_slice(target.host.as_bytes());
        }

        transport.write_all(&request).await?;
        transport.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl OutboundProxy for VlessProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        self.validate()?;

        let mut transport = self.connect_transport().await?;
        self.write_request(&mut transport, target).await?;
        Ok(Box::new(VlessResponseStream::new(transport)))
    }

    async fn delay_test(&self, url: &str, timeout: Duration) -> anyhow::Result<Duration> {
        let parsed = Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a host"))?;
        let port = parsed
            .port_or_known_default()
            .ok_or_else(|| anyhow::anyhow!("test URL is missing a port"))?;
        let target = Target::new(host.to_string(), port);

        let start = Instant::now();
        tokio::time::timeout(timeout, self.connect_tcp(&target)).await??;
        Ok(start.elapsed())
    }
}

enum WrappedVlessTransport {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl AsyncRead for WrappedVlessTransport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for WrappedVlessTransport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

struct VlessResponseStream<T> {
    inner: T,
    response_pending: bool,
    response_buffer: Vec<u8>,
}

impl<T> VlessResponseStream<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            response_pending: true,
            response_buffer: Vec::new(),
        }
    }
}

impl<T> AsyncRead for VlessResponseStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.response_pending {
            while self.response_buffer.len() < 2 {
                let mut chunk = [0_u8; 2];
                let mut read_buf = ReadBuf::new(&mut chunk);
                match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        let filled = read_buf.filled();
                        if filled.is_empty() {
                            return Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "connection closed while reading VLESS response",
                            )));
                        }
                        self.response_buffer.extend_from_slice(filled);
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                }
            }

            let version = self.response_buffer[0];
            if version != 0 {
                return Poll::Ready(Err(std::io::Error::other(format!(
                    "invalid VLESS response version: {}",
                    version
                ))));
            }

            let addon_length = self.response_buffer[1] as usize;
            let total_len = 2 + addon_length;
            while self.response_buffer.len() < total_len {
                let remaining = total_len - self.response_buffer.len();
                let mut extra = vec![0_u8; remaining];
                let mut read_buf = ReadBuf::new(&mut extra);
                match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
                    Poll::Ready(Ok(())) => {
                        let filled = read_buf.filled();
                        if filled.is_empty() {
                            return Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "connection closed while reading VLESS response addons",
                            )));
                        }
                        self.response_buffer.extend_from_slice(filled);
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => return Poll::Pending,
                }
            }

            self.response_pending = false;
        }

        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<T> AsyncWrite for VlessResponseStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
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
