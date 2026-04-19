use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub telegram: TelegramConfig,
    pub codex: CodexConfig,
    pub storage: StorageConfig,
    pub policy: PolicyConfig,
    pub workspaces: Vec<WorkspaceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub run_mode: RunMode,
    pub poll_timeout_sec: u64,
    pub shutdown_grace_sec: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    Console,
    Service,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    pub token_secret_ref: String,
    pub allowed_chat_types: Vec<String>,
    pub admin_sender_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexConfig {
    pub binary: String,
    pub model: String,
    pub sandbox: String,
    pub approval: String,
    pub profile: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub state_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PolicyConfig {
    pub default_mode: LaneMode,
    pub progress_edit_interval_ms: u64,
    pub max_output_chars: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceConfig {
    pub id: String,
    pub path: PathBuf,
    pub writable_roots: Vec<PathBuf>,
    pub default_mode: LaneMode,
    pub continue_prompt: String,
    pub checks_profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaneMode {
    AwaitReply,
    Infinite,
    CompletionChecks,
    MaxTurns,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let config: Self = toml::from_str(&raw).context("failed to parse bridge.toml")?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.telegram.admin_sender_ids.is_empty() {
            bail!("telegram.admin_sender_ids must not be empty");
        }
        if self.workspaces.is_empty() {
            bail!("workspaces must not be empty");
        }
        Ok(())
    }

    pub fn default_workspace(&self) -> &WorkspaceConfig {
        &self.workspaces[0]
    }
}
