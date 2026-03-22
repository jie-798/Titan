use anyhow::Context;
use tracing::info;

pub async fn select(config_path: &str, group_name: &str, proxy_name: &str) -> anyhow::Result<()> {
    info!("loading config from {}", config_path);
    let mut config = titan_config::load_config(config_path)?;

    let group = config
        .proxy_groups
        .iter_mut()
        .find(|group| group.name == group_name)
        .with_context(|| format!("proxy group not found: {}", group_name))?;

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
    titan_config::save_config(&config, config_path)?;

    println!("Selected {} for group {}", proxy_name, group_name);
    println!("This rewrote the YAML file through serde, so comments and formatting may change.");
    Ok(())
}
