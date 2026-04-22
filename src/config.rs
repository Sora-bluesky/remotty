use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use reqwest::Url;
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
    #[serde(default = "default_telegram_api_base_url")]
    pub api_base_url: String,
    #[serde(default = "default_telegram_file_base_url")]
    pub file_base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodexConfig {
    pub binary: String,
    pub model: String,
    pub sandbox: String,
    pub approval: String,
    #[serde(default)]
    pub transport: CodexTransport,
    #[serde(default)]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CodexTransport {
    #[default]
    Exec,
    AppServer,
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
    #[serde(default = "default_max_turns_limit")]
    pub max_turns_limit: i64,
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

pub const DEFAULT_MAX_TURNS_BUDGET: i64 = 3;

fn default_check_timeout_sec() -> u64 {
    300
}

fn default_max_turns_limit() -> i64 {
    DEFAULT_MAX_TURNS_BUDGET
}

fn default_telegram_api_base_url() -> String {
    "https://api.telegram.org".to_owned()
}

fn default_telegram_file_base_url() -> String {
    "https://api.telegram.org/file".to_owned()
}

fn resolve_relative_path(base_dir: &Path, value: &Path) -> PathBuf {
    if value.is_absolute() {
        value.to_path_buf()
    } else {
        base_dir.join(value)
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let mut config: Self = toml::from_str(&raw).context("failed to parse bridge.toml")?;
        config.resolve_relative_paths(path);
        config.validate()?;
        Ok(config)
    }

    fn resolve_relative_paths(&mut self, path: &Path) {
        let Some(base_dir) = path.parent() else {
            return;
        };
        self.storage.db_path = resolve_relative_path(base_dir, &self.storage.db_path);
        self.storage.state_dir = resolve_relative_path(base_dir, &self.storage.state_dir);
        self.storage.temp_dir = resolve_relative_path(base_dir, &self.storage.temp_dir);
        self.storage.log_dir = resolve_relative_path(base_dir, &self.storage.log_dir);
    }

    fn validate(&self) -> Result<()> {
        if self.telegram.api_base_url.trim().is_empty() {
            bail!("telegram.api_base_url must not be empty");
        }
        if self.telegram.file_base_url.trim().is_empty() {
            bail!("telegram.file_base_url must not be empty");
        }
        validate_telegram_api_base_url(&self.telegram.api_base_url)?;
        validate_telegram_file_base_url(&self.telegram.file_base_url)?;
        if let Some(profile) = self.codex.profile.as_deref() {
            if profile.trim().is_empty() {
                bail!("codex.profile must not be blank");
            }
        }
        if self.workspaces.is_empty() {
            bail!("workspaces must not be empty");
        }
        let mut seen_workspace_ids = std::collections::BTreeSet::new();
        for workspace in &self.workspaces {
            if workspace.id.trim().is_empty() {
                bail!("workspace id must not be empty");
            }
            if !seen_workspace_ids.insert(workspace.id.clone()) {
                bail!("duplicate workspace id '{}'", workspace.id);
            }
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
        if self.policy.max_turns_limit <= 0 {
            bail!("policy.max_turns_limit must be greater than zero");
        }
        Ok(())
    }

    pub fn default_workspace(&self) -> &WorkspaceConfig {
        &self.workspaces[0]
    }

    pub fn workspace(&self, workspace_id: &str) -> Option<&WorkspaceConfig> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.id == workspace_id)
    }
}

fn validate_telegram_api_base_url(value: &str) -> Result<()> {
    let field_name = "telegram.api_base_url";
    let url = Url::parse(value).with_context(|| format!("{field_name} must be a valid URL"))?;
    let Some(host) = url.host_str() else {
        bail!("{field_name} must include a host");
    };
    let scheme = url.scheme();
    let is_official = scheme == "https"
        && host.eq_ignore_ascii_case("api.telegram.org")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/")
        && url.query().is_none()
        && url.fragment().is_none();
    let is_local = matches!(host, "localhost" | "127.0.0.1" | "::1")
        && matches!(scheme, "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/")
        && url.query().is_none()
        && url.fragment().is_none();
    if !is_official && !is_local {
        bail!("{field_name} must point to `https://api.telegram.org` or a localhost test server");
    }
    Ok(())
}

fn validate_telegram_file_base_url(value: &str) -> Result<()> {
    let field_name = "telegram.file_base_url";
    let url = Url::parse(value).with_context(|| format!("{field_name} must be a valid URL"))?;
    let Some(host) = url.host_str() else {
        bail!("{field_name} must include a host");
    };
    let scheme = url.scheme();
    let is_official = scheme == "https"
        && host.eq_ignore_ascii_case("api.telegram.org")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "/file" | "/file/")
        && url.query().is_none()
        && url.fragment().is_none();
    let is_local = matches!(host, "localhost" | "127.0.0.1" | "::1")
        && matches!(scheme, "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "/file" | "/file/")
        && url.query().is_none()
        && url.fragment().is_none();
    if !is_official && !is_local {
        bail!("{field_name} must point to `https://api.telegram.org` or a localhost test server");
    }
    Ok(())
}
