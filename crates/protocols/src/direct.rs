use async_trait::async_trait;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

use crate::{BoxedStream, OutboundProxy, Target};

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

#[async_trait]
impl OutboundProxy for DirectProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        let addr = target.addr();
        let stream = TcpStream::connect(&addr).await?;
        Ok(Box::new(stream))
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
