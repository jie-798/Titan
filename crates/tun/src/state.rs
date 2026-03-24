use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::config::{TunConfig, TunStack, TunSummary};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TunState {
    Down,
    Up,
}

impl Default for TunState {
    fn default() -> Self {
        Self::Down
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunOperation {
    pub requested_at_unix: u64,
    pub state: TunState,
    pub config: TunSummary,
    pub message: String,
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunStatus {
    pub state: TunState,
    pub config_path: Option<PathBuf>,
    pub config: Option<TunConfig>,
    pub last_operation: Option<TunOperation>,
    pub last_error: Option<String>,
    pub last_changed_unix: Option<u64>,
    #[serde(default)]
    pub adapter_ready: bool,
    #[serde(default)]
    pub driver_version: Option<u32>,
}

impl Default for TunStatus {
    fn default() -> Self {
        Self {
            state: TunState::Down,
            config_path: None,
            config: None,
            last_operation: None,
            last_error: None,
            last_changed_unix: None,
            adapter_ready: false,
            driver_version: None,
        }
    }
}

impl TunStatus {
    pub fn mark_up(
        &mut self,
        path: Option<PathBuf>,
        config: TunConfig,
        message: String,
        applied: bool,
        commands: Vec<String>,
        adapter_ready: bool,
        driver_version: Option<u32>,
    ) {
        let summary = config.summary();
        self.state = TunState::Up;
        self.config_path = path;
        self.config = Some(config);
        self.last_error = None;
        self.last_changed_unix = Some(now_unix());
        self.adapter_ready = adapter_ready;
        self.driver_version = driver_version;
        self.last_operation = Some(TunOperation {
            requested_at_unix: now_unix(),
            state: TunState::Up,
            config: summary,
            message,
            applied,
            commands,
        });
    }

    pub fn mark_down(&mut self, message: String) {
        self.state = TunState::Down;
        self.config = None;
        self.last_changed_unix = Some(now_unix());
        self.adapter_ready = false;
        self.driver_version = None;
        self.last_operation = Some(TunOperation {
            requested_at_unix: now_unix(),
            state: TunState::Down,
            config: TunSummary {
                stack: TunStack::Mixed,
                adapter_name: "TitanTUN".to_string(),
                interface_name: None,
                address_count: 0,
                dns_count: 0,
                dns_hijack_count: 0,
                exclude_route_count: 0,
                route_count: 0,
                route_all_traffic: false,
                strict_route: false,
                mtu: None,
            },
            message,
            applied: false,
            commands: Vec::new(),
        });
    }

    pub fn mark_preview(&mut self, path: Option<PathBuf>, config: &TunConfig, message: String) {
        self.config_path = path;
        self.last_error = None;
        self.last_changed_unix = Some(now_unix());
        self.last_operation = Some(TunOperation {
            requested_at_unix: now_unix(),
            state: self.state.clone(),
            config: config.summary(),
            message,
            applied: false,
            commands: Vec::new(),
        });
    }

    pub fn mark_error(&mut self, message: String) {
        self.last_error = Some(message);
        self.last_changed_unix = Some(now_unix());
        self.adapter_ready = false;
    }

    pub fn describe(&self) -> String {
        match self.state {
            TunState::Up => "Up".to_string(),
            TunState::Down => "Down".to_string(),
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
