use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionInfo {
    id: String,
    source: String,
    target: String,
    proxy: String,
    status: String,
    policy: Option<String>,
    matched_rule: Option<String>,
    upload: u64,
    download: u64,
    total_traffic: u64,
    duration_secs: u64,
    closed_at_local: Option<String>,
}

pub async fn sessions(
    api_base: &str,
    state: Option<&str>,
    limit: usize,
    close_all: bool,
    clear_history: bool,
) -> anyhow::Result<()> {
    let api_base = api_base.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;

    if close_all {
        post_json::<bool>(&client, api_base, "/sessions/close-all").await?;
    }
    if clear_history {
        post_json::<bool>(&client, api_base, "/sessions/clear-history").await?;
    }

    let state = state.unwrap_or("active");
    let sessions = get_json::<Vec<SessionInfo>>(
        &client,
        api_base,
        &format!("/sessions?state={state}&limit={limit}"),
    )
    .await?;

    println!("\n========== Titan Sessions ==========");
    println!("API: {}", api_base);
    println!("State: {}", state);
    println!("Limit: {}", limit);
    println!("Count: {}", sessions.len());

    if sessions.is_empty() {
        println!("No sessions");
        return Ok(());
    }

    println!("\n--- Sessions ---");
    for session in &sessions {
        println!(
            "#{} {} -> {} via {} | {} | policy={} | rule={} | up={} down={} total={} | {}s",
            session.id,
            session.source,
            session.target,
            session.proxy,
            session.status,
            session.policy.as_deref().unwrap_or("-"),
            session.matched_rule.as_deref().unwrap_or("-"),
            format_bytes(session.upload),
            format_bytes(session.download),
            format_bytes(session.total_traffic),
            session.duration_secs
        );
        if let Some(closed_at) = &session.closed_at_local {
            println!("closed at: {}", closed_at);
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

async fn post_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    api_base: &str,
    path: &str,
) -> anyhow::Result<T> {
    let response = client.post(format!("{api_base}{path}")).send().await?;
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
