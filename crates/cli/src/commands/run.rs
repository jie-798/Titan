use std::future::pending;
use std::net::SocketAddr;
use std::sync::Arc;
use std::path::PathBuf;

use crate::commands::path::{display_path, resolve_workspace_str, workspace_root};
use tracing::info;

pub async fn run(
    config_path: &str,
    bind: &str,
    port: u16,
    api_port: Option<u16>,
    api_bind: &str,
    set_system_proxy: bool,
) -> anyhow::Result<()> {
    let resolved_config_path = resolve_workspace_str(config_path);
    info!("loading config from {}", display_path(&resolved_config_path));
    let config = titan_config::load_config(&resolved_config_path)?;
    let subscription = config.subscription.clone();
    let runtime_mode = config.mode.clone();

    info!("initializing proxy engine");
    let engine = Arc::new(
        titan_core::ProxyEngine::new_with_path(config, Some(display_path(&resolved_config_path)))
            .await?,
    );

    info!("starting proxy engine");
    engine.start().await?;

    let subscription_task = spawn_subscription_refresh_task(
        resolved_config_path.clone(),
        engine.clone(),
        subscription.clone(),
    );

    let bind_addr: SocketAddr = format!("{}:{}", bind, port).parse()?;
    let inbound = titan_core::inbound::InboundServer::new(bind_addr, engine.clone());
    let api_addr = match api_port {
        Some(api_port) => Some(format!("{}:{}", api_bind, api_port).parse::<SocketAddr>()?),
        None => None,
    };
    let mut system_proxy = if set_system_proxy {
        let mut proxy = titan_system_proxy::SystemProxy::new();
        proxy.set(&titan_system_proxy::ProxyConfig::new(bind, port))?;
        Some(proxy)
    } else {
        None
    };

    println!("\n========== Titan Runtime ==========\n");
    println!("Config: {}", display_path(&resolved_config_path));
    println!("Workspace: {}", display_path(workspace_root()));
    println!("Mode: {}", runtime_mode);
    println!("Listen: {}", bind_addr);
    println!("Protocol: Mixed (SOCKS5/HTTP)");
    let api_display = api_addr
        .as_ref()
        .map(|addr| format!("http://{}", addr))
        .unwrap_or_else(|| "disabled".to_string());
    println!(
        "API: {}",
        api_display
    );
    println!(
        "System proxy: {}",
        if set_system_proxy {
            format!("enabled -> {}:{}", bind, port)
        } else {
            "disabled".to_string()
        }
    );
    println!("Tip: use `info` to inspect config and runtime paths, `select` to switch nodes.");
    if let Some(subscription) = subscription.as_ref().filter(|sub| sub.enabled) {
        if let Some(hours) = subscription.interval_hours {
            println!(
                "Subscription auto refresh: every {} hour(s) from {}",
                hours,
                redact_url(&subscription.url)
            );
        }
    }
    println!("Press Ctrl+C to stop\n");

    let inbound_task = tokio::spawn(async move { inbound.start().await });
    let api_task = api_addr.map(|api_addr| {
        let api = titan_api::ApiServer::new(api_addr, engine.clone());
        tokio::spawn(async move { api.start().await })
    });

    let result = tokio::select! {
        result = inbound_task => result.map_err(anyhow::Error::from)?,
        result = async {
            match api_task {
                Some(task) => task.await.map_err(anyhow::Error::from)?,
                None => pending::<anyhow::Result<()>>().await,
            }
        } => result,
        signal = tokio::signal::ctrl_c() => {
            signal?;
            println!("Stopping Titan...");
            Ok(())
        }
    };

    if let Some(proxy) = system_proxy.as_mut() {
        proxy.restore()?;
    }
    if let Some(task) = subscription_task {
        task.abort();
    }

    result
}

fn spawn_subscription_refresh_task(
    config_path: PathBuf,
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
    let interval = std::time::Duration::from_secs(hours.saturating_mul(3600));
    Some(tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match titan_config::refresh_subscription_from_config(&config_path).await {
                Ok(Some(_)) => {
                    tracing::info!(
                        "subscription refreshed from {}",
                        display_path(&config_path)
                    );
                    if let Err(err) = engine.reload_from_disk().await {
                        tracing::warn!("failed to reload engine after subscription refresh: {}", err);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        "subscription refresh failed for {}: {}",
                        display_path(&config_path),
                        err
                    );
                }
            }
        }
    }))
}

fn redact_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}
