use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use crate::commands::path::{display_path, resolve_workspace_str};
use tracing::info;

pub async fn geosite_update(
    config_path: &str,
    base_url: &str,
    output_dir: &str,
    category: Option<&str>,
    interval_hours: Option<u64>,
) -> anyhow::Result<()> {
    if matches!(interval_hours, Some(0)) {
        anyhow::bail!("--interval-hours must be greater than 0");
    }

    let config_path = resolve_workspace_str(config_path);
    let output_dir = resolve_workspace_str(output_dir);

    loop {
        let categories = resolve_categories(&config_path, category)?;
        if categories.is_empty() {
            anyhow::bail!("no GEOSITE categories were requested or found in the config");
        }

        for category in &categories {
            download_category(base_url, &output_dir, category).await?;
        }

        let Some(hours) = interval_hours else {
            return Ok(());
        };

        println!("Next GEOSITE update in {} hour(s)", hours);
        tokio::time::sleep(Duration::from_secs(hours * 60 * 60)).await;
    }
}

fn resolve_categories(config_path: impl AsRef<Path>, category: Option<&str>) -> anyhow::Result<Vec<String>> {
    if let Some(category) = category {
        return Ok(vec![category.trim().to_ascii_lowercase()]);
    }

    let config = titan_config::load_config(config_path)?;
    let mut categories = BTreeSet::new();

    for rule in config.rules {
        let parts: Vec<&str> = rule.split(',').collect();
        if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("GEOSITE") {
            categories.insert(parts[1].trim().to_ascii_lowercase());
        }
    }

    Ok(categories.into_iter().collect())
}

async fn download_category(
    base_url: &str,
    output_dir: impl AsRef<Path>,
    category: &str,
) -> anyhow::Result<()> {
    info!("downloading GEOSITE category {}", category);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("Titan/0.1")
        .build()?;

    let base_url = base_url.trim_end_matches('/');
    let url = format!("{}/{}", base_url, category);
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "GEOSITE download failed for {} with status {}",
            category,
            response.status()
        );
    }

    let body = response.text().await?;
    if body.trim().is_empty() {
        anyhow::bail!("downloaded GEOSITE category {} is empty", category);
    }

    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;
    let output_path = output_dir.join(format!("{}.txt", category));
    let temp_path = output_path.with_extension("txt.tmp");
    std::fs::write(&temp_path, body.as_bytes())?;
    std::fs::rename(&temp_path, &output_path)?;

    println!(
        "Saved GEOSITE category {} to {} ({} bytes)",
        category,
        display_path(&output_path),
        body.len()
    );

    Ok(())
}
