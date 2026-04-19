use anyhow::{Result, anyhow};
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::codex::{CodexRequest, CodexRunner};
use crate::config::{Config, LaneMode, checks::CheckRunSummary, checks::run_profile};
use crate::store::{AuthorizedSender, LaneState, NewRun, Store};
use crate::telegram::{
    IncomingMessage, SavedTelegramAttachment, TelegramAttachmentKind, TelegramClient,
    TelegramControlCommand,
};
use crate::windows_secret::load_secret;

const MAX_COMPLETION_REPAIR_TURNS: usize = 2;
const MAX_TELEGRAM_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

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

            let chat_id = update.chat_id;
            if let Err(error) =
                handle_message(&config, &store, &telegram, &codex, sender_id, update).await
            {
                warn!("failed to handle chat {chat_id}: {error:#}");
                let _ = telegram
                    .send_message(chat_id, &format_runtime_failure_message())
                    .await;
            }
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
    if let Some(command) = update.control_command() {
        let reply = handle_control_command(
            store,
            workspace,
            update.chat_id,
            &update.thread_key,
            command,
        )?;
        let sent = telegram.send_message(update.chat_id, &reply).await?;
        if let Some(lane) = store.find_lane(update.chat_id, &update.thread_key)? {
            store.insert_message(
                &lane.lane_id,
                None,
                "outbound",
                "telegram_control",
                Some(sent.message_id),
                Some(&reply),
                None,
            )?;
        }
        return Ok(());
    }

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

    let progress_text = format_processing_message(lane.codex_session_id.is_some());
    let progress_message = telegram
        .send_message(update.chat_id, &progress_text)
        .await?;
    store.insert_message(
        &lane.lane_id,
        Some(&run.run_id),
        "outbound",
        "telegram_progress",
        Some(progress_message.message_id),
        Some(&progress_text),
        None,
    )?;

    let saved_attachments = if update.attachments.is_empty() {
        Vec::new()
    } else {
        let attachment_dir = config.storage.temp_dir.join("telegram").join(&run.run_id);
        telegram
            .save_attachments(
                &update.attachments,
                &attachment_dir,
                MAX_TELEGRAM_ATTACHMENT_BYTES,
            )
            .await?
    };
    let request = build_user_request(&update.text, &saved_attachments);
    let outcome = if let Some(session_id) = lane.codex_session_id.as_deref() {
        codex.resume(workspace, session_id, request).await?
    } else {
        codex.start(workspace, request).await?
    };
    let (outcome, unresolved_checks) =
        settle_completion_checks(config, workspace, codex, lane.mode, outcome).await?;

    let reply = if let Some(summary) = unresolved_checks.as_ref() {
        truncate(
            &format_reply_with_failed_checks(&outcome.last_message, summary),
            config.policy.max_output_chars,
        )
    } else if outcome.last_message.trim().is_empty() {
        "応答本文を取得できませんでした。ローカルのログを確認してください。".to_owned()
    } else {
        truncate(&outcome.last_message, config.policy.max_output_chars)
    };

    if let Err(error) = telegram
        .edit_message(update.chat_id, progress_message.message_id, &reply)
        .await
    {
        warn!("failed to edit progress message: {error:#}");
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
    } else {
        store.insert_message(
            &lane.lane_id,
            Some(&run.run_id),
            "outbound",
            "telegram_text",
            Some(progress_message.message_id),
            Some(&reply),
            None,
        )?;
    }

    let next_state = if outcome.approval_pending {
        LaneState::NeedsLocalApproval
    } else if unresolved_checks.is_some() {
        LaneState::Failed
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

fn handle_control_command(
    store: &Store,
    workspace: &crate::config::WorkspaceConfig,
    chat_id: i64,
    thread_key: &str,
    command: TelegramControlCommand,
) -> Result<String> {
    match command {
        TelegramControlCommand::Help => Ok(format_help_message()),
        TelegramControlCommand::Status => {
            Ok(format_status_message(store.find_lane(chat_id, thread_key)?))
        }
        TelegramControlCommand::Stop => {
            let Some(lane) = store.find_lane(chat_id, thread_key)? else {
                return Ok("停止する対象はありません。".to_owned());
            };
            store.clear_lane_session(&lane.lane_id)?;
            Ok("現在のセッションを止めました。次の入力は新しい開始として扱います。".to_owned())
        }
        TelegramControlCommand::Mode(raw_mode) => {
            let mode = parse_lane_mode_name(&raw_mode)?;
            let lane = store.get_or_create_lane(chat_id, thread_key, &workspace.id, mode)?;
            store.update_lane_mode(&lane.lane_id, mode)?;
            Ok(format!(
                "この会話のモードを `{}` に更新しました。",
                lane_mode_name(mode)
            ))
        }
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let trimmed: String = text.chars().take(max_chars).collect();
    format!("{trimmed}\n\n[truncated]")
}

async fn settle_completion_checks(
    config: &Config,
    workspace: &crate::config::WorkspaceConfig,
    codex: &CodexRunner,
    lane_mode: LaneMode,
    initial_outcome: crate::codex::CodexOutcome,
) -> Result<(crate::codex::CodexOutcome, Option<CheckRunSummary>)> {
    if lane_mode != LaneMode::CompletionChecks || initial_outcome.approval_pending {
        return Ok((initial_outcome, None));
    }

    let profile = config
        .checks
        .profiles
        .get(&workspace.checks_profile)
        .ok_or_else(|| anyhow!("missing checks profile '{}'", workspace.checks_profile))?;

    let mut outcome = initial_outcome;
    for attempt in 0..=MAX_COMPLETION_REPAIR_TURNS {
        let summary = run_profile(&workspace.checks_profile, profile, &workspace.path).await?;
        if summary.success {
            return Ok((outcome, None));
        }

        if attempt == MAX_COMPLETION_REPAIR_TURNS {
            return Ok((outcome, Some(summary)));
        }

        let session_id = match outcome.session_id.as_deref() {
            Some(session_id) => session_id,
            None => return Ok((outcome, Some(summary))),
        };
        let retry_prompt = build_completion_retry_prompt(&workspace.continue_prompt, &summary);
        outcome = codex.resume(workspace, session_id, &retry_prompt).await?;

        if outcome.approval_pending {
            return Ok((outcome, None));
        }
    }

    Ok((outcome, None))
}

fn format_processing_message(is_resume: bool) -> String {
    if is_resume {
        "前回の続きとして処理しています。完了したら、このメッセージを更新します。".to_owned()
    } else {
        "処理を開始しました。完了したら、このメッセージを更新します。".to_owned()
    }
}

fn format_runtime_failure_message() -> String {
    "処理中に失敗しました。少し待ってから再送してください。必要ならローカルのログを確認します。"
        .to_owned()
}

fn format_help_message() -> String {
    [
        "使えるコマンド:",
        "/help",
        "/status",
        "/stop",
        "/mode await_reply|completion_checks|infinite|max_turns",
    ]
    .join("\n")
}

fn format_status_message(lane: Option<crate::store::LaneRecord>) -> String {
    let Some(lane) = lane else {
        return "この会話にはまだレーンがありません。".to_owned();
    };

    let session = if lane.codex_session_id.is_some() {
        "あり"
    } else {
        "なし"
    };
    format!(
        "状態: `{}`\nモード: `{}`\nセッション: {}",
        lane_state_name(lane.state),
        lane_mode_name(lane.mode),
        session
    )
}

fn build_user_request(text: &str, attachments: &[SavedTelegramAttachment]) -> CodexRequest {
    let image_paths = attachments
        .iter()
        .filter(|attachment| attachment.attachment.kind == TelegramAttachmentKind::Photo)
        .map(|attachment| attachment.local_path.clone())
        .collect::<Vec<_>>();

    let document_paths = attachments
        .iter()
        .filter(|attachment| attachment.attachment.kind == TelegramAttachmentKind::Document)
        .map(|attachment| attachment.local_path.display().to_string())
        .collect::<Vec<_>>();

    let prompt = if document_paths.is_empty() {
        text.to_owned()
    } else {
        format!(
            "{text}\n\n添付ファイルを保存しました。必要なら内容を確認してください。\n{}",
            document_paths
                .into_iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    if image_paths.is_empty() {
        CodexRequest::new(prompt)
    } else {
        CodexRequest::with_images(prompt, image_paths)
    }
}

fn build_completion_retry_prompt(continue_prompt: &str, summary: &CheckRunSummary) -> String {
    format!(
        "{continue_prompt}\n\n以下の確認に失敗しました。原因を直し、必要ならテストを追加してから続けてください。\n{}\n",
        summary.summary()
    )
}

fn format_reply_with_failed_checks(last_message: &str, summary: &CheckRunSummary) -> String {
    let mut sections = Vec::new();
    if !last_message.trim().is_empty() {
        sections.push(truncate(last_message, usize::MAX));
    }
    sections.push(format!(
        "確認で失敗しました。ローカルで追加の修正が必要です。\n{}",
        summary.summary()
    ));
    sections.join("\n\n")
}

fn parse_lane_mode_name(value: &str) -> Result<LaneMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "await_reply" => Ok(LaneMode::AwaitReply),
        "completion_checks" => Ok(LaneMode::CompletionChecks),
        "infinite" => Ok(LaneMode::Infinite),
        "max_turns" => Ok(LaneMode::MaxTurns),
        _ => Err(anyhow!(
            "不正なモードです。`await_reply`、`completion_checks`、`infinite`、`max_turns` を使ってください。"
        )),
    }
}

fn lane_mode_name(mode: LaneMode) -> &'static str {
    match mode {
        LaneMode::AwaitReply => "await_reply",
        LaneMode::CompletionChecks => "completion_checks",
        LaneMode::Infinite => "infinite",
        LaneMode::MaxTurns => "max_turns",
    }
}

fn lane_state_name(state: LaneState) -> &'static str {
    match state {
        LaneState::Running => "running",
        LaneState::WaitingReply => "waiting_reply",
        LaneState::Idle => "idle",
        LaneState::NeedsLocalApproval => "needs_local_approval",
        LaneState::Failed => "failed",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorkspaceConfig;
    use crate::store::Store;
    use crate::telegram::{SavedTelegramAttachment, TelegramAttachment, TelegramRemoteFile};
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn failed_summary() -> CheckRunSummary {
        CheckRunSummary {
            profile_name: "quick".to_owned(),
            total_commands: 2,
            completed_commands: 1,
            success: false,
            timed_out: false,
            failed_command: Some("cargo test".to_owned()),
            exit_code: Some(101),
        }
    }

    #[test]
    fn completion_retry_prompt_mentions_summary_and_continue_prompt() {
        let prompt =
            build_completion_retry_prompt("必要な確認を進めてください。", &failed_summary());
        assert!(prompt.contains("必要な確認を進めてください。"));
        assert!(prompt.contains("以下の確認に失敗しました。"));
        assert!(prompt.contains("completion checks failed on 'cargo test'"));
    }

    #[test]
    fn failed_check_reply_includes_agent_message_and_summary() {
        let reply = format_reply_with_failed_checks("修正を試しました。", &failed_summary());
        assert!(reply.contains("修正を試しました。"));
        assert!(reply.contains("確認で失敗しました。"));
        assert!(reply.contains("completion checks failed on 'cargo test'"));
    }

    #[test]
    fn runtime_failure_message_prompts_retry() {
        let message = format_runtime_failure_message();
        assert!(message.contains("失敗しました"));
        assert!(message.contains("再送"));
    }

    #[test]
    fn build_user_request_sends_images_and_mentions_document_paths() {
        let request = build_user_request(
            "確認してください。",
            &[
                saved_attachment(
                    TelegramAttachmentKind::Photo,
                    "C:/tmp/photo.png",
                    None,
                    "photos/file.png",
                ),
                saved_attachment(
                    TelegramAttachmentKind::Document,
                    "C:/tmp/report.pdf",
                    Some("report.pdf"),
                    "documents/report.pdf",
                ),
            ],
        );

        assert_eq!(request.image_paths, vec![PathBuf::from("C:/tmp/photo.png")]);
        assert!(request.prompt.contains("確認してください。"));
        assert!(request.prompt.contains("C:/tmp/report.pdf"));
    }

    #[test]
    fn format_status_message_without_lane_is_clear() {
        assert_eq!(
            format_status_message(None),
            "この会話にはまだレーンがありません。"
        );
    }

    #[test]
    fn parse_lane_mode_name_accepts_completion_checks() {
        assert_eq!(
            parse_lane_mode_name("completion_checks").expect("mode should parse"),
            LaneMode::CompletionChecks
        );
    }

    #[test]
    fn parse_lane_mode_name_rejects_unknown_mode() {
        let error = parse_lane_mode_name("unknown").expect_err("mode should fail");
        assert!(error.to_string().contains("不正なモード"));
    }

    #[test]
    fn mode_command_updates_lane_mode() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let workspace = test_workspace();

        let reply = handle_control_command(
            &store,
            &workspace,
            10,
            "dm",
            TelegramControlCommand::Mode("completion_checks".to_owned()),
        )
        .expect("mode command should succeed");

        let lane = store
            .find_lane(10, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");
        assert_eq!(lane.mode, LaneMode::CompletionChecks);
        assert!(reply.contains("completion_checks"));
    }

    #[test]
    fn stop_command_clears_session() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let workspace = test_workspace();
        let lane = store
            .get_or_create_lane(20, "dm", &workspace.id, LaneMode::AwaitReply)
            .expect("lane should be created");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("lane should update");

        let reply =
            handle_control_command(&store, &workspace, 20, "dm", TelegramControlCommand::Stop)
                .expect("stop command should succeed");

        let lane = store
            .find_lane(20, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");
        assert_eq!(lane.state, LaneState::Idle);
        assert_eq!(lane.codex_session_id, None);
        assert!(reply.contains("セッションを止めました"));
    }

    fn saved_attachment(
        kind: TelegramAttachmentKind,
        local_path: &str,
        file_name: Option<&str>,
        remote_path: &str,
    ) -> SavedTelegramAttachment {
        SavedTelegramAttachment {
            attachment: TelegramAttachment {
                kind,
                file_id: "file-id".to_owned(),
                file_unique_id: "unique-id".to_owned(),
                file_name: file_name.map(ToOwned::to_owned),
                mime_type: None,
                file_size: Some(12),
                width: None,
                height: None,
            },
            remote_file: TelegramRemoteFile {
                file_id: "file-id".to_owned(),
                file_unique_id: "unique-id".to_owned(),
                file_path: remote_path.to_owned(),
                file_size: Some(12),
            },
            local_path: PathBuf::from(local_path),
            bytes_written: 12,
        }
    }

    fn test_workspace() -> WorkspaceConfig {
        WorkspaceConfig {
            id: "main".to_owned(),
            path: PathBuf::from("C:/workspace"),
            writable_roots: vec![PathBuf::from("C:/workspace")],
            default_mode: LaneMode::AwaitReply,
            continue_prompt: "continue".to_owned(),
            checks_profile: "default".to_owned(),
        }
    }
}
