use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    running: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrafficInfo {
    upload: u64,
    download: u64,
    active_connections: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionUsageInfo {
    fetched_at_local: Option<String>,
    used: Option<u64>,
    total: Option<u64>,
    remaining: Option<u64>,
    usage_percent: Option<f64>,
    expire_local: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigSummary {
    mode: String,
    log_level: String,
    mixed_port: Option<u16>,
    socks_port: Option<u16>,
    http_port: Option<u16>,
    proxy_count: usize,
    proxy_group_count: usize,
    rule_count: usize,
    manual_selections: Vec<String>,
    subscription: Option<SubscriptionUsageInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionInfo {
    source: String,
    target: String,
    proxy: String,
    policy: Option<String>,
    matched_rule: Option<String>,
    upload: u64,
    download: u64,
    total_traffic: u64,
    duration_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProxyFailureInfo {
    name: String,
    message: String,
    last_failed_local: Option<String>,
    failures: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProxyListResponse {
    unhealthy_proxies: Vec<String>,
    recent_failures: Vec<ProxyFailureInfo>,
}

pub async fn runtime(api_base: &str) -> anyhow::Result<()> {
    let api_base = api_base.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;

    let health = get_json::<HealthResponse>(&client, api_base, "/health").await?;
    let config = get_json::<ConfigSummary>(&client, api_base, "/config").await?;
    let stats = get_json::<TrafficInfo>(&client, api_base, "/stats").await?;
    let sessions = get_json::<Vec<SessionInfo>>(&client, api_base, "/sessions").await?;
    let proxies = get_json::<ProxyListResponse>(&client, api_base, "/proxies").await?;

    println!("\n========== Titan Runtime ==========\n");
    println!("API: {}", api_base);
    println!("Running: {}", health.running);
    println!("Mode: {}", config.mode);
    println!("Log level: {}", config.log_level);
    if let Some(port) = config.mixed_port {
        println!("Mixed port: {}", port);
    }
    if let Some(port) = config.socks_port {
        println!("SOCKS port: {}", port);
    }
    if let Some(port) = config.http_port {
        println!("HTTP port: {}", port);
    }
    println!("Proxy count: {}", config.proxy_count);
    println!("Proxy group count: {}", config.proxy_group_count);
    println!("Rule count: {}", config.rule_count);

    println!("\n--- Traffic ---");
    println!("Upload: {}", format_bytes(stats.upload));
    println!("Download: {}", format_bytes(stats.download));
    println!("Active connections: {}", stats.active_connections);

    if let Some(subscription) = config.subscription {
        println!("\n--- Subscription ---");
        if let Some(source) = subscription.source {
            println!("Source: {}", source);
        }
        if let Some(fetched_at) = subscription.fetched_at_local {
            println!("Fetched at: {}", fetched_at);
        }
        if let Some(used) = subscription.used {
            println!("Used: {}", format_bytes(used));
        }
        if let Some(total) = subscription.total {
            println!("Total: {}", format_bytes(total));
        }
        if let Some(remaining) = subscription.remaining {
            println!("Remaining: {}", format_bytes(remaining));
        }
        if let Some(percent) = subscription.usage_percent {
            println!("Usage: {:.2}%", percent);
        }
        if let Some(expire_at) = subscription.expire_local {
            println!("Expire at: {}", expire_at);
        }
    }

    println!("\n--- Manual Selections ---");
    if config.manual_selections.is_empty() {
        println!("None");
    } else {
        for selection in config.manual_selections {
            println!("{}", selection);
        }
    }

    println!("\n--- Proxy Health ---");
    println!("Unhealthy proxies: {}", proxies.unhealthy_proxies.len());
    if proxies.recent_failures.is_empty() {
        println!("Recent failures: none");
    } else {
        for failure in proxies.recent_failures.iter().take(8) {
            println!(
                "{} | failures={} | at={} | {}",
                failure.name,
                failure.failures,
                failure.last_failed_local.as_deref().unwrap_or("-"),
                failure.message
            );
        }
        if proxies.recent_failures.len() > 8 {
            println!("... and {} more", proxies.recent_failures.len() - 8);
        }
    }

    println!("\n--- Sessions ---");
    if sessions.is_empty() {
        println!("No active sessions");
    } else {
        for session in sessions.iter().take(8) {
            println!(
                "{} -> {} via {} | policy={} | up={} down={} total={} | {}s",
                session.source,
                session.target,
                session.proxy,
                session.policy.as_deref().unwrap_or("-"),
                format_bytes(session.upload),
                format_bytes(session.download),
                format_bytes(session.total_traffic),
                session.duration_secs
            );
            println!(
                "rule: {}",
                session.matched_rule.as_deref().unwrap_or("-")
            );
        }
        if sessions.len() > 8 {
            println!("... and {} more", sessions.len() - 8);
        }
    }

    Ok(())
}

async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    api_base: &str,
    path: &str,
) -> anyhow::Result<T> {
    let response = client.get(format!("{api_base}{path}")).send().await?;
    let response = response.error_for_status()?;
    Ok(response.json::<T>().await?)
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
