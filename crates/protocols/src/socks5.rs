use async_trait::async_trait;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use url::Url;

use titan_config::ProxyConfig;

use crate::{BoxedStream, OutboundProxy, Target};

pub struct Socks5Proxy {
    name: String,
    server: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
}

impl Socks5Proxy {
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

    async fn authenticate(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        let mut methods = vec![0x00];
        if self.username.is_some() {
            methods.push(0x02);
        }

        let mut greeting = vec![0x05, methods.len() as u8];
        greeting.extend_from_slice(&methods);
        stream.write_all(&greeting).await?;

        let version = stream.read_u8().await?;
        let method = stream.read_u8().await?;
        if version != 0x05 {
            return Err(anyhow::anyhow!("invalid SOCKS5 proxy version: {}", version));
        }

        match method {
            0x00 => Ok(()),
            0x02 => {
                let username = self.username.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("SOCKS5 proxy requires username/password auth")
                })?;
                let password = self.password.as_deref().unwrap_or("");
                if username.len() > u8::MAX as usize || password.len() > u8::MAX as usize {
                    return Err(anyhow::anyhow!("SOCKS5 credentials are too long"));
                }

                let mut auth_request = Vec::with_capacity(3 + username.len() + password.len());
                auth_request.push(0x01);
                auth_request.push(username.len() as u8);
                auth_request.extend_from_slice(username.as_bytes());
                auth_request.push(password.len() as u8);
                auth_request.extend_from_slice(password.as_bytes());
                stream.write_all(&auth_request).await?;

                let auth_version = stream.read_u8().await?;
                let status = stream.read_u8().await?;
                if auth_version != 0x01 || status != 0x00 {
                    return Err(anyhow::anyhow!("SOCKS5 username/password auth failed"));
                }

                Ok(())
            }
            0xFF => Err(anyhow::anyhow!("SOCKS5 proxy rejected all auth methods")),
            other => Err(anyhow::anyhow!(
                "SOCKS5 proxy selected unsupported auth method: {}",
                other
            )),
        }
    }

    async fn send_connect_request(
        &self,
        stream: &mut TcpStream,
        target: &Target,
    ) -> anyhow::Result<()> {
        let mut request = vec![0x05, 0x01, 0x00];

        if let Ok(ipv4) = target.host.parse::<std::net::Ipv4Addr>() {
            request.push(0x01);
            request.extend_from_slice(&ipv4.octets());
        } else if let Ok(ipv6) = target.host.parse::<std::net::Ipv6Addr>() {
            request.push(0x04);
            request.extend_from_slice(&ipv6.octets());
        } else {
            if target.host.len() > u8::MAX as usize {
                return Err(anyhow::anyhow!("SOCKS5 target hostname is too long"));
            }

            request.push(0x03);
            request.push(target.host.len() as u8);
            request.extend_from_slice(target.host.as_bytes());
        }

        request.extend_from_slice(&target.port.to_be_bytes());
        stream.write_all(&request).await?;

        let version = stream.read_u8().await?;
        let status = stream.read_u8().await?;
        let _reserved = stream.read_u8().await?;
        let atyp = stream.read_u8().await?;
        if version != 0x05 {
            return Err(anyhow::anyhow!(
                "invalid SOCKS5 response version: {}",
                version
            ));
        }
        if status != 0x00 {
            return Err(anyhow::anyhow!(
                "SOCKS5 CONNECT failed with status {}",
                status
            ));
        }

        match atyp {
            0x01 => {
                let mut buf = [0_u8; 4];
                stream.read_exact(&mut buf).await?;
            }
            0x03 => {
                let len = stream.read_u8().await? as usize;
                let mut buf = vec![0_u8; len];
                stream.read_exact(&mut buf).await?;
            }
            0x04 => {
                let mut buf = [0_u8; 16];
                stream.read_exact(&mut buf).await?;
            }
            other => {
                return Err(anyhow::anyhow!(
                    "SOCKS5 proxy returned unsupported address type {}",
                    other
                ));
            }
        }

        let _bound_port = stream.read_u16().await?;
        Ok(())
    }
}

#[async_trait]
impl OutboundProxy for Socks5Proxy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn connect_tcp(&self, target: &Target) -> anyhow::Result<BoxedStream> {
        let addr = format!("{}:{}", self.server, self.port);
        let mut stream = TcpStream::connect(&addr).await?;
        self.authenticate(&mut stream).await?;
        self.send_connect_request(&mut stream, target).await?;
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
