use base64::{engine::general_purpose, Engine as _};
use tracing::info;

use crate::{Config, ProxyConfig};

/// 获取订阅内容
pub async fn fetch_subscribe(url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("clash-verge/v2.0.0")
        .build()?;

    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("HTTP请求失败: {}", response.status()));
    }

    Ok(response.text().await?)
}

/// 解析订阅内容
pub fn parse_subscribe(content: &str) -> anyhow::Result<Config> {
    // 1. 尝试解析为Clash YAML
    if let Ok(config) = serde_yaml::from_str::<Config>(content) {
        info!("成功解析为Clash YAML格式");
        return Ok(config);
    }

    // 2. 尝试Base64解码
    let content = content.trim().replace('\n', "").replace('\r', "");
    if let Ok(decoded) = general_purpose::STANDARD
        .decode(&content)
        .or_else(|_| general_purpose::URL_SAFE.decode(&content))
    {
        if let Ok(decoded_str) = String::from_utf8(decoded) {
            let proxies = parse_proxy_links(&decoded_str)?;
            if !proxies.is_empty() {
                info!("成功解析为Base64格式，获取{}个节点", proxies.len());
                return Ok(Config {
                    proxies,
                    ..Default::default()
                });
            }
        }
    }

    // 3. 尝试解析为链接列表
    let proxies = parse_proxy_links(&content)?;
    if !proxies.is_empty() {
        info!("成功解析为链接格式，获取{}个节点", proxies.len());
        return Ok(Config {
            proxies,
            ..Default::default()
        });
    }

    Err(anyhow::anyhow!("无法解析订阅内容"))
}

/// 解析代理链接列表
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

/// 解析单个代理链接
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

/// 解析VMess链接
fn parse_vmess(link: &str) -> Option<ProxyConfig> {
    let b64 = link.strip_prefix("vmess://")?;
    let decoded = general_purpose::STANDARD.decode(b64).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    Some(ProxyConfig {
        name: json
            .get("ps")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        proxy_type: "vmess".to_string(),
        server: json
            .get("add")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        port: json.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        uuid: json.get("id").and_then(|v| v.as_str()).map(String::from),
        cipher: json.get("scy").and_then(|v| v.as_str()).map(String::from),
        network: json.get("net").and_then(|v| v.as_str()).map(String::from),
        tls: json
            .get("tls")
            .and_then(|v| v.as_str())
            .map_or(false, |s| s == "tls"),
        ..Default::default()
    })
}

/// 解析VLESS链接
fn parse_vless(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("vless://")?;
    let url = url::Url::parse(&format!("vless://{}", url_str)).ok()?;

    let name = url.fragment().unwrap_or("").to_string();
    let uuid = url.username().to_string();
    let server = url.host_str().unwrap_or("").to_string();
    let port = url.port().unwrap_or(443);

    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();

    Some(ProxyConfig {
        name: urlencoding::decode(&name).ok()?.to_string(),
        proxy_type: "vless".to_string(),
        server,
        port,
        uuid: Some(uuid),
        network: params.get("type").cloned(),
        tls: params
            .get("security")
            .map_or(false, |s| s == "tls" || s == "reality"),
        sni: params.get("sni").cloned(),
        flow: params.get("flow").cloned(),
        ..Default::default()
    })
}

/// 解析Trojan链接
fn parse_trojan(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("trojan://")?;
    let url = url::Url::parse(&format!("trojan://{}", url_str)).ok()?;

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

/// 解析Shadowsocks链接
fn parse_shadowsocks(link: &str) -> Option<ProxyConfig> {
    let content = link.strip_prefix("ss://")?;

    // SIP002格式
    if content.contains('@') {
        let url = url::Url::parse(link).ok()?;
        let name = url.fragment().unwrap_or("").to_string();
        let server = url.host_str().unwrap_or("").to_string();
        let port = url.port().unwrap_or(8388);

        let userinfo = general_purpose::URL_SAFE_NO_PAD
            .decode(url.username())
            .ok()?;
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

/// 解析Hysteria2链接
fn parse_hysteria2(link: &str) -> Option<ProxyConfig> {
    let url_str = link
        .strip_prefix("hysteria2://")
        .or_else(|| link.strip_prefix("hy2://"))?;
    let url = url::Url::parse(&format!("hy2://{}", url_str)).ok()?;

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

/// 解析TUIC链接
fn parse_tuic(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("tuic://")?;
    let url = url::Url::parse(&format!("tuic://{}", url_str)).ok()?;

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
        uuid: parts.get(0).map(|s| s.to_string()),
        password: parts.get(1).map(|s| s.to_string()),
        ..Default::default()
    })
}

/// 解析AnyTLS链接
fn parse_anytls(link: &str) -> Option<ProxyConfig> {
    let url_str = link.strip_prefix("anytls://")?;
    let url = url::Url::parse(&format!("anytls://{}", url_str)).ok()?;

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
