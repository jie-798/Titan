use anyhow::Context;

use crate::commands::path::{display_path, resolve_workspace_str};
use tracing::info;

pub async fn select(config_path: &str, group_name: &str, proxy_name: &str) -> anyhow::Result<()> {
    let resolved_config_path = resolve_workspace_str(config_path);
    info!("loading config from {}", display_path(&resolved_config_path));
    let mut config = titan_config::load_config(&resolved_config_path)?;

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
    titan_config::save_config(&config, &resolved_config_path)?;

    println!("\n========== Select ==========\n");
    println!("Group : {}", group_name);
    println!("Proxy : {}", proxy_name);
    println!("Config: {}", display_path(&resolved_config_path));
    println!("Status: saved");
    println!("Note  : YAML is rewritten through serde, so comments and formatting may change.");
    Ok(())
}
