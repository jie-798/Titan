use std::path::Path;

use tracing::info;

pub async fn subscribe(url: &str, output: &str) -> anyhow::Result<()> {
    info!("fetching subscription: {}", url);

    let content = titan_config::fetch_subscribe(url).await?;
    let config = titan_config::parse_subscribe(&content)?;

    println!("\n========== Subscription Result ==========\n");
    println!("Proxy count: {}", config.proxies.len());

    println!("\n--- Proxies ---\n");
    for (index, proxy) in config.proxies.iter().enumerate() {
        println!("{}. {}", index + 1, proxy);
    }

    if !config.proxy_groups.is_empty() {
        println!("\n--- Proxy Groups ---\n");
        for (index, group) in config.proxy_groups.iter().enumerate() {
            println!("{}. {}", index + 1, group);
        }
    }

    if !config.rules.is_empty() {
        println!("\n--- Rules (First 10) ---\n");
        for (index, rule) in config.rules.iter().take(10).enumerate() {
            println!("{}. {}", index + 1, rule);
        }
        if config.rules.len() > 10 {
            println!("... and {} more rules", config.rules.len() - 10);
        }
    }

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    titan_config::save_config(&config, output)?;
    println!("\nSaved config to: {}", output);

    Ok(())
}
