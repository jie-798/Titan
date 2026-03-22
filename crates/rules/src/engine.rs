use std::net::IpAddr;

use titan_config::Config;
use tracing::warn;

use crate::geoip::{GeoIpMatcher, DEFAULT_GEOIP_DB_PATH};
use crate::geosite::{GeositeMatcher, DEFAULT_GEOSITE_DIR};
use crate::ipcidr::match_cidr;

/// Match input for the rule engine.
#[derive(Debug, Clone)]
pub struct MatchRequest {
    pub host: Option<String>,
    pub ip: Option<IpAddr>,
    pub port: u16,
    pub process_name: Option<String>,
    pub process_path: Option<String>,
}

/// Result returned by the rule engine.
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub rule_index: usize,
    pub rule_str: String,
    pub policy: String,
}

/// Rule engine for Clash-style rules.
pub struct RuleEngine {
    rules: Vec<String>,
    default_policy: String,
    requires_ip_resolution: bool,
    geoip_db: Option<GeoIpMatcher>,
    geosite_db: Option<GeositeMatcher>,
}

impl RuleEngine {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let rules = config.rules.clone();

        let default_policy = rules
            .iter()
            .find(|r| r.starts_with("MATCH,") || r.starts_with("FINAL,"))
            .map(|r| {
                let parts: Vec<&str> = r.split(',').collect();
                if parts.len() >= 2 {
                    parts[1].to_string()
                } else {
                    "DIRECT".to_string()
                }
            })
            .unwrap_or_else(|| "DIRECT".to_string());

        let requires_ip_resolution = rules.iter().any(|rule| {
            let parts: Vec<&str> = rule.split(',').collect();
            matches!(
                parts.first().copied(),
                Some("IP-CIDR" | "IP-CIDR6" | "GEOIP")
            ) && !parts.get(3).is_some_and(|value| *value == "no-resolve")
        });

        let needs_geoip = rules.iter().any(|rule| rule.starts_with("GEOIP,"));
        let geoip_db = if needs_geoip {
            match GeoIpMatcher::from_path(DEFAULT_GEOIP_DB_PATH) {
                Ok(db) => Some(db),
                Err(err) => {
                    warn!(
                        "failed to load GEOIP database from {}: {}",
                        DEFAULT_GEOIP_DB_PATH, err
                    );
                    None
                }
            }
        } else {
            None
        };

        let needs_geosite = rules.iter().any(|rule| rule.starts_with("GEOSITE,"));
        let geosite_db = if needs_geosite {
            Some(GeositeMatcher::new(DEFAULT_GEOSITE_DIR))
        } else {
            None
        };

        Ok(Self {
            rules,
            default_policy,
            requires_ip_resolution,
            geoip_db,
            geosite_db,
        })
    }

    /// Matches one request against the configured rule list.
    pub fn match_request(&self, req: &MatchRequest) -> MatchResult {
        for (index, rule) in self.rules.iter().enumerate() {
            if let Some(policy) = self.matches_rule(rule, req) {
                return MatchResult {
                    rule_index: index,
                    rule_str: rule.clone(),
                    policy,
                };
            }
        }

        MatchResult {
            rule_index: self.rules.len(),
            rule_str: "MATCH,default".to_string(),
            policy: self.default_policy.clone(),
        }
    }

    fn matches_rule(&self, rule: &str, req: &MatchRequest) -> Option<String> {
        let parts: Vec<&str> = rule.split(',').collect();
        if parts.len() < 3 {
            return None;
        }

        let rule_type = parts[0];
        let value = parts[1];
        let policy = parts[2].to_string();

        match rule_type {
            "DOMAIN" => {
                if let Some(host) = &req.host {
                    if host.eq_ignore_ascii_case(value) {
                        return Some(policy);
                    }
                }
            }
            "DOMAIN-SUFFIX" => {
                if let Some(host) = &req.host {
                    let host_lower = host.to_lowercase();
                    let value_lower = value.to_lowercase();
                    if host_lower == value_lower
                        || host_lower.ends_with(&format!(".{}", value_lower))
                    {
                        return Some(policy);
                    }
                }
            }
            "DOMAIN-KEYWORD" => {
                if let Some(host) = &req.host {
                    if host.to_lowercase().contains(&value.to_lowercase()) {
                        return Some(policy);
                    }
                }
            }
            "IP-CIDR" | "IP-CIDR6" => {
                if let Some(ip) = req.ip.as_ref() {
                    if match_cidr(ip, value) {
                        return Some(policy);
                    }
                }
            }
            "GEOIP" => {
                if let Some(ip) = req.ip.as_ref() {
                    if self
                        .geoip_db
                        .as_ref()
                        .is_some_and(|db| db.contains_country(ip, value))
                    {
                        return Some(policy);
                    }
                }
            }
            "GEOSITE" => {
                if let Some(host) = req.host.as_ref() {
                    if self
                        .geosite_db
                        .as_ref()
                        .is_some_and(|db| db.matches(value, host))
                    {
                        return Some(policy);
                    }
                }
            }
            "DST-PORT" => {
                if let Ok(port) = value.parse::<u16>() {
                    if req.port == port {
                        return Some(policy);
                    }
                }
            }
            "PROCESS-NAME" => {
                if let Some(name) = &req.process_name {
                    if name.eq_ignore_ascii_case(value) {
                        return Some(policy);
                    }
                }
            }
            "MATCH" | "FINAL" => {
                return Some(policy);
            }
            _ => {}
        }

        None
    }

    /// Total number of configured rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Returns true when hostname-to-IP resolution may be needed for IP rules.
    pub fn requires_ip_resolution(&self) -> bool {
        self.requires_ip_resolution
    }
}
