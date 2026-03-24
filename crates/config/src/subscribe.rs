use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{Config, ProxyConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub fetched_at_unix: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expire: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_web_page: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_disposition: Option<String>,
}

impl SubscriptionMetadata {
    pub fn used(&self) -> Option<u64> {
        Some(self.upload.unwrap_or(0).saturating_add(self.download.unwrap_or(0)))
    }

    pub fn usage_percent(&self) -> Option<f64> {
        match (self.used(), self.total) {
            (_, Some(0)) => Some(0.0),
            (Some(used), Some(total)) => Some((used as f64 / total as f64) * 100.0),
            _ => None,
        }
    }

    pub fn remaining(&self) -> Option<u64> {
        match (self.total, self.used()) {
            (Some(total), Some(used)) => Some(total.saturating_sub(used)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubscriptionFetchResult {
    pub content: String,
    pub metadata: SubscriptionMetadata,
}

#[derive(Debug, Clone)]
pub struct SubscriptionRefreshResult {
    pub metadata: SubscriptionMetadata,
    pub source: String,
}

pub async fn fetch_subscribe(url: &str) -> anyhow::Result<String> {
    Ok(fetch_subscribe_details(url).await?.content)
}

pub async fn refresh_subscription_from_config(
    config_path: impl AsRef<Path>,
) -> anyhow::Result<Option<SubscriptionRefreshResult>> {
    let config_path = config_path.as_ref();
    let config = crate::load_config(config_path)?;
    let Some(subscription) = config.subscription.clone().filter(|sub| sub.enabled) else {
        return Ok(None);
    };

    let result = fetch_subscribe_details(&subscription.url).await?;
    let mut updated = parse_subscribe(&result.content)?;
    updated.subscription = Some(subscription);
    crate::save_config(&updated, config_path)?;
    save_subscription_metadata(&result.metadata, subscription_metadata_path(config_path))?;

    Ok(Some(SubscriptionRefreshResult {
        metadata: result.metadata,
        source: updated
            .subscription
            .as_ref()
            .map(|sub| sub.url.clone())
            .unwrap_or_default(),
    }))
}

pub async fn fetch_subscribe_details(url: &str) -> anyhow::Result<SubscriptionFetchResult> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("clash-verge/v2.0.0")
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("subscription request failed: {}", response.status());
    }

    let metadata = SubscriptionMetadata {
        source: Some(url.to_string()),
        fetched_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        upload: header_stat(response.headers(), "subscription-userinfo", "upload"),
        download: header_stat(response.headers(), "subscription-userinfo", "download"),
        total: header_stat(response.headers(), "subscription-userinfo", "total"),
        expire: header_stat(response.headers(), "subscription-userinfo", "expire"),
        profile_web_page: header_string(response.headers(), "profile-web-page"),
        content_disposition: header_string(response.headers(), "content-disposition"),
    };
    let content = response.text().await?;

    Ok(SubscriptionFetchResult { content, metadata })
}

pub fn parse_subscribe(content: &str) -> anyhow::Result<Config> {
    if let Ok(config) = serde_yaml::from_str::<Config>(content) {
        info!("parsed Clash YAML subscription");
        return Ok(config);
    }

    let content = content.trim().replace('\n', "").replace('\r', "");
    if let Ok(decoded) = general_purpose::STANDARD
        .decode(&content)
        .or_else(|_| general_purpose::URL_SAFE.decode(&content))
    {
        if let Ok(decoded_str) = String::from_utf8(decoded) {
            let proxies = parse_proxy_links(&decoded_str)?;
            if !proxies.is_empty() {
                info!("parsed Base64 subscription with {} proxies", proxies.len());
                return Ok(Config {
                    proxies,
                    ..Default::default()
                });
            }
        }
    }

    let proxies = parse_proxy_links(&content)?;
    if !proxies.is_empty() {
        info!("parsed proxy link subscription with {} proxies", proxies.len());
        return Ok(Config {
            proxies,
            ..Default::default()
        });
    }

    anyhow::bail!("unable to parse subscription content")
}

pub fn parse_proxy_links(content: &str) -> anyhow::Result<Vec<ProxyConfig>> {
    let mut proxies = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(proxy) = parse_proxy_link(line) {
            proxies.push(proxy);
        }
    }

    Ok(proxies)
}

pub fn parse_proxy_link(link: &str) -> Option<ProxyConfig> {
    if link.starts_with("vmess://") {
        parse_vmess(link)
    } else if link.starts_with("vless://") {
        parse_vless(link)
    } else if link.starts_with("trojan://") {
        parse_trojan(link)
    } else if link.starts_with("ss://") {
        parse_shadowsocks(link)
    } else if link.starts_with("hysteria2://") || link.starts_with("hy2://") {
        parse_hysteria2(link)
    } else if link.starts_with("tuic://") {
        parse_tuic(link)
    } else if link.starts_with("anytls://") {
        parse_anytls(link)
    } else {
        None
    }
}

fn parse_vmess(link: &str) -> Option<ProxyConfig> {
    let b64 = link.strip_prefix("vmess://")?;
    let decoded = general_purpose::STANDARD.decode(b64).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    Some(ProxyConfig {
        name: json.get("ps").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        proxy_type: "vmess".to_string(),
        server: json.get("add").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        port: json.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        uuid: json.get("id").and_then(|v| v.as_str()).map(String::from),
        cipher: json.get("scy").and_then(|v| v.as_str()).map(String::from),
        network: json.get("net").and_then(|v| v.as_str()).map(String::from),
        tls: json
            .get("tls")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s == "tls"),
        ..Default::default()
    })
}

fn parse_vless(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("vless://")?;
    let url = url::Url::parse(&format!("vless://{url_str}")).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let uuid = url.username().to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(443);
    let params: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "vless".to_string(),
        server,
        port,
        uuid: Some(uuid),
        network: params.get("type").cloned(),
        tls: params
            .get("security")
            .is_some_and(|s| s == "tls" || s == "reality"),
        sni: params.get("sni").cloned(),
        flow: params.get("flow").cloned(),
        ..Default::default()
    })
}

fn parse_trojan(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("trojan://")?;
    let url = url::Url::parse(&format!("trojan://{url_str}")).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let password = url.username().to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(443);

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "trojan".to_string(),
        server,
        port,
        password: Some(password),
        tls: true,
        ..Default::default()
    })
}

fn parse_shadowsocks(link: &str) -> Option<ProxyConfig> {
    let content = link.strip_prefix("ss://")?;
    if content.contains('@') {
        let url = url::Url::parse(link).ok()?;
        let name = url.fragment().unwrap_or("").to_string();
        let server = url.host_str().unwrap_or("").to_string();
        let port = url.port().unwrap_or(8388);

        let userinfo = general_purpose::URL_SAFE_NO_PAD.decode(url.username()).ok()?;
        let userinfo_str = String::from_utf8(userinfo).ok()?;
        let parts: Vec<&str> = userinfo_str.split(':').collect();

        if parts.len() == 2 {
            return Some(ProxyConfig {
                name: urlencoding::decode(&name).ok()?.to_string(),
                proxy_type: "ss".to_string(),
                server,
                port,
                cipher: Some(parts[0].to_string()),
                password: Some(parts[1].to_string()),
                ..Default::default()
            });
        }
    }
    None
}

fn parse_hysteria2(link: &str) -> Option<ProxyConfig> {
    let url_str = link
        .strip_prefix("hysteria2://")
        .or_else(|| link.strip_prefix("hy2://"))?;
    let url = url::Url::parse(&format!("hy2://{url_str}")).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let password = url.username().to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(443);

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "hysteria2".to_string(),
        server,
        port,
        password: Some(password),
        ..Default::default()
    })
}

fn parse_tuic(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("tuic://")?;
    let url = url::Url::parse(&format!("tuic://{url_str}")).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(443);
    let userinfo = url.username();
    let parts: Vec<&str> = userinfo.split(':').collect();

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "tuic".to_string(),
        server,
        port,
        uuid: parts.first().map(|s| (*s).to_string()),
        password: parts.get(1).map(|s| (*s).to_string()),
        ..Default::default()
    })
}

fn parse_anytls(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("anytls://")?;
    let url = url::Url::parse(&format!("anytls://{url_str}")).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let password = url.username().to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(8443);

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "anytls".to_string(),
        server,
        port,
        password: Some(password),
        ..Default::default()
    })
}

pub fn subscription_metadata_path(config_path: impl AsRef<Path>) -> PathBuf {
    let config_path = config_path.as_ref();
    match (config_path.parent(), config_path.file_stem()) {
        (Some(parent), Some(stem)) => {
            parent.join(format!("{}.subscription.yaml", stem.to_string_lossy()))
        }
        _ => PathBuf::from("subscription.subscription.yaml"),
    }
}

pub fn save_subscription_metadata(
    metadata: &SubscriptionMetadata,
    path: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_yaml::to_string(metadata)?)?;
    Ok(())
}

pub fn load_subscription_metadata(path: impl AsRef<Path>) -> anyhow::Result<SubscriptionMetadata> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

fn header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn header_stat(headers: &reqwest::header::HeaderMap, header_name: &str, key: &str) -> Option<u64> {
    let raw = header_string(headers, header_name)?;
    raw.split(';').map(str::trim).find_map(|part| {
        let (name, value) = part.split_once('=')?;
        if name.trim().eq_ignore_ascii_case(key) {
            value.trim().parse::<u64>().ok()
        } else {
            None
        }
    })
}
