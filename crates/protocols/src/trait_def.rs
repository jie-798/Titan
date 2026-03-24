use async_trait::async_trait;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
}

impl Target {
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn socket_addr(&self) -> anyhow::Result<SocketAddr> {
        let ip = self
            .host
            .parse::<IpAddr>()
            .map_err(|_| anyhow::anyhow!("target host is not an IP address: {}", self.host))?;
        Ok(SocketAddr::new(ip, self.port))
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

pub type BoxedStream = Box<dyn Stream>;
pub type BoxedDatagram = Box<dyn Datagram>;

pub trait Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync {}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync> Stream for T {}

#[async_trait]
pub trait Datagram: Unpin + Send + Sync {
    async fn send(&mut self, data: &[u8]) -> anyhow::Result<usize>;
    async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize>;
}

#[async_trait]
pub trait OutboundProxy: Send + Sync {
    fn name(&self) -> &str;

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream>;

    fn supports_udp(&self) -> bool {
        false
    }

    async fn connect_udp(&self, _target: &Target) -> anyhow::Result<BoxedDatagram> {
        anyhow::bail!("UDP is not supported by this outbound")
    }

    async fn delay_test(&self, url: &str, timeout: Duration) -> anyhow::Result<Duration>;
}
