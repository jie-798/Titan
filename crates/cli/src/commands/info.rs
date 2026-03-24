use std::collections::BTreeMap;

use chrono::{Local, TimeZone};

use crate::commands::path::{data_dir, display_path, resolve_workspace_str, workspace_root};
use tracing::info;

pub async fn info(config_path: &str) -> anyhow::Result<()> {
    let resolved_config_path = resolve_workspace_str(config_path);
    info!("loading config from {}", display_path(&resolved_config_path));
    let config = titan_config::load_config(&resolved_config_path)?;

    println!("\n========== Titan Config ==========\n");
    println!("Workspace root: {}", display_path(workspace_root()));
    println!("Data dir: {}", display_path(data_dir()));
    println!("Config file: {}", display_path(&resolved_config_path));
    println!("Config exists: {}", resolved_config_path.exists());
    println!("Mode: {}", config.mode);
    println!("Log level: {}", config.log_level);

    if let Some(port) = config.mixed_port {
        println!("Mixed port: {}", port);
    }
    if let Some(port) = config.socks_port {
        println!("SOCKS port: {}", port);
    }
    if let Some(port) = config.port {
        println!("HTTP port: {}", port);
    }

    println!("\n--- Protocol Stats ---\n");
    let mut stats = BTreeMap::new();
    for proxy in &config.proxies {
        *stats.entry(proxy.proxy_type.to_ascii_uppercase()).or_insert(0_usize) += 1;
    }
    for (proto, count) in stats {
        println!("{:<16} {}", proto, count);
    }

    let udp_enabled = config.proxies.iter().filter(|proxy| proxy.udp).count();
    println!("UDP-capable proxies: {}", udp_enabled);

    println!("\nProxy count: {}", config.proxies.len());
    println!("Proxy group count: {}", config.proxy_groups.len());
    println!("Rule count: {}", config.rules.len());

    if let Some(subscription) = config.subscription.as_ref() {
        println!("\n--- Subscription Config ---");
        println!(
            "Enabled: {}",
            if subscription.enabled { "yes" } else { "no" }
        );
        println!(
            "URL: {}",
            subscription
                .url
                .split('?')
                .next()
                .unwrap_or(subscription.url.as_str())
        );
        if let Some(interval) = subscription.interval_hours {
            println!("Auto refresh interval: {} hour(s)", interval);
        } else {
            println!("Auto refresh interval: disabled");
        }
    }

    let metadata_path = titan_config::subscription_metadata_path(&resolved_config_path);
    if metadata_path.exists() {
        if let Ok(metadata) = titan_config::load_subscription_metadata(&metadata_path) {
            println!("\n--- Subscription Usage ---");
            println!("Metadata file: {}", display_path(&metadata_path));
            if let Some(source) = metadata.source.as_deref() {
                println!("Source: {}", source.split('?').next().unwrap_or(source));
            }
            println!("Fetched at: {}", format_unix(metadata.fetched_at_unix));
            if let Some(upload) = metadata.upload {
                println!("Upload: {}", format_bytes(upload));
            }
            if let Some(download) = metadata.download {
                println!("Download: {}", format_bytes(download));
            }
            if let Some(used) = metadata.used() {
                println!("Used: {}", format_bytes(used));
            }
            if let Some(total) = metadata.total {
                println!("Total: {}", format_bytes(total));
            }
            if let Some(remaining) = metadata.remaining() {
                println!("Remaining: {}", format_bytes(remaining));
            }
            if let Some(percent) = metadata.usage_percent() {
                println!("Usage: {:.2}%", percent);
            }
            if let Some(expire) = metadata.expire {
                println!("Expire at: {}", format_unix(expire));
            }
        }
    }

    let selected_groups: Vec<_> = config
        .proxy_groups
        .iter()
        .filter_map(|group| {
            group
                .selected
                .as_ref()
                .map(|selected| format!("{} -> {}", group.name, selected))
        })
        .collect();

    println!("\n--- Manual Selections ---");
    if selected_groups.is_empty() {
        println!("None");
    } else {
        for line in selected_groups {
            println!("{}", line);
        }
    }

    Ok(())
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
