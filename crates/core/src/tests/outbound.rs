use std::collections::HashSet;
use std::time::Duration;

use titan_config::{Config, ProxyConfig, ProxyGroupConfig};

use crate::outbound::OutboundManager;

#[tokio::test]
async fn falls_back_to_next_proxy_when_selected_proxy_is_unhealthy() {
    let config = Config {
        proxies: vec![
            ProxyConfig {
                name: "node-a".to_string(),
                proxy_type: "anytls".to_string(),
                server: "example.com".to_string(),
                port: 443,
                password: Some("secret-a".to_string()),
                ..Default::default()
            },
            ProxyConfig {
                name: "node-b".to_string(),
                proxy_type: "anytls".to_string(),
                server: "example.org".to_string(),
                port: 443,
                password: Some("secret-b".to_string()),
                ..Default::default()
            },
        ],
        proxy_groups: vec![ProxyGroupConfig {
            name: "auto".to_string(),
            group_type: "select".to_string(),
            proxies: vec!["node-a".to_string(), "node-b".to_string()],
            selected: Some("node-a".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let manager = OutboundManager::from_config(&config).await.unwrap();
    manager.mark_unhealthy("node-a", Duration::from_secs(60));

    let (resolved_name, _) = manager.resolve("auto", &HashSet::new()).unwrap();
    assert_eq!(resolved_name, "node-b");
}

#[tokio::test]
async fn excludes_marked_proxy_when_resolving_group() {
    let config = Config {
        proxies: vec![
            ProxyConfig {
                name: "node-a".to_string(),
                proxy_type: "anytls".to_string(),
                server: "example.com".to_string(),
                port: 443,
                password: Some("secret-a".to_string()),
                ..Default::default()
            },
            ProxyConfig {
                name: "node-b".to_string(),
                proxy_type: "anytls".to_string(),
                server: "example.org".to_string(),
                port: 443,
                password: Some("secret-b".to_string()),
                ..Default::default()
            },
        ],
        proxy_groups: vec![ProxyGroupConfig {
            name: "auto".to_string(),
            group_type: "select".to_string(),
            proxies: vec!["node-a".to_string(), "node-b".to_string()],
            selected: Some("node-a".to_string()),
            ..Default::default()
        }],
        ..Default::default()
    };

    let manager = OutboundManager::from_config(&config).await.unwrap();
    let excluded = HashSet::from(["node-a".to_string()]);

    let (resolved_name, _) = manager.resolve("auto", &excluded).unwrap();
    assert_eq!(resolved_name, "node-b");
}
