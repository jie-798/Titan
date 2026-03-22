use std::net::IpAddr;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{debug, info};

use titan_config::Config;
use titan_dns::DnsResolver;
use titan_protocols::OutboundProxy;
use titan_rules::RuleEngine;

use crate::outbound::{OutboundManager, ProxyGroupState};
use crate::session::SessionManager;

/// Main runtime proxy engine.
pub struct ProxyEngine {
    config: Arc<RwLock<Config>>,
    config_path: Arc<RwLock<Option<String>>>,
    session_manager: Arc<SessionManager>,
    outbound_manager: Arc<RwLock<Arc<OutboundManager>>>,
    rule_engine: Arc<RwLock<Arc<RuleEngine>>>,
    dns_resolver: Arc<RwLock<Arc<DnsResolver>>>,
    running: Arc<RwLock<bool>>,
}

impl ProxyEngine {
    /// Creates a new proxy engine from config.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        Self::new_with_path(config, None).await
    }

    /// Creates a new proxy engine from config and remembers the source path.
    pub async fn new_with_path(
        config: Config,
        config_path: Option<String>,
    ) -> anyhow::Result<Self> {
        info!("initializing proxy engine");

        let rule_engine = Arc::new(RuleEngine::new(&config)?);
        let dns_resolver = Arc::new(DnsResolver::new(&config.dns)?);
        let outbound_manager = Arc::new(OutboundManager::from_config(&config).await?);

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path: Arc::new(RwLock::new(config_path)),
            session_manager: Arc::new(SessionManager::new()),
            outbound_manager: Arc::new(RwLock::new(outbound_manager)),
            rule_engine: Arc::new(RwLock::new(rule_engine)),
            dns_resolver: Arc::new(RwLock::new(dns_resolver)),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Marks the engine as started.
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Err(anyhow::anyhow!("engine is already running"));
        }

        info!("starting proxy engine");
        *running = true;
        Ok(())
    }

    /// Stops the engine and closes all sessions.
    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Ok(());
        }

        info!("stopping proxy engine");
        *running = false;
        self.session_manager.close_all().await;
        Ok(())
    }

    /// Replaces the in-memory config and rebuilds runtime components.
    pub async fn reload(&self, config: Config) -> anyhow::Result<()> {
        info!("reloading config");

        let rule_engine = Arc::new(RuleEngine::new(&config)?);
        let dns_resolver = Arc::new(DnsResolver::new(&config.dns)?);
        let outbound_manager = Arc::new(OutboundManager::from_config(&config).await?);

        *self.config.write().await = config;
        *self.rule_engine.write().await = rule_engine;
        *self.dns_resolver.write().await = dns_resolver;
        *self.outbound_manager.write().await = outbound_manager;

        Ok(())
    }

    /// Reloads from the remembered config file path.
    pub async fn reload_from_disk(&self) -> anyhow::Result<()> {
        let path = self
            .config_path
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow::anyhow!("engine does not know a config file path"))?;

        let config = titan_config::load_config(&path)?;
        self.reload(config).await
    }

    /// Updates the remembered config file path.
    pub async fn set_config_path(&self, path: Option<String>) {
        *self.config_path.write().await = path;
    }

    /// Returns the remembered config file path.
    pub async fn config_path(&self) -> Option<String> {
        self.config_path.read().await.clone()
    }

    /// Selects the routing policy for a target.
    pub async fn select_proxy(&self, target: &crate::Target) -> Option<String> {
        let request = titan_rules::MatchRequest {
            host: Some(target.host.clone()),
            ip: self.resolve_target_ip(target).await,
            port: target.port,
            process_name: None,
            process_path: None,
        };

        let rule_engine = self.rule_engine.read().await.clone();
        let result = rule_engine.match_request(&request);
        Some(result.policy)
    }

    async fn resolve_target_ip(&self, target: &crate::Target) -> Option<IpAddr> {
        if let Ok(ip) = target.host.parse::<IpAddr>() {
            return Some(ip);
        }

        let rule_engine = self.rule_engine.read().await.clone();
        if !rule_engine.requires_ip_resolution() {
            return None;
        }

        let dns_resolver = self.dns_resolver.read().await.clone();
        match dns_resolver.resolve_one(&target.host).await {
            Ok(ip) => Some(ip),
            Err(err) => {
                debug!(
                    "failed to resolve {} for rule matching: {}",
                    target.host, err
                );
                None
            }
        }
    }

    /// Gets a usable outbound by name or group.
    pub async fn get_outbound(&self, name: &str) -> Option<Arc<dyn OutboundProxy>> {
        let outbound_manager = self.outbound_manager.read().await.clone();
        outbound_manager.get(name)
    }

    pub async fn resolve_outbound(
        &self,
        name: &str,
        excluded: &HashSet<String>,
    ) -> Option<(String, Arc<dyn OutboundProxy>)> {
        let outbound_manager = self.outbound_manager.read().await.clone();
        outbound_manager.resolve(name, excluded)
    }

    pub async fn mark_outbound_unhealthy(&self, name: &str, duration: Duration) {
        let outbound_manager = self.outbound_manager.read().await.clone();
        outbound_manager.mark_unhealthy(name, duration);
    }

    pub async fn unhealthy_outbounds(&self) -> Vec<String> {
        let outbound_manager = self.outbound_manager.read().await.clone();
        outbound_manager.unhealthy_proxies()
    }

    pub async fn outbound_group_states(&self) -> Vec<ProxyGroupState> {
        let outbound_manager = self.outbound_manager.read().await.clone();
        outbound_manager.group_states()
    }

    /// Returns a copy of the current config.
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Returns whether the engine is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Returns the session manager.
    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }
}
