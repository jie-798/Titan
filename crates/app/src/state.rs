use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Default)]
pub enum RuntimeStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed(String),
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeState {
    pub status: RuntimeStatus,
    pub config_path: Option<String>,
    pub bind_addr: Option<SocketAddr>,
    pub api_addr: Option<SocketAddr>,
    pub system_proxy_enabled: bool,
}

struct RuntimeHandle {
    engine: Arc<titan_core::ProxyEngine>,
    inbound_task: JoinHandle<anyhow::Result<()>>,
    api_task: Option<JoinHandle<anyhow::Result<()>>>,
    system_proxy: Option<titan_system_proxy::SystemProxy>,
    subscription_task: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct RuntimeInner {
    state: RwLock<RuntimeState>,
    handle: Mutex<Option<RuntimeHandle>>,
}

#[derive(Clone, Default)]
pub struct RuntimeController {
    inner: Arc<RuntimeInner>,
}

impl RuntimeController {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn snapshot(&self) -> RuntimeState {
        self.inner.state.read().await.clone()
    }

    pub async fn set_starting(&self, config_path: String, bind_addr: SocketAddr) {
        let mut state = self.inner.state.write().await;
        state.status = RuntimeStatus::Starting;
        state.config_path = Some(config_path);
        state.bind_addr = Some(bind_addr);
    }

    pub async fn set_running(&self, api_addr: Option<SocketAddr>, system_proxy_enabled: bool) {
        let mut state = self.inner.state.write().await;
        state.status = RuntimeStatus::Running;
        state.api_addr = api_addr;
        state.system_proxy_enabled = system_proxy_enabled;
    }

    pub async fn set_stopping(&self) {
        let mut state = self.inner.state.write().await;
        state.status = RuntimeStatus::Stopping;
    }

    pub async fn set_stopped(&self) {
        let mut state = self.inner.state.write().await;
        *state = RuntimeState::default();
    }

    pub async fn set_failed(&self, message: String) {
        let mut state = self.inner.state.write().await;
        state.status = RuntimeStatus::Failed(message);
    }

    pub async fn set_handle(
        &self,
        engine: Arc<titan_core::ProxyEngine>,
        inbound_task: JoinHandle<anyhow::Result<()>>,
        api_task: Option<JoinHandle<anyhow::Result<()>>>,
        system_proxy: Option<titan_system_proxy::SystemProxy>,
        subscription_task: Option<JoinHandle<()>>,
    ) {
        let mut handle = self.inner.handle.lock().await;
        *handle = Some(RuntimeHandle {
            engine,
            inbound_task,
            api_task,
            system_proxy,
            subscription_task,
        });
    }

    pub async fn engine(&self) -> Option<Arc<titan_core::ProxyEngine>> {
        let handle = self.inner.handle.lock().await;
        handle.as_ref().map(|runtime| runtime.engine.clone())
    }

    pub async fn take_handle(&self) -> Option<(
        Arc<titan_core::ProxyEngine>,
        JoinHandle<anyhow::Result<()>>,
        Option<JoinHandle<anyhow::Result<()>>>,
        Option<titan_system_proxy::SystemProxy>,
        Option<JoinHandle<()>>,
    )> {
        let mut handle = self.inner.handle.lock().await;
        handle.take().map(|runtime| {
            (
                runtime.engine,
                runtime.inbound_task,
                runtime.api_task,
                runtime.system_proxy,
                runtime.subscription_task,
            )
        })
    }
}
