use anyhow::{Result, anyhow};
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::codex::CodexRunner;
use crate::config::Config;
use crate::store::{AuthorizedSender, LaneState, NewRun, Store};
use crate::telegram::{IncomingMessage, TelegramClient};
use crate::windows_secret::load_secret;

pub async fn run_console(config: Config) -> Result<()> {
    let shutdown = CancellationToken::new();
    let ctrl_c_token = shutdown.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            ctrl_c_token.cancel();
        }
    });
    run_with_shutdown(config, shutdown).await
}

pub async fn run_with_shutdown(config: Config, shutdown: CancellationToken) -> Result<()> {
    let token = load_secret(&config.telegram.token_secret_ref)
        .unwrap_or_else(|_| std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default());
    if token.is_empty() {
        return Err(anyhow!(
            "telegram token is empty; set TELEGRAM_BOT_TOKEN or store a DPAPI secret"
        ));
    }

    let telegram = TelegramClient::new(token);
    let store = Store::open(&config.storage.db_path)?;
    seed_admin_senders(&store, &config.telegram.admin_sender_ids)?;
    let codex = CodexRunner::new(config.codex.clone());

    let mut offset = None;
    loop {
        let updates = tokio::select! {
            _ = shutdown.cancelled() => {
                info!("shutdown requested");
                break;
            }
            result = telegram.get_updates(offset, config.service.poll_timeout_sec) => result?,
        };
        for update in updates {
            offset = Some(update.update_id + 1);
            if !store.insert_seen_update(
                update.update_id,
                update.chat_id,
                update.sender_id,
                "message",
                &update.payload_json,
            )? {
                continue;
            }

            if !config
                .telegram
                .allowed_chat_types
                .iter()
                .any(|kind| kind == &update.chat_type)
            {
                continue;
            }

            let sender_id = match update.sender_id {
                Some(sender_id) if store.is_authorized_sender(sender_id)? => sender_id,
                Some(sender_id) => {
                    warn!("rejected unauthorized sender: {sender_id}");
                    continue;
                }
                None => continue,
            };

            handle_message(&config, &store, &telegram, &codex, sender_id, update).await?;
        }
        sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}

async fn handle_message(
    config: &Config,
    store: &Store,
    telegram: &TelegramClient,
    codex: &CodexRunner,
    _sender_id: i64,
    update: IncomingMessage,
) -> Result<()> {
    let workspace = config.default_workspace();
    let lane = store.get_or_create_lane(
        update.chat_id,
        &update.thread_key,
        &workspace.id,
        workspace.default_mode,
    )?;

    store.insert_message(
        &lane.lane_id,
        None,
        "inbound",
        "telegram_text",
        Some(update.telegram_message_id),
        Some(&update.text),
        Some(&update.payload_json),
    )?;

    store.update_lane_state(
        &lane.lane_id,
        LaneState::Running,
        lane.codex_session_id.as_deref(),
    )?;
    let run = store.insert_run(NewRun {
        lane_id: lane.lane_id.clone(),
        run_kind: if lane.codex_session_id.is_some() {
            "resume".to_owned()
        } else {
            "start".to_owned()
        },
    })?;

    let outcome = if let Some(session_id) = lane.codex_session_id.as_deref() {
        codex.resume(workspace, session_id, &update.text).await?
    } else {
        codex.start(workspace, &update.text).await?
    };

    let reply = if outcome.last_message.trim().is_empty() {
        "応答本文を取得できませんでした。ローカルのログを確認してください。".to_owned()
    } else {
        truncate(&outcome.last_message, config.policy.max_output_chars)
    };

    let sent = telegram.send_message(update.chat_id, &reply).await?;
    store.insert_message(
        &lane.lane_id,
        Some(&run.run_id),
        "outbound",
        "telegram_text",
        Some(sent.message_id),
        Some(&reply),
        None,
    )?;

    let next_state = if outcome.approval_pending {
        LaneState::NeedsLocalApproval
    } else {
        LaneState::WaitingReply
    };
    store.update_lane_state(&lane.lane_id, next_state, outcome.session_id.as_deref())?;
    store.finish_run(
        &run.run_id,
        outcome.exit_code,
        next_state.as_str(),
        outcome.approval_pending,
    )?;

    info!("handled lane {}", lane.lane_id);
    Ok(())
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let trimmed: String = text.chars().take(max_chars).collect();
    format!("{trimmed}\n\n[truncated]")
}

fn seed_admin_senders(store: &Store, sender_ids: &[i64]) -> Result<()> {
    for sender_id in sender_ids {
        store.upsert_authorized_sender(AuthorizedSender {
            sender_id: *sender_id,
            platform: "telegram".to_owned(),
            display_name: None,
            status: "active".to_owned(),
            approved_at_ms: chrono::Utc::now().timestamp_millis(),
        })?;
    }
    Ok(())
}

trait LaneStateLabel {
    fn as_str(self) -> &'static str;
}

impl LaneStateLabel for LaneState {
    fn as_str(self) -> &'static str {
        match self {
            LaneState::Running => "running",
            LaneState::WaitingReply => "waiting_reply",
            LaneState::Idle => "idle",
            LaneState::NeedsLocalApproval => "needs_local_approval",
            LaneState::Failed => "failed",
        }
    }
}
