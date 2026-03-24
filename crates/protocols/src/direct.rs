use async_trait::async_trait;
use std::time::{Duration, Instant};
use tokio::net::{lookup_host, TcpStream, UdpSocket};

use crate::{BoxedDatagram, BoxedStream, Datagram, OutboundProxy, Target};

/// 直连代理
pub struct DirectProxy {
    name: String,
}

impl DirectProxy {
    pub fn new() -> Self {
        Self {
            name: "DIRECT".to_string(),
        }
    }
}

struct DirectDatagram {
    socket: UdpSocket,
}

#[async_trait]
impl Datagram for DirectDatagram {
    async fn send(&mut self, data: &[u8]) -> anyhow::Result<usize> {
        Ok(self.socket.send(data).await?)
    }

    async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        Ok(self.socket.recv(buf).await?)
    }
}

#[async_trait]
impl OutboundProxy for DirectProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        let mut addrs = lookup_host((target.host.as_str(), target.port)).await?;
        let addr = addrs
            .next()
            .ok_or_else(|| anyhow::anyhow!("failed to resolve direct target {}", target))?;
        let stream = TcpStream::connect(addr).await?;
        Ok(Box::new(stream))
    }

    fn supports_udp(&self) -> bool {
        true
    }

    async fn connect_udp(&self, target: &Target) -> anyhow::Result<BoxedDatagram> {
        let mut addrs = lookup_host((target.host.as_str(), target.port)).await?;
        let target_addr = addrs
            .next()
            .ok_or_else(|| anyhow::anyhow!("failed to resolve direct target {}", target))?;
        let bind_addr = match target_addr {
            std::net::SocketAddr::V4(_) => "0.0.0.0:0",
            std::net::SocketAddr::V6(_) => "[::]:0",
        };
        let socket = UdpSocket::bind(bind_addr).await?;
        socket.connect(target_addr).await?;
        Ok(Box::new(DirectDatagram { socket }))
    }

    async fn delay_test(&self, _url: &str, timeout: Duration) -> anyhow::Result<Duration> {
        let start = Instant::now();

        let client = reqwest::Client::builder().timeout(timeout).build()?;

        client
            .get("http://www.gstatic.com/generate_204")
            .send()
            .await?;

        Ok(start.elapsed())
    }
}
