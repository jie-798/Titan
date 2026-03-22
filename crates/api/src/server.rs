use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use titan_core::ProxyEngine;

#[derive(Clone)]
pub(crate) struct ApiState {
    pub(crate) engine: Arc<ProxyEngine>,
}

/// Lightweight HTTP API server for runtime inspection.
pub struct ApiServer {
    bind_addr: SocketAddr,
    engine: Arc<ProxyEngine>,
}

impl ApiServer {
    pub fn new(bind_addr: SocketAddr, engine: Arc<ProxyEngine>) -> Self {
        Self { bind_addr, engine }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::info!("API server listening on {}", self.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(
            listener,
            build_router(ApiState {
                engine: self.engine.clone(),
            }),
        )
        .await?;

        Ok(())
    }
}

fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/sessions", get(sessions))
        .route("/config", get(config_summary))
        .route("/proxies", get(proxy_list))
        .route("/reload", post(reload))
        .with_state(state)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInfo {
    pub name: String,
    pub proxy_type: String,
    pub server: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGroupInfo {
    pub name: String,
    pub group_type: String,
    pub proxies: Vec<String>,
    pub selected: Option<String>,
    pub runtime_selected: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficInfo {
    pub upload: u64,
    pub download: u64,
    pub active_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HealthResponse {
    pub(crate) running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfigSummary {
    pub(crate) mode: String,
    pub(crate) log_level: String,
    pub(crate) mixed_port: Option<u16>,
    pub(crate) socks_port: Option<u16>,
    pub(crate) http_port: Option<u16>,
    pub(crate) proxy_count: usize,
    pub(crate) proxy_group_count: usize,
    pub(crate) rule_count: usize,
    pub(crate) manual_selections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionInfo {
    id: String,
    source: String,
    target: String,
    proxy: String,
    upload: u64,
    download: u64,
    total_traffic: u64,
    duration_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProxyListResponse {
    pub(crate) proxies: Vec<ProxyInfo>,
    pub(crate) proxy_groups: Vec<ProxyGroupInfo>,
    pub(crate) unhealthy_proxies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReloadResponse {
    pub(crate) reloaded: bool,
    pub(crate) config_path: Option<String>,
    pub(crate) message: String,
}

async fn health(State(state): State<ApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        running: state.engine.is_running().await,
    })
}

#[cfg(test)]
pub(crate) async fn health_for_test(
    state: State<ApiState>,
) -> Json<HealthResponse> {
    health(state).await
}

async fn stats(State(state): State<ApiState>) -> Json<TrafficInfo> {
    let session_manager = state.engine.session_manager();
    Json(TrafficInfo {
        upload: session_manager.total_upload(),
        download: session_manager.total_download(),
        active_connections: session_manager.count(),
    })
}

async fn sessions(State(state): State<ApiState>) -> Json<Vec<SessionInfo>> {
    let items = state
        .engine
        .session_manager()
        .get_all()
        .into_iter()
        .map(|session| {
            let total_traffic = session.total_traffic();
            let duration_secs = session.duration().as_secs();

            SessionInfo {
                id: session.id,
                source: session.source,
                target: session.target,
                proxy: session.proxy,
                upload: session.upload,
                download: session.download,
                total_traffic,
                duration_secs,
            }
        })
        .collect();
    Json(items)
}

async fn config_summary(State(state): State<ApiState>) -> Json<ConfigSummary> {
    let config = state.engine.get_config().await;
    let manual_selections = config
        .proxy_groups
        .iter()
        .filter_map(|group| {
            group
                .selected
                .as_ref()
                .map(|selected| format!("{} -> {}", group.name, selected))
        })
        .collect();

    Json(ConfigSummary {
        mode: config.mode,
        log_level: config.log_level,
        mixed_port: config.mixed_port,
        socks_port: config.socks_port,
        http_port: config.port,
        proxy_count: config.proxies.len(),
        proxy_group_count: config.proxy_groups.len(),
        rule_count: config.rules.len(),
        manual_selections,
    })
}

#[cfg(test)]
pub(crate) async fn config_summary_for_test(
    state: State<ApiState>,
) -> Json<ConfigSummary> {
    config_summary(state).await
}

async fn proxy_list(State(state): State<ApiState>) -> Json<ProxyListResponse> {
    let config = state.engine.get_config().await;
    let group_states = state.engine.outbound_group_states().await;
    let unhealthy_proxies = state.engine.unhealthy_outbounds().await;
    Json(ProxyListResponse {
        proxies: config
            .proxies
            .into_iter()
            .map(|proxy| ProxyInfo {
                name: proxy.name,
                proxy_type: proxy.proxy_type,
                server: proxy.server,
                port: proxy.port,
            })
            .collect(),
        proxy_groups: config
            .proxy_groups
            .into_iter()
            .map(|group| ProxyGroupInfo {
                runtime_selected: group_states
                    .iter()
                    .find(|state| state.name == group.name)
                    .and_then(|state| state.selected.clone()),
                name: group.name,
                group_type: group.group_type,
                proxies: group.proxies,
                selected: group.selected,
            })
            .collect(),
        unhealthy_proxies,
    })
}

#[cfg(test)]
pub(crate) async fn proxy_list_for_test(
    state: State<ApiState>,
) -> Json<ProxyListResponse> {
    proxy_list(state).await
}

async fn reload(State(state): State<ApiState>) -> Json<ReloadResponse> {
    let config_path = state.engine.config_path().await;
    match state.engine.reload_from_disk().await {
        Ok(()) => Json(ReloadResponse {
            reloaded: true,
            config_path,
            message: "config reloaded from disk".to_string(),
        }),
        Err(err) => Json(ReloadResponse {
            reloaded: false,
            config_path,
            message: err.to_string(),
        }),
    }
}

#[cfg(test)]
pub(crate) async fn reload_for_test(
    state: State<ApiState>,
) -> Json<ReloadResponse> {
    reload(state).await
}
