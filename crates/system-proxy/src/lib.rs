#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub host: String,
    pub port: u16,
    pub bypass: Vec<String>,
}

impl ProxyConfig {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            bypass: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "<local>".to_string(),
            ],
        }
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub struct SystemProxy {
    original_state: Option<ProxyState>,
}

#[derive(Debug, Clone)]
struct ProxyState {
    enabled: bool,
    server: String,
    bypass: String,
}

impl SystemProxy {
    pub fn new() -> Self {
        Self {
            original_state: None,
        }
    }

    pub fn set(&mut self, config: &ProxyConfig) -> anyhow::Result<()> {
        self.original_state = Some(self.get_current_state()?);
        tracing::info!("setting system proxy: {}", config.addr());
        self.apply_proxy(config)
    }

    pub fn restore(&mut self) -> anyhow::Result<()> {
        if let Some(state) = self.original_state.take() {
            tracing::info!("restoring system proxy settings");
            self.restore_state(&state)?;
        }
        Ok(())
    }

    pub fn disable(&mut self) -> anyhow::Result<()> {
        self.original_state = None;
        tracing::info!("disabling system proxy");
        self.disable_current_proxy()
    }

    pub fn current_state(&self) -> anyhow::Result<(bool, String, String)> {
        let state = self.get_current_state()?;
        Ok((state.enabled, state.server, state.bypass))
    }

    pub fn is_set(&self) -> bool {
        self.original_state.is_some()
    }
}

impl Drop for SystemProxy {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

impl Default for SystemProxy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl SystemProxy {
    fn get_current_state(&self) -> anyhow::Result<ProxyState> {
        windows::get_current_state()
    }

    fn apply_proxy(&self, config: &ProxyConfig) -> anyhow::Result<()> {
        windows::apply_proxy(config)
    }

    fn restore_state(&self, state: &ProxyState) -> anyhow::Result<()> {
        windows::restore_state(state)
    }

    fn disable_current_proxy(&self) -> anyhow::Result<()> {
        windows::disable_proxy()
    }
}

#[cfg(not(target_os = "windows"))]
impl SystemProxy {
    fn get_current_state(&self) -> anyhow::Result<ProxyState> {
        anyhow::bail!("system proxy is not implemented on this platform yet")
    }

    fn apply_proxy(&self, _config: &ProxyConfig) -> anyhow::Result<()> {
        anyhow::bail!("system proxy is not implemented on this platform yet")
    }

    fn restore_state(&self, _state: &ProxyState) -> anyhow::Result<()> {
        anyhow::bail!("system proxy is not implemented on this platform yet")
    }

    fn disable_current_proxy(&self) -> anyhow::Result<()> {
        anyhow::bail!("system proxy is not implemented on this platform yet")
    }
}
