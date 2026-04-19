use anyhow::{Result, anyhow};
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::codex::{CodexRequest, CodexRunner};
use crate::config::{Config, LaneMode, checks::CheckRunSummary, checks::run_profile};
use crate::store::{AuthorizedSender, LaneRecord, LaneState, NewRun, Store};
use crate::telegram::{
    IncomingMessage, SavedTelegramAttachment, TelegramAttachmentKind, TelegramClient,
    TelegramControlCommand,
};
use crate::windows_secret::load_secret;

const MAX_COMPLETION_REPAIR_TURNS: usize = 2;
const MAX_TELEGRAM_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
const MAX_INFINITE_AUTO_TURNS: usize = 16;
const AUTO_CONTINUE_STOP_MARKER: &str = "CHANNEL_WAITING";

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
            config.policy.max_turns_limit,
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
        configured_extra_turn_budget(workspace.default_mode, None, config.policy.max_turns_limit),
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
    let request = build_user_request(
        &update.text,
        &saved_attachments,
        lane.mode,
        &workspace.continue_prompt,
    );
    let initial_outcome = if let Some(session_id) = lane.codex_session_id.as_deref() {
        codex.resume(workspace, session_id, request).await?
    } else {
        codex.start(workspace, request).await?
    };
    let (outcome, unresolved_checks, auto_turns_completed) =
        continue_lane_after_completion(config, workspace, codex, &lane, initial_outcome).await?;

    let reply = if let Some(summary) = unresolved_checks.as_ref() {
        truncate(
            &format_reply_with_failed_checks(&outcome.last_message, summary, auto_turns_completed),
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
    default_max_turns_limit: i64,
) -> Result<String> {
    match command {
        TelegramControlCommand::Help => Ok(format_help_message()),
        TelegramControlCommand::Status => Ok(format_status_message(
            store.find_lane(chat_id, thread_key)?,
            default_max_turns_limit,
        )),
        TelegramControlCommand::Stop => {
            let Some(lane) = store.find_lane(chat_id, thread_key)? else {
                return Ok("停止する対象はありません。".to_owned());
            };
            store.clear_lane_session(&lane.lane_id)?;
            Ok("現在のセッションを止めました。次の入力は新しい開始として扱います。".to_owned())
        }
        TelegramControlCommand::Mode { mode, max_turns } => {
            let mode = parse_lane_mode_name(&mode)?;
            let extra_turn_budget =
                configured_extra_turn_budget(mode, max_turns, default_max_turns_limit);
            let lane = store.get_or_create_lane(
                chat_id,
                thread_key,
                &workspace.id,
                mode,
                extra_turn_budget,
            )?;
            store.update_lane_mode(&lane.lane_id, mode, extra_turn_budget)?;
            Ok(format!(
                "この会話のモードを {} に更新しました。{}",
                lane_mode_name(mode),
                format_lane_mode_details(mode, extra_turn_budget)
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

async fn continue_lane_after_completion(
    config: &Config,
    workspace: &crate::config::WorkspaceConfig,
    codex: &CodexRunner,
    lane: &LaneRecord,
    initial_outcome: crate::codex::CodexOutcome,
) -> Result<(crate::codex::CodexOutcome, Option<CheckRunSummary>, usize)> {
    let (mut outcome, mut unresolved_checks) =
        settle_completion_checks(config, workspace, codex, lane.mode, initial_outcome).await?;
    let mut auto_turns_completed = 0usize;
    let mut last_visible_message = sanitize_auto_continue_message(&outcome.last_message);
    let auto_turn_limit = automatic_turn_limit(
        lane.mode,
        lane.extra_turn_budget,
        config.policy.max_turns_limit,
    );

    while should_continue_automatically(
        lane.mode,
        auto_turn_limit,
        auto_turns_completed,
        &outcome,
        unresolved_checks.as_ref(),
    ) {
        let session_id = outcome
            .session_id
            .as_deref()
            .ok_or_else(|| anyhow!("missing session_id for auto-continue"))?;
        let resumed = codex
            .resume(
                workspace,
                session_id,
                build_auto_continue_prompt(&workspace.continue_prompt),
            )
            .await?;
        auto_turns_completed += 1;

        let (next_outcome, next_unresolved_checks) =
            settle_completion_checks(config, workspace, codex, lane.mode, resumed).await?;
        let visible_message = sanitize_auto_continue_message(&next_outcome.last_message);
        if !visible_message.trim().is_empty() {
            last_visible_message = visible_message;
        }
        outcome = next_outcome;
        unresolved_checks = next_unresolved_checks;
    }

    outcome.last_message = if last_visible_message.trim().is_empty()
        && matches!(lane.mode, LaneMode::Infinite | LaneMode::MaxTurns)
        && !outcome.approval_pending
        && unresolved_checks.is_none()
    {
        format_auto_continue_waiting_message(auto_turns_completed)
    } else {
        last_visible_message
    };
    Ok((outcome, unresolved_checks, auto_turns_completed))
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

fn format_auto_continue_waiting_message(auto_turns_completed: usize) -> String {
    if auto_turns_completed == 0 {
        "この依頼は完了しました。次の入力を待ちます。".to_owned()
    } else {
        format!(
            "自動継続を {} 回実行し、この依頼は完了しました。次の入力を待ちます。",
            auto_turns_completed
        )
    }
}

fn format_help_message() -> String {
    [
        "使えるコマンド:",
        "/help",
        "/status",
        "/stop",
        "/mode await_reply",
        "/mode completion_checks",
        "/mode infinite",
        "/mode max_turns [count]",
    ]
    .join("\n")
}

fn format_status_message(
    lane: Option<crate::store::LaneRecord>,
    default_max_turns_limit: i64,
) -> String {
    let Some(lane) = lane else {
        return "この会話にはまだレーンがありません。".to_owned();
    };

    let session = if lane.codex_session_id.is_some() {
        "あり"
    } else {
        "なし"
    };
    let configured_budget = configured_extra_turn_budget(
        lane.mode,
        Some(lane.extra_turn_budget),
        default_max_turns_limit,
    );
    format!(
        "状態: `{}`\nモード: `{}`\n{}\nセッション: {}",
        lane_state_name(lane.state),
        lane_mode_name(lane.mode),
        format_lane_mode_details(lane.mode, configured_budget),
        session
    )
}

fn build_user_request(
    text: &str,
    attachments: &[SavedTelegramAttachment],
    lane_mode: LaneMode,
    continue_prompt: &str,
) -> CodexRequest {
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

    let prompt = if matches!(lane_mode, LaneMode::Infinite | LaneMode::MaxTurns) {
        append_auto_continue_instruction(&prompt, continue_prompt)
    } else {
        prompt
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

fn format_reply_with_failed_checks(
    last_message: &str,
    summary: &CheckRunSummary,
    auto_turns_completed: usize,
) -> String {
    let mut sections = Vec::new();
    if !last_message.trim().is_empty() {
        sections.push(truncate(last_message, usize::MAX));
    }
    if auto_turns_completed > 0 {
        sections.push(format!(
            "自動継続を {} 回実行したあとで止まりました。",
            auto_turns_completed
        ));
    }
    sections.push(format!(
        "確認で失敗しました。ローカルで追加の修正が必要です。\n{}",
        summary.summary()
    ));
    sections.join("\n\n")
}

fn build_auto_continue_prompt(continue_prompt: &str) -> String {
    format!(
        "{continue_prompt}\n\nまだ続ける作業がある時だけ続けてください。区切りがついたら `{AUTO_CONTINUE_STOP_MARKER}` とだけ返してください。"
    )
}

fn append_auto_continue_instruction(prompt: &str, continue_prompt: &str) -> String {
    format!(
        "{prompt}\n\n補足:\n- まだ続ける作業がある時だけ、そのまま続けてください。\n- 区切りがついたら `{AUTO_CONTINUE_STOP_MARKER}` とだけ返してください。\n- 次に進む時は次の方針を優先してください: {continue_prompt}"
    )
}

fn sanitize_auto_continue_message(message: &str) -> String {
    if message.trim() == AUTO_CONTINUE_STOP_MARKER {
        String::new()
    } else {
        message.to_owned()
    }
}

fn automatic_turn_limit(
    mode: LaneMode,
    configured_budget: i64,
    default_max_turns_limit: i64,
) -> Option<usize> {
    match mode {
        LaneMode::Infinite => None,
        LaneMode::MaxTurns => Some(configured_extra_turn_budget(
            mode,
            Some(configured_budget),
            default_max_turns_limit,
        ) as usize),
        LaneMode::AwaitReply | LaneMode::CompletionChecks => Some(0),
    }
}

fn should_continue_automatically(
    mode: LaneMode,
    auto_turn_limit: Option<usize>,
    auto_turns_completed: usize,
    outcome: &crate::codex::CodexOutcome,
    unresolved_checks: Option<&CheckRunSummary>,
) -> bool {
    if matches!(mode, LaneMode::AwaitReply | LaneMode::CompletionChecks) {
        return false;
    }
    if outcome.approval_pending
        || unresolved_checks.is_some()
        || outcome.session_id.is_none()
        || outcome.last_message.trim().is_empty()
        || outcome.last_message.trim() == AUTO_CONTINUE_STOP_MARKER
        || outcome.exit_code != Some(0)
    {
        return false;
    }
    match mode {
        LaneMode::Infinite => auto_turns_completed < MAX_INFINITE_AUTO_TURNS,
        LaneMode::MaxTurns => auto_turn_limit
            .map(|limit| auto_turns_completed < limit)
            .unwrap_or(false),
        LaneMode::AwaitReply | LaneMode::CompletionChecks => false,
    }
}

fn configured_extra_turn_budget(
    mode: LaneMode,
    requested_budget: Option<i64>,
    default_max_turns_limit: i64,
) -> i64 {
    match mode {
        LaneMode::MaxTurns => requested_budget
            .filter(|value| *value > 0)
            .unwrap_or(default_max_turns_limit)
            .min(default_max_turns_limit),
        LaneMode::AwaitReply | LaneMode::CompletionChecks | LaneMode::Infinite => 0,
    }
}

fn format_lane_mode_details(mode: LaneMode, extra_turn_budget: i64) -> String {
    match mode {
        LaneMode::MaxTurns => format!("追加ターン上限: {}", extra_turn_budget),
        LaneMode::Infinite => format!("自動継続: 安全上限 {} 回/入力", MAX_INFINITE_AUTO_TURNS),
        LaneMode::AwaitReply | LaneMode::CompletionChecks => "追加ターン上限: なし".to_owned(),
    }
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
        let reply = format_reply_with_failed_checks("修正を試しました。", &failed_summary(), 2);
        assert!(reply.contains("修正を試しました。"));
        assert!(reply.contains("自動継続を 2 回実行"));
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
    fn auto_continue_waiting_message_is_clear() {
        assert_eq!(
            format_auto_continue_waiting_message(0),
            "この依頼は完了しました。次の入力を待ちます。"
        );
        assert!(format_auto_continue_waiting_message(2).contains("自動継続を 2 回実行"));
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
            LaneMode::MaxTurns,
            "必要なら続けてください。",
        );

        assert_eq!(request.image_paths, vec![PathBuf::from("C:/tmp/photo.png")]);
        assert!(request.prompt.contains("確認してください。"));
        assert!(request.prompt.contains("C:/tmp/report.pdf"));
        assert!(request.prompt.contains(AUTO_CONTINUE_STOP_MARKER));
    }

    #[test]
    fn format_status_message_without_lane_is_clear() {
        assert_eq!(
            format_status_message(None, 3),
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
            TelegramControlCommand::Mode {
                mode: "completion_checks".to_owned(),
                max_turns: None,
            },
            3,
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
    fn mode_command_updates_max_turns_budget() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let workspace = test_workspace();

        let reply = handle_control_command(
            &store,
            &workspace,
            10,
            "dm",
            TelegramControlCommand::Mode {
                mode: "max_turns".to_owned(),
                max_turns: Some(5),
            },
            3,
        )
        .expect("mode command should succeed");

        let lane = store
            .find_lane(10, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");
        assert_eq!(lane.mode, LaneMode::MaxTurns);
        assert_eq!(lane.extra_turn_budget, 3);
        assert!(reply.contains("追加ターン上限: 3"));
    }

    #[test]
    fn stop_command_clears_session() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let workspace = test_workspace();
        let lane = store
            .get_or_create_lane(20, "dm", &workspace.id, LaneMode::MaxTurns, 5)
            .expect("lane should be created");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("lane should update");

        let reply = handle_control_command(
            &store,
            &workspace,
            20,
            "dm",
            TelegramControlCommand::Stop,
            3,
        )
        .expect("stop command should succeed");

        let lane = store
            .find_lane(20, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");
        assert_eq!(lane.state, LaneState::Idle);
        assert_eq!(lane.codex_session_id, None);
        assert_eq!(lane.extra_turn_budget, 5);
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

    #[test]
    fn automatic_turn_limit_uses_default_for_zero_budget() {
        assert_eq!(automatic_turn_limit(LaneMode::MaxTurns, 0, 4), Some(4));
        assert_eq!(automatic_turn_limit(LaneMode::MaxTurns, 8, 4), Some(4));
    }

    #[test]
    fn sanitize_auto_continue_message_hides_stop_marker() {
        assert_eq!(
            sanitize_auto_continue_message(AUTO_CONTINUE_STOP_MARKER),
            ""
        );
        assert_eq!(
            sanitize_auto_continue_message("修正しました。"),
            "修正しました。"
        );
    }

    #[test]
    fn should_continue_automatically_for_max_turns_until_limit() {
        let outcome = crate::codex::CodexOutcome {
            session_id: Some("session-1".to_owned()),
            last_message: "続けます".to_owned(),
            exit_code: Some(0),
            approval_pending: false,
        };

        assert!(should_continue_automatically(
            LaneMode::MaxTurns,
            Some(2),
            1,
            &outcome,
            None,
        ));
        assert!(!should_continue_automatically(
            LaneMode::MaxTurns,
            Some(2),
            2,
            &outcome,
            None,
        ));
    }

    #[test]
    fn should_not_continue_automatically_after_non_zero_exit() {
        let outcome = crate::codex::CodexOutcome {
            session_id: Some("session-1".to_owned()),
            last_message: "失敗しました".to_owned(),
            exit_code: Some(1),
            approval_pending: false,
        };

        assert!(!should_continue_automatically(
            LaneMode::Infinite,
            None,
            0,
            &outcome,
            None,
        ));
    }
}
