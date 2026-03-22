use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::proxy::{ProxyConfig, ProxyGroupConfig};

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// 混合端口
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<u16>,

    /// SOCKS端口
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks_port: Option<u16>,

    /// HTTP端口
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// 允许局域网
    #[serde(default)]
    pub allow_lan: bool,

    /// 运行模式
    #[serde(default = "default_mode")]
    pub mode: String,

    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// 代理节点列表
    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,

    /// 代理组列表
    #[serde(default, rename = "proxy-groups")]
    pub proxy_groups: Vec<ProxyGroupConfig>,

    /// 规则列表
    #[serde(default)]
    pub rules: Vec<String>,

    /// DNS配置
    #[serde(default)]
    pub dns: DnsConfig,
}

fn default_mode() -> String {
    "rule".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

/// DNS配置
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

/// 从文件加载配置
pub fn load_config<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// 保存配置到文件
pub fn save_config<P: AsRef<Path>>(config: &Config, path: P) -> anyhow::Result<()> {
    let content = serde_yaml::to_string(config)?;
    std::fs::write(path, content)?;
    Ok(())
}
