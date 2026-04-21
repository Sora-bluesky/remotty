use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension, params};
use tempfile::tempdir;
use tokio::time::sleep;

use crate::config::Config;
use crate::store::Store;
use crate::telegram::{TelegramClient, TelegramPoller};
use crate::windows_secret::load_secret;

const LIVE_WORKSPACE_MARKER: &str = ".remotty-live-smoke-ok";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeScenario {
    ApprovalAccept,
    ApprovalDecline,
}

impl SmokeScenario {
    pub fn label(self) -> &'static str {
        match self {
            Self::ApprovalAccept => "approval-accept",
            Self::ApprovalDecline => "approval-decline",
        }
    }
}

pub async fn run_smoke(config_path: impl AsRef<Path>, scenario: SmokeScenario) -> Result<String> {
    let base_config = Config::load(config_path.as_ref())?;
    let live = LiveApprovalEnv::from_env(&base_config)?;
    if live.approval_mode != "app_server" {
        bail!("set LIVE_APPROVAL_MODE=app_server before running smoke");
    }

    let telegram = TelegramClient::with_base_urls(
        live.bot_token.clone(),
        base_config.telegram.api_base_url.clone(),
        base_config.telegram.file_base_url.clone(),
    );
    let poller = TelegramPoller::acquire(telegram.clone()).await?;
    ensure_polling_mode(&telegram).await?;
    drop(poller);

    let temp = tempdir()?;
    let config_path = write_live_config(temp.path(), &base_config, &live)?;
    let db_path = temp.path().join("state").join("bridge.db");
    let nonce = unique_nonce(match scenario {
        SmokeScenario::ApprovalAccept => "SMOKE_APPROVE",
        SmokeScenario::ApprovalDecline => "SMOKE_DENY",
    });
    let file_name = format!("{nonce}.txt");
    let expected_file = live.workspace.join(&file_name);
    let expected_reply = match scenario {
        SmokeScenario::ApprovalAccept => nonce.clone(),
        SmokeScenario::ApprovalDecline => format!("DENIED_{nonce}"),
    };
    let prompt = build_prompt(scenario, &file_name, &nonce, &expected_reply);

    let _ = fs::remove_file(&expected_file);

    let bridge_exe = bridge_binary()?;
    let mut child = ChildGuard::new(spawn_bridge(&bridge_exe, &config_path, &live.bot_token)?);
    sleep(Duration::from_secs(1)).await;

    telegram
        .send_message(
            live.chat_id,
            &format!(
                "Manual smoke `{}` is waiting.\nSend this exact prompt to the bot:\n{}",
                scenario.label(),
                prompt
            ),
        )
        .await
        .context("failed to send smoke instruction to Telegram")?;

    let deadline = Instant::now() + Duration::from_secs(live.timeout_sec);
    wait_for_inbound(&db_path, live.sender_id, &prompt, deadline, &mut child).await?;
    let request_id = wait_for_pending_approval(&db_path, deadline, &mut child).await?;
    println!(
        "Approval request `{request_id}` is pending. Use Telegram to press `{}`.",
        match scenario {
            SmokeScenario::ApprovalAccept => "承認",
            SmokeScenario::ApprovalDecline => "非承認",
        }
    );
    wait_for_approval_status(
        &db_path,
        &request_id,
        match scenario {
            SmokeScenario::ApprovalAccept => "approved",
            SmokeScenario::ApprovalDecline => "declined",
        },
        deadline,
        &mut child,
    )
    .await?;
    wait_for_outbound(&db_path, &expected_reply, deadline, &mut child).await?;

    match scenario {
        SmokeScenario::ApprovalAccept => {
            if !expected_file.exists() {
                bail!("expected file was not created: {}", expected_file.display());
            }
            let written = fs::read_to_string(&expected_file)
                .with_context(|| format!("failed to read {}", expected_file.display()))?;
            if written.trim() != nonce {
                bail!(
                    "approval accept wrote unexpected file content. expected {}, got {}",
                    nonce,
                    written.trim()
                );
            }
            let _ = fs::remove_file(&expected_file);
        }
        SmokeScenario::ApprovalDecline => {
            if expected_file.exists() {
                bail!(
                    "declined approval still created a file at {}",
                    expected_file.display()
                );
            }
        }
    }

    child.stop();
    Ok(format!(
        "manual smoke `{}` succeeded. approval request `{}` reached `{}` and the workspace check passed.",
        scenario.label(),
        request_id,
        match scenario {
            SmokeScenario::ApprovalAccept => "approved",
            SmokeScenario::ApprovalDecline => "declined",
        }
    ))
}

struct LiveApprovalEnv {
    bot_token: String,
    chat_id: i64,
    sender_id: i64,
    workspace: PathBuf,
    codex_bin: String,
    codex_profile: Option<String>,
    timeout_sec: u64,
    approval_mode: String,
}

impl LiveApprovalEnv {
    fn from_env(base_config: &Config) -> Result<Self> {
        let workspace = resolve_live_workspace()?;
        let sender_id = resolve_live_sender_id(base_config)?;
        Ok(Self {
            bot_token: resolve_live_bot_token(base_config)?,
            chat_id: resolve_live_chat_id(sender_id)?,
            sender_id,
            workspace,
            codex_bin: env::var("LIVE_CODEX_BIN")
                .unwrap_or_else(|_| base_config.codex.binary.clone()),
            codex_profile: env::var("LIVE_CODEX_PROFILE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .or_else(|| base_config.codex.profile.clone()),
            timeout_sec: env::var("LIVE_TIMEOUT_SEC")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(240),
            approval_mode: env::var("LIVE_APPROVAL_MODE")
                .unwrap_or_else(|_| "app_server".to_owned()),
        })
    }
}

fn resolve_live_workspace() -> Result<PathBuf> {
    if let Some(value) = optional_env("LIVE_WORKSPACE") {
        return validated_live_workspace(PathBuf::from(value));
    }

    let workspace = env::current_dir()
        .context("failed to resolve current directory")?
        .join("target")
        .join("live-smoke-workspace");
    fs::create_dir_all(&workspace)
        .with_context(|| format!("failed to create {}", workspace.display()))?;
    let marker = workspace.join(LIVE_WORKSPACE_MARKER);
    if !marker.exists() {
        fs::write(&marker, "ok")
            .with_context(|| format!("failed to write {}", marker.display()))?;
    }
    validated_live_workspace(workspace)
}

fn resolve_live_bot_token(base_config: &Config) -> Result<String> {
    optional_env("LIVE_TELEGRAM_BOT_TOKEN")
        .or_else(|| load_secret(&base_config.telegram.token_secret_ref).ok())
        .ok_or_else(|| {
            anyhow!(
                "missing Telegram bot token. Run `/remotty-configure` or set LIVE_TELEGRAM_BOT_TOKEN."
            )
        })
}

fn resolve_live_sender_id(base_config: &Config) -> Result<i64> {
    if let Some(value) = optional_env("LIVE_TELEGRAM_SENDER_ID") {
        return value
            .parse()
            .context("LIVE_TELEGRAM_SENDER_ID must be an integer");
    }

    infer_single_sender_id(base_config)
}

fn resolve_live_chat_id(sender_id: i64) -> Result<i64> {
    optional_env("LIVE_TELEGRAM_CHAT_ID")
        .map(|value| {
            value
                .parse()
                .context("LIVE_TELEGRAM_CHAT_ID must be an integer")
        })
        .transpose()
        .map(|value| value.unwrap_or(sender_id))
}

fn infer_single_sender_id(base_config: &Config) -> Result<i64> {
    let mut sender_ids = BTreeSet::new();
    sender_ids.extend(base_config.telegram.admin_sender_ids.iter().copied());

    if base_config.storage.db_path.exists() {
        for sender in
            Store::open_read_only(&base_config.storage.db_path)?.list_active_authorized_senders()?
        {
            sender_ids.insert(sender.sender_id);
        }
    }

    match sender_ids.len() {
        1 => Ok(*sender_ids.iter().next().expect("one sender id")),
        0 => bail!("no paired Telegram sender found. Run `/remotty-pair` first."),
        _ => bail!(
            "multiple Telegram senders are allowed. Set LIVE_TELEGRAM_SENDER_ID and LIVE_TELEGRAM_CHAT_ID explicitly."
        ),
    }
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn validated_live_workspace(workspace: PathBuf) -> Result<PathBuf> {
    let workspace = fs::canonicalize(&workspace).with_context(|| {
        format!(
            "LIVE_WORKSPACE must point to an existing directory: {}",
            workspace.display()
        )
    })?;
    if !workspace.is_dir() {
        bail!(
            "LIVE_WORKSPACE must point to a directory, but got {}",
            workspace.display()
        );
    }

    if workspace.parent().is_none() {
        bail!("LIVE_WORKSPACE must not be a drive root. Use a dedicated subdirectory instead.");
    }

    if let Some(user_profile) = env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);
        if let Ok(user_profile) = fs::canonicalize(user_profile) {
            if workspace == user_profile {
                bail!(
                    "LIVE_WORKSPACE must not be your user profile directory. Use a dedicated workspace."
                );
            }
        }
    }

    let marker = workspace.join(LIVE_WORKSPACE_MARKER);
    if !marker.is_file() {
        bail!(
            "LIVE_WORKSPACE must contain `{}` so the manual smoke only writes into an explicit opt-in folder.",
            LIVE_WORKSPACE_MARKER
        );
    }

    Ok(workspace)
}

fn build_prompt(
    scenario: SmokeScenario,
    file_name: &str,
    nonce: &str,
    expected_reply: &str,
) -> String {
    match scenario {
        SmokeScenario::ApprovalAccept => format!(
            "Create a file named {file_name} in the workspace root with the exact text {nonce}. After it succeeds, reply with the exact text {expected_reply} and nothing else."
        ),
        SmokeScenario::ApprovalDecline => format!(
            "Create a file named {file_name} in the workspace root with the exact text {nonce}. If you cannot complete it after approval is denied, reply with the exact text {expected_reply} and nothing else."
        ),
    }
}

fn unique_nonce(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    format!("{}_{}_{}", prefix, std::process::id(), nanos)
}

fn write_live_config(root: &Path, base_config: &Config, live: &LiveApprovalEnv) -> Result<PathBuf> {
    let state_dir = root.join("state");
    let temp_dir = state_dir.join("tmp");
    let log_dir = state_dir.join("logs");
    fs::create_dir_all(&temp_dir)?;
    fs::create_dir_all(&log_dir)?;
    let config_path = root.join("bridge.toml");
    let workspace = base_config.default_workspace();
    let profile_line = live
        .codex_profile
        .as_deref()
        .map(|profile| format!("profile = {}\n", toml_string(profile)))
        .unwrap_or_default();
    let config = format!(
        r#"[service]
run_mode = "console"
poll_timeout_sec = 5
shutdown_grace_sec = {shutdown_grace_sec}

[telegram]
token_secret_ref = "unused-live-smoke"
allowed_chat_types = [{allowed_chat_types}]
admin_sender_ids = [{sender_id}]
api_base_url = {api_base_url}
file_base_url = {file_base_url}

[codex]
binary = {codex_bin}
model = {model}
sandbox = {sandbox}
approval = "on-request"
transport = "app_server"
{profile_line}

[storage]
db_path = {db_path}
state_dir = {state_dir}
temp_dir = {temp_dir}
log_dir = {log_dir}

[policy]
default_mode = {default_mode}
progress_edit_interval_ms = {progress_edit_interval_ms}
max_output_chars = {max_output_chars}
max_turns_limit = {max_turns_limit}

[[workspaces]]
id = "main"
path = {workspace}
writable_roots = [{workspace}]
default_mode = {workspace_mode}
continue_prompt = {continue_prompt}
checks_profile = "default"
"#,
        shutdown_grace_sec = base_config.service.shutdown_grace_sec,
        allowed_chat_types = base_config
            .telegram
            .allowed_chat_types
            .iter()
            .map(|kind| toml_string(kind))
            .collect::<Vec<_>>()
            .join(", "),
        sender_id = live.sender_id,
        api_base_url = toml_string(&base_config.telegram.api_base_url),
        file_base_url = toml_string(&base_config.telegram.file_base_url),
        codex_bin = toml_string(&live.codex_bin),
        model = toml_string(&base_config.codex.model),
        sandbox = toml_string(&base_config.codex.sandbox),
        profile_line = profile_line,
        db_path = toml_string(&state_dir.join("bridge.db").display().to_string()),
        state_dir = toml_string(&state_dir.display().to_string()),
        temp_dir = toml_string(&temp_dir.display().to_string()),
        log_dir = toml_string(&log_dir.display().to_string()),
        default_mode = toml_string(match base_config.policy.default_mode {
            crate::config::LaneMode::AwaitReply => "await_reply",
            crate::config::LaneMode::Infinite => "infinite",
            crate::config::LaneMode::CompletionChecks => "completion_checks",
            crate::config::LaneMode::MaxTurns => "max_turns",
        }),
        progress_edit_interval_ms = base_config.policy.progress_edit_interval_ms,
        max_output_chars = base_config.policy.max_output_chars,
        max_turns_limit = base_config.policy.max_turns_limit,
        workspace = toml_string(&live.workspace.display().to_string()),
        workspace_mode = toml_string(match workspace.default_mode {
            crate::config::LaneMode::AwaitReply => "await_reply",
            crate::config::LaneMode::Infinite => "infinite",
            crate::config::LaneMode::CompletionChecks => "completion_checks",
            crate::config::LaneMode::MaxTurns => "max_turns",
        }),
        continue_prompt = toml_string(&workspace.continue_prompt),
    );
    fs::write(&config_path, config)?;
    Ok(config_path)
}

fn bridge_binary() -> Result<PathBuf> {
    std::env::current_exe().context("failed to resolve current bridge binary")
}

fn spawn_bridge(bridge_exe: &Path, config_path: &Path, bot_token: &str) -> Result<Child> {
    Command::new(bridge_exe)
        .arg("--config")
        .arg(config_path)
        .env("TELEGRAM_BOT_TOKEN", bot_token)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to start bridge {}", bridge_exe.display()))
}

async fn ensure_polling_mode(telegram: &TelegramClient) -> Result<()> {
    let webhook = telegram.get_webhook_info().await?;
    if webhook.url.trim().is_empty() {
        return Ok(());
    }
    bail!(
        "manual smoke requires polling mode, but this bot still has a webhook configured: {}. Remove or restore the webhook outside this command, then rerun the smoke check.",
        webhook.url
    )
}

async fn wait_for_inbound(
    db_path: &Path,
    sender_id: i64,
    inbound_prompt: &str,
    deadline: Instant,
    child: &mut ChildGuard,
) -> Result<()> {
    while Instant::now() < deadline {
        ensure_child_alive(child)?;
        if let Some(found) = find_inbound_text(db_path, sender_id, inbound_prompt)? {
            println!("received inbound text: {found}");
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!(
        "timed out waiting for inbound Telegram message '{}'",
        inbound_prompt
    ))
}

async fn wait_for_pending_approval(
    db_path: &Path,
    deadline: Instant,
    child: &mut ChildGuard,
) -> Result<String> {
    while Instant::now() < deadline {
        ensure_child_alive(child)?;
        if let Some(request_id) = query_optional_text(
            db_path,
            r#"
            SELECT request_id
            FROM approval_requests
            WHERE status = 'pending'
            ORDER BY requested_at_ms DESC
            LIMIT 1
            "#,
            [],
        )? {
            return Ok(request_id);
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!("timed out waiting for pending approval request"))
}

async fn wait_for_approval_status(
    db_path: &Path,
    request_id: &str,
    status: &str,
    deadline: Instant,
    child: &mut ChildGuard,
) -> Result<()> {
    while Instant::now() < deadline {
        ensure_child_alive(child)?;
        let found = query_optional_text(
            db_path,
            r#"
            SELECT status
            FROM approval_requests
            WHERE request_id = ?1
            "#,
            params![request_id],
        )?;
        if found.as_deref() == Some(status) {
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!(
        "timed out waiting for approval request '{}' to become '{}'",
        request_id,
        status
    ))
}

async fn wait_for_outbound(
    db_path: &Path,
    expected_fragment: &str,
    deadline: Instant,
    child: &mut ChildGuard,
) -> Result<()> {
    while Instant::now() < deadline {
        ensure_child_alive(child)?;
        if let Some(found) = find_outbound_text(db_path, expected_fragment)? {
            println!("received outbound text: {found}");
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!(
        "timed out waiting for outbound bridge reply that contains '{}'",
        expected_fragment
    ))
}

fn find_inbound_text(
    db_path: &Path,
    sender_id: i64,
    expected_text: &str,
) -> Result<Option<String>> {
    query_optional_text(
        db_path,
        r#"
        SELECT body_text
        FROM telegram_updates tu
        JOIN messages m ON m.payload_json = tu.payload_json
        WHERE tu.sender_id = ?1
          AND m.direction = 'inbound'
          AND m.body_text = ?2
        ORDER BY tu.update_id DESC
        LIMIT 1
        "#,
        params![sender_id, expected_text],
    )
}

fn find_outbound_text(db_path: &Path, expected_fragment: &str) -> Result<Option<String>> {
    query_optional_text(
        db_path,
        r#"
        SELECT body_text
        FROM messages
        WHERE direction = 'outbound'
          AND message_kind = 'telegram_text'
          AND body_text LIKE ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
        params![format!("%{expected_fragment}%")],
    )
}

fn query_optional_text<P>(db_path: &Path, sql: &str, params: P) -> Result<Option<String>>
where
    P: rusqlite::Params,
{
    if !db_path.exists() {
        return Ok(None);
    }
    let connection = Connection::open(db_path)
        .with_context(|| format!("failed to open sqlite db {}", db_path.display()))?;
    connection
        .query_row(sql, params, |row| row.get::<_, String>(0))
        .optional()
        .context("sqlite smoke query failed")
}

fn ensure_child_alive(child: &mut ChildGuard) -> Result<()> {
    if let Some(status) = child.try_wait()? {
        bail!("bridge exited early with status {status}");
    }
    Ok(())
}

fn toml_string(value: &str) -> String {
    use std::fmt::Write;

    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            ch if ch <= '\u{1f}' || ch == '\u{7f}' => {
                let _ = write!(&mut escaped, "\\u{:04X}", ch as u32);
            }
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
        match self.child.as_mut() {
            Some(child) => child.try_wait().context("failed to query bridge process"),
            None => Ok(None),
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::{LIVE_WORKSPACE_MARKER, toml_string, validated_live_workspace};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn toml_string_keeps_non_ascii_valid_for_toml() {
        let original = "必要な確認を進めて、\"止まる\"理由\\path\n";
        let encoded = toml_string(original);

        assert!(!encoded.contains("\\u{"));
        let parsed: toml::Value =
            toml::from_str(&format!("value = {encoded}")).expect("string should parse as toml");
        assert_eq!(parsed["value"].as_str(), Some(original));
    }

    #[test]
    fn validated_live_workspace_requires_marker_file() {
        let temp = tempdir().expect("tempdir");
        let error = validated_live_workspace(temp.path().to_path_buf())
            .expect_err("workspace without marker should fail");
        assert!(error.to_string().contains(LIVE_WORKSPACE_MARKER));
    }

    #[test]
    fn validated_live_workspace_accepts_opted_in_directory() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(LIVE_WORKSPACE_MARKER), "ok").expect("marker");
        let workspace =
            validated_live_workspace(temp.path().to_path_buf()).expect("marked workspace");
        assert_eq!(
            workspace,
            fs::canonicalize(temp.path()).expect("canonical workspace")
        );
    }
}
