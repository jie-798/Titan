use std::path::Path;
use std::time::Duration;

use tracing::info;

pub async fn geoip_update(
    url: &str,
    output: &str,
    interval_hours: Option<u64>,
) -> anyhow::Result<()> {
    if matches!(interval_hours, Some(0)) {
        anyhow::bail!("--interval-hours must be greater than 0");
    }

    loop {
        download_once(url, output).await?;

        let Some(hours) = interval_hours else {
            return Ok(());
        };

        println!("Next GEOIP update in {} hour(s)", hours);
        tokio::time::sleep(Duration::from_secs(hours * 60 * 60)).await;
    }
}

async fn download_once(url: &str, output: &str) -> anyhow::Result<()> {
    info!("downloading GEOIP data from {}", url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("Titan/0.1")
        .build()?;

    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("GEOIP download failed with status {}", response.status());
    }

    let body = response.text().await?;
    if !body.contains("|CN|") && !body.contains("|US|") {
        anyhow::bail!("downloaded GEOIP data does not look like delegated APNIC data");
    }

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let temp_path = format!("{}.tmp", output);
    std::fs::write(&temp_path, body.as_bytes())?;
    std::fs::rename(&temp_path, output)?;

    println!("Saved GEOIP data to {} ({} bytes)", output, body.len());
    Ok(())
}
