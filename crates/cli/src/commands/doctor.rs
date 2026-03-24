use std::collections::BTreeMap;

use crate::commands::path::{
    candidate_log_paths, data_dir, default_config_path, default_geoip_path, default_geosite_dir, display_path,
    default_tun_config_path, default_tun_state_path, resolve_workspace_str, workspace_root,
};

pub async fn doctor(config_path: &str) -> anyhow::Result<()> {
    let workspace_root = workspace_root();
    let config_path = resolve_workspace_str(config_path);
    let geoip_path = default_geoip_path();
    let geosite_dir = default_geosite_dir();
    let tun_config_path = default_tun_config_path();
    let tun_state_path = default_tun_state_path();
    let wintun = titan_tun::probe_wintun();

    println!("\n========== Titan Doctor ==========\n");
    println!("Workspace root: {}", display_path(&workspace_root));
    println!("Current dir: {}", display_path(std::env::current_dir()?));
    println!("Executable: {}", display_path(std::env::current_exe()?));
    println!("Default config: {}", display_path(&default_config_path()));
    println!("Config path: {}", display_path(&config_path));
    println!("GeoIP path: {}", display_path(&geoip_path));
    println!("GEOSITE dir: {}", display_path(&geosite_dir));
    println!("TUN config: {}", display_path(&tun_config_path));
    println!("TUN state: {}", display_path(&tun_state_path));
    println!("Wintun DLL: {}", display_path(&wintun.dll_path));

    println!("\n--- Files ---");
    report_file(&config_path, "Config");
    report_file(&geoip_path, "GeoIP");
    report_dir(&geosite_dir, "GEOSITE");
    report_file(&tun_config_path, "TUN config");
    report_file(&tun_state_path, "TUN state");
    report_file(&wintun.dll_path, "Wintun DLL");
    for path in candidate_log_paths() {
        report_file(&path, "Log");
    }

    println!("\n--- TUN ---");
    println!("Wintun present: {}", wintun.dll_exists);
    println!("Wintun loadable: {}", wintun.loadable);
    println!(
        "Wintun driver version: {}",
        wintun
            .driver_version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "-".to_string())
    );
    if let Some(err) = &wintun.error {
        println!("Wintun error: {}", err);
    }

    println!("\n--- System Proxy ---");
    let proxy = titan_system_proxy::SystemProxy::new();
    match proxy.current_state() {
        Ok((enabled, server, bypass)) => {
            println!("Enabled: {}", enabled);
            println!("Server: {}", if server.is_empty() { "-" } else { &server });
            println!("Bypass: {}", if bypass.is_empty() { "-" } else { &bypass });
        }
        Err(err) => {
            println!("Status: unavailable ({err})");
        }
    }

    if config_path.exists() {
        match titan_config::load_config(&config_path) {
            Ok(config) => {
                println!("\n--- Config Summary ---");
                println!("Mode: {}", config.mode);
                println!("Log level: {}", config.log_level);
                println!("Proxies: {}", config.proxies.len());
                println!("Proxy groups: {}", config.proxy_groups.len());
                println!("Rules: {}", config.rules.len());
                if let Some(subscription) = config.subscription.as_ref() {
                    println!("Subscription config: enabled={}", subscription.enabled);
                    println!(
                        "Subscription URL: {}",
                        subscription
                            .url
                            .split('?')
                            .next()
                            .unwrap_or(subscription.url.as_str())
                    );
                    if let Some(interval) = subscription.interval_hours {
                        println!("Subscription auto refresh: every {} hour(s)", interval);
                    } else {
                        println!("Subscription auto refresh: disabled");
                    }
                }

                let metadata_path = titan_config::subscription_metadata_path(&config_path);
                if metadata_path.exists() {
                    match titan_config::load_subscription_metadata(&metadata_path) {
                        Ok(metadata) => {
                            println!("Subscription metadata: {}", display_path(&metadata_path));
                            if let Some(remaining) = metadata.remaining() {
                                println!(
                                    "Subscription remaining: {} / {}",
                                    format_bytes(remaining),
                                    metadata
                                        .total
                                        .map(format_bytes)
                                        .unwrap_or_else(|| "-".to_string())
                                );
                            }
                            if let Some(percent) = metadata.usage_percent() {
                                println!("Subscription usage: {:.2}%", percent);
                            }
                        }
                        Err(err) => {
                            println!(
                                "Subscription metadata: unreadable ({}) {}",
                                err,
                                display_path(&metadata_path)
                            );
                        }
                    }
                }

                let mut mix = BTreeMap::new();
                for proxy in &config.proxies {
                    *mix.entry(proxy.proxy_type.to_ascii_uppercase()).or_insert(0_usize) += 1;
                }
                if !mix.is_empty() {
                    println!("Protocol mix:");
                    for (proto, count) in mix {
                        println!("  {}: {}", proto, count);
                    }
                }
            }
            Err(err) => {
                println!("\n--- Config Summary ---");
                println!("Load error: {err}");
            }
        }
    }

    println!("\n--- Hints ---");
    if !data_dir().exists() {
        println!("Create the data directory or run a command that writes one.");
    } else {
        println!("The workspace data directory exists.");
    }

    Ok(())
}

fn report_file(path: &std::path::Path, label: &str) {
    if path.exists() {
        match std::fs::metadata(path) {
            Ok(meta) => println!("{}: ok ({} bytes) {}", label, meta.len(), display_path(path)),
            Err(err) => println!("{}: unreadable ({}) {}", label, err, display_path(path)),
        }
    } else {
        println!("{}: missing {}", label, display_path(path));
    }
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

fn report_dir(path: &std::path::Path, label: &str) {
    if path.exists() {
        match std::fs::read_dir(path) {
            Ok(entries) => println!("{}: ok ({} entries) {}", label, entries.count(), display_path(path)),
            Err(err) => println!("{}: unreadable ({}) {}", label, err, display_path(path)),
        }
    } else {
        println!("{}: missing {}", label, display_path(path));
    }
}
