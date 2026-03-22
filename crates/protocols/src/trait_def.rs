use async_trait::async_trait;
use std::time::Duration;

/// 目标地址
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
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// 箱式流类型
pub type BoxedStream = Box<dyn Stream>;

/// Stream trait
pub trait Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync {}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync> Stream for T {}

/// 统一的出站代理接口
#[async_trait]
pub trait OutboundProxy: Send + Sync {
    /// 获取代理名称
    fn name(&self) -> &str;

    /// 建立TCP连接
    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream>;

    /// 延迟测试
    async fn delay_test(&self, url: &str, timeout: Duration) -> anyhow::Result<Duration>;
}
