use async_trait::async_trait;
use std::time::Duration;

use crate::{BoxedStream, OutboundProxy, Target};

/// 拒绝代理
pub struct RejectProxy {
    name: String,
}

impl RejectProxy {
    pub fn new() -> Self {
        Self {
            name: "REJECT".to_string(),
        }
    }
}

#[async_trait]
impl OutboundProxy for RejectProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, _target: &Target) -> anyhow::Result<BoxedStream> {
        Err(anyhow::anyhow!("连接被拒绝"))
    }

    async fn delay_test(&self, _url: &str, _timeout: Duration) -> anyhow::Result<Duration> {
        Err(anyhow::anyhow!("连接被拒绝"))
    }
}
