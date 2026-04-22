use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use rand::{Rng, distributions::Alphanumeric, rngs::OsRng};
use rpassword as hidden_input;

use crate::app_server::CodexThreadSummary;
use crate::codex::CodexRunner;
use crate::config::Config;
use crate::store::{AuthorizedSender, PendingAccessPairCode, Store};
use crate::telegram::{PairingUpdate, TelegramClient, TelegramPoller};
use crate::windows_secret::{load_secret, store_secret};

const DEFAULT_SECRET_REF: &str = "remotty-telegram-bot";
const PAIR_CODE_TTL_SECONDS: i64 = 180;

pub async fn configure(config_path: PathBuf) -> Result<String> {
    let secret_ref = resolve_secret_ref(config_path.as_path())?;
    let token = hidden_input::prompt_password("Telegram bot token: ")
        .context("failed to read Telegram bot token from terminal")?;
    if token.trim().is_empty() {
        bail!("Telegram bot token must not be empty");
    }

    store_secret(&secret_ref, token.trim())?;
    Ok(format!("Telegram token を `{secret_ref}` へ保存しました。"))
}

pub async fn pair(config_path: impl AsRef<Path>) -> Result<String> {
    let pair_code = generate_pair_code();
    let issued_at_s = Utc::now().timestamp();
    println!("Send `/pair {pair_code}` to the bot within {PAIR_CODE_TTL_SECONDS} seconds.");
    pair_with_code_until(
        config_path,
        &pair_code,
        issued_at_s,
        issued_at_s + PAIR_CODE_TTL_SECONDS,
    )
    .await
}

pub async fn access_pair(config_path: impl AsRef<Path>, pair_code: &str) -> Result<String> {
    let now_s = Utc::now().timestamp();
    let config = Config::load(config_path.as_ref())?;
    ensure_storage_dirs(&config)?;
    let store = Store::open(&config.storage.db_path)?;
    if let Some(candidate) = store.consume_access_pair_code(pair_code, now_s * 1000)? {
        ensure_private_access_pair_candidate(&candidate)?;
        authorize_paired_sender(&store, candidate.sender_id)?;
        return Ok(format!(
            "Telegram sender `{}` を allowlist へ追加しました。chat_id=`{}`",
            candidate.sender_id, candidate.chat_id
        ));
    }

    pair_with_code_until(
        config_path,
        pair_code,
        now_s - PAIR_CODE_TTL_SECONDS,
        now_s + 30,
    )
    .await
}

#[doc(hidden)]
pub async fn pair_with_code(
    config_path: impl AsRef<Path>,
    pair_code: &str,
    issued_at_s: i64,
) -> Result<String> {
    pair_with_code_until(
        config_path,
        pair_code,
        issued_at_s,
        issued_at_s + PAIR_CODE_TTL_SECONDS,
    )
    .await
}

async fn pair_with_code_until(
    config_path: impl AsRef<Path>,
    pair_code: &str,
    issued_after_s: i64,
    deadline_s: i64,
) -> Result<String> {
    let config = Config::load(config_path.as_ref())?;
    ensure_storage_dirs(&config)?;
    let token = load_token(&config)?;
    let telegram = TelegramClient::with_base_urls(
        token,
        config.telegram.api_base_url.clone(),
        config.telegram.file_base_url.clone(),
    );
    ensure_polling_mode_for_pairing(&telegram).await?;
    let poller = TelegramPoller::acquire(telegram).await?;
    let candidate = wait_for_pair_candidate(
        &poller,
        poller.bot().username.as_deref(),
        pair_code,
        issued_after_s,
        deadline_s,
    )
    .await?;
    println!(
        "Pairing target: sender_id=`{}`, chat_id=`{}`, chat_type=`{}`",
        candidate.sender_id, candidate.chat_id, candidate.chat_type
    );
    ensure_private_pair_candidate(&candidate)?;

    let store = Store::open(&config.storage.db_path)?;
    authorize_paired_sender(&store, candidate.sender_id)?;

    Ok(format!(
        "Telegram sender `{}` を allowlist へ追加しました。chat_id=`{}`",
        candidate.sender_id, candidate.chat_id
    ))
}

fn authorize_paired_sender(store: &Store, sender_id: i64) -> Result<()> {
    store.upsert_authorized_sender(AuthorizedSender {
        sender_id,
        platform: "telegram".to_owned(),
        display_name: None,
        status: "active".to_owned(),
        approved_at_ms: Utc::now().timestamp_millis(),
        source: "paired".to_owned(),
    })
}

fn ensure_private_access_pair_candidate(candidate: &PendingAccessPairCode) -> Result<()> {
    if candidate.chat_type == "private" {
        return Ok(());
    }
    bail!("pairing は `private` chat だけに対応しています。bot との DM でやり直してください。")
}

pub async fn send_access_pair_code(
    config: &Config,
    store: &Store,
    telegram: &TelegramClient,
    chat_id: i64,
    sender_id: i64,
    chat_type: &str,
) -> Result<()> {
    if !config
        .telegram
        .allowed_chat_types
        .iter()
        .any(|kind| kind == "private")
    {
        return Ok(());
    }

    let pair_code = generate_pair_code();
    let issued_at_ms = Utc::now().timestamp_millis();
    store.insert_access_pair_code(&PendingAccessPairCode {
        code: pair_code.clone(),
        sender_id,
        chat_id,
        chat_type: chat_type.to_owned(),
        issued_at_ms,
        expires_at_ms: issued_at_ms + (PAIR_CODE_TTL_SECONDS * 1000),
    })?;
    telegram
        .send_message(
            chat_id,
            &format!(
                "remotty pairing code: `{pair_code}`\nRun `/remotty-access-pair {pair_code}` in Codex within {PAIR_CODE_TTL_SECONDS} seconds."
            ),
        )
        .await?;
    Ok(())
}

async fn ensure_polling_mode_for_pairing(telegram: &TelegramClient) -> Result<()> {
    let webhook = telegram.get_webhook_info().await?;
    if webhook.url.trim().is_empty() {
        return Ok(());
    }
    bail!(
        "Telegram bot に webhook が残っています。pairing の前に webhook を外して polling に切り替えてください: {}",
        webhook.url
    )
}

pub fn policy_allowlist(config_path: impl AsRef<Path>) -> Result<String> {
    let config = Config::load(config_path.as_ref())?;
    let senders = if config.storage.db_path.exists() {
        Store::open_read_only(&config.storage.db_path)?.list_active_authorized_senders()?
    } else {
        Vec::new()
    };
    Ok(format_allowlist_summary(
        &config.telegram.admin_sender_ids,
        &senders,
    ))
}

pub async fn live_env_check(config_path: impl AsRef<Path>) -> Result<String> {
    let config = Config::load(config_path.as_ref()).ok();
    let optional = [
        "LIVE_CODEX_BIN",
        "LIVE_CODEX_PROFILE",
        "LIVE_TIMEOUT_SEC",
        "LIVE_APPROVAL_MODE",
    ];

    let mut lines = vec!["live environment check".to_owned(), "required:".to_owned()];
    lines.push(format!(
        "- `LIVE_TELEGRAM_BOT_TOKEN`: {}",
        live_token_presence(config.as_ref())
    ));
    lines.push(format!(
        "- `LIVE_TELEGRAM_CHAT_ID`: {}",
        live_chat_presence(config.as_ref())
    ));
    lines.push(format!(
        "- `LIVE_TELEGRAM_SENDER_ID`: {}",
        live_sender_presence(config.as_ref())
    ));
    lines.push(format!("- `LIVE_WORKSPACE`: {}", live_workspace_presence()));
    lines.push(format!(
        "- Telegram webhook: {}",
        live_webhook_presence(config.as_ref()).await
    ));
    lines.push("optional:".to_owned());
    for key in optional {
        lines.push(format!("- `{key}`: {}", env_presence(key)));
    }
    Ok(lines.join("\n"))
}

pub async fn sessions(config_path: impl AsRef<Path>, filter: Option<&str>) -> Result<String> {
    let config = Config::load(config_path.as_ref())?;
    let runner = CodexRunner::new(config.codex.clone());
    let threads = runner.list_threads(10, filter).await?;
    Ok(format_sessions_summary(&threads))
}

fn format_sessions_summary(threads: &[CodexThreadSummary]) -> String {
    if threads.is_empty() {
        return "No Codex threads returned.".to_owned();
    }
    let mut lines = vec!["Codex threads:".to_owned()];
    for thread in threads.iter().take(10) {
        let title = thread.title.as_deref().unwrap_or("untitled");
        let cwd = thread.cwd.as_deref().unwrap_or("cwd unavailable");
        lines.push(format!("- `{}` {title}", thread.thread_id));
        lines.push(format!("  cwd: `{cwd}`"));
    }
    lines.push("Telegram select: `/remotty-sessions <thread_id>`".to_owned());
    lines.join("\n")
}

fn resolve_secret_ref(config_path: &Path) -> Result<String> {
    if config_path.exists() {
        return Ok(Config::load(config_path)?.telegram.token_secret_ref);
    }
    Ok(DEFAULT_SECRET_REF.to_owned())
}

fn load_token(config: &Config) -> Result<String> {
    load_secret(&config.telegram.token_secret_ref)
        .or_else(|_| std::env::var("TELEGRAM_BOT_TOKEN").context("TELEGRAM_BOT_TOKEN is not set"))
        .and_then(|value| {
            if value.trim().is_empty() {
                Err(anyhow!("Telegram token must not be empty"))
            } else {
                Ok(value)
            }
        })
}

fn ensure_storage_dirs(config: &Config) -> Result<()> {
    fs::create_dir_all(&config.storage.state_dir)?;
    fs::create_dir_all(&config.storage.temp_dir)?;
    fs::create_dir_all(&config.storage.log_dir)?;
    if let Some(parent) = config.storage.db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn latest_pair_candidate(
    updates: &[PairingUpdate],
    bot_username: Option<&str>,
    expected_code: &str,
    issued_after_s: i64,
) -> Result<PairCandidate> {
    updates
        .iter()
        .rev()
        .find(|update| {
            let message = &update.message;
            message.chat_type == "private"
                && is_pairing_intent(&message.text, bot_username, expected_code)
                && message.sent_at_s >= issued_after_s
                && message.sent_at_s > 0
                && (Utc::now().timestamp() - message.sent_at_s) <= PAIR_CODE_TTL_SECONDS
        })
        .map(|update| PairCandidate {
            chat_id: update.message.chat_id,
            sender_id: update.message.sender_id,
            chat_type: update.message.chat_type.clone(),
        })
        .ok_or_else(|| anyhow!("pairing に使える一致メッセージがありません。bot へ `/pair <code>` を送ってから再実行してください。"))
}

fn generate_pair_code() -> String {
    OsRng
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .map(|ch| ch.to_ascii_uppercase())
        .collect()
}

fn env_presence(key: &str) -> &'static str {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => "set",
        _ => "missing",
    }
}

fn live_token_presence(config: Option<&Config>) -> &'static str {
    if env_presence("LIVE_TELEGRAM_BOT_TOKEN") == "set" {
        return "set";
    }
    match config {
        Some(config) if load_secret(&config.telegram.token_secret_ref).is_ok() => "stored",
        _ => "missing",
    }
}

fn live_sender_presence(config: Option<&Config>) -> &'static str {
    if env_presence("LIVE_TELEGRAM_SENDER_ID") == "set" {
        return "set";
    }
    match inferred_sender_count(config) {
        1 => "inferred",
        0 => "missing",
        _ => "ambiguous",
    }
}

fn live_chat_presence(config: Option<&Config>) -> &'static str {
    if env_presence("LIVE_TELEGRAM_CHAT_ID") == "set" {
        return "set";
    }
    match inferred_sender_count(config) {
        1 => "inferred",
        0 => "missing",
        _ => "ambiguous",
    }
}

fn live_workspace_presence() -> &'static str {
    if env_presence("LIVE_WORKSPACE") == "set" {
        "set"
    } else {
        "default"
    }
}

async fn live_webhook_presence(config: Option<&Config>) -> &'static str {
    let Some(config) = config else {
        return "unknown";
    };
    let Ok(token) = load_token(config) else {
        return "unknown";
    };
    let telegram = TelegramClient::with_base_urls(
        token,
        config.telegram.api_base_url.clone(),
        config.telegram.file_base_url.clone(),
    );
    match telegram.get_webhook_info().await {
        Ok(webhook) if webhook.url.trim().is_empty() => "polling-ready",
        Ok(_) => "webhook-configured",
        Err(_) => "unknown",
    }
}

fn inferred_sender_count(config: Option<&Config>) -> usize {
    let Some(config) = config else {
        return 0;
    };

    let mut sender_ids = std::collections::BTreeSet::new();
    sender_ids.extend(config.telegram.admin_sender_ids.iter().copied());
    if config.storage.db_path.exists() {
        if let Ok(store) = Store::open_read_only(&config.storage.db_path) {
            if let Ok(senders) = store.list_active_authorized_senders() {
                sender_ids.extend(senders.into_iter().map(|sender| sender.sender_id));
            }
        }
    }
    sender_ids.len()
}

fn ensure_private_pair_candidate(candidate: &PairCandidate) -> Result<()> {
    if candidate.chat_type == "private" {
        return Ok(());
    }

    bail!(
        "pairing は `private` chat だけに対応しています。bot との DM で `/pair <code>` を送り直してください。"
    )
}

async fn wait_for_pair_candidate(
    poller: &TelegramPoller,
    bot_username: Option<&str>,
    expected_code: &str,
    issued_after_s: i64,
    deadline_s: i64,
) -> Result<PairCandidate> {
    let mut offset = None;

    while Utc::now().timestamp() <= deadline_s {
        let updates = poller.get_pairing_updates(offset, 5).await?;
        if let Some(last_update_id) = updates.last().map(|update| update.update_id) {
            offset = Some(last_update_id + 1);
        }
        if let Ok(candidate) =
            latest_pair_candidate(&updates, bot_username, expected_code, issued_after_s)
        {
            return Ok(candidate);
        }
    }

    bail!("pairing code expired; send `/pair <code>` again and rerun `telegram pair`")
}

fn is_pairing_intent(text: &str, bot_username: Option<&str>, expected_code: &str) -> bool {
    let mut parts = text.split_whitespace();
    let Some(command_token) = parts.next() else {
        return false;
    };
    let Some(code) = parts.next() else {
        return false;
    };
    if parts.next().is_some() || code != expected_code {
        return false;
    }

    if command_token == "/pair" {
        return true;
    }

    let Some(bot_username) = bot_username else {
        return false;
    };
    let Some((command, mentioned_bot)) = command_token.split_once('@') else {
        return false;
    };
    command == "/pair" && mentioned_bot.eq_ignore_ascii_case(bot_username)
}

fn format_allowlist_summary(admin_sender_ids: &[i64], senders: &[AuthorizedSender]) -> String {
    let mut lines = vec!["allowlist は有効です。許可済み sender:".to_owned()];
    let mut wrote_any = false;

    for sender_id in admin_sender_ids {
        lines.push(format!("- `{sender_id}` (bridge.toml admin)"));
        wrote_any = true;
    }
    for sender in senders {
        if admin_sender_ids.contains(&sender.sender_id) {
            continue;
        }
        lines.push(format!("- `{}` ({})", sender.sender_id, sender.platform));
        wrote_any = true;
    }

    if !wrote_any {
        return "allowlist は有効ですが、まだ許可済み sender はありません。`telegram pair` を実行してください。"
            .to_owned();
    }

    lines.join("\n")
}

#[derive(Debug)]
struct PairCandidate {
    chat_id: i64,
    sender_id: i64,
    chat_type: String,
}

#[cfg(test)]
mod tests {
    use super::{
        PAIR_CODE_TTL_SECONDS, PairCandidate, ensure_private_pair_candidate, env_presence,
        format_allowlist_summary, generate_pair_code, latest_pair_candidate, resolve_secret_ref,
    };
    use crate::store::AuthorizedSender;
    use crate::telegram::{PairingMessage, PairingUpdate};
    use anyhow::Result;
    use chrono::Utc;
    use std::{fs, path::Path};
    use tempfile::tempdir;

    #[test]
    fn latest_pair_candidate_prefers_latest_allowed_chat() {
        let updates = vec![
            PairingUpdate {
                update_id: 1,
                message: PairingMessage {
                    chat_id: 11,
                    chat_type: "group".to_owned(),
                    sender_id: 101,
                    text: "/pair 123456".to_owned(),
                    sent_at_s: Utc::now().timestamp(),
                },
            },
            PairingUpdate {
                update_id: 2,
                message: PairingMessage {
                    chat_id: 22,
                    chat_type: "private".to_owned(),
                    sender_id: 202,
                    text: "/pair 123456".to_owned(),
                    sent_at_s: Utc::now().timestamp(),
                },
            },
        ];

        let candidate = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - 1,
        )
        .expect("candidate");
        assert_eq!(candidate.chat_id, 22);
        assert_eq!(candidate.sender_id, 202);
        assert_eq!(candidate.chat_type, "private");
    }

    #[test]
    fn latest_pair_candidate_errors_when_no_allowed_message_exists() {
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 11,
                chat_type: "private".to_owned(),
                sender_id: 101,
                text: "ignored".to_owned(),
                sent_at_s: Utc::now().timestamp(),
            },
        }];

        let error = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - 1,
        )
        .expect_err("candidate should fail");
        assert!(error.to_string().contains("一致メッセージ"));
    }

    #[test]
    fn live_env_check_reports_missing_when_env_is_absent() {
        assert_eq!(env_presence("REMOTTY_TEST_MISSING_ENV"), "missing");
    }

    #[test]
    fn resolve_secret_ref_defaults_without_config() {
        assert_eq!(
            resolve_secret_ref(Path::new("does-not-exist.toml")).expect("default secret ref"),
            "remotty-telegram-bot"
        );
    }

    #[test]
    fn generate_pair_code_uses_twelve_alphanumeric_characters() {
        let code = generate_pair_code();
        assert_eq!(code.len(), 12);
        assert!(
            code.chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
        );
    }

    #[test]
    fn format_allowlist_summary_includes_static_and_dynamic_senders() {
        let summary = format_allowlist_summary(
            &[11],
            &[AuthorizedSender {
                sender_id: 22,
                platform: "telegram".to_owned(),
                display_name: None,
                status: "active".to_owned(),
                approved_at_ms: 1,
                source: "paired".to_owned(),
            }],
        );

        assert!(summary.contains("`11` (bridge.toml admin)"));
        assert!(summary.contains("`22` (telegram)"));
    }

    #[test]
    fn policy_allowlist_does_not_create_storage_when_database_is_absent() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("bridge.toml");
        let state_dir = dir.path().join("state");
        let db_path = state_dir.join("bridge.db");
        fs::write(
            &config_path,
            format!(
                r#"
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "secret"
allowed_chat_types = ["private"]
admin_sender_ids = [11]

[codex]
binary = "codex"
model = "gpt-5.4"
sandbox = "workspace-write"
approval = "on-request"

[storage]
db_path = "{}"
state_dir = "{}"
temp_dir = "{}"
log_dir = "{}"

[policy]
default_mode = "await_reply"
progress_edit_interval_ms = 5000
max_output_chars = 12000

[[workspaces]]
id = "main"
path = "C:/workspace"
writable_roots = ["C:/workspace"]
default_mode = "await_reply"
continue_prompt = "continue"
checks_profile = "default"
"#,
                db_path.display().to_string().replace('\\', "/"),
                state_dir.display().to_string().replace('\\', "/"),
                dir.path()
                    .join("tmp")
                    .display()
                    .to_string()
                    .replace('\\', "/"),
                dir.path()
                    .join("logs")
                    .display()
                    .to_string()
                    .replace('\\', "/"),
            ),
        )?;

        let summary = super::policy_allowlist(&config_path)?;
        assert!(summary.contains("`11` (bridge.toml admin)"));
        assert!(!state_dir.exists());
        Ok(())
    }

    #[test]
    fn latest_pair_candidate_rejects_stale_pair_message() {
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 22,
                chat_type: "private".to_owned(),
                sender_id: 202,
                text: "/pair 123456".to_owned(),
                sent_at_s: Utc::now().timestamp() - (PAIR_CODE_TTL_SECONDS + 1),
            },
        }];

        let error = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - (PAIR_CODE_TTL_SECONDS + 2),
        )
        .expect_err("stale pair message should fail");
        assert!(error.to_string().contains("一致メッセージ"));
    }

    #[test]
    fn latest_pair_candidate_rejects_pair_command_for_other_bot() {
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 22,
                chat_type: "private".to_owned(),
                sender_id: 202,
                text: "/pair@otherbot 123456".to_owned(),
                sent_at_s: Utc::now().timestamp(),
            },
        }];

        let error = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - 1,
        )
        .expect_err("foreign bot pair command should fail");
        assert!(error.to_string().contains("一致メッセージ"));
    }

    #[test]
    fn latest_pair_candidate_accepts_pair_command_for_current_bot() {
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 22,
                chat_type: "private".to_owned(),
                sender_id: 202,
                text: "/pair@remotty_test_bot 123456".to_owned(),
                sent_at_s: Utc::now().timestamp(),
            },
        }];

        let candidate = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - 1,
        )
        .expect("pair command for current bot should pass");
        assert_eq!(candidate.sender_id, 202);
    }

    #[test]
    fn latest_pair_candidate_rejects_messages_older_than_pair_start() {
        let issued_after_s = Utc::now().timestamp();
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 22,
                chat_type: "private".to_owned(),
                sender_id: 202,
                text: "/pair 123456".to_owned(),
                sent_at_s: issued_after_s - 1,
            },
        }];

        let error =
            latest_pair_candidate(&updates, Some("remotty_test_bot"), "123456", issued_after_s)
                .expect_err("older pair command should fail");
        assert!(error.to_string().contains("一致メッセージ"));
    }

    #[test]
    fn latest_pair_candidate_ignores_group_pair_message_even_if_config_allows_group() {
        let updates = vec![PairingUpdate {
            update_id: 1,
            message: PairingMessage {
                chat_id: 22,
                chat_type: "group".to_owned(),
                sender_id: 202,
                text: "/pair 123456".to_owned(),
                sent_at_s: Utc::now().timestamp(),
            },
        }];

        let error = latest_pair_candidate(
            &updates,
            Some("remotty_test_bot"),
            "123456",
            Utc::now().timestamp() - 1,
        )
        .expect_err("group pair message should fail");
        assert!(error.to_string().contains("一致メッセージ"));
    }

    #[test]
    fn ensure_private_pair_candidate_rejects_non_private_chat() {
        let error = ensure_private_pair_candidate(&PairCandidate {
            chat_id: 22,
            sender_id: 202,
            chat_type: "group".to_owned(),
        })
        .expect_err("group pair candidate should fail");
        assert!(error.to_string().contains("`private`"));
    }
}
