use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyTestStatus {
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestItem {
    pub requested_name: String,
    pub resolved_name: Option<String>,
    pub status: ProxyTestStatus,
    pub latency_ms: Option<u128>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestReport {
    pub mode_label: String,
    pub scope_label: String,
    pub test_url: String,
    pub timeout_ms: u64,
    pub elapsed_ms: u128,
    pub ok: usize,
    pub failed: usize,
    pub skipped: usize,
    pub items: Vec<ProxyTestItem>,
}

pub async fn run_proxy_tests(
    config_path: &str,
    proxy_name: Option<&str>,
    group_name: Option<&str>,
    url: &str,
    timeout: Duration,
) -> anyhow::Result<ProxyTestReport> {
    info!("loading config from {}", config_path);
    let config = titan_config::load_config(config_path)?;
    let manager = Arc::new(crate::outbound::OutboundManager::from_config(&config).await?);

    let (mode_label, scope_label, targets) = if let Some(proxy_name) = proxy_name {
        (
            "single proxy",
            proxy_name.to_string(),
            vec![proxy_name.to_string()],
        )
    } else if let Some(group_name) = group_name {
        (
            "group",
            group_name.to_string(),
            collect_group_proxies(&config, group_name)?,
        )
    } else {
        (
            "all proxies",
            format!("{} configured proxies", config.proxies.len()),
            config.proxies.iter().map(|proxy| proxy.name.clone()).collect(),
        )
    };

    if targets.is_empty() {
        anyhow::bail!("no test targets were resolved");
    }

    let started_at = Instant::now();
    let futures = targets.into_iter().map(|target_name| {
        let manager = Arc::clone(&manager);
        let url = url.to_string();
        async move {
            let Some((resolved_name, proxy)) = manager.resolve(&target_name, &HashSet::new()) else {
                return ProxyTestItem {
                    requested_name: target_name,
                    resolved_name: None,
                    status: ProxyTestStatus::Skipped,
                    latency_ms: None,
                    message: Some("unsupported or unresolved".to_string()),
                };
            };

            match proxy.delay_test(&url, timeout).await {
                Ok(latency) => ProxyTestItem {
                    requested_name: target_name,
                    resolved_name: Some(resolved_name),
                    status: ProxyTestStatus::Ok,
                    latency_ms: Some(latency.as_millis()),
                    message: Some(format!("ok {} ms", latency.as_millis())),
                },
                Err(err) => ProxyTestItem {
                    requested_name: target_name,
                    resolved_name: Some(resolved_name),
                    status: ProxyTestStatus::Failed,
                    latency_ms: None,
                    message: Some(err.to_string()),
                },
            }
        }
    });

    let items = join_all(futures).await;
    let (ok, failed, skipped) = items.iter().fold((0, 0, 0), |mut acc, item| {
        match item.status {
            ProxyTestStatus::Ok => acc.0 += 1,
            ProxyTestStatus::Failed => acc.1 += 1,
            ProxyTestStatus::Skipped => acc.2 += 1,
        }
        acc
    });

    Ok(ProxyTestReport {
        mode_label: mode_label.to_string(),
        scope_label,
        test_url: url.to_string(),
        timeout_ms: timeout.as_millis() as u64,
        elapsed_ms: started_at.elapsed().as_millis(),
        ok,
        failed,
        skipped,
        items,
    })
}

fn collect_group_proxies(
    config: &titan_config::Config,
    group_name: &str,
) -> anyhow::Result<Vec<String>> {
    let mut visited_groups = HashSet::new();
    let mut resolved = BTreeSet::new();
    collect_group_proxies_inner(config, group_name, &mut visited_groups, &mut resolved)?;
    Ok(resolved.into_iter().collect())
}

fn collect_group_proxies_inner(
    config: &titan_config::Config,
    group_name: &str,
    visited_groups: &mut HashSet<String>,
    resolved: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    if !visited_groups.insert(group_name.to_string()) {
        return Ok(());
    }

    let group = config
        .proxy_groups
        .iter()
        .find(|group| group.name == group_name)
        .ok_or_else(|| anyhow::anyhow!("proxy group not found: {}", group_name))?;

    for member in &group.proxies {
        if member.eq_ignore_ascii_case("DIRECT") || member.eq_ignore_ascii_case("REJECT") {
            continue;
        }

        if config.proxies.iter().any(|proxy| proxy.name == *member) {
            resolved.insert(member.clone());
            continue;
        }

        if config.proxy_groups.iter().any(|nested| nested.name == *member) {
            collect_group_proxies_inner(config, member, visited_groups, resolved)?;
        }
    }

    Ok(())
}

pub fn summarize_report(report: &ProxyTestReport) -> BTreeMap<&'static str, usize> {
    let mut summary = BTreeMap::new();
    summary.insert("ok", report.ok);
    summary.insert("failed", report.failed);
    summary.insert("skipped", report.skipped);
    summary
}
