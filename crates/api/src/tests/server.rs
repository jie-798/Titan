use std::sync::Arc;

use axum::extract::State;

use crate::server::{
    config_summary_for_test, health_for_test, proxy_list_for_test, reload_for_test, ApiState,
};
use titan_core::ProxyEngine;

#[tokio::test]
async fn reports_health_state() {
    let engine = Arc::new(ProxyEngine::new(titan_config::Config::default()).await.unwrap());
    let response = health_for_test(State(ApiState {
        engine: engine.clone(),
    }))
    .await;
    assert!(!response.0.running);

    engine.start().await.unwrap();
    let response = health_for_test(State(ApiState { engine })).await;
    assert!(response.0.running);
}

#[tokio::test]
async fn returns_config_summary() {
    let mut config = titan_config::Config::default();
    config.mode = "rule".to_string();
    config.log_level = "info".to_string();
    config.rules = vec!["MATCH,DIRECT".to_string()];

    let engine = Arc::new(ProxyEngine::new(config).await.unwrap());
    let response = config_summary_for_test(State(ApiState { engine })).await;

    assert_eq!(response.0.mode, "rule");
    assert_eq!(response.0.rule_count, 1);
}

#[tokio::test]
async fn reload_reports_missing_path() {
    let engine = Arc::new(ProxyEngine::new(titan_config::Config::default()).await.unwrap());

    let response = reload_for_test(State(ApiState { engine })).await;
    assert!(!response.0.reloaded);
    assert!(response
        .0
        .message
        .contains("does not know a config file path"));
}

#[tokio::test]
async fn proxy_list_includes_runtime_state() {
    let mut config = titan_config::Config::default();
    config.proxies = vec![titan_config::ProxyConfig {
        name: "node-a".to_string(),
        proxy_type: "anytls".to_string(),
        server: "example.com".to_string(),
        port: 443,
        password: Some("secret".to_string()),
        ..Default::default()
    }];
    config.proxy_groups = vec![titan_config::ProxyGroupConfig {
        name: "auto".to_string(),
        group_type: "select".to_string(),
        proxies: vec!["node-a".to_string()],
        ..Default::default()
    }];

    let engine = Arc::new(ProxyEngine::new(config).await.unwrap());
    engine
        .mark_outbound_unhealthy("node-a", std::time::Duration::from_secs(60))
        .await;

    let response = proxy_list_for_test(State(ApiState { engine })).await;
    assert_eq!(response.0.proxy_groups.len(), 1);
    assert_eq!(
        response.0.proxy_groups[0].runtime_selected.as_deref(),
        Some("node-a")
    );
    assert_eq!(response.0.unhealthy_proxies, vec!["node-a".to_string()]);
}
