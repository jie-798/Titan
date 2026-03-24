use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StartOptions {
    pub config_path: String,
    pub bind: String,
    pub port: u16,
    pub api_bind: Option<String>,
    pub api_port: Option<u16>,
    pub set_system_proxy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportSubscriptionResult {
    pub output_path: String,
    pub proxy_count: usize,
    pub proxy_group_count: usize,
    pub rule_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionUsageView {
    pub fetched_at_unix: u64,
    pub fetched_at_local: Option<String>,
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub used: Option<u64>,
    pub total: Option<u64>,
    pub remaining: Option<u64>,
    pub usage_percent: Option<f64>,
    pub expire: Option<u64>,
    pub expire_local: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemProxyStatus {
    pub supported: bool,
    pub enabled: bool,
    pub server: Option<String>,
    pub bypass: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeSnapshot {
    pub running: bool,
    pub config_path: Option<String>,
    pub mode: Option<String>,
    pub mixed_bind: Option<String>,
    pub mixed_port: Option<u16>,
    pub api_bind: Option<String>,
    pub api_port: Option<u16>,
    pub system_proxy_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyNodeView {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    pub port: u16,
    pub healthy: bool,
    pub udp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyGroupView {
    pub name: String,
    pub group_type: String,
    pub selected: Option<String>,
    pub runtime_selected: Option<String>,
    pub proxies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileSummary {
    pub config_path: String,
    pub mode: String,
    pub log_level: String,
    pub proxy_count: usize,
    pub proxy_group_count: usize,
    pub rule_count: usize,
    pub udp_proxy_count: usize,
    pub manual_selections: Vec<String>,
    pub rule_preview: Vec<String>,
    pub subscription: Option<SubscriptionUsageView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppStatus {
    pub runtime: RuntimeSnapshot,
    pub system_proxy: SystemProxyStatus,
    pub unhealthy_proxies: Vec<String>,
    pub proxy_groups: Vec<ProxyGroupView>,
}
