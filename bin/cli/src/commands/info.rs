use tracing::info;

pub async fn info(config_path: &str) -> anyhow::Result<()> {
    info!("loading config from {}", config_path);
    let config = titan_config::load_config(config_path)?;

    println!("\n========== Config Info ==========\n");
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
    let mut stats = std::collections::HashMap::new();
    for proxy in &config.proxies {
        *stats.entry(proxy.proxy_type.clone()).or_insert(0_usize) += 1;
    }
    for (proto, count) in stats {
        println!("{}: {}", proto.to_uppercase(), count);
    }

    println!("\nProxy count: {}", config.proxies.len());
    println!("Proxy group count: {}", config.proxy_groups.len());
    println!("Rule count: {}", config.rules.len());

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

    if !selected_groups.is_empty() {
        println!("\n--- Manual Selections ---\n");
        for line in selected_groups {
            println!("{}", line);
        }
    }

    Ok(())
}
