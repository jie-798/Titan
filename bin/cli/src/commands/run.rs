use std::net::SocketAddr;
use std::sync::Arc;
use std::future::pending;

use tracing::info;

pub async fn run(
    config_path: &str,
    bind: &str,
    port: u16,
    api_port: Option<u16>,
    api_bind: &str,
    set_system_proxy: bool,
) -> anyhow::Result<()> {
    info!("loading config from {}", config_path);
    let config = titan_config::load_config(config_path)?;

    info!("initializing proxy engine");
    let engine = Arc::new(
        titan_core::ProxyEngine::new_with_path(config, Some(config_path.to_string())).await?,
    );

    info!("starting proxy engine");
    engine.start().await?;

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

    println!("Titan proxy server started");
    println!("Listen: {}", bind_addr);
    println!("Protocol: Mixed (SOCKS5/HTTP)");
    if let Some(api_addr) = api_addr {
        println!("API: http://{}", api_addr);
    }
    if set_system_proxy {
        println!("System proxy: enabled");
    }
    println!("Press Ctrl+C to stop");

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

    result
}
