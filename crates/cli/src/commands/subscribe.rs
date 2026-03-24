use crate::commands::path::{display_path, resolve_workspace_str};
use chrono::{Local, TimeZone};
use tracing::info;

pub async fn subscribe(
    url: Option<&str>,
    output: &str,
    interval_hours: Option<u64>,
) -> anyhow::Result<()> {
    let resolved_output = resolve_workspace_str(output);
    let existing_config = if resolved_output.exists() {
        Some(titan_config::load_config(&resolved_output)?)
    } else {
        None
    };
    let subscription = existing_config
        .as_ref()
        .and_then(|config| config.subscription.clone());
    let resolved_url = match url {
        Some(url) => url.to_string(),
        None => subscription
            .as_ref()
            .map(|sub| sub.url.clone())
            .ok_or_else(|| anyhow::anyhow!("subscription url not found; pass --url once to seed the config"))?,
    };
    let redacted_url = redact_url(&resolved_url);
    info!(
        "fetching subscription: {} -> {}",
        redacted_url,
        display_path(&resolved_output)
    );
    let result = titan_config::fetch_subscribe_details(&resolved_url).await?;
    let config = titan_config::parse_subscribe(&result.content)?;
    let metadata_path = titan_config::subscription_metadata_path(&resolved_output);
    let mut config = config;
    config.subscription = Some(titan_config::SubscriptionConfig {
        url: resolved_url.clone(),
        interval_hours: interval_hours
            .or_else(|| subscription.as_ref().and_then(|sub| sub.interval_hours)),
        enabled: true,
    });

    println!("\n========== Subscription Result ==========\n");
    println!("Source: {}", redacted_url);
    println!("Output: {}", display_path(&resolved_output));
    println!("Proxy count: {}", config.proxies.len());
    println!("Proxy group count: {}", config.proxy_groups.len());
    println!("Rule count: {}", config.rules.len());

    println!("\n--- Proxy Summary ---");
    for proxy in config.proxies.iter().take(8) {
        println!("{}", proxy);
    }
    if config.proxies.len() > 8 {
        println!("... and {} more proxies", config.proxies.len() - 8);
    }

    if !config.proxy_groups.is_empty() {
        println!("\n--- Proxy Groups ---");
        for group in config.proxy_groups.iter().take(8) {
            println!("{}", group);
        }
        if config.proxy_groups.len() > 8 {
            println!("... and {} more groups", config.proxy_groups.len() - 8);
        }
    }

    if !config.rules.is_empty() {
        println!("\n--- Rules Preview ---");
        for rule in config.rules.iter().take(10) {
            println!("{}", rule);
        }
        if config.rules.len() > 10 {
            println!("... and {} more rules", config.rules.len() - 10);
        }
    }

    if let Some(parent) = resolved_output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    titan_config::save_config(&config, &resolved_output)?;
    titan_config::save_subscription_metadata(&result.metadata, &metadata_path)?;
    println!("\nSaved config to: {}", display_path(&resolved_output));
    println!("Saved subscription info to: {}", display_path(&metadata_path));
    if let Some(interval) = config
        .subscription
        .as_ref()
        .and_then(|sub| sub.interval_hours)
    {
        println!("Auto refresh: enabled every {} hour(s)", interval);
    }

    if let Some(total) = result.metadata.total {
        println!("\n--- Subscription Usage ---");
        println!("Upload: {}", format_bytes(result.metadata.upload.unwrap_or(0)));
        println!("Download: {}", format_bytes(result.metadata.download.unwrap_or(0)));
        println!("Used: {}", format_bytes(result.metadata.used().unwrap_or(0)));
        println!("Total: {}", format_bytes(total));
        println!(
            "Remaining: {}",
            format_bytes(result.metadata.remaining().unwrap_or(0))
        );
        if let Some(percent) = result.metadata.usage_percent() {
            println!("Usage: {:.2}%", percent);
        }
        if let Some(expire) = result.metadata.expire {
            println!("Expire at: {}", format_unix(expire));
        }
    }

    Ok(())
}

fn redact_url(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn format_unix(unix: u64) -> String {
    Local
        .timestamp_opt(unix as i64, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S %z").to_string())
        .unwrap_or_else(|| format!("{unix}"))
}
