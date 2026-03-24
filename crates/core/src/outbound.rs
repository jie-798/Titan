use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use futures::stream::{FuturesUnordered, StreamExt};

use titan_config::{Config, ProxyGroupConfig};
use titan_protocols::{DirectProxy, OutboundProxy, RejectProxy};

#[derive(Debug, Clone)]
pub struct ProxyGroup {
    pub name: String,
    pub group_type: String,
    pub proxies: Vec<String>,
    pub selected: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProxyGroupState {
    pub name: String,
    pub group_type: String,
    pub selected: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProxyFailureInfo {
    pub name: String,
    pub message: String,
    pub first_failed_unix: u64,
    pub last_failed_unix: u64,
    pub failures: u32,
}

#[derive(Debug, Clone)]
struct ProxyFailureState {
    message: String,
    first_failed_unix: u64,
    last_failed_unix: u64,
    failures: u32,
}

pub struct OutboundManager {
    proxies: DashMap<String, Arc<dyn OutboundProxy>>,
    proxy_protocols: DashMap<String, String>,
    unhealthy_until: DashMap<String, Instant>,
    failure_reasons: DashMap<String, ProxyFailureState>,
    groups: DashMap<String, ProxyGroup>,
    direct: Arc<dyn OutboundProxy>,
    reject: Arc<dyn OutboundProxy>,
}

impl OutboundManager {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        let manager = Self {
            proxies: DashMap::new(),
            proxy_protocols: DashMap::new(),
            unhealthy_until: DashMap::new(),
            failure_reasons: DashMap::new(),
            groups: DashMap::new(),
            direct: Arc::new(DirectProxy::new()),
            reject: Arc::new(RejectProxy::new()),
        };

        for proxy_config in &config.proxies {
            if let Some(proxy) = create_proxy_from_config(proxy_config) {
                manager.proxies.insert(proxy_config.name.clone(), proxy);
                manager.proxy_protocols.insert(
                    proxy_config.name.clone(),
                    proxy_config.proxy_type.to_ascii_lowercase(),
                );
            }
        }

        for group_config in &config.proxy_groups {
            let group = ProxyGroup {
                name: group_config.name.clone(),
                group_type: group_config.group_type.clone(),
                proxies: group_config.proxies.clone(),
                selected: group_config.selected.clone(),
            };
            manager.groups.insert(group_config.name.clone(), group);
        }

        manager.resolve_group_selections(config).await?;

        tracing::info!(
            "loaded {} outbound proxies and {} proxy groups",
            manager.proxies.len(),
            manager.groups.len()
        );
        Ok(manager)
    }

    async fn resolve_group_selections(&self, config: &Config) -> anyhow::Result<()> {
        for group_config in &config.proxy_groups {
            let selected = match group_config.group_type.to_ascii_lowercase().as_str() {
                "select" => self.resolve_select_group(group_config),
                "url-test" => self.resolve_url_test_group(group_config).await,
                "fallback" => self.resolve_fallback_group(group_config).await,
                _ => None,
            };

            if let Some(selected_name) = selected {
                if let Some(mut group) = self.groups.get_mut(&group_config.name) {
                    group.selected = Some(selected_name.clone());
                }
                tracing::info!(
                    "group {} ({}) selected {}",
                    group_config.name,
                    group_config.group_type,
                    selected_name
                );
            } else {
                tracing::warn!(
                    "group {} ({}) did not resolve to any usable proxy",
                    group_config.name,
                    group_config.group_type
                );
            }
        }

        Ok(())
    }

    fn resolve_select_group(&self, group: &ProxyGroupConfig) -> Option<String> {
        if let Some(selected) = group.selected.as_ref() {
            if self.resolve_candidate(selected).is_some() {
                return Some(selected.clone());
            }
        }

        self
            .preferred_group_members(&group.proxies)
            .iter()
            .find(|name| self.resolve_candidate(name).is_some())
            .cloned()
    }

    async fn resolve_url_test_group(&self, group: &ProxyGroupConfig) -> Option<String> {
        let test_url = group
            .url
            .clone()
            .unwrap_or_else(|| "http://www.gstatic.com/generate_204".to_string());
        let timeout = Duration::from_secs(5);
        let mut first_usable: Option<String> = None;
        let candidates = self.preferred_candidates(group);
        let mut tests = FuturesUnordered::new();

        for candidate in &candidates {
            let Some(proxy) = self.resolve_candidate(candidate) else {
                continue;
            };
            if first_usable.is_none() {
                first_usable = Some(candidate.clone());
            }
            let candidate_name = candidate.clone();
            let test_url = test_url.clone();
            let group_name = group.name.clone();
            tests.push(async move {
                match proxy.delay_test(&test_url, timeout).await {
                    Ok(latency) => Some((candidate_name, latency)),
                    Err(err) => {
                        tracing::debug!(
                            "url-test candidate {} in group {} failed: {}",
                            candidate_name,
                            group_name,
                            err
                        );
                        None
                    }
                }
            });
        }

        let mut best: Option<(String, Duration)> = None;
        while let Some(result) = tests.next().await {
            if let Some((name, latency)) = result {
                match &best {
                    Some((_, best_latency)) if latency >= *best_latency => {}
                    _ => best = Some((name, latency)),
                }
            }
        }

        best.map(|(name, _)| name).or(first_usable)
    }

    async fn resolve_fallback_group(&self, group: &ProxyGroupConfig) -> Option<String> {
        let test_url = group
            .url
            .clone()
            .unwrap_or_else(|| "http://www.gstatic.com/generate_204".to_string());
        let timeout = Duration::from_secs(5);
        let mut first_usable: Option<String> = None;
        let candidates = self.preferred_candidates(group);
        let mut tests = FuturesUnordered::new();

        for candidate in &candidates {
            let Some(proxy) = self.resolve_candidate(candidate) else {
                continue;
            };
            if first_usable.is_none() {
                first_usable = Some(candidate.clone());
            }
            let candidate_name = candidate.clone();
            let test_url = test_url.clone();
            let group_name = group.name.clone();
            tests.push(async move {
                match proxy.delay_test(&test_url, timeout).await {
                    Ok(_) => Some(candidate_name),
                    Err(err) => {
                        tracing::debug!(
                            "fallback candidate {} in group {} failed: {}",
                            candidate_name,
                            group_name,
                            err
                        );
                        None
                    }
                }
            });
        }

        while let Some(result) = tests.next().await {
            if let Some(name) = result {
                return Some(name);
            }
        }

        first_usable
    }

    fn resolve_candidate(&self, name: &str) -> Option<Arc<dyn OutboundProxy>> {
        self.get_proxy_recursive(name, &mut HashSet::new())
    }

    pub fn resolve(
        &self,
        name: &str,
        excluded: &HashSet<String>,
    ) -> Option<(String, Arc<dyn OutboundProxy>)> {
        self.get_proxy_recursive_with_name(name, &mut HashSet::new(), excluded)
    }

    pub fn mark_unhealthy(&self, name: &str, duration: Duration) {
        self.mark_unhealthy_with_reason(name, duration, "outbound connection failed".to_string());
    }

    pub fn mark_unhealthy_with_reason(&self, name: &str, duration: Duration, reason: String) {
        if matches!(name, "DIRECT" | "REJECT") {
            return;
        }
        self.unhealthy_until
            .insert(name.to_string(), Instant::now() + duration);
        let now_unix = now_unix();
        self.failure_reasons
            .entry(name.to_string())
            .and_modify(|state| {
                state.message = reason.clone();
                state.last_failed_unix = now_unix;
                state.failures = state.failures.saturating_add(1);
            })
            .or_insert(ProxyFailureState {
                message: reason.clone(),
                first_failed_unix: now_unix,
                last_failed_unix: now_unix,
                failures: 1,
            });
        tracing::warn!(
            "mark proxy {} as unhealthy for {}s: {}",
            name,
            duration.as_secs(),
            reason
        );
    }

    pub fn unhealthy_proxies(&self) -> Vec<String> {
        let now = Instant::now();
        self.unhealthy_until
            .iter()
            .filter_map(|entry| {
                if *entry.value() > now {
                    Some(entry.key().clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn group_states(&self) -> Vec<ProxyGroupState> {
        self.groups
            .iter()
            .map(|entry| ProxyGroupState {
                name: entry.value().name.clone(),
                group_type: entry.value().group_type.clone(),
                selected: entry.value().selected.clone(),
            })
            .collect()
    }

    pub fn failure_states(&self) -> Vec<ProxyFailureInfo> {
        self.failure_reasons
            .iter()
            .map(|entry| ProxyFailureInfo {
                name: entry.key().clone(),
                message: entry.value().message.clone(),
                first_failed_unix: entry.value().first_failed_unix,
                last_failed_unix: entry.value().last_failed_unix,
                failures: entry.value().failures,
            })
            .collect()
    }

    fn preferred_candidates(&self, group: &ProxyGroupConfig) -> Vec<String> {
        self.preferred_group_members(&group.proxies)
    }

    fn preferred_group_members(&self, members: &[String]) -> Vec<String> {
        let anytls_candidates: Vec<String> = members
            .iter()
            .filter(|name| self.candidate_contains_protocol(name, "anytls", &mut HashSet::new()))
            .cloned()
            .collect();

        if anytls_candidates.is_empty() {
            members.to_vec()
        } else {
            anytls_candidates
        }
    }

    fn candidate_contains_protocol(
        &self,
        name: &str,
        protocol: &str,
        visited: &mut HashSet<String>,
    ) -> bool {
        if visited.contains(name) {
            return false;
        }
        visited.insert(name.to_string());

        if let Some(proxy_protocol) = self.proxy_protocols.get(name) {
            return proxy_protocol.eq_ignore_ascii_case(protocol);
        }

        let Some(group) = self.groups.get(name) else {
            return false;
        };

        group.proxies.iter().any(|candidate| {
            self.candidate_contains_protocol(candidate, protocol, visited)
        })
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn OutboundProxy>> {
        self.get_proxy_recursive(name, &mut HashSet::new())
    }

    fn get_proxy_recursive(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
    ) -> Option<Arc<dyn OutboundProxy>> {
        self.get_proxy_recursive_with_name(name, visited, &HashSet::new())
            .map(|(_, proxy)| proxy)
    }

    fn get_proxy_recursive_with_name(
        &self,
        name: &str,
        visited: &mut HashSet<String>,
        excluded: &HashSet<String>,
    ) -> Option<(String, Arc<dyn OutboundProxy>)> {
        if visited.contains(name) {
            return None;
        }
        visited.insert(name.to_string());

        match name {
            "DIRECT" => Some(("DIRECT".to_string(), self.direct.clone())),
            "REJECT" => Some(("REJECT".to_string(), self.reject.clone())),
            _ => {
                if let Some(entry) = self.proxies.get(name) {
                    if excluded.contains(name) || !self.is_proxy_available(name) {
                        return None;
                    }
                    return Some((name.to_string(), entry.clone()));
                }

                let group_entry = self.groups.get(name)?;
                let group = group_entry.clone();

                if let Some(selected) = group.selected.as_deref() {
                    if let Some(proxy) =
                        self.get_proxy_recursive_with_name(selected, visited, excluded)
                    {
                        return Some(proxy);
                    }
                }

                let preferred_members = self.preferred_group_members(&group.proxies);
                for proxy_name in &preferred_members {
                    if let Some(proxy) =
                        self.get_proxy_recursive_with_name(proxy_name, visited, excluded)
                    {
                        return Some(proxy);
                    }
                }

                None
            }
        }
    }

    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .proxies
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        names.push("DIRECT".to_string());
        names.push("REJECT".to_string());
        names
    }

    fn is_proxy_available(&self, name: &str) -> bool {
        let now = Instant::now();
        match self.unhealthy_until.get(name) {
            Some(until) if *until.value() > now => false,
            Some(_) => {
                self.unhealthy_until.remove(name);
                true
            }
            None => true,
        }
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn create_proxy_from_config(config: &titan_config::ProxyConfig) -> Option<Arc<dyn OutboundProxy>> {
    match config.proxy_type.to_lowercase().as_str() {
        "anytls" => Some(Arc::new(titan_protocols::AnyTlsProxy::from_config(config))),
        "socks5" | "socks" => Some(Arc::new(titan_protocols::Socks5Proxy::from_config(config))),
        "http" => Some(Arc::new(titan_protocols::HttpProxy::from_config(config))),
        "vless" | "vmess" => {
            tracing::warn!(
                "skip proxy {} because protocol {} is not stable enough for runtime selection yet",
                config.name,
                config.proxy_type
            );
            None
        }
        "trojan" | "hysteria2" | "hy2" | "ss" | "shadowsocks" => {
            tracing::warn!(
                "skip proxy {} because protocol {} is not implemented end-to-end yet",
                config.name,
                config.proxy_type
            );
            None
        }
        _ => {
            tracing::warn!("unsupported proxy type: {}", config.proxy_type);
            None
        }
    }
}
