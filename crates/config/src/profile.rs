use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::proxy::{ProxyConfig, ProxyGroupConfig};

/// Main configuration file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Mixed proxy port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<u16>,

    /// SOCKS proxy port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks_port: Option<u16>,

    /// HTTP proxy port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Allow LAN access.
    #[serde(default)]
    pub allow_lan: bool,

    /// Runtime mode.
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Proxy list.
    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,

    /// Proxy groups.
    #[serde(default, rename = "proxy-groups")]
    pub proxy_groups: Vec<ProxyGroupConfig>,

    /// Rules.
    #[serde(default)]
    pub rules: Vec<String>,

    /// DNS settings.
    #[serde(default)]
    pub dns: DnsConfig,

    /// Subscription settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<SubscriptionConfig>,
}

fn default_mode() -> String {
    "rule".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

/// DNS settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DnsConfig {
    #[serde(default = "default_true")]
    pub enable: bool,

    #[serde(default)]
    pub nameserver: Vec<String>,

    #[serde(default)]
    pub fallback: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// Subscription settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionConfig {
    /// Subscription URL.
    pub url: String,

    /// Auto refresh interval in hours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval_hours: Option<u64>,

    /// Whether auto refresh is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Loads configuration from file.
pub fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Saves configuration to file.
pub fn save_config<P: AsRef<Path>>(config: &Config, path: P) -> anyhow::Result<()> {
    let content = serde_yaml::to_string(config)?;
    std::fs::write(path, content)?;
    Ok(())
}
