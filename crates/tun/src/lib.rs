use std::path::{Path, PathBuf};

pub mod config;
pub mod ffi;
pub mod manager;
pub mod packet;
pub mod state;
pub mod wintun;
pub mod windows;

#[cfg(test)]
mod tests;

pub use config::{DnsServer, TunAddress, TunConfig, TunRoute, TunStack, TunSummary};
pub use ffi::{Guid, NetLuid};
pub use manager::TunManager;
pub use packet::{
    build_ipv4_icmp_port_unreachable,
    build_ipv6_icmp_port_unreachable,
    build_ipv4_tcp_ack,
    build_ipv4_tcp_data,
    build_ipv4_tcp_fin_ack,
    build_ipv4_tcp_reset,
    build_ipv4_tcp_syn_ack,
    build_ipv4_udp_response,
    build_ipv6_tcp_ack,
    build_ipv6_tcp_data,
    build_ipv6_tcp_fin_ack,
    build_ipv6_tcp_reset,
    build_ipv6_tcp_syn_ack,
    build_ipv6_udp_response,
    parse_ip_packet,
    IpPacketSummary,
    TransportProtocol,
};
pub use state::{TunOperation, TunState, TunStatus};
pub use wintun::{Adapter, Error as WintunError, ReceivePacket, Result as WintunResult, SendPacket, Session, WintunLibrary};
pub use windows::{ActiveWindowsTun, WindowsTunPlan};
pub use windows::{
    build_exclude_route_commands,
    build_proxy_bypass_commands,
    detect_default_gateway_ipv4,
};

#[derive(Debug, Clone)]
pub struct WintunDiagnostics {
    pub dll_path: PathBuf,
    pub dll_exists: bool,
    pub loadable: bool,
    pub driver_version: Option<u32>,
    pub error: Option<String>,
}

pub fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

pub fn default_wintun_dll_path() -> PathBuf {
    let arch_dir = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else {
        "amd64"
    };

    workspace_root()
        .join("third_party")
        .join("wintun")
        .join("bin")
        .join(arch_dir)
        .join("wintun.dll")
}

pub fn probe_wintun() -> WintunDiagnostics {
    let dll_path = default_wintun_dll_path();
    let dll_exists = dll_path.is_file();

    if !cfg!(windows) {
        return WintunDiagnostics {
            dll_path,
            dll_exists,
            loadable: false,
            driver_version: None,
            error: Some("Wintun probing is only available on Windows".to_string()),
        };
    }

    if !dll_exists {
        return WintunDiagnostics {
            dll_path,
            dll_exists,
            loadable: false,
            driver_version: None,
            error: Some("wintun.dll not found at default project path".to_string()),
        };
    }

    match WintunLibrary::load_from_path(&dll_path) {
        Ok(library) => WintunDiagnostics {
            dll_path,
            dll_exists,
            loadable: true,
            driver_version: library.running_driver_version(),
            error: None,
        },
        Err(err) => WintunDiagnostics {
            dll_path,
            dll_exists,
            loadable: false,
            driver_version: None,
            error: Some(err.to_string()),
        },
    }
}
