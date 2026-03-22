use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub proxy_type: String,

    #[serde(default)]
    pub server: String,

    #[serde(default)]
    pub port: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,

    #[serde(default)]
    pub tls: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,

    #[serde(default, rename = "skip-cert-verify")]
    pub skip_cert_verify: bool,

    #[serde(default)]
    pub udp: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub up: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub down: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub obfs: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl fmt::Display for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({}:{})",
            self.proxy_type.to_uppercase(),
            self.name,
            self.server,
            self.port
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyGroupConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub group_type: String,

    pub proxies: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<u64>,

    #[serde(default)]
    pub lazy: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
}

impl fmt::Display for ProxyGroupConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(selected) = &self.selected {
            write!(
                f,
                "[{}] {} ({} proxies, selected: {})",
                self.group_type,
                self.name,
                self.proxies.len(),
                selected
            )
        } else {
            write!(
                f,
                "[{}] {} ({} proxies)",
                self.group_type,
                self.name,
                self.proxies.len()
            )
        }
    }
}
