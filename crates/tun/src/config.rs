use std::net::IpAddr;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TunStack {
    System,
    Gvisor,
    #[default]
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunAddress {
    pub address: IpAddr,
    pub prefix_len: u8,
}

impl TunAddress {
    pub fn new(address: IpAddr, prefix_len: u8) -> Self {
        Self { address, prefix_len }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsServer {
    pub address: IpAddr,
    pub port: u16,
}

impl DnsServer {
    pub fn new(address: IpAddr, port: u16) -> Self {
        Self { address, port }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunRoute {
    pub destination: TunAddress,
}

impl TunRoute {
    pub fn new(destination: TunAddress) -> Self {
        Self { destination }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct TunConfig {
    pub stack: TunStack,
    pub adapter_name: String,
    pub interface_name: Option<String>,
    pub addresses: Vec<TunAddress>,
    pub gateway: Option<IpAddr>,
    pub dns_servers: Vec<DnsServer>,
    pub mtu: Option<u32>,
    pub route_all_traffic: bool,
    pub strict_route: bool,
    pub auto_detect_interface: bool,
    pub dns_hijack: Vec<String>,
    pub exclude_routes: Vec<TunAddress>,
    pub proxy_fallback_bind: String,
    pub proxy_fallback_port: u16,
    pub routes: Vec<TunRoute>,
}

impl Default for TunConfig {
    fn default() -> Self {
        Self {
            stack: TunStack::Mixed,
            adapter_name: "TitanTUN".to_string(),
            interface_name: Some("Titan".to_string()),
            addresses: Vec::new(),
            gateway: None,
            dns_servers: Vec::new(),
            mtu: Some(1500),
            route_all_traffic: true,
            strict_route: false,
            auto_detect_interface: true,
            dns_hijack: vec!["any:53".to_string()],
            exclude_routes: Vec::new(),
            proxy_fallback_bind: "127.0.0.1".to_string(),
            proxy_fallback_port: 7890,
            routes: Vec::new(),
        }
    }
}

impl TunConfig {
    pub fn starter() -> Self {
        Self {
            stack: TunStack::Mixed,
            adapter_name: "TitanTUN".to_string(),
            interface_name: Some("Titan".to_string()),
            addresses: vec![TunAddress::new(
                "198.18.0.1".parse().expect("valid starter ipv4"),
                30,
            )],
            gateway: None,
            dns_servers: vec![
                DnsServer::new("1.1.1.1".parse().expect("valid dns"), 53),
                DnsServer::new("8.8.8.8".parse().expect("valid dns"), 53),
            ],
            mtu: Some(1500),
            route_all_traffic: true,
            strict_route: false,
            auto_detect_interface: true,
            dns_hijack: vec!["any:53".to_string()],
            exclude_routes: vec![
                TunAddress::new("192.168.0.0".parse().expect("valid private route"), 16),
                TunAddress::new("10.0.0.0".parse().expect("valid private route"), 8),
                TunAddress::new("172.16.0.0".parse().expect("valid private route"), 12),
            ],
            proxy_fallback_bind: "127.0.0.1".to_string(),
            proxy_fallback_port: 7890,
            routes: Vec::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.adapter_name.trim().is_empty() {
            anyhow::bail!("adapter_name cannot be empty");
        }

        if self.addresses.is_empty() {
            anyhow::bail!("at least one address is required");
        }

        for address in &self.addresses {
            validate_prefix(address)?;
        }

        for route in &self.exclude_routes {
            validate_prefix(route)?;
        }

        if let Some(mtu) = self.mtu {
            if !(576..=9000).contains(&mtu) {
                anyhow::bail!("mtu must be between 576 and 9000");
            }
        }

        if self.proxy_fallback_bind.trim().is_empty() {
            anyhow::bail!("proxy_fallback_bind cannot be empty");
        }

        if self.proxy_fallback_port == 0 {
            anyhow::bail!("proxy_fallback_port must be greater than 0");
        }

        Ok(())
    }

    pub fn summary(&self) -> TunSummary {
        TunSummary {
            stack: self.stack.clone(),
            adapter_name: self.adapter_name.clone(),
            interface_name: self.interface_name.clone(),
            address_count: self.addresses.len(),
            dns_count: self.dns_servers.len(),
            dns_hijack_count: self.dns_hijack.len(),
            exclude_route_count: self.exclude_routes.len(),
            route_count: self.routes.len(),
            route_all_traffic: self.route_all_traffic,
            strict_route: self.strict_route,
            mtu: self.mtu,
        }
    }

    pub fn plan_hint(&self) -> String {
        format!(
            "{:?} stack, {} interface with {} address(es), {} DNS server(s), {} dns hijack rule(s), {} excluded route(s), {} route(s), MTU {:?}, route_all_traffic={}, strict_route={}",
            self.stack,
            self.adapter_name,
            self.addresses.len(),
            self.dns_servers.len(),
            self.dns_hijack.len(),
            self.exclude_routes.len(),
            self.routes.len(),
            self.mtu,
            self.route_all_traffic,
            self.strict_route
        )
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

fn validate_prefix(address: &TunAddress) -> anyhow::Result<()> {
    let max_prefix = match address.address {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if address.prefix_len > max_prefix {
        anyhow::bail!(
            "invalid prefix length {} for {}",
            address.prefix_len,
            address.address
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TunSummary {
    pub stack: TunStack,
    pub adapter_name: String,
    pub interface_name: Option<String>,
    pub address_count: usize,
    pub dns_count: usize,
    pub dns_hijack_count: usize,
    pub exclude_route_count: usize,
    pub route_count: usize,
    pub route_all_traffic: bool,
    pub strict_route: bool,
    pub mtu: Option<u32>,
}
