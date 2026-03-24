use std::path::{Path, PathBuf};

pub const DEFAULT_CONFIG_PATH: &str = "data/config.yaml";
pub const DEFAULT_GEOIP_PATH: &str = "data/geoip-apnic.raw";
pub const DEFAULT_GEOSITE_DIR: &str = "data/geosite";
pub const DEFAULT_TUN_CONFIG_PATH: &str = "data/tun.yaml";
pub const DEFAULT_TUN_STATE_PATH: &str = "data/tun-state.yaml";

pub fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or(manifest_dir)
}

pub fn data_dir() -> PathBuf {
    workspace_root().join("data")
}

pub fn resolve_workspace_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root().join(path)
    }
}

pub fn resolve_workspace_str(path: &str) -> PathBuf {
    resolve_workspace_path(path)
}

pub fn default_config_path() -> PathBuf {
    data_dir().join("config.yaml")
}

pub fn default_geoip_path() -> PathBuf {
    data_dir().join("geoip-apnic.raw")
}

pub fn default_geosite_dir() -> PathBuf {
    data_dir().join("geosite")
}

pub fn default_cli_log_path() -> PathBuf {
    data_dir().join("titan.log")
}

pub fn default_desktop_log_path() -> PathBuf {
    data_dir().join("desktop.log")
}

pub fn default_tun_config_path() -> PathBuf {
    data_dir().join("tun.yaml")
}

pub fn default_tun_state_path() -> PathBuf {
    data_dir().join("tun-state.yaml")
}

pub fn display_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().to_string()
}

pub fn candidate_log_paths() -> Vec<PathBuf> {
    vec![default_cli_log_path(), default_desktop_log_path()]
}
