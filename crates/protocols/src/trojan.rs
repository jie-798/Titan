use async_trait::async_trait;
use std::time::Duration;
use tokio::net::TcpStream;

use titan_config::ProxyConfig;

use crate::{BoxedStream, OutboundProxy, Target};

pub struct TrojanProxy {
    name: String,
    server: String,
    port: u16,
}

impl TrojanProxy {
    pub fn from_config(config: &ProxyConfig) -> Self {
        Self {
            name: config.name.clone(),
            server: config.server.clone(),
            port: config.port,
        }
    }
}

#[async_trait]
impl OutboundProxy for TrojanProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, _target: &Target) -> anyhow::Result<BoxedStream> {
        let addr = format!("{}:{}", self.server, self.port);
        let stream = TcpStream::connect(&addr).await?;
        // TODO: 实现协议
        Ok(Box::new(stream))
    }

    async fn delay_test(&self, _url: &str, timeout: Duration) -> anyhow::Result<Duration> {
        let start = std::time::Instant::now();
        let addr = format!("{}:{}", self.server, self.port);
        tokio::time::timeout(timeout, TcpStream::connect(&addr)).await??;
        Ok(start.elapsed())
    }
}
