use crate::{MatchRequest, RuleEngine};

#[test]
fn test_domain_rule() {
    let config = titan_config::Config {
        rules: vec![
            "DOMAIN-SUFFIX,google.com,DIRECT".to_string(),
            "MATCH,PROXY".to_string(),
        ],
        ..Default::default()
    };

    let engine = RuleEngine::new(&config).unwrap();

    let req = MatchRequest {
        host: Some("www.google.com".to_string()),
        ip: None,
        port: 443,
        process_name: None,
        process_path: None,
    };

    let result = engine.match_request(&req);
    assert_eq!(result.policy, "DIRECT");
}

#[test]
fn test_ip_cidr_rule() {
    let config = titan_config::Config {
        rules: vec![
            "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve".to_string(),
            "MATCH,PROXY".to_string(),
        ],
        ..Default::default()
    };

    let engine = RuleEngine::new(&config).unwrap();

    let req = MatchRequest {
        host: Some("192.168.1.10".to_string()),
        ip: Some("192.168.1.10".parse().unwrap()),
        port: 443,
        process_name: None,
        process_path: None,
    };

    let result = engine.match_request(&req);
    assert_eq!(result.policy, "DIRECT");
}

#[test]
fn detects_need_for_ip_resolution() {
    let config = titan_config::Config {
        rules: vec!["GEOIP,CN,PROXY".to_string(), "MATCH,DIRECT".to_string()],
        ..Default::default()
    };

    let engine = RuleEngine::new(&config).unwrap();
    assert!(engine.requires_ip_resolution());
}

#[test]
fn geosite_rule_does_not_require_ip_resolution() {
    let config = titan_config::Config {
        rules: vec!["GEOSITE,google,PROXY".to_string(), "MATCH,DIRECT".to_string()],
        ..Default::default()
    };

    let engine = RuleEngine::new(&config).unwrap();
    assert!(!engine.requires_ip_resolution());
}
