use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[path = "checks.rs"]
pub mod checks;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub telegram: TelegramConfig,
    pub codex: CodexConfig,
    pub storage: StorageConfig,
    pub policy: PolicyConfig,
    #[serde(default)]
    pub checks: ChecksConfig,
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
pub struct ChecksConfig {
    #[serde(default)]
    pub profiles: BTreeMap<String, CheckProfile>,
}

impl Default for ChecksConfig {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        profiles.insert("default".to_owned(), CheckProfile::default());
        Self { profiles }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct CheckProfile {
    #[serde(default)]
    pub commands: Vec<CheckCommand>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CheckCommand {
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_check_timeout_sec")]
    pub timeout_sec: u64,
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

fn default_check_timeout_sec() -> u64 {
    300
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
        for workspace in &self.workspaces {
            if !self.checks.profiles.contains_key(&workspace.checks_profile) {
                bail!(
                    "workspace '{}' references unknown checks profile '{}'",
                    workspace.id,
                    workspace.checks_profile
                );
            }
        }
        for (profile_name, profile) in &self.checks.profiles {
            for command in &profile.commands {
                if command.name.trim().is_empty() {
                    bail!(
                        "checks profile '{}' has a command with an empty name",
                        profile_name
                    );
                }
                if command.program.trim().is_empty() {
                    bail!(
                        "checks profile '{}' has a command '{}' with an empty program",
                        profile_name,
                        command.name
                    );
                }
                if command.timeout_sec == 0 {
                    bail!(
                        "checks profile '{}' has a command '{}' with timeout_sec = 0",
                        profile_name,
                        command.name
                    );
                }
            }
        }
        Ok(())
    }

    pub fn default_workspace(&self) -> &WorkspaceConfig {
        &self.workspaces[0]
    }
}
