use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, TimeZone};
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
        .route("/sessions/close-all", post(close_all_sessions))
        .route("/sessions/clear-history", post(clear_session_history))
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
    pub udp: bool,
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
    pub(crate) subscription: Option<SubscriptionUsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubscriptionUsageInfo {
    pub(crate) fetched_at_unix: u64,
    pub(crate) fetched_at_local: Option<String>,
    pub(crate) upload: Option<u64>,
    pub(crate) download: Option<u64>,
    pub(crate) used: Option<u64>,
    pub(crate) total: Option<u64>,
    pub(crate) remaining: Option<u64>,
    pub(crate) usage_percent: Option<f64>,
    pub(crate) expire: Option<u64>,
    pub(crate) expire_local: Option<String>,
    pub(crate) source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionInfo {
    id: String,
    source: String,
    target: String,
    proxy: String,
    status: String,
    policy: Option<String>,
    matched_rule: Option<String>,
    upload: u64,
    download: u64,
    total_traffic: u64,
    duration_secs: u64,
    closed_at_local: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionsQuery {
    state: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyFailureInfo {
    pub name: String,
    pub message: String,
    pub first_failed_unix: u64,
    pub first_failed_local: Option<String>,
    pub last_failed_unix: u64,
    pub last_failed_local: Option<String>,
    pub failures: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProxyListResponse {
    pub(crate) proxies: Vec<ProxyInfo>,
    pub(crate) proxy_groups: Vec<ProxyGroupInfo>,
    pub(crate) unhealthy_proxies: Vec<String>,
    pub(crate) recent_failures: Vec<ProxyFailureInfo>,
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

async fn sessions(Query(query): Query<SessionsQuery>, State(state): State<ApiState>) -> Json<Vec<SessionInfo>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let manager = state.engine.session_manager();
    let sessions = match query.state.as_deref() {
        Some("closed") => manager.get_recent_closed(limit),
        Some("all") => {
            let mut items = manager.get_active();
            items.extend(manager.get_recent_closed(limit));
            items
        }
        _ => manager.get_active(),
    };

    let items = sessions
        .into_iter()
        .take(limit)
        .map(|session| {
            let total_traffic = session.total_traffic();
            let duration_secs = session.duration().as_secs();
            let closed_at_local = session.closed_at.map(|instant| {
                let elapsed = instant.elapsed().as_secs();
                format!("{elapsed}s ago")
            });

            SessionInfo {
                id: session.id,
                source: session.source,
                target: session.target,
                proxy: session.proxy,
                status: match session.state {
                    titan_core::session::SessionState::Active => "active".to_string(),
                    titan_core::session::SessionState::Closed => "closed".to_string(),
                },
                policy: session.policy,
                matched_rule: session.matched_rule,
                upload: session.upload,
                download: session.download,
                total_traffic,
                duration_secs,
                closed_at_local,
            }
        })
        .collect();
    Json(items)
}

async fn close_all_sessions(State(state): State<ApiState>) -> Json<bool> {
    state.engine.session_manager().close_all().await;
    Json(true)
}

async fn clear_session_history(State(state): State<ApiState>) -> Json<bool> {
    state.engine.session_manager().clear_history();
    Json(true)
}

async fn config_summary(State(state): State<ApiState>) -> Json<ConfigSummary> {
    let config = state.engine.get_config().await;
    let subscription = state
        .engine
        .config_path()
        .await
        .and_then(|config_path| {
            let metadata_path = titan_config::subscription_metadata_path(&config_path);
            titan_config::load_subscription_metadata(metadata_path).ok()
        })
        .map(|metadata| SubscriptionUsageInfo {
            fetched_at_unix: metadata.fetched_at_unix,
            fetched_at_local: format_unix_local(Some(metadata.fetched_at_unix)),
            upload: metadata.upload,
            download: metadata.download,
            used: metadata.used(),
            total: metadata.total,
            remaining: metadata.remaining(),
            usage_percent: metadata.usage_percent(),
            expire: metadata.expire,
            expire_local: format_unix_local(metadata.expire),
            source: metadata.source.map(|source| source.split('?').next().unwrap_or(&source).to_string()),
        });
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
        subscription,
    })
}

fn format_unix_local(value: Option<u64>) -> Option<String> {
    value.and_then(|unix| {
        Local
            .timestamp_opt(unix as i64, 0)
            .single()
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %z").to_string())
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
    let recent_failures = state
        .engine
        .outbound_failure_states()
        .await
        .into_iter()
        .map(|failure| ProxyFailureInfo {
            name: failure.name,
            message: failure.message,
            first_failed_unix: failure.first_failed_unix,
            first_failed_local: format_unix_local(Some(failure.first_failed_unix)),
            last_failed_unix: failure.last_failed_unix,
            last_failed_local: format_unix_local(Some(failure.last_failed_unix)),
            failures: failure.failures,
        })
        .collect();
    Json(ProxyListResponse {
        proxies: config
            .proxies
            .into_iter()
        .map(|proxy| ProxyInfo {
            name: proxy.name,
            proxy_type: proxy.proxy_type,
            server: proxy.server,
            port: proxy.port,
            udp: proxy.udp,
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
        recent_failures,
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
