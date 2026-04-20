#![cfg(feature = "live-e2e")]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Client;
use rusqlite::{Connection, OptionalExtension, params};
use tempfile::tempdir;
use tokio::time::sleep;

#[tokio::test]
#[ignore = "live"]
async fn live_end_to_end_bridge_round_trip() -> Result<()> {
    let live = LiveEnv::from_env()?;
    let temp = tempdir()?;
    let config_path = write_live_config(temp.path(), &live)?;
    let db_path = temp.path().join("state").join("bridge.db");
    let nonce = format!(
        "LIVE_E2E_{}",
        Instant::now().elapsed().as_nanos() + (std::process::id() as u128)
    );
    let inbound_prompt = format!("Reply with the exact text {nonce} and nothing else.");

    drain_pending_updates(&live.bot_token).await?;
    try_send_instruction(&live, &inbound_prompt).await?;
    eprintln!(
        "live e2e instruction sent to chat {}. Reply to the bot with:\n{}",
        live.chat_id, inbound_prompt
    );

    let bridge_exe = bridge_binary()?;
    let mut child = ChildGuard::new(spawn_bridge(&bridge_exe, &config_path, &live.bot_token)?);
    let start = Instant::now();
    let timeout = Duration::from_secs(live.timeout_sec);
    let inbound_deadline = start + timeout;
    wait_for_inbound(
        &db_path,
        live.sender_id,
        &inbound_prompt,
        inbound_deadline,
        &mut child,
    )
    .await?;
    wait_for_outbound(&db_path, &nonce, inbound_deadline, &mut child).await?;

    child.stop();
    Ok(())
}

struct LiveEnv {
    bot_token: String,
    chat_id: i64,
    sender_id: i64,
    workspace: PathBuf,
    codex_bin: String,
    codex_profile: Option<String>,
    timeout_sec: u64,
}

impl LiveEnv {
    fn from_env() -> Result<Self> {
        Ok(Self {
            bot_token: required_env("LIVE_TELEGRAM_BOT_TOKEN")?,
            chat_id: required_env("LIVE_TELEGRAM_CHAT_ID")?
                .parse()
                .context("LIVE_TELEGRAM_CHAT_ID must be an integer")?,
            sender_id: required_env("LIVE_TELEGRAM_SENDER_ID")?
                .parse()
                .context("LIVE_TELEGRAM_SENDER_ID must be an integer")?,
            workspace: PathBuf::from(required_env("LIVE_WORKSPACE")?),
            codex_bin: env::var("LIVE_CODEX_BIN").unwrap_or_else(|_| "codex".to_owned()),
            codex_profile: env::var("LIVE_CODEX_PROFILE")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            timeout_sec: env::var("LIVE_TIMEOUT_SEC")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(180),
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    let value = env::var(name).with_context(|| format!("missing env var {name}"))?;
    if value.trim().is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(value)
}

fn write_live_config(root: &Path, live: &LiveEnv) -> Result<PathBuf> {
    let state_dir = root.join("state");
    let temp_dir = state_dir.join("tmp");
    let log_dir = state_dir.join("logs");
    fs::create_dir_all(&temp_dir)?;
    fs::create_dir_all(&log_dir)?;
    let config_path = root.join("bridge.toml");
    let profile_line = live
        .codex_profile
        .as_deref()
        .map(|profile| format!("profile = {}\n", toml_string(profile)))
        .unwrap_or_default();
    let config = format!(
        r#"[service]
run_mode = "console"
poll_timeout_sec = 5
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "unused-live-e2e"
allowed_chat_types = ["private"]
admin_sender_ids = [{sender_id}]

[codex]
binary = {codex_bin}
model = "gpt-5.4"
sandbox = "read-only"
approval = "never"
{profile_line}

[storage]
db_path = {db_path}
state_dir = {state_dir}
temp_dir = {temp_dir}
log_dir = {log_dir}

[policy]
default_mode = "await_reply"
progress_edit_interval_ms = 2000
max_output_chars = 12000
max_turns_limit = 3

[[workspaces]]
id = "main"
path = {workspace}
writable_roots = [{workspace}]
default_mode = "await_reply"
continue_prompt = "continue"
checks_profile = "default"
"#,
        sender_id = live.sender_id,
        codex_bin = toml_string(&live.codex_bin),
        profile_line = profile_line,
        db_path = toml_string(&state_dir.join("bridge.db").display().to_string()),
        state_dir = toml_string(&state_dir.display().to_string()),
        temp_dir = toml_string(&temp_dir.display().to_string()),
        log_dir = toml_string(&log_dir.display().to_string()),
        workspace = toml_string(&live.workspace.display().to_string()),
    );
    fs::write(&config_path, config)?;
    Ok(config_path)
}

fn bridge_binary() -> Result<PathBuf> {
    let exe = PathBuf::from(env!("CARGO_BIN_EXE_codex-telegram-bridge"));
    if !exe.exists() {
        bail!("bridge binary is missing at {}", exe.display());
    }
    Ok(exe)
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

async fn try_send_instruction(live: &LiveEnv, inbound_prompt: &str) -> Result<()> {
    let message = format!(
        "Live E2E test is waiting.\nReply to this bot with the next line exactly:\n{}",
        inbound_prompt
    );
    let client = Client::new();
    let payload = serde_json::json!({
        "chat_id": live.chat_id,
        "text": message,
    });
    client
        .post(format!(
            "https://api.telegram.org/bot{}/sendMessage",
            live.bot_token
        ))
        .json(&payload)
        .send()
        .await
        .context("failed to send Telegram live-e2e instruction")?
        .error_for_status()
        .context("telegram sendMessage returned error status")?;
    Ok(())
}

async fn drain_pending_updates(bot_token: &str) -> Result<()> {
    let client = Client::new();
    let mut offset = None;

    loop {
        let payload = serde_json::json!({
            "offset": offset,
            "timeout": 0,
            "allowed_updates": ["message", "edited_message"],
        });
        let response = client
            .post(format!(
                "https://api.telegram.org/bot{}/getUpdates",
                bot_token
            ))
            .json(&payload)
            .send()
            .await
            .context("failed to fetch pending Telegram updates")?
            .error_for_status()
            .context("telegram getUpdates returned error status")?
            .json::<serde_json::Value>()
            .await
            .context("failed to decode pending Telegram updates")?;

        let Some(results) = response.get("result").and_then(|value| value.as_array()) else {
            bail!("telegram getUpdates response is missing result array");
        };
        let Some(last_update_id) = results
            .iter()
            .filter_map(|item| item.get("update_id").and_then(|value| value.as_i64()))
            .max()
        else {
            return Ok(());
        };
        offset = Some(last_update_id + 1);
    }
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
            eprintln!("received inbound text: {found}");
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
    Err(anyhow!(
        "timed out waiting for inbound Telegram message '{}'",
        inbound_prompt
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
            eprintln!("received outbound text: {found}");
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
        .context("sqlite live-e2e query failed")
}

fn ensure_child_alive(child: &mut ChildGuard) -> Result<()> {
    if let Some(status) = child.try_wait()? {
        bail!("bridge exited early with status {status}");
    }
    Ok(())
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.escape_default())
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
