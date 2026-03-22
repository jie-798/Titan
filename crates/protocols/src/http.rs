use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

use titan_config::ProxyConfig;

use crate::{BoxedStream, OutboundProxy, Target};

pub struct HttpProxy {
    name: String,
    server: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
}

impl HttpProxy {
    pub fn from_config(config: &ProxyConfig) -> Self {
        Self {
            name: config.name.clone(),
            server: config.server.clone(),
            port: config.port,
            username: config
                .extra
                .get("username")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            password: config.password.clone().or_else(|| {
                config
                    .extra
                    .get("password")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string())
            }),
        }
    }

    fn proxy_authorization_header(&self) -> Option<String> {
        let username = self.username.as_ref()?;
        let credentials = format!("{}:{}", username, self.password.as_deref().unwrap_or(""));
        Some(format!(
            "Proxy-Authorization: Basic {}\r\n",
            BASE64_STANDARD.encode(credentials)
        ))
    }
}

#[async_trait]
impl OutboundProxy for HttpProxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        let addr = format!("{}:{}", self.server, self.port);
        let mut stream = TcpStream::connect(&addr).await?;

        let mut request = format!(
            "CONNECT {} HTTP/1.1\r\nHost: {}\r\nProxy-Connection: Keep-Alive\r\nUser-Agent: Titan/0.1\r\n",
            target.addr(),
            target.addr()
        );
        if let Some(header) = self.proxy_authorization_header() {
            request.push_str(&header);
        }
        request.push_str("\r\n");

        stream.write_all(request.as_bytes()).await?;

        let mut response = Vec::with_capacity(1024);
        let mut buf = [0_u8; 512];
        loop {
            let read = stream.read(&mut buf).await?;
            if read == 0 {
                return Err(anyhow::anyhow!(
                    "HTTP proxy closed the connection before replying to CONNECT"
                ));
            }
            response.extend_from_slice(&buf[..read]);

            if response.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }

            if response.len() > 16 * 1024 {
                return Err(anyhow::anyhow!("HTTP proxy response header is too large"));
            }
        }

        let response_text = String::from_utf8_lossy(&response);
        let status_line = response_text.lines().next().unwrap_or_default();
        if !status_line.starts_with("HTTP/1.1 200") && !status_line.starts_with("HTTP/1.0 200") {
            return Err(anyhow::anyhow!(
                "HTTP proxy CONNECT failed: {}",
                status_line
            ));
        }

        Ok(Box::new(stream))
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
