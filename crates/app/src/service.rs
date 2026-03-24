use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;

use crate::state::{RuntimeController, RuntimeStatus};
use crate::types::{
    AppStatus, ImportSubscriptionResult, ProfileSummary, ProxyGroupView, ProxyNodeView,
    RuntimeSnapshot, StartOptions, SubscriptionUsageView, SystemProxyStatus,
};

#[derive(Clone, Default)]
pub struct AppService {
    runtime: RuntimeController,
}

impl AppService {
    pub fn new() -> Self {
        Self {
            runtime: RuntimeController::new(),
        }
    }

    pub fn runtime_controller(&self) -> &RuntimeController {
        &self.runtime
    }

    pub async fn load_config(&self, path: &str) -> anyhow::Result<titan_config::Config> {
        titan_config::load_config(path).with_context(|| format!("failed to load config: {path}"))
    }

    pub async fn get_status(&self) -> anyhow::Result<AppStatus> {
        if let Some(engine) = self.runtime.engine().await {
            let mut status = self.attach_engine_status(engine).await?;
            let runtime_state = self.runtime.snapshot().await;
            status.runtime.mixed_bind = runtime_state.bind_addr.map(|addr| addr.ip().to_string());
            status.runtime.mixed_port = runtime_state.bind_addr.map(|addr| addr.port());
            status.runtime.api_bind = runtime_state.api_addr.map(|addr| addr.ip().to_string());
            status.runtime.api_port = runtime_state.api_addr.map(|addr| addr.port());
            status.runtime.system_proxy_enabled = runtime_state.system_proxy_enabled;
            return Ok(status);
        }

        let runtime_state = self.runtime.snapshot().await;
        let runtime = RuntimeSnapshot {
            running: matches!(runtime_state.status, RuntimeStatus::Running),
            config_path: runtime_state.config_path.clone(),
            mode: None,
            mixed_bind: runtime_state.bind_addr.map(|addr| addr.ip().to_string()),
            mixed_port: runtime_state.bind_addr.map(|addr| addr.port()),
            api_bind: runtime_state.api_addr.map(|addr| addr.ip().to_string()),
            api_port: runtime_state.api_addr.map(|addr| addr.port()),
            system_proxy_enabled: runtime_state.system_proxy_enabled,
        };

        Ok(AppStatus {
            runtime,
            system_proxy: self.get_system_proxy_status().await?,
            unhealthy_proxies: Vec::new(),
            proxy_groups: Vec::new(),
        })
    }

    pub async fn get_system_proxy_status(&self) -> anyhow::Result<SystemProxyStatus> {
        let proxy = titan_system_proxy::SystemProxy::new();
        match proxy.current_state() {
            Ok((enabled, server, bypass)) => Ok(SystemProxyStatus {
                supported: true,
                enabled,
                server: if server.is_empty() { None } else { Some(server) },
                bypass: if bypass.is_empty() { None } else { Some(bypass) },
            }),
            Err(_) => Ok(SystemProxyStatus {
                supported: false,
                enabled: false,
                server: None,
                bypass: None,
            }),
        }
    }

    pub async fn import_subscription(
        &self,
        url: &str,
        output_path: &str,
    ) -> anyhow::Result<ImportSubscriptionResult> {
        let result = titan_config::fetch_subscribe_details(url).await?;
        let config = titan_config::parse_subscribe(&result.content)?;
        let mut config = config;
        config.subscription = Some(titan_config::SubscriptionConfig {
            url: url.to_string(),
            interval_hours: None,
            enabled: true,
        });
        std::fs::create_dir_all(
            std::path::Path::new(output_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        )?;
        titan_config::save_config(&config, output_path)?;
        let metadata_path = titan_config::subscription_metadata_path(output_path);
        titan_config::save_subscription_metadata(&result.metadata, metadata_path)?;

        Ok(ImportSubscriptionResult {
            output_path: output_path.to_string(),
            proxy_count: config.proxies.len(),
            proxy_group_count: config.proxy_groups.len(),
            rule_count: config.rules.len(),
        })
    }

    pub async fn get_profile_summary(&self, path: &str) -> anyhow::Result<ProfileSummary> {
        let config = self.load_config(path).await?;
        let metadata_path = titan_config::subscription_metadata_path(path);
        let subscription = titan_config::load_subscription_metadata(metadata_path)
            .ok()
            .map(|metadata| SubscriptionUsageView {
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
                source: metadata
                    .source
                    .map(|source| source.split('?').next().unwrap_or(&source).to_string()),
            });

        Ok(ProfileSummary {
            config_path: path.to_string(),
            mode: config.mode,
            log_level: config.log_level,
            proxy_count: config.proxies.len(),
            proxy_group_count: config.proxy_groups.len(),
            rule_count: config.rules.len(),
            udp_proxy_count: config.proxies.iter().filter(|proxy| proxy.udp).count(),
            manual_selections: config
                .proxy_groups
                .iter()
                .filter_map(|group| group.selected.as_ref().map(|selected| format!("{} -> {}", group.name, selected)))
                .collect(),
            rule_preview: config.rules.into_iter().take(12).collect(),
            subscription,
        })
    }

    pub async fn start_runtime(&self, options: StartOptions) -> anyhow::Result<RuntimeSnapshot> {
        if matches!(self.runtime.snapshot().await.status, RuntimeStatus::Running) {
            anyhow::bail!("runtime is already running");
        }

        let bind_addr: SocketAddr = format!("{}:{}", options.bind, options.port).parse()?;
        let api_addr = match (options.api_bind.as_deref(), options.api_port) {
            (Some(bind), Some(port)) => Some(format!("{bind}:{port}").parse()?),
            _ => None,
        };

        let config = titan_config::load_config(&options.config_path)?;
        let subscription = config.subscription.clone();
        let engine = Arc::new(
            titan_core::ProxyEngine::new_with_path(config, Some(options.config_path.clone())).await?,
        );

        self.runtime
            .set_starting(options.config_path.clone(), bind_addr)
            .await;

        engine.start().await?;

        let inbound = titan_core::inbound::InboundServer::new(bind_addr, engine.clone());
        let inbound_task = tokio::spawn(async move { inbound.start().await });

        let api_task = api_addr.map(|addr| {
            let api = titan_api::ApiServer::new(addr, engine.clone());
            tokio::spawn(async move { api.start().await })
        });

        let subscription_task = spawn_subscription_refresh_task(
            options.config_path.clone(),
            engine.clone(),
            subscription,
        );

        let mut system_proxy = None;
        if options.set_system_proxy {
            let mut proxy = titan_system_proxy::SystemProxy::new();
            if let Err(err) = proxy.set(&titan_system_proxy::ProxyConfig::new(&options.bind, options.port)) {
                inbound_task.abort();
                if let Some(task) = api_task {
                    task.abort();
                }
                engine.stop().await?;
                self.runtime.set_failed(err.to_string()).await;
                return Err(err);
            }
            system_proxy = Some(proxy);
        }

        self.runtime
            .set_handle(
                engine.clone(),
                inbound_task,
                api_task,
                system_proxy,
                subscription_task,
            )
            .await;
        self.runtime
            .set_running(api_addr, options.set_system_proxy)
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;
        let snapshot = self.runtime.snapshot().await;
        Ok(RuntimeSnapshot {
            running: matches!(snapshot.status, RuntimeStatus::Running),
            config_path: snapshot.config_path,
            mode: None,
            mixed_bind: snapshot.bind_addr.map(|addr| addr.ip().to_string()),
            mixed_port: snapshot.bind_addr.map(|addr| addr.port()),
            api_bind: snapshot.api_addr.map(|addr| addr.ip().to_string()),
            api_port: snapshot.api_addr.map(|addr| addr.port()),
            system_proxy_enabled: snapshot.system_proxy_enabled,
        })
    }

    pub async fn stop_runtime(&self) -> anyhow::Result<RuntimeSnapshot> {
        self.runtime.set_stopping().await;
        if let Some((
            engine,
            inbound_task,
            api_task,
            mut system_proxy,
            subscription_task,
        )) = self.runtime.take_handle().await
        {
            inbound_task.abort();
            if let Some(task) = api_task {
                task.abort();
            }
            if let Some(task) = subscription_task {
                task.abort();
            }
            engine.stop().await?;
            if let Some(proxy) = system_proxy.as_mut() {
                proxy.restore()?;
            }
        }
        self.runtime.set_stopped().await;
        Ok(RuntimeSnapshot::default())
    }

    pub async fn reload_runtime(&self) -> anyhow::Result<RuntimeSnapshot> {
        if let Some(engine) = self.runtime.engine().await {
            engine.reload_from_disk().await?;
            return Ok(self.get_status().await?.runtime);
        }

        let snapshot = self.runtime.snapshot().await;
        Ok(RuntimeSnapshot {
            running: matches!(snapshot.status, RuntimeStatus::Running),
            config_path: snapshot.config_path,
            mode: None,
            mixed_bind: snapshot.bind_addr.map(|addr| addr.ip().to_string()),
            mixed_port: snapshot.bind_addr.map(|addr| addr.port()),
            api_bind: snapshot.api_addr.map(|addr| addr.ip().to_string()),
            api_port: snapshot.api_addr.map(|addr| addr.port()),
            system_proxy_enabled: snapshot.system_proxy_enabled,
        })
    }

    pub async fn get_proxy_overview(
        &self,
        path: &str,
    ) -> anyhow::Result<(Vec<ProxyNodeView>, Vec<ProxyGroupView>)> {
        let config = self.load_config(path).await?;

        let nodes = config
            .proxies
            .into_iter()
            .map(|proxy| ProxyNodeView {
                name: proxy.name,
                proxy_type: proxy.proxy_type,
                server: proxy.server,
                port: proxy.port,
                healthy: true,
                udp: proxy.udp,
            })
            .collect();

        let groups = config
            .proxy_groups
            .into_iter()
            .map(|group| ProxyGroupView {
                name: group.name,
                group_type: group.group_type,
                selected: group.selected,
                runtime_selected: None,
                proxies: group.proxies,
            })
            .collect();

        Ok((nodes, groups))
    }

    pub async fn select_proxy(
        &self,
        path: &str,
        group_name: &str,
        proxy_name: &str,
    ) -> anyhow::Result<()> {
        let mut config = self.load_config(path).await?;

        let group = config
            .proxy_groups
            .iter_mut()
            .find(|group| group.name == group_name)
            .with_context(|| format!("proxy group not found: {group_name}"))?;

        if !group.group_type.eq_ignore_ascii_case("select") {
            anyhow::bail!(
                "proxy group {} is type {}, only select groups support manual selection",
                group_name,
                group.group_type
            );
        }

        if !group.proxies.iter().any(|name| name == proxy_name) {
            anyhow::bail!(
                "proxy {} is not a member of group {}",
                proxy_name,
                group_name
            );
        }

        group.selected = Some(proxy_name.to_string());
        titan_config::save_config(&config, path)?;
        if let Some(engine) = self.runtime.engine().await {
            if engine.config_path().await.as_deref() == Some(path) {
                engine.reload_from_disk().await?;
            }
        }
        Ok(())
    }
    pub async fn attach_engine_status(
        &self,
        engine: Arc<titan_core::ProxyEngine>,
    ) -> anyhow::Result<AppStatus> {
        let config = engine.get_config().await;
        let group_states = engine.outbound_group_states().await;
        let unhealthy_proxies = engine.unhealthy_outbounds().await;

        Ok(AppStatus {
            runtime: RuntimeSnapshot {
                running: engine.is_running().await,
                config_path: engine.config_path().await,
                mode: Some(config.mode),
                mixed_bind: None,
                mixed_port: config.mixed_port,
                api_bind: None,
                api_port: None,
                system_proxy_enabled: self.get_system_proxy_status().await?.enabled,
            },
            system_proxy: self.get_system_proxy_status().await?,
            unhealthy_proxies,
            proxy_groups: config
                .proxy_groups
                .into_iter()
                .map(|group| ProxyGroupView {
                    runtime_selected: group_states
                        .iter()
                        .find(|state| state.name == group.name)
                        .and_then(|state| state.selected.clone()),
                    name: group.name,
                    group_type: group.group_type,
                    selected: group.selected,
                    proxies: group.proxies,
                })
                .collect(),
        })
    }
}

fn spawn_subscription_refresh_task(
    config_path: String,
    engine: Arc<titan_core::ProxyEngine>,
    subscription: Option<titan_config::SubscriptionConfig>,
) -> Option<tokio::task::JoinHandle<()>> {
    let subscription = subscription?;
    if !subscription.enabled {
        return None;
    }
    let hours = subscription.interval_hours?;
    if hours == 0 {
        return None;
    }
    let interval = Duration::from_secs(hours.saturating_mul(3600));
    Some(tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match titan_config::refresh_subscription_from_config(&config_path).await {
                Ok(Some(_)) => {
                    tracing::info!("subscription refreshed from {}", config_path);
                    if let Err(err) = engine.reload_from_disk().await {
                        tracing::warn!("failed to reload engine after subscription refresh: {}", err);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!("subscription refresh failed for {}: {}", config_path, err);
                }
            }
        }
    }))
}
fn format_unix_local(value: Option<u64>) -> Option<String> {
    value.map(|unix| unix.to_string())
}







