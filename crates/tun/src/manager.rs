use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::config::TunConfig;
use crate::state::TunStatus;

#[derive(Debug)]
pub struct TunManager {
    state_path: PathBuf,
    status: RwLock<TunStatus>,
}

impl TunManager {
    pub fn new(state_path: impl AsRef<Path>) -> Self {
        let state_path = state_path.as_ref().to_path_buf();
        let status = load_status(&state_path).unwrap_or_default();
        Self {
            state_path,
            status: RwLock::new(status),
        }
    }

    pub fn status(&self) -> TunStatus {
        self.status
            .read()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub fn plan(&self, config: &TunConfig) -> anyhow::Result<crate::windows::WindowsTunPlan> {
        config.validate()?;
        Ok(crate::windows::build_plan(config)?)
    }

    pub fn activate(&self, config: &TunConfig) -> anyhow::Result<crate::windows::ActiveWindowsTun> {
        let plan = self.plan(config)?;
        crate::windows::activate(&plan)
    }

    pub fn up(&self, config_path: Option<PathBuf>, config: TunConfig) -> anyhow::Result<TunStatus> {
        let plan = self.plan(&config)?;
        let message = format!("prepared {}", plan.describe());

        let mut status = self
            .status
            .write()
            .map_err(|_| anyhow::anyhow!("tun status lock poisoned"))?;
        status.mark_up(config_path, config, message, false, Vec::new(), false, None);
        save_status(&self.state_path, &status)?;
        Ok(status.clone())
    }

    pub fn up_live(
        &self,
        config_path: Option<PathBuf>,
        config: TunConfig,
        active: &crate::windows::ActiveWindowsTun,
    ) -> anyhow::Result<TunStatus> {
        let message = format!(
            "activated adapter {} on {} with {} command(s){}",
            config.adapter_name,
            active.interface_name(),
            active.commands().len(),
            if active.created_adapter() {
                ", adapter created"
            } else {
                ", adapter opened"
            }
        );

        let mut status = self
            .status
            .write()
            .map_err(|_| anyhow::anyhow!("tun status lock poisoned"))?;
        status.mark_up(
            config_path,
            config,
            message,
            true,
            active.commands().to_vec(),
            true,
            active.driver_version(),
        );
        save_status(&self.state_path, &status)?;
        Ok(status.clone())
    }

    pub fn down(&self) -> anyhow::Result<TunStatus> {
        let mut status = self
            .status
            .write()
            .map_err(|_| anyhow::anyhow!("tun status lock poisoned"))?;
        status.mark_down(
            "tun state marked down locally; no adapter teardown was attempted".to_string(),
        );
        save_status(&self.state_path, &status)?;
        Ok(status.clone())
    }
}

fn load_status(path: &Path) -> anyhow::Result<TunStatus> {
    if !path.exists() {
        return Ok(TunStatus::default());
    }

    let content = std::fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&content)?)
}

fn save_status(path: &Path, status: &TunStatus) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_yaml::to_string(status)?;
    std::fs::write(path, content)?;
    Ok(())
}
