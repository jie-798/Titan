#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

const DESKTOP_CONFIG_FILE: &str = "desktop-config.json";
const DESKTOP_LOG_FILE: &str = "desktop.log";
const DEFAULT_CONFIG_RELATIVE_PATH: &str = "data/config.yaml";

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn app_base_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(root) = find_workspace_root(&cwd) {
        return root;
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = find_workspace_root(exe_dir) {
                return root;
            }

            if exe_dir.join("data").exists() {
                return exe_dir.to_path_buf();
            }
        }
    }

    cwd
}

fn app_data_dir() -> PathBuf {
    app_base_dir().join("data")
}

fn desktop_config_path() -> PathBuf {
    app_data_dir().join(DESKTOP_CONFIG_FILE)
}

fn desktop_log_path() -> PathBuf {
    app_data_dir().join(DESKTOP_LOG_FILE)
}

fn resolve_user_path(value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        app_base_dir().join(path)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopConfig {
    inbound_bind: String,
    inbound_port: u16,
    api_bind: String,
    api_port: u16,
    auto_start_engine: bool,
    auto_enable_system_proxy: bool,
    config_path: String,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            inbound_bind: "127.0.0.1".to_string(),
            inbound_port: 7890,
            api_bind: "127.0.0.1".to_string(),
            api_port: 9090,
            auto_start_engine: false,
            auto_enable_system_proxy: false,
            config_path: DEFAULT_CONFIG_RELATIVE_PATH.to_string(),
        }
    }
}

impl DesktopConfig {
    fn load() -> Self {
        let path = desktop_config_path();
        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str::<DesktopConfig>(&content).ok())
        {
            Some(config) => config,
            None => Self::default(),
        }
    }

    fn save(&self) -> anyhow::Result<()> {
        let path = desktop_config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    fn resolved_config_path(&self) -> PathBuf {
        resolve_user_path(&self.config_path)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendAppStatus {
    engine_running: bool,
    system_proxy_enabled: bool,
    inbound_bind: String,
    inbound_port: u16,
    active_node: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyNodeDto {
    id: String,
    name: String,
    protocol: String,
    server: String,
    port: u16,
    latency: Option<u64>,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyGroupDto {
    id: String,
    name: String,
    #[serde(rename = "type")]
    group_type: String,
    current_node_id: Option<String>,
    nodes: Vec<ProxyNodeDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionDto {
    id: String,
    source_ip: String,
    source_port: u16,
    destination_host: String,
    destination_port: u16,
    proxy_node_id: Option<String>,
    rule_id: Option<String>,
    upload_bytes: u64,
    download_bytes: u64,
    start_time: u64,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrafficDto {
    upload_bytes_per_second: u64,
    download_bytes_per_second: u64,
    total_upload_bytes: u64,
    total_download_bytes: u64,
    active_connections: usize,
}

#[derive(Clone)]
struct DesktopState {
    service: titan_app::AppService,
    config: Arc<RwLock<DesktopConfig>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiStatsResponse {
    upload: u64,
    download: u64,
    active_connections: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProxyInfo {
    name: String,
    proxy_type: String,
    server: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProxyGroupInfo {
    name: String,
    group_type: String,
    proxies: Vec<String>,
    selected: Option<String>,
    runtime_selected: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiProxyListResponse {
    proxies: Vec<ApiProxyInfo>,
    proxy_groups: Vec<ApiProxyGroupInfo>,
    unhealthy_proxies: Vec<String>,
}

#[tauri::command]
async fn get_profile_summary(
    state: tauri::State<'_, DesktopState>,
) -> Result<titan_app::ProfileSummary, String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    state
        .service
        .get_profile_summary(&resolved_config_path)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn refresh_subscription(state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    log_info(format!("refresh_subscription requested: path={resolved_config_path}"));

    titan_config::refresh_subscription_from_config(&resolved_config_path)
        .await
        .map_err(|err| {
            let message = err.to_string();
            log_error(format!("refresh_subscription failed: {message}"));
            message
        })?;

    if let Some(engine) = state.service.runtime_controller().engine().await {
        if engine.config_path().await.as_deref() == Some(resolved_config_path.as_str()) {
            if let Err(err) = engine.reload_from_disk().await {
                log_warn(format!("refresh_subscription reload failed: {err}"));
            }
        }
    }

    log_info("refresh_subscription succeeded");
    Ok(())
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn append_desktop_log(level: &str, message: &str) {
    let path = desktop_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{}] {:<5} {}", unix_ts(), level, message);
    }
}

fn log_info(message: impl AsRef<str>) {
    append_desktop_log("INFO", message.as_ref());
}

fn log_warn(message: impl AsRef<str>) {
    append_desktop_log("WARN", message.as_ref());
}

fn log_error(message: impl AsRef<str>) {
    append_desktop_log("ERROR", message.as_ref());
}

fn read_recent_logs(limit: usize) -> Vec<String> {
    std::fs::read_to_string(desktop_log_path())
        .ok()
        .map(|content| {
            let mut lines: Vec<String> = content.lines().map(|line| line.to_string()).collect();
            if lines.len() > limit {
                lines.drain(0..lines.len().saturating_sub(limit));
            }
            lines
        })
        .unwrap_or_default()
}

fn api_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(1200))
        .build()
        .expect("failed to build desktop api client")
}

fn api_base_url(config: &DesktopConfig) -> String {
    format!("http://{}:{}", config.api_bind, config.api_port)
}

async fn fetch_api_json<T>(config: &DesktopConfig, path: &str) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let url = format!("{}{}", api_base_url(config), path);
    let response = api_client().get(url).send().await?;
    let response = response.error_for_status()?;
    Ok(response.json::<T>().await?)
}

fn split_host_port(input: &str) -> (String, u16) {
    input
        .rsplit_once(':')
        .and_then(|(host, port)| port.parse::<u16>().ok().map(|port| (host.to_string(), port)))
        .unwrap_or_else(|| (input.to_string(), 0))
}

#[tauri::command]
async fn get_status(state: tauri::State<'_, DesktopState>) -> Result<FrontendAppStatus, String> {
    let config = state.config.read().await.clone();
    let runtime = state.service.runtime_controller().snapshot().await;
    let system_proxy = state
        .service
        .get_system_proxy_status()
        .await
        .map_err(|err| err.to_string())?;

    let active_node = if matches!(runtime.status, titan_app::RuntimeStatus::Running) {
        match fetch_api_json::<ApiProxyListResponse>(&config, "/proxies").await {
            Ok(response) => response
                .proxy_groups
                .into_iter()
                .find_map(|group| group.runtime_selected.or(group.selected)),
            Err(err) => {
                log_warn(format!("get_status fallback to runtime snapshot because API /proxies failed: {err}"));
                None
            }
        }
    } else {
        None
    };

    let error = match runtime.status {
        titan_app::RuntimeStatus::Failed(ref message) => Some(message.clone()),
        _ => None,
    };

    Ok(FrontendAppStatus {
        engine_running: matches!(runtime.status, titan_app::RuntimeStatus::Running),
        system_proxy_enabled: system_proxy.enabled,
        inbound_bind: runtime
            .bind_addr
            .map(|addr| addr.ip().to_string())
            .unwrap_or(config.inbound_bind),
        inbound_port: runtime.bind_addr.map(|addr| addr.port()).unwrap_or(config.inbound_port),
        active_node,
        error,
    })
}

#[tauri::command]
async fn get_proxies(state: tauri::State<'_, DesktopState>) -> Result<Vec<ProxyGroupDto>, String> {
    let config = state.config.read().await.clone();
    let runtime = state.service.runtime_controller().snapshot().await;

    if matches!(runtime.status, titan_app::RuntimeStatus::Running) {
        match fetch_api_json::<ApiProxyListResponse>(&config, "/proxies").await {
            Ok(response) => {
                let nodes = response.proxies;
                let unhealthy = response.unhealthy_proxies;
                return Ok(response
                    .proxy_groups
                    .into_iter()
                    .map(|group| ProxyGroupDto {
                        id: group.name.clone(),
                        name: group.name.clone(),
                        group_type: if group.group_type.eq_ignore_ascii_case("select") {
                            "selector".to_string()
                        } else {
                            group.group_type.clone()
                        },
                        current_node_id: group.runtime_selected.clone().or(group.selected.clone()),
                        nodes: group
                            .proxies
                            .iter()
                            .filter_map(|proxy_name| {
                                nodes.iter().find(|node| node.name == *proxy_name).map(|node| ProxyNodeDto {
                                    id: node.name.clone(),
                                    name: node.name.clone(),
                                    protocol: node.proxy_type.clone(),
                                    server: node.server.clone(),
                                    port: node.port,
                                    latency: None,
                                    status: if unhealthy.iter().any(|name| name == &node.name) {
                                        "error".to_string()
                                    } else if group
                                        .runtime_selected
                                        .as_deref()
                                        .or(group.selected.as_deref())
                                        == Some(node.name.as_str())
                                    {
                                        "active".to_string()
                                    } else {
                                        "inactive".to_string()
                                    },
                                })
                            })
                            .collect(),
                    })
                    .collect());
            }
            Err(err) => {
                log_warn(format!("get_proxies fallback to config file because API /proxies failed: {err}"));
            }
        }
    }

    let resolved_config_path = display_path(&config.resolved_config_path());
    let (nodes, groups) = state
        .service
        .get_proxy_overview(&resolved_config_path)
        .await
        .map_err(|err| err.to_string())?;

    Ok(groups
        .into_iter()
        .map(|group| ProxyGroupDto {
            id: group.name.clone(),
            name: group.name.clone(),
            group_type: if group.group_type.eq_ignore_ascii_case("select") {
                "selector".to_string()
            } else {
                group.group_type.clone()
            },
            current_node_id: group.selected.clone(),
            nodes: group
                .proxies
                .iter()
                .filter_map(|proxy_name| {
                    nodes.iter().find(|node| node.name == *proxy_name).map(|node| ProxyNodeDto {
                        id: node.name.clone(),
                        name: node.name.clone(),
                        protocol: node.proxy_type.clone(),
                        server: node.server.clone(),
                        port: node.port,
                        latency: None,
                        status: if group.selected.as_deref() == Some(node.name.as_str()) {
                            "active".to_string()
                        } else {
                            "inactive".to_string()
                        },
                    })
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
async fn get_sessions(
    state: tauri::State<'_, DesktopState>,
    state_filter: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SessionDto>, String> {
    let Some(engine) = state.service.runtime_controller().engine().await else {
        return Ok(Vec::new());
    };

    let limit = limit.unwrap_or(100).clamp(1, 500);
    let sessions = match state_filter.as_deref() {
        Some("closed") => engine.session_manager().get_recent_closed(limit),
        Some("all") => {
            let mut items = engine.session_manager().get_active();
            items.extend(engine.session_manager().get_recent_closed(limit));
            items
        }
        _ => engine.session_manager().get_active(),
    };

    Ok(sessions
        .into_iter()
        .take(limit)
        .map(|session| {
            let duration_secs = session.duration().as_secs();
            let (destination_host, destination_port) = split_host_port(&session.target);
            let (source_ip, source_port) = split_host_port(&session.source);

            SessionDto {
                id: session.id,
                source_ip,
                source_port,
                destination_host,
                destination_port,
                proxy_node_id: Some(session.proxy),
                rule_id: session.matched_rule.clone(),
                upload_bytes: session.upload,
                download_bytes: session.download,
                start_time: duration_secs,
                status: match session.state {
                    titan_core::session::SessionState::Active => "active".to_string(),
                    titan_core::session::SessionState::Closed => "closed".to_string(),
                },
            }
        })
        .collect())
}

#[tauri::command]
async fn close_all_sessions(state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    log_info("close_all_sessions requested");
    let Some(engine) = state.service.runtime_controller().engine().await else {
        return Ok(());
    };
    engine.session_manager().close_all().await;
    log_info("close_all_sessions succeeded");
    Ok(())
}

#[tauri::command]
async fn clear_session_history(state: tauri::State<'_, DesktopState>) -> Result<(), String> {
    log_info("clear_session_history requested");
    let Some(engine) = state.service.runtime_controller().engine().await else {
        return Ok(());
    };
    engine.session_manager().clear_history();
    log_info("clear_session_history succeeded");
    Ok(())
}

#[tauri::command]
async fn get_traffic(state: tauri::State<'_, DesktopState>) -> Result<TrafficDto, String> {
    let config = state.config.read().await.clone();
    let runtime = state.service.runtime_controller().snapshot().await;

    if matches!(runtime.status, titan_app::RuntimeStatus::Running) {
        match fetch_api_json::<ApiStatsResponse>(&config, "/stats").await {
            Ok(stats) => {
                return Ok(TrafficDto {
                    upload_bytes_per_second: 0,
                    download_bytes_per_second: 0,
                    total_upload_bytes: stats.upload,
                    total_download_bytes: stats.download,
                    active_connections: stats.active_connections,
                });
            }
            Err(err) => {
                log_warn(format!("get_traffic fallback to runtime manager because API /stats failed: {err}"));
            }
        }
    }

    let Some(engine) = state.service.runtime_controller().engine().await else {
        return Ok(TrafficDto {
            upload_bytes_per_second: 0,
            download_bytes_per_second: 0,
            total_upload_bytes: 0,
            total_download_bytes: 0,
            active_connections: 0,
        });
    };

    let session_manager = engine.session_manager();
    Ok(TrafficDto {
        upload_bytes_per_second: 0,
        download_bytes_per_second: 0,
        total_upload_bytes: session_manager.total_upload(),
        total_download_bytes: session_manager.total_download(),
        active_connections: session_manager.count(),
    })
}

#[tauri::command]
async fn get_config(state: tauri::State<'_, DesktopState>) -> Result<DesktopConfig, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
async fn set_config(
    state: tauri::State<'_, DesktopState>,
    config: DesktopConfig,
) -> Result<DesktopConfig, String> {
    log_info(format!(
        "set_config requested: bind={}:{} api={}:{} path={} resolved_path={}",
        config.inbound_bind,
        config.inbound_port,
        config.api_bind,
        config.api_port,
        config.config_path,
        display_path(&config.resolved_config_path())
    ));
    match config.save() {
        Ok(()) => {
            *state.config.write().await = config.clone();
            log_info("set_config succeeded");
            Ok(config)
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("set_config failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn start_engine(state: tauri::State<'_, DesktopState>) -> Result<FrontendAppStatus, String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    log_info(format!(
        "start_engine requested: bind={}:{} api={}:{} path={} resolved_path={} auto_proxy={}",
        config.inbound_bind,
        config.inbound_port,
        config.api_bind,
        config.api_port,
        config.config_path,
        resolved_config_path,
        config.auto_enable_system_proxy
    ));
    match state
        .service
        .start_runtime(titan_app::StartOptions {
            config_path: resolved_config_path,
            bind: config.inbound_bind.clone(),
            port: config.inbound_port,
            api_bind: Some(config.api_bind.clone()),
            api_port: Some(config.api_port),
            set_system_proxy: config.auto_enable_system_proxy,
        })
        .await
    {
        Ok(_) => {
            log_info("start_engine succeeded");
            get_status(state).await
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("start_engine failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn stop_engine(state: tauri::State<'_, DesktopState>) -> Result<FrontendAppStatus, String> {
    log_info("stop_engine requested");
    match state.service.stop_runtime().await {
        Ok(_) => {
            log_info("stop_engine succeeded");
            get_status(state).await
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("stop_engine failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn enable_system_proxy(
    state: tauri::State<'_, DesktopState>,
) -> Result<FrontendAppStatus, String> {
    let config = state.config.read().await.clone();
    log_info(format!(
        "enable_system_proxy requested: {}:{}",
        config.inbound_bind, config.inbound_port
    ));
    let mut proxy = titan_system_proxy::SystemProxy::new();
    match proxy.set(&titan_system_proxy::ProxyConfig::new(&config.inbound_bind, config.inbound_port)) {
        Ok(()) => {
            std::mem::forget(proxy);
            log_info("enable_system_proxy succeeded");
            get_status(state).await
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("enable_system_proxy failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn disable_system_proxy(
    state: tauri::State<'_, DesktopState>,
) -> Result<FrontendAppStatus, String> {
    log_info("disable_system_proxy requested");
    let mut proxy = titan_system_proxy::SystemProxy::new();
    match proxy.disable() {
        Ok(()) => {
            log_info("disable_system_proxy succeeded");
            get_status(state).await
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("disable_system_proxy failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn select_proxy(
    state: tauri::State<'_, DesktopState>,
    group_id: String,
    node_id: String,
) -> Result<(), String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    log_info(format!(
        "select_proxy requested: group={group_id} node={node_id} path={resolved_config_path}"
    ));
    match state
        .service
        .select_proxy(&resolved_config_path, &group_id, &node_id)
        .await
    {
        Ok(()) => {
            log_info("select_proxy succeeded");
            Ok(())
        }
        Err(err) => {
            let message = err.to_string();
            log_error(format!("select_proxy failed: {message}"));
            Err(message)
        }
    }
}

#[tauri::command]
async fn get_recent_logs(limit: Option<usize>) -> Result<Vec<String>, String> {
    let count = limit.unwrap_or(80).clamp(10, 500);
    Ok(read_recent_logs(count))
}

#[tauri::command]
async fn test_proxy(
    state: tauri::State<'_, DesktopState>,
    node_id: String,
) -> Result<titan_core::ProxyTestReport, String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    let report = titan_core::testing::run_proxy_tests(
        &resolved_config_path,
        Some(node_id.as_str()),
        None,
        "http://www.gstatic.com/generate_204",
        std::time::Duration::from_secs(5),
    )
    .await
    .map_err(|err| err.to_string())?;
    log_info(format!(
        "test_proxy completed: {} ok, {} failed, {} skipped",
        report.ok, report.failed, report.skipped
    ));
    Ok(report)
}

#[tauri::command]
async fn test_all_proxies(
    state: tauri::State<'_, DesktopState>,
    group_id: String,
) -> Result<titan_core::ProxyTestReport, String> {
    let config = state.config.read().await.clone();
    let resolved_config_path = display_path(&config.resolved_config_path());
    let report = titan_core::testing::run_proxy_tests(
        &resolved_config_path,
        None,
        Some(group_id.as_str()),
        "http://www.gstatic.com/generate_204",
        std::time::Duration::from_secs(5),
    )
    .await
    .map_err(|err| err.to_string())?;
    log_info(format!(
        "test_all_proxies completed: {} ok, {} failed, {} skipped",
        report.ok, report.failed, report.skipped
    ));
    Ok(report)
}

fn main() {

    let config = DesktopConfig::load();
    log_info(format!(
        "desktop app booting with base_dir={} bind={}:{} api={}:{} path={} resolved_path={}",
        display_path(&app_base_dir()),
        config.inbound_bind,
        config.inbound_port,
        config.api_bind,
        config.api_port,
        config.config_path,
        display_path(&config.resolved_config_path())
    ));

    let state = DesktopState {
        service: titan_app::AppService::new(),
        config: Arc::new(RwLock::new(config)),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_proxies,
            get_sessions,
            get_traffic,
            get_config,
            get_profile_summary,
            set_config,
            start_engine,
            stop_engine,
            enable_system_proxy,
            disable_system_proxy,
            select_proxy,
            close_all_sessions,
            clear_session_history,
            get_recent_logs,
            refresh_subscription,
            test_proxy,
            test_all_proxies,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Titan desktop");
}


