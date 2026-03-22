use std::time::Duration;

use tracing::info;

pub async fn test(config_path: &str) -> anyhow::Result<()> {
    info!("loading config from {}", config_path);
    let config = titan_config::load_config(config_path)?;

    println!("\n========== Latency Test ==========\n");

    for proxy_config in &config.proxies {
        print!("Testing {} ... ", proxy_config.name);

        let proxy = match proxy_config.proxy_type.as_str() {
            "anytls" => {
                let p = titan_protocols::AnyTlsProxy::from_config(proxy_config);
                test_proxy(&p).await
            }
            "socks5" | "socks" => {
                let p = titan_protocols::Socks5Proxy::from_config(proxy_config);
                test_proxy(&p).await
            }
            "http" => {
                let p = titan_protocols::HttpProxy::from_config(proxy_config);
                test_proxy(&p).await
            }
            "vless" => {
                let p = titan_protocols::VlessProxy::from_config(proxy_config);
                test_proxy(&p).await
            }
            "vmess" => {
                let p = titan_protocols::VMessProxy::from_config(proxy_config);
                test_proxy(&p).await
            }
            _ => {
                println!("protocol test not implemented yet");
                continue;
            }
        };

        match proxy {
            Ok(latency) => println!("{:.0}ms", latency.as_millis()),
            Err(err) => println!("failed: {}", err),
        }
    }

    Ok(())
}

async fn test_proxy(proxy: &impl titan_protocols::OutboundProxy) -> anyhow::Result<Duration> {
    proxy
        .delay_test(
            "http://www.gstatic.com/generate_204",
            Duration::from_secs(5),
        )
        .await
}
