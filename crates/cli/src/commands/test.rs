use std::time::Duration;

use crate::commands::path::{display_path, resolve_workspace_str};

pub async fn test(
    config_path: &str,
    proxy_name: Option<&str>,
    group_name: Option<&str>,
    url: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let resolved_config_path = resolve_workspace_str(config_path);
    let resolved_config_path = display_path(&resolved_config_path);
    let report = titan_core::testing::run_proxy_tests(
        resolved_config_path.as_str(),
        proxy_name,
        group_name,
        url,
        timeout,
    )
    .await?;

    println!("\n========== Proxy Test ==========\n");
    println!("Config: {}", resolved_config_path);
    println!("Test URL: {}", report.test_url);
    println!("Timeout: {} ms", report.timeout_ms);
    println!("Mode: {}", report.mode_label);
    println!("Scope: {}", report.scope_label);
    println!("Resolved targets: {}", report.items.len());

    println!("\n--- Results ---");
    for item in &report.items {
        let resolved = item
            .resolved_name
            .as_ref()
            .map(|name| format!(" -> {name}"))
            .unwrap_or_default();
        match item.status {
            titan_core::ProxyTestStatus::Ok => {
                println!(
                    "[ok] {}{} {}",
                    item.requested_name,
                    resolved,
                    item.message.as_deref().unwrap_or_default()
                );
            }
            titan_core::ProxyTestStatus::Failed => {
                println!(
                    "[fail] {}{} {}",
                    item.requested_name,
                    resolved,
                    item.message.as_deref().unwrap_or_default()
                );
            }
            titan_core::ProxyTestStatus::Skipped => {
                println!(
                    "[skip] {}{} {}",
                    item.requested_name,
                    resolved,
                    item.message.as_deref().unwrap_or_default()
                );
            }
        }
    }

    println!("\n--- Summary ---");
    println!("{:<8} {}", "ok", report.ok);
    println!("{:<8} {}", "failed", report.failed);
    println!("{:<8} {}", "skipped", report.skipped);
    println!("Elapsed  : {} ms", report.elapsed_ms);
    Ok(())
}
