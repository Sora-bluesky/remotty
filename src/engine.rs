use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use tokio::process::Command as TokioCommand;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::app_server::CodexThreadSummary;
use crate::codex::{CodexRequest, CodexRunner};
use crate::config::{
    CodexTransport, Config, LaneMode, checks::CheckRunSummary, checks::run_profile,
};
use crate::store::{
    ApprovalRequestStatus, ApprovalRequestTransport, CodexThreadBinding, LaneRecord, LaneState,
    NewCodexThreadBinding, NewRun, Store,
};
use crate::telegram::{
    IncomingMessage, SavedTelegramAttachment, TelegramAttachmentKind, TelegramClient,
    TelegramControlCommand, TelegramPoller,
};
use crate::telegram_cli::send_access_pair_code;
use crate::windows_secret::load_secret;

const MAX_COMPLETION_REPAIR_TURNS: usize = 2;
const MAX_TELEGRAM_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
const MAX_INFINITE_AUTO_TURNS: usize = 16;
const AUTO_CONTINUE_STOP_MARKER: &str = "CHANNEL_WAITING";

#[derive(Clone, Default)]
struct ActiveTurnRegistry {
    inner: Arc<Mutex<HashMap<String, ActiveTurnSender>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct ActiveTurnSender {
    id: u64,
    sender: mpsc::UnboundedSender<CodexRequest>,
}

struct ActiveTurnGuard {
    lane_id: String,
    id: u64,
    registry: ActiveTurnRegistry,
}

impl ActiveTurnRegistry {
    fn register(&self, lane_id: &str) -> (ActiveTurnGuard, mpsc::UnboundedReceiver<CodexRequest>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.inner.lock().expect("active turn registry").insert(
            lane_id.to_owned(),
            ActiveTurnSender {
                id,
                sender: sender.clone(),
            },
        );
        (
            ActiveTurnGuard {
                lane_id: lane_id.to_owned(),
                id,
                registry: self.clone(),
            },
            receiver,
        )
    }

    fn send_followup(&self, lane_id: &str, request: CodexRequest) -> bool {
        let sender = self
            .inner
            .lock()
            .expect("active turn registry")
            .get(lane_id)
            .map(|entry| entry.sender.clone());
        sender
            .map(|sender| sender.send(request).is_ok())
            .unwrap_or(false)
    }
}

impl Drop for ActiveTurnGuard {
    fn drop(&mut self) {
        let mut senders = self.registry.inner.lock().expect("active turn registry");
        if senders
            .get(&self.lane_id)
            .map(|entry| entry.id == self.id)
            .unwrap_or(false)
        {
            senders.remove(&self.lane_id);
        }
    }
}

#[async_trait]
trait TelegramApi {
    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
    ) -> Result<crate::telegram::SendMessageResult>;
    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: &str,
        buttons: &[crate::telegram::InlineKeyboardButton],
    ) -> Result<crate::telegram::SendMessageResult>;
    async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
    ) -> Result<()>;
    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<()>;
    async fn edit_message_clearing_inline_keyboard(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<()>;
    async fn save_attachments(
        &self,
        attachments: &[crate::telegram::TelegramAttachment],
        state_dir: &std::path::Path,
        max_bytes: usize,
    ) -> Result<Vec<crate::telegram::SavedTelegramAttachment>>;
}

#[async_trait]
impl TelegramApi for TelegramClient {
    async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
    ) -> Result<crate::telegram::SendMessageResult> {
        TelegramClient::send_message(self, chat_id, text).await
    }

    async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: &str,
        buttons: &[crate::telegram::InlineKeyboardButton],
    ) -> Result<crate::telegram::SendMessageResult> {
        TelegramClient::send_message_with_inline_keyboard(self, chat_id, text, buttons).await
    }

    async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
    ) -> Result<()> {
        TelegramClient::answer_callback_query(self, callback_query_id, text).await
    }

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<()> {
        TelegramClient::edit_message(self, chat_id, message_id, text).await
    }

    async fn edit_message_clearing_inline_keyboard(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<()> {
        TelegramClient::edit_message_clearing_inline_keyboard(self, chat_id, message_id, text).await
    }

    async fn save_attachments(
        &self,
        attachments: &[crate::telegram::TelegramAttachment],
        state_dir: &std::path::Path,
        max_bytes: usize,
    ) -> Result<Vec<crate::telegram::SavedTelegramAttachment>> {
        TelegramClient::save_attachments(self, attachments, state_dir, max_bytes).await
    }
}

struct ApprovalDecisionReply {
    reply_text: String,
    callback_text: String,
}

impl ApprovalDecisionReply {
    fn new(reply_text: impl Into<String>, callback_text: impl Into<String>) -> Self {
        Self {
            reply_text: reply_text.into(),
            callback_text: callback_text.into(),
        }
    }
}

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

    let telegram = TelegramClient::with_base_urls(
        token,
        config.telegram.api_base_url.clone(),
        config.telegram.file_base_url.clone(),
    );
    let poller = TelegramPoller::acquire(telegram.clone()).await?;
    let store = Store::open(&config.storage.db_path)?;
    seed_admin_senders(&store, &config.telegram.admin_sender_ids)?;
    let codex = CodexRunner::new(config.codex.clone());
    let active_turns = ActiveTurnRegistry::default();
    fail_resolving_approval_notifications(&store, &telegram).await;
    invalidate_pending_approval_notifications_for_restart(&store, &telegram).await;

    let mut offset = None;
    loop {
        let updates = tokio::select! {
            _ = shutdown.cancelled() => {
                info!("shutdown requested");
                break;
            }
            result = poller.get_updates(offset, config.service.poll_timeout_sec) => result?,
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

            let sender_id = match authorize_sender_for_update(&store, &update)? {
                Some(sender_id) => sender_id,
                None => {
                    if update.chat_type == "private" {
                        if let Some(sender_id) = update.sender_id {
                            if let Err(error) = send_access_pair_code(
                                &config,
                                &store,
                                &telegram,
                                update.chat_id,
                                sender_id,
                                &update.chat_type,
                            )
                            .await
                            {
                                warn!("failed to send pairing code: {error:#}");
                            }
                        }
                    }
                    continue;
                }
            };

            let chat_id = update.chat_id;
            let task_config = config.clone();
            let task_store = store.clone();
            let task_telegram = telegram.clone();
            let task_codex = codex.clone();
            let task_active_turns = active_turns.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_message(
                    &task_config,
                    &task_store,
                    &task_telegram,
                    &task_codex,
                    &task_active_turns,
                    sender_id,
                    update,
                )
                .await
                {
                    warn!("failed to handle chat {chat_id}: {error:#}");
                    let _ = task_telegram
                        .send_message(chat_id, &format_runtime_failure_message())
                        .await;
                }
            });
        }
        sleep(Duration::from_millis(250)).await;
    }
    Ok(())
}

async fn handle_message(
    config: &Config,
    store: &Store,
    telegram: &impl TelegramApi,
    codex: &CodexRunner,
    active_turns: &ActiveTurnRegistry,
    sender_id: i64,
    update: IncomingMessage,
) -> Result<()> {
    let existing_lane = store.find_lane(update.chat_id, &update.thread_key)?;
    if let Some(command) = update.control_command() {
        let (reply_result, callback_text) = match command {
            TelegramControlCommand::Approve { request_id } => {
                let result = handle_approval_decision_message(
                    config,
                    store,
                    telegram,
                    codex,
                    sender_id,
                    update.chat_id,
                    &update.thread_key,
                    existing_lane.as_ref(),
                    update
                        .callback_query_id
                        .as_ref()
                        .map(|_| update.telegram_message_id),
                    &request_id,
                    true,
                )
                .await;
                let callback_text = match &result {
                    Ok(reply) => reply.callback_text.clone(),
                    Err(_) => "Could not process the approval decision.".to_owned(),
                };
                (result.map(|reply| reply.reply_text), Some(callback_text))
            }
            TelegramControlCommand::Deny { request_id } => {
                let result = handle_approval_decision_message(
                    config,
                    store,
                    telegram,
                    codex,
                    sender_id,
                    update.chat_id,
                    &update.thread_key,
                    existing_lane.as_ref(),
                    update
                        .callback_query_id
                        .as_ref()
                        .map(|_| update.telegram_message_id),
                    &request_id,
                    false,
                )
                .await;
                let callback_text = match &result {
                    Ok(reply) => reply.callback_text.clone(),
                    Err(_) => "Could not process the approval decision.".to_owned(),
                };
                (result.map(|reply| reply.reply_text), Some(callback_text))
            }
            TelegramControlCommand::Sessions { thread_id } => {
                let result = handle_sessions_command(
                    store,
                    config,
                    codex,
                    update.chat_id,
                    &update.thread_key,
                    existing_lane.as_ref(),
                    thread_id.as_deref(),
                )
                .await;
                (result, None)
            }
            other => handle_control_command(
                store,
                config,
                sender_id,
                update.chat_id,
                &update.thread_key,
                existing_lane.as_ref(),
                other,
                config.policy.max_turns_limit,
            )
            .map(|reply| (Ok(reply), None))?,
        };
        if let Some(callback_query_id) = update.callback_query_id.as_deref() {
            let _ = telegram
                .answer_callback_query(
                    callback_query_id,
                    Some(callback_text.as_deref().unwrap_or("操作を処理しました。")),
                )
                .await;
        }
        let reply = reply_result?;
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

    if config.codex.transport == CodexTransport::AppServer {
        if let Some(lane) = existing_lane.as_ref() {
            if lane.state == LaneState::Running {
                store.insert_message(
                    &lane.lane_id,
                    None,
                    "inbound",
                    "telegram_steer",
                    Some(update.telegram_message_id),
                    Some(&update.text),
                    Some(&update.payload_json),
                )?;
                let reply = if !update.attachments.is_empty() {
                    "処理中のターンへ送れる追加入力はテキストだけです。添付は完了後に送ってください。"
                        .to_owned()
                } else if active_turns
                    .send_followup(&lane.lane_id, CodexRequest::new(update.text.clone()))
                {
                    "実行中のターンへ追加入力を送りました。".to_owned()
                } else {
                    "現在の処理が続いています。完了後にもう一度送ってください。".to_owned()
                };
                let sent = telegram.send_message(update.chat_id, &reply).await?;
                store.insert_message(
                    &lane.lane_id,
                    None,
                    "outbound",
                    "telegram_steer_status",
                    Some(sent.message_id),
                    Some(&reply),
                    None,
                )?;
                return Ok(());
            }
        }
    }

    let saved_thread_binding = if config.codex.transport == CodexTransport::AppServer {
        store.find_codex_thread_binding(update.chat_id, &update.thread_key)?
    } else {
        None
    };
    let workspace = resolve_workspace_for_message(
        config,
        existing_lane.as_ref(),
        saved_thread_binding.as_ref(),
    )?;
    let lane = store.get_or_create_lane(
        update.chat_id,
        &update.thread_key,
        &workspace.id,
        workspace.default_mode,
        configured_extra_turn_budget(workspace.default_mode, None, config.policy.max_turns_limit),
    )?;
    let selected_session_id =
        selected_codex_session_id(&lane, saved_thread_binding.as_ref()).map(ToOwned::to_owned);

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
        selected_session_id.as_deref(),
    )?;
    let run = store.insert_run(NewRun {
        lane_id: lane.lane_id.clone(),
        run_kind: if selected_session_id.is_some() {
            "resume".to_owned()
        } else {
            "start".to_owned()
        },
    })?;

    warn_if_workspace_has_uncommitted_changes(
        config,
        store,
        telegram,
        update.chat_id,
        &lane.lane_id,
        &run.run_id,
        workspace,
    )
    .await?;

    let progress_text = format_processing_message(selected_session_id.is_some());
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
    let active_turn = if config.codex.transport == CodexTransport::AppServer {
        let (guard, followups) = active_turns.register(&lane.lane_id);
        Some((guard, followups))
    } else {
        None
    };
    let initial_outcome = if let Some(session_id) = selected_session_id.as_deref() {
        if let Some((guard, followups)) = active_turn {
            let outcome = codex
                .resume_with_followups(workspace, session_id, request, Some(followups))
                .await?;
            drop(guard);
            outcome
        } else {
            codex.resume(workspace, session_id, request).await?
        }
    } else {
        if let Some((guard, followups)) = active_turn {
            let outcome = codex
                .start_with_followups(workspace, request, Some(followups))
                .await?;
            drop(guard);
            outcome
        } else {
            codex.start(workspace, request).await?
        }
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
    if let Err(error) = persist_approval_requests(
        store,
        telegram,
        update.chat_id,
        &lane.lane_id,
        &run.run_id,
        config.codex.transport,
        &outcome.approval_requests,
    )
    .await
    {
        store.update_lane_state(
            &lane.lane_id,
            LaneState::Failed,
            outcome.session_id.as_deref(),
        )?;
        store.finish_run(
            &run.run_id,
            outcome.exit_code,
            LaneState::Failed.as_str(),
            false,
            outcome.approval_request_count,
            outcome.approval_resolved_count,
        )?;
        return Err(error);
    }
    store.update_lane_state(&lane.lane_id, next_state, outcome.session_id.as_deref())?;
    store.finish_run(
        &run.run_id,
        outcome.exit_code,
        next_state.as_str(),
        outcome.approval_pending,
        outcome.approval_request_count,
        outcome.approval_resolved_count,
    )?;

    info!("handled lane {}", lane.lane_id);
    Ok(())
}

fn handle_control_command(
    store: &Store,
    config: &Config,
    _sender_id: i64,
    chat_id: i64,
    thread_key: &str,
    lane: Option<&LaneRecord>,
    command: TelegramControlCommand,
    default_max_turns_limit: i64,
) -> Result<String> {
    match command {
        TelegramControlCommand::Help => Ok(format_help_message()),
        TelegramControlCommand::Status => Ok(format_status_message(
            lane.cloned(),
            default_max_turns_limit,
            lane.map(|lane| {
                store
                    .list_unresolved_approval_requests_for_lane(&lane.lane_id)
                    .map(|requests| {
                        requests
                            .into_iter()
                            .map(|request| request.request_id)
                            .collect::<Vec<_>>()
                    })
            })
            .transpose()?
            .unwrap_or_default(),
        )),
        TelegramControlCommand::Stop => {
            let Some(lane) = lane else {
                return Ok("停止する対象はありません。".to_owned());
            };
            store.clear_lane_session(&lane.lane_id)?;
            Ok("現在のセッションを止めました。次の入力は新しい開始として扱います。".to_owned())
        }
        TelegramControlCommand::Workspace { workspace_id } => handle_workspace_command(
            store,
            config,
            chat_id,
            thread_key,
            lane,
            workspace_id.as_deref(),
            default_max_turns_limit,
        ),
        TelegramControlCommand::Mode { mode, max_turns } => {
            let workspace = resolve_workspace(config, lane)?;
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
        TelegramControlCommand::Sessions { .. } => Ok(
            "This internal path does not handle session selection. Use the async handler."
                .to_owned(),
        ),
        TelegramControlCommand::Approve { .. } | TelegramControlCommand::Deny { .. } => Ok(
            "This internal path does not handle approval decisions. Use a normal Telegram message or button."
                .to_owned(),
        ),
    }
}

async fn handle_sessions_command(
    store: &Store,
    config: &Config,
    codex: &CodexRunner,
    chat_id: i64,
    thread_key: &str,
    lane: Option<&LaneRecord>,
    thread_id: Option<&str>,
) -> Result<String> {
    let Some(thread_id) = thread_id.map(str::trim).filter(|value| !value.is_empty()) else {
        let threads = codex.list_threads(10, None).await?;
        return Ok(format_sessions_message(&threads));
    };

    let threads = codex.list_threads(25, Some(thread_id)).await?;
    let Some(selected) = find_selected_thread(&threads, thread_id) else {
        return Ok(format_thread_not_found_message(thread_id));
    };
    let workspace = resolve_workspace(config, lane)?;
    store.upsert_codex_thread_binding(NewCodexThreadBinding {
        chat_id,
        thread_key: thread_key.to_owned(),
        codex_thread_id: selected.thread_id.clone(),
        workspace_id: workspace.id.clone(),
        title: selected.title.clone(),
        cwd: selected.cwd.clone(),
        model: selected.model.clone(),
        codex_updated_at: selected.updated_at.clone(),
    })?;
    Ok(format!(
        "この会話を Codex スレッド `{}` に対応付けました。\n次の入力から、このスレッドへ戻せるようになります。",
        selected.thread_id
    ))
}

fn find_selected_thread<'a>(
    threads: &'a [CodexThreadSummary],
    thread_id: &str,
) -> Option<&'a CodexThreadSummary> {
    threads
        .iter()
        .find(|thread| thread.thread_id.eq_ignore_ascii_case(thread_id))
        .or_else(|| {
            threads
                .iter()
                .find(|thread| thread.thread_id.starts_with(thread_id))
        })
}

async fn handle_approval_decision_message(
    config: &Config,
    store: &Store,
    telegram: &impl TelegramApi,
    codex: &CodexRunner,
    sender_id: i64,
    chat_id: i64,
    thread_key: &str,
    lane: Option<&LaneRecord>,
    callback_message_id: Option<i64>,
    request_id: &str,
    approved: bool,
) -> Result<ApprovalDecisionReply> {
    let request = match find_approval_request_for_locator(store, request_id)? {
        ApprovalRequestLookup::Found(request) => request,
        ApprovalRequestLookup::Stale(request) => {
            let text = format!(
                "Approval request `{}` is stale. Use the latest notification.",
                request.request_id
            );
            if let Some(message_id) = callback_message_id {
                let _ = telegram
                    .edit_message_clearing_inline_keyboard(chat_id, message_id, &text)
                    .await;
            }
            return Ok(ApprovalDecisionReply::new(
                text,
                "This button is stale. Use the latest notification.",
            ));
        }
        ApprovalRequestLookup::Missing => {
            let text = format!("Approval request `{request_id}` was not found.");
            if let Some(message_id) = callback_message_id {
                let _ = telegram
                    .edit_message_clearing_inline_keyboard(chat_id, message_id, &text)
                    .await;
            }
            return Ok(ApprovalDecisionReply::new(
                text,
                "Approval request not found.",
            ));
        }
    };
    let request_id = request.request_id.as_str();
    let Some(lane) = lane else {
        return Ok(ApprovalDecisionReply::new(
            "This chat has no lane for the approval request.",
            "This chat cannot handle that request.",
        ));
    };
    if lane.lane_id != request.lane_id {
        return Ok(ApprovalDecisionReply::new(
            "This approval request belongs to a different chat.",
            "Wrong chat for this request.",
        ));
    }
    let current_lane = store
        .find_lane(chat_id, thread_key)?
        .ok_or_else(|| anyhow!("approval lane not found"))?;
    if current_lane.lane_id != request.lane_id {
        return Ok(ApprovalDecisionReply::new(
            "This approval request belongs to a different chat.",
            "Wrong chat for this request.",
        ));
    }
    if request.request_kind == crate::store::ApprovalRequestKind::ToolUserInput {
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` needs additional input and cannot be handled from Telegram yet."
            ),
            "This request type cannot be answered from Telegram.",
        ));
    }

    let next_status = if approved {
        ApprovalRequestStatus::Approved
    } else {
        ApprovalRequestStatus::Declined
    };
    if request.status == ApprovalRequestStatus::Dispatching {
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was just sent. Wait a few seconds and try again."
            ),
            "Wait a few seconds and try again.",
        ));
    }
    if request.status != ApprovalRequestStatus::Pending {
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was already handled. Current status: `{}`",
                approval_status_name(request.status)
            ),
            format!("Already `{}`.", approval_status_name(request.status)),
        ));
    }

    if request.transport == ApprovalRequestTransport::Exec {
        let updated = store.resolve_approval_request(request_id, next_status, sender_id)?;
        if !updated {
            let current_status = store
                .find_approval_request(request_id)?
                .map(|current| approval_status_name(current.status).to_owned())
                .unwrap_or_else(|| "unknown".to_owned());
            return Ok(ApprovalDecisionReply::new(
                format!(
                    "Approval request `{request_id}` was already handled. Current status: `{current_status}`"
                ),
                format!("Already `{current_status}`."),
            ));
        }
        if let Some(message_id) = request.telegram_message_id {
            let status_text = format_approval_resolution_message(request_id, approved);
            let _ = telegram
                .edit_message_clearing_inline_keyboard(chat_id, message_id, &status_text)
                .await;
        }
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was recorded as {}. Enable `app_server` to resume the same turn.",
                if approved { "approved" } else { "declined" }
            ),
            if approved {
                "Approval recorded.".to_owned()
            } else {
                "Decline recorded.".to_owned()
            },
        ));
    }
    if request.transport_request_id.trim().is_empty() {
        let _ = store.expire_approval_request(request_id, &lane.lane_id, &request.run_id)?;
        if let Some(message_id) = request.telegram_message_id {
            let text = format!(
                "Approval request `{request_id}` used an old format and was invalidated. Send the original request again."
            );
            let _ = telegram
                .edit_message_clearing_inline_keyboard(chat_id, message_id, &text)
                .await;
        }
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` uses an old format and cannot continue. Send the original request again."
            ),
            "Old-format request invalidated.".to_owned(),
        ));
    }

    let workspace = resolve_workspace(config, Some(lane))?;
    let updated = store.begin_approval_resolution(request_id, sender_id)?;
    if !updated {
        let current_status = store
            .find_approval_request(request_id)?
            .map(|current| approval_status_name(current.status).to_owned())
            .unwrap_or_else(|| "unknown".to_owned());
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was already handled. Current status: `{current_status}`"
            ),
            format!("Already `{current_status}`."),
        ));
    }
    let continued_outcome = match codex.resolve_approval(&request, approved).await {
        Ok(outcome) => outcome,
        Err(error) => {
            warn!(
                "failed to resume app-server turn after approval {}: {error:#}",
                request_id
            );
            let _ = store.fail_resolving_approval_request(
                request_id,
                &lane.lane_id,
                &request.run_id,
            )?;
            if let Some(message_id) = request.telegram_message_id {
                let text = format!(
                    "Approval request `{request_id}` has an unknown continuation result. Check local logs before deciding what to do."
                );
                let _ = telegram
                    .edit_message_clearing_inline_keyboard(chat_id, message_id, &text)
                    .await;
            }
            return Ok(ApprovalDecisionReply::new(
                format!(
                    "Approval request `{request_id}` has an unknown continuation result. Check local logs and do not resend until the state is clear."
                ),
                "Continuation result unknown. Check local logs.".to_owned(),
            ));
        }
    };
    let resolved = store.resolve_approval_request(request_id, next_status, sender_id)?;
    if !resolved {
        let current_status = store
            .find_approval_request(request_id)?
            .map(|current| approval_status_name(current.status).to_owned())
            .unwrap_or_else(|| "unknown".to_owned());
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was already handled. Current status: `{current_status}`"
            ),
            format!("Already `{current_status}`."),
        ));
    }
    if let Some(message_id) = request.telegram_message_id {
        let status_text = format_approval_resolution_message(request_id, approved);
        let _ = telegram
            .edit_message_clearing_inline_keyboard(chat_id, message_id, &status_text)
            .await;
    }
    let continued_session_id = continued_outcome.session_id.clone();
    let (outcome, unresolved_checks, auto_turns_completed) = match continue_lane_after_completion(
        config,
        workspace,
        codex,
        lane,
        continued_outcome,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            warn!(
                "failed to finish post-approval processing for {}: {error:#}",
                request_id
            );
            store.update_lane_state(
                &lane.lane_id,
                LaneState::Failed,
                continued_session_id.as_deref(),
            )?;
            return Ok(ApprovalDecisionReply::new(
                format!(
                    "Approval request `{request_id}` was recorded, but post-approval processing failed. Check local logs."
                ),
                "Post-approval processing failed. Check local logs.".to_owned(),
            ));
        }
    };

    let run = store.insert_run(NewRun {
        lane_id: lane.lane_id.clone(),
        run_kind: "approval_resume".to_owned(),
    })?;
    let next_state = if outcome.approval_pending {
        LaneState::NeedsLocalApproval
    } else if unresolved_checks.is_some() {
        LaneState::Failed
    } else {
        LaneState::WaitingReply
    };
    if let Err(_error) = persist_approval_requests(
        store,
        telegram,
        chat_id,
        &lane.lane_id,
        &run.run_id,
        config.codex.transport,
        &outcome.approval_requests,
    )
    .await
    {
        let _ = store.finish_run(
            &run.run_id,
            outcome.exit_code,
            LaneState::Failed.as_str(),
            false,
            outcome.approval_request_count,
            outcome.approval_resolved_count,
        );
        store.update_lane_state(
            &lane.lane_id,
            LaneState::Failed,
            outcome.session_id.as_deref(),
        )?;
        return Ok(ApprovalDecisionReply::new(
            format!(
                "Approval request `{request_id}` was recorded, but saving the next approval notification failed. Check local logs."
            ),
            "Failed to save the next approval notification.".to_owned(),
        ));
    }
    store.update_lane_state(&lane.lane_id, next_state, outcome.session_id.as_deref())?;
    store.finish_run(
        &run.run_id,
        outcome.exit_code,
        next_state.as_str(),
        outcome.approval_pending,
        outcome.approval_request_count,
        outcome.approval_resolved_count,
    )?;

    if outcome.approval_pending {
        return Ok(ApprovalDecisionReply::new(
            format!(
                "{} Sent the next approval request.",
                format_approval_resolution_message(request_id, approved)
            ),
            if approved {
                "Approval applied; sent the next approval request.".to_owned()
            } else {
                "Decline applied; sent the next approval request.".to_owned()
            },
        ));
    }

    let reply = if outcome.approval_pending {
        format_approval_pending_message(outcome.approval_requests.len())
    } else if let Some(summary) = unresolved_checks.as_ref() {
        truncate(
            &format_reply_with_failed_checks(&outcome.last_message, summary, auto_turns_completed),
            config.policy.max_output_chars,
        )
    } else if outcome.last_message.trim().is_empty() {
        "Could not read the response after approval. Check local logs.".to_owned()
    } else {
        truncate(&outcome.last_message, config.policy.max_output_chars)
    };
    Ok(ApprovalDecisionReply::new(
        reply,
        if approved {
            "Approval applied.".to_owned()
        } else {
            "Decline applied.".to_owned()
        },
    ))
}

fn handle_workspace_command(
    store: &Store,
    config: &Config,
    chat_id: i64,
    thread_key: &str,
    lane: Option<&LaneRecord>,
    requested_workspace_id: Option<&str>,
    default_max_turns_limit: i64,
) -> Result<String> {
    let current_workspace_id = lane
        .map(|lane| lane.workspace_id.as_str())
        .unwrap_or(config.default_workspace().id.as_str());

    let Some(requested_workspace_id) = requested_workspace_id else {
        return Ok(format_workspace_message(
            current_workspace_id,
            &config.workspaces,
            default_max_turns_limit,
            lane.map(|lane| lane.mode),
            lane.map(|lane| lane.extra_turn_budget),
        ));
    };

    let workspace = config.workspace(requested_workspace_id).ok_or_else(|| {
        anyhow!("不明な workspace です。`/workspace` で利用可能な一覧を確認してください。")
    })?;

    if let Some(lane) = lane {
        if lane.workspace_id == workspace.id {
            return Ok(format!(
                "この会話はすでに workspace `{}` を使っています。",
                workspace.id
            ));
        }
        store.update_lane_workspace(&lane.lane_id, &workspace.id)?;
    } else {
        store.get_or_create_lane(
            chat_id,
            thread_key,
            &workspace.id,
            workspace.default_mode,
            configured_extra_turn_budget(workspace.default_mode, None, default_max_turns_limit),
        )?;
    }

    Ok(format!(
        "この会話の workspace を `{}` に更新しました。進行中のセッションはリセットしました。",
        workspace.id
    ))
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

async fn warn_if_workspace_has_uncommitted_changes(
    config: &Config,
    store: &Store,
    telegram: &impl TelegramApi,
    chat_id: i64,
    lane_id: &str,
    run_id: &str,
    workspace: &crate::config::WorkspaceConfig,
) -> Result<()> {
    if config.codex.transport != CodexTransport::AppServer {
        return Ok(());
    }

    match workspace_has_uncommitted_changes(&workspace.path).await {
        Ok(true) => {
            let text = format_workspace_dirty_warning();
            let sent = telegram.send_message(chat_id, &text).await?;
            store.insert_message(
                lane_id,
                Some(run_id),
                "outbound",
                "telegram_workspace_warning",
                Some(sent.message_id),
                Some(&text),
                None,
            )?;
        }
        Ok(false) => {}
        Err(error) => {
            warn!(
                "failed to inspect workspace {}: {error:#}",
                workspace.path.display()
            );
        }
    }
    Ok(())
}

async fn workspace_has_uncommitted_changes(path: &Path) -> Result<bool> {
    let inside = TokioCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .await?;
    if !inside.status.success() {
        return Ok(false);
    }

    let status = TokioCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(["status", "--porcelain=v1", "--untracked-files=normal"])
        .output()
        .await?;
    if !status.status.success() {
        return Err(anyhow!(
            "git status failed with exit code {:?}",
            status.status.code()
        ));
    }
    Ok(!status.stdout.is_empty())
}

fn format_workspace_dirty_warning() -> String {
    "このリポジトリには未コミットの変更があります。remotty は処理を続けます。必要なら完了後に `git status` を確認してください。"
        .to_owned()
}

fn format_runtime_failure_message() -> String {
    "処理中に失敗しました。少し待ってから再送してください。必要ならローカルのログを確認します。"
        .to_owned()
}

fn format_approval_pending_message(request_count: usize) -> String {
    if request_count <= 1 {
        "Approval is pending. Use the approval message below to continue.".to_owned()
    } else {
        format!(
            "Approval is pending. {request_count} approval messages were sent; handle them in order."
        )
    }
}

fn format_approval_resolution_message(request_id: &str, approved: bool) -> String {
    format!(
        "Approval request `{request_id}` was {}.",
        if approved { "approved" } else { "declined" }
    )
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
        "/approve <request_id>",
        "/deny <request_id>",
        "/workspace",
        "/workspace <id>",
        "/remotty-sessions",
        "/remotty-sessions <thread_id>",
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
    pending_request_ids: Vec<String>,
) -> String {
    let Some(lane) = lane else {
        return "この会話にはまだレーンがありません。".to_owned();
    };

    let session = if lane.codex_session_id.is_some() {
        "yes"
    } else {
        "no"
    };
    let configured_budget = configured_extra_turn_budget(
        lane.mode,
        Some(lane.extra_turn_budget),
        default_max_turns_limit,
    );
    let latest_request_id = pending_request_ids
        .last()
        .map(String::as_str)
        .unwrap_or("none");
    format!(
        "status: `{}`\nworkspace: `{}`\nmode: `{}`\n{}\nsession: {}\npending approval requests: {}\nlatest approval request: {}",
        lane_state_name(lane.state),
        lane.workspace_id,
        lane_mode_name(lane.mode),
        format_lane_mode_details(lane.mode, configured_budget),
        session,
        pending_request_ids.len(),
        latest_request_id
    )
}

fn format_workspace_message(
    current_workspace_id: &str,
    workspaces: &[crate::config::WorkspaceConfig],
    default_max_turns_limit: i64,
    lane_mode: Option<LaneMode>,
    extra_turn_budget: Option<i64>,
) -> String {
    let mode = lane_mode.unwrap_or(workspaces[0].default_mode);
    let configured_budget =
        configured_extra_turn_budget(mode, extra_turn_budget, default_max_turns_limit);
    format!(
        "現在の workspace: `{}`\n利用可能:\n{}\n現在のモード: `{}`\n{}\n切り替え: `/workspace <id>`",
        current_workspace_id,
        workspaces
            .iter()
            .map(|workspace| format!("- `{}`", workspace.id))
            .collect::<Vec<_>>()
            .join("\n"),
        lane_mode_name(mode),
        format_lane_mode_details(mode, configured_budget)
    )
}

fn format_sessions_message(threads: &[CodexThreadSummary]) -> String {
    if threads.is_empty() {
        return "保存済みの Codex スレッドは見つかりませんでした。".to_owned();
    }
    let mut lines = vec!["保存済みの Codex スレッド:".to_owned()];
    for thread in threads.iter().take(10) {
        let title = thread.title.as_deref().unwrap_or("タイトルなし");
        lines.push(format!("- `{}` {}", thread.thread_id, title));
    }
    lines.push("選択: `/remotty-sessions <thread_id>`".to_owned());
    lines.join("\n")
}

fn format_thread_not_found_message(thread_id: &str) -> String {
    format!(
        "Codex スレッド `{thread_id}` は見つかりませんでした。\n`/remotty-sessions` で最新の一覧を確認し、表示された ID を指定してください。"
    )
}

async fn persist_approval_requests(
    store: &Store,
    telegram: &impl TelegramApi,
    chat_id: i64,
    lane_id: &str,
    run_id: &str,
    transport: CodexTransport,
    requests: &[crate::app_server::CodexApprovalRequest],
) -> Result<()> {
    let mut dispatched = Vec::new();
    for request in requests {
        let should_dispatch =
            store.prepare_approval_request_for_dispatch(crate::store::NewApprovalRequest {
                request_id: request.request_id.clone(),
                transport_request_id: request.transport_request_id.clone(),
                lane_id: lane_id.to_owned(),
                run_id: run_id.to_owned(),
                thread_id: request.thread_id.clone(),
                turn_id: request.turn_id.clone(),
                item_id: request.item_id.clone(),
                transport: approval_request_transport(transport),
                request_kind: request.request_kind,
                summary_text: request.summary_text.clone(),
                raw_payload_json: request.raw_payload_json.clone(),
                status: ApprovalRequestStatus::Dispatching,
            })?;
        if !should_dispatch {
            continue;
        }

        let stored_request = store
            .find_approval_request(&request.request_id)?
            .ok_or_else(|| {
                anyhow!(
                    "approval request row missing immediately after insert: {}",
                    request.request_id
                )
            })?;
        let notice = build_approval_notice(&stored_request);
        let sent = match &notice.buttons {
            Some(buttons) => {
                telegram
                    .send_message_with_inline_keyboard(chat_id, &notice.text, buttons)
                    .await
            }
            None => telegram.send_message(chat_id, &notice.text).await,
        };
        let sent = match sent {
            Ok(sent) => sent,
            Err(error) => {
                let _ = store.invalidate_approval_request(&request.request_id)?;
                invalidate_dispatched_approval_notifications(
                    store,
                    telegram,
                    chat_id,
                    lane_id,
                    run_id,
                    &dispatched,
                )
                .await;
                return Err(error);
            }
        };
        let current_notice = DispatchedApprovalNotice {
            request_id: request.request_id.clone(),
            message_id: sent.message_id,
            text: notice.text.clone(),
        };
        if let Err(error) = store
            .set_approval_request_message_id(&request.request_id, sent.message_id)
            .and_then(|_| {
                store.insert_message(
                    lane_id,
                    Some(run_id),
                    "outbound",
                    "telegram_approval_request",
                    Some(sent.message_id),
                    Some(&notice.text),
                    None,
                )
            })
            .and_then(|_| store.mark_approval_request_pending(&request.request_id, sent.message_id))
            .and_then(|updated| {
                if !updated {
                    return Err(anyhow!(
                        "approval request {} could not move to pending",
                        request.request_id
                    ));
                }
                Ok(())
            })
        {
            let mut notices_to_invalidate = dispatched;
            notices_to_invalidate.push(current_notice);
            invalidate_dispatched_approval_notifications(
                store,
                telegram,
                chat_id,
                lane_id,
                run_id,
                &notices_to_invalidate,
            )
            .await;
            return Err(error);
        }
        dispatched.push(current_notice);
    }
    Ok(())
}

async fn invalidate_pending_approval_notifications_for_restart(
    store: &Store,
    telegram: &impl TelegramApi,
) {
    let pending = match store
        .invalidate_pending_approval_notifications_for_restart(ApprovalRequestTransport::AppServer)
    {
        Ok(pending) => pending,
        Err(error) => {
            warn!("failed to invalidate pending approval notifications: {error:#}");
            return;
        }
    };

    for pending_request in pending {
        let text = format!(
            "Approval request `{}` was invalidated by a bridge restart. Send the original request again.",
            pending_request.request.request_id
        );
        let should_send_new_message = match pending_request.request.telegram_message_id {
            Some(message_id) => telegram
                .edit_message_clearing_inline_keyboard(pending_request.chat_id, message_id, &text)
                .await
                .is_err(),
            None => true,
        };
        if !should_send_new_message {
            if let Err(error) = store.insert_message(
                &pending_request.request.lane_id,
                Some(&pending_request.request.run_id),
                "outbound",
                "telegram_approval_invalidated_on_restart",
                pending_request.request.telegram_message_id,
                Some(&text),
                None,
            ) {
                warn!(
                    "failed to record invalidated approval edit {}: {error:#}",
                    pending_request.request.request_id
                );
            }
            continue;
        }
        match telegram.send_message(pending_request.chat_id, &text).await {
            Ok(sent) => {
                if let Err(error) = store.insert_message(
                    &pending_request.request.lane_id,
                    Some(&pending_request.request.run_id),
                    "outbound",
                    "telegram_approval_invalidated_on_restart",
                    Some(sent.message_id),
                    Some(&text),
                    None,
                ) {
                    warn!(
                        "failed to record invalidated approval request {}: {error:#}",
                        pending_request.request.request_id
                    );
                }
            }
            Err(error) => {
                warn!(
                    "failed to notify invalidated approval request {}: {error:#}",
                    pending_request.request.request_id
                );
            }
        }
    }
}

async fn fail_resolving_approval_notifications(store: &Store, telegram: &impl TelegramApi) {
    let resolving = match store
        .list_recent_resolving_approval_notifications(ApprovalRequestTransport::AppServer, i64::MIN)
    {
        Ok(resolving) => resolving,
        Err(error) => {
            warn!("failed to load resolving approval notifications: {error:#}");
            return;
        }
    };

    for notification in resolving {
        let failed = match store.fail_resolving_approval_request(
            &notification.request.request_id,
            &notification.request.lane_id,
            &notification.request.run_id,
        ) {
            Ok(failed) => failed,
            Err(error) => {
                warn!(
                    "failed to invalidate resolving approval request {}: {error:#}",
                    notification.request.request_id
                );
                continue;
            }
        };
        if !failed {
            continue;
        }

        let text = format!(
            "Approval request `{}` could not be finalized during bridge restart. The state is unknown; check local logs before resending the original request.",
            notification.request.request_id
        );
        let should_send_new_message = match notification.request.telegram_message_id {
            Some(message_id) => telegram
                .edit_message_clearing_inline_keyboard(notification.chat_id, message_id, &text)
                .await
                .is_err(),
            None => true,
        };
        if !should_send_new_message {
            if let Err(error) = store.insert_message(
                &notification.request.lane_id,
                Some(&notification.request.run_id),
                "outbound",
                "telegram_approval_inconclusive",
                notification.request.telegram_message_id,
                Some(&text),
                None,
            ) {
                warn!(
                    "failed to record inconclusive approval {}: {error:#}",
                    notification.request.request_id
                );
            }
            continue;
        }
        match telegram.send_message(notification.chat_id, &text).await {
            Ok(sent) => {
                if let Err(error) = store.insert_message(
                    &notification.request.lane_id,
                    Some(&notification.request.run_id),
                    "outbound",
                    "telegram_approval_inconclusive",
                    Some(sent.message_id),
                    Some(&text),
                    None,
                ) {
                    warn!(
                        "failed to record inconclusive approval {}: {error:#}",
                        notification.request.request_id
                    );
                }
            }
            Err(error) => {
                warn!(
                    "failed to notify inconclusive approval {}: {error:#}",
                    notification.request.request_id
                );
            }
        }
    }
}

async fn invalidate_dispatched_approval_notifications(
    store: &Store,
    telegram: &impl TelegramApi,
    chat_id: i64,
    lane_id: &str,
    run_id: &str,
    dispatched: &[DispatchedApprovalNotice],
) {
    for dispatched_notice in dispatched {
        if let Err(error) = store.invalidate_approval_request(&dispatched_notice.request_id) {
            warn!(
                "failed to invalidate partially dispatched approval {}: {error:#}",
                dispatched_notice.request_id
            );
        }
        let invalidated_text = format!(
            "{}\n\nThis approval notification was invalidated by an internal error. Send the original request again.",
            dispatched_notice.text
        );
        let _ = telegram
            .edit_message_clearing_inline_keyboard(
                chat_id,
                dispatched_notice.message_id,
                &invalidated_text,
            )
            .await;
        if let Err(error) = store.insert_message(
            lane_id,
            Some(run_id),
            "outbound",
            "telegram_approval_invalidated",
            Some(dispatched_notice.message_id),
            Some(&invalidated_text),
            None,
        ) {
            warn!(
                "failed to record invalidated approval {}: {error:#}",
                dispatched_notice.request_id
            );
        }
    }
}

struct DispatchedApprovalNotice {
    request_id: String,
    message_id: i64,
    text: String,
}

enum ApprovalRequestLookup {
    Found(crate::store::ApprovalRequestRecord),
    Stale(crate::store::ApprovalRequestRecord),
    Missing,
}

struct ApprovalNotice {
    text: String,
    buttons: Option<Vec<crate::telegram::InlineKeyboardButton>>,
}

fn find_approval_request_for_locator(
    store: &Store,
    locator: &str,
) -> Result<ApprovalRequestLookup> {
    if let Some((request_key, dispatch_version)) = locator.split_once(':') {
        for candidate in approval_request_id_candidates(request_key) {
            if let Some(request) = store.find_approval_request(&candidate)? {
                if approval_dispatch_version(request.requested_at_ms) == dispatch_version {
                    return Ok(ApprovalRequestLookup::Found(request));
                }
                return Ok(ApprovalRequestLookup::Stale(request));
            }
        }
        return Ok(ApprovalRequestLookup::Missing);
    }

    Ok(store
        .find_approval_request(locator)?
        .map(ApprovalRequestLookup::Found)
        .unwrap_or(ApprovalRequestLookup::Missing))
}

fn approval_request_id_candidates(request_key: &str) -> Vec<String> {
    if request_key.starts_with("approval-") {
        return vec![request_key.to_owned()];
    }

    vec![format!("approval-{request_key}"), request_key.to_owned()]
}

fn approval_callback_locator(request_id: &str, requested_at_ms: i64) -> String {
    format!(
        "{}:{}",
        request_id.strip_prefix("approval-").unwrap_or(request_id),
        approval_dispatch_version(requested_at_ms)
    )
}

fn approval_dispatch_version(requested_at_ms: i64) -> String {
    encode_base36(requested_at_ms.max(0) as u64)
}

fn encode_base36(mut value: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_owned();
    }

    let mut encoded = Vec::new();
    while value > 0 {
        encoded.push(DIGITS[(value % 36) as usize] as char);
        value /= 36;
    }
    encoded.reverse();
    encoded.into_iter().collect()
}

fn build_approval_notice(request: &crate::store::ApprovalRequestRecord) -> ApprovalNotice {
    build_approval_notice_parts(
        &request.summary_text,
        &request.request_id,
        request.requested_at_ms,
        request.request_kind,
    )
}

fn build_approval_notice_parts(
    summary_text: &str,
    request_id: &str,
    requested_at_ms: i64,
    request_kind: crate::store::ApprovalRequestKind,
) -> ApprovalNotice {
    if request_kind == crate::store::ApprovalRequestKind::ToolUserInput {
        return ApprovalNotice {
            text: format!(
                "{summary_text}\n\nRequest ID: `{request_id}`\n\nThis request type cannot be answered from Telegram yet. Additional local input is required."
            ),
            buttons: None,
        };
    }

    let callback_target = approval_callback_locator(request_id, requested_at_ms);
    ApprovalNotice {
        text: format!("{summary_text}\n\nRequest ID: `{request_id}`"),
        buttons: Some(vec![
            crate::telegram::InlineKeyboardButton {
                text: "Approve".to_owned(),
                callback_data: format!("approve:{callback_target}"),
            },
            crate::telegram::InlineKeyboardButton {
                text: "Deny".to_owned(),
                callback_data: format!("deny:{callback_target}"),
            },
        ]),
    }
}

fn resolve_workspace<'a>(
    config: &'a Config,
    lane: Option<&LaneRecord>,
) -> Result<&'a crate::config::WorkspaceConfig> {
    let workspace_id = lane
        .map(|lane| lane.workspace_id.as_str())
        .unwrap_or(config.default_workspace().id.as_str());
    config
        .workspace(workspace_id)
        .ok_or_else(|| anyhow!("workspace `{workspace_id}` が設定に見つかりません。"))
}

fn resolve_workspace_for_message<'a>(
    config: &'a Config,
    lane: Option<&LaneRecord>,
    binding: Option<&CodexThreadBinding>,
) -> Result<&'a crate::config::WorkspaceConfig> {
    let workspace_id = lane
        .map(|lane| lane.workspace_id.as_str())
        .or_else(|| binding.map(|binding| binding.workspace_id.as_str()))
        .unwrap_or(config.default_workspace().id.as_str());
    config
        .workspace(workspace_id)
        .ok_or_else(|| anyhow!("workspace `{workspace_id}` が設定に見つかりません。"))
}

fn selected_codex_session_id<'a>(
    lane: &'a LaneRecord,
    binding: Option<&'a CodexThreadBinding>,
) -> Option<&'a str> {
    lane.codex_session_id
        .as_deref()
        .or_else(|| binding.map(|binding| binding.codex_thread_id.as_str()))
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

fn approval_status_name(status: ApprovalRequestStatus) -> &'static str {
    match status {
        ApprovalRequestStatus::Dispatching => "dispatching",
        ApprovalRequestStatus::Pending => "pending",
        ApprovalRequestStatus::Resolving => "resolving",
        ApprovalRequestStatus::Invalidated => "invalidated",
        ApprovalRequestStatus::Approved => "approved",
        ApprovalRequestStatus::Declined => "declined",
        ApprovalRequestStatus::TimedOut => "timed_out",
    }
}

fn approval_request_transport(transport: CodexTransport) -> ApprovalRequestTransport {
    match transport {
        CodexTransport::Exec => ApprovalRequestTransport::Exec,
        CodexTransport::AppServer => ApprovalRequestTransport::AppServer,
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
    store.sync_config_authorized_senders(sender_ids)
}

fn authorize_sender_for_update(store: &Store, update: &IncomingMessage) -> Result<Option<i64>> {
    let Some(sender_id) = update.sender_id else {
        return Ok(None);
    };

    let Some(sender) = store.active_authorized_sender(sender_id)? else {
        warn!("rejected unauthorized sender: {sender_id}");
        return Ok(None);
    };

    if sender.source == "paired" && update.chat_type != "private" {
        warn!(
            "rejected paired sender outside private chat: sender_id={sender_id}, chat_type={}",
            update.chat_type
        );
        return Ok(None);
    }

    Ok(Some(sender_id))
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
    use crate::store::{
        ApprovalRequestKind, ApprovalRequestStatus, ApprovalRequestTransport, NewApprovalRequest,
        Store,
    };
    use crate::telegram::{
        IncomingMessage, InlineKeyboardButton, SavedTelegramAttachment, SendMessageResult,
        TelegramAttachment, TelegramControlCommand, TelegramRemoteFile,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SentMessageCall {
        chat_id: i64,
        text: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct EditedMessageCall {
        chat_id: i64,
        message_id: i64,
        text: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CallbackAnswerCall {
        callback_query_id: String,
        text: Option<String>,
    }

    #[derive(Clone, Default)]
    struct MockTelegram {
        sent_messages: Arc<Mutex<Vec<SentMessageCall>>>,
        edited_messages: Arc<Mutex<Vec<EditedMessageCall>>>,
        callback_answers: Arc<Mutex<Vec<CallbackAnswerCall>>>,
    }

    impl MockTelegram {
        fn sent_messages(&self) -> Vec<SentMessageCall> {
            self.sent_messages.lock().expect("sent messages").clone()
        }

        fn edited_messages(&self) -> Vec<EditedMessageCall> {
            self.edited_messages
                .lock()
                .expect("edited messages")
                .clone()
        }

        fn callback_answers(&self) -> Vec<CallbackAnswerCall> {
            self.callback_answers
                .lock()
                .expect("callback answers")
                .clone()
        }
    }

    #[async_trait]
    impl TelegramApi for MockTelegram {
        async fn send_message(&self, chat_id: i64, text: &str) -> Result<SendMessageResult> {
            self.sent_messages
                .lock()
                .expect("sent messages")
                .push(SentMessageCall {
                    chat_id,
                    text: text.to_owned(),
                });
            Ok(SendMessageResult {
                message_id: self.sent_messages.lock().expect("sent messages").len() as i64,
            })
        }

        async fn send_message_with_inline_keyboard(
            &self,
            chat_id: i64,
            text: &str,
            _buttons: &[InlineKeyboardButton],
        ) -> Result<SendMessageResult> {
            self.send_message(chat_id, text).await
        }

        async fn answer_callback_query(
            &self,
            callback_query_id: &str,
            text: Option<&str>,
        ) -> Result<()> {
            self.callback_answers
                .lock()
                .expect("callback answers")
                .push(CallbackAnswerCall {
                    callback_query_id: callback_query_id.to_owned(),
                    text: text.map(ToOwned::to_owned),
                });
            Ok(())
        }

        async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<()> {
            self.edit_message_clearing_inline_keyboard(chat_id, message_id, text)
                .await
        }

        async fn edit_message_clearing_inline_keyboard(
            &self,
            chat_id: i64,
            message_id: i64,
            text: &str,
        ) -> Result<()> {
            self.edited_messages
                .lock()
                .expect("edited messages")
                .push(EditedMessageCall {
                    chat_id,
                    message_id,
                    text: text.to_owned(),
                });
            Ok(())
        }

        async fn save_attachments(
            &self,
            _attachments: &[crate::telegram::TelegramAttachment],
            _state_dir: &std::path::Path,
            _max_bytes: usize,
        ) -> Result<Vec<crate::telegram::SavedTelegramAttachment>> {
            Ok(Vec::new())
        }
    }

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
            format_status_message(None, 3, Vec::new()),
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
    fn authorize_sender_for_update_accepts_config_sender_in_group_chat() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        store
            .sync_config_authorized_senders(&[42])
            .expect("config sender should sync");
        let update = IncomingMessage {
            update_id: 1,
            chat_id: -100,
            chat_type: "group".to_owned(),
            sender_id: Some(42),
            text: "hello".to_owned(),
            attachments: Vec::new(),
            telegram_message_id: 10,
            thread_key: "dm".to_owned(),
            callback_query_id: None,
            control_command_override: None,
            payload_json: "{}".to_owned(),
        };

        assert_eq!(
            authorize_sender_for_update(&store, &update).expect("auth should succeed"),
            Some(42)
        );
    }

    #[test]
    fn authorize_sender_for_update_rejects_paired_sender_in_group_chat() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        store
            .upsert_authorized_sender(crate::store::AuthorizedSender {
                sender_id: 77,
                platform: "telegram".to_owned(),
                display_name: None,
                status: "active".to_owned(),
                approved_at_ms: 1,
                source: "paired".to_owned(),
            })
            .expect("paired sender should save");
        let update = IncomingMessage {
            update_id: 1,
            chat_id: -100,
            chat_type: "group".to_owned(),
            sender_id: Some(77),
            text: "hello".to_owned(),
            attachments: Vec::new(),
            telegram_message_id: 10,
            thread_key: "dm".to_owned(),
            callback_query_id: None,
            control_command_override: None,
            payload_json: "{}".to_owned(),
        };

        assert_eq!(
            authorize_sender_for_update(&store, &update).expect("auth should succeed"),
            None
        );
    }

    #[test]
    fn mode_command_updates_lane_mode() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();

        let reply = handle_control_command(
            &store,
            &config,
            1,
            10,
            "dm",
            None,
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
        let config = test_config();

        let reply = handle_control_command(
            &store,
            &config,
            1,
            10,
            "dm",
            None,
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
        let config = test_config();
        let workspace = config.default_workspace();
        let lane = store
            .get_or_create_lane(20, "dm", &workspace.id, LaneMode::MaxTurns, 5)
            .expect("lane should be created");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("lane should update");

        let reply = handle_control_command(
            &store,
            &config,
            1,
            20,
            "dm",
            Some(&lane),
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

    #[test]
    fn workspace_command_lists_current_and_available_workspaces() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();

        let reply = handle_control_command(
            &store,
            &config,
            1,
            10,
            "dm",
            None,
            TelegramControlCommand::Workspace { workspace_id: None },
            3,
        )
        .expect("workspace command should succeed");

        assert!(reply.contains("現在の workspace: `main`"));
        assert!(reply.contains("- `main`"));
        assert!(reply.contains("- `docs`"));
        assert!(reply.contains("/workspace <id>"));
    }

    #[test]
    fn workspace_command_updates_lane_workspace_and_resets_session() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();
        let lane = store
            .get_or_create_lane(20, "dm", "main", LaneMode::MaxTurns, 2)
            .expect("lane should be created");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("lane should update");
        let lane = store
            .find_lane(20, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");

        let reply = handle_control_command(
            &store,
            &config,
            1,
            20,
            "dm",
            Some(&lane),
            TelegramControlCommand::Workspace {
                workspace_id: Some("docs".to_owned()),
            },
            3,
        )
        .expect("workspace command should succeed");

        let lane = store
            .find_lane(20, "dm")
            .expect("lane lookup should succeed")
            .expect("lane should exist");
        assert_eq!(lane.workspace_id, "docs");
        assert_eq!(lane.state, LaneState::Idle);
        assert_eq!(lane.codex_session_id, None);
        assert!(reply.contains("workspace を `docs` に更新"));
    }

    #[test]
    fn status_message_includes_workspace() {
        let lane = LaneRecord {
            lane_id: "lane-1".to_owned(),
            chat_id: 1,
            thread_key: "dm".to_owned(),
            workspace_id: "docs".to_owned(),
            mode: LaneMode::AwaitReply,
            state: LaneState::WaitingReply,
            codex_session_id: Some("session-1".to_owned()),
            extra_turn_budget: 0,
            waiting_since_ms: Some(1),
        };

        let message = format_status_message(Some(lane), 3, Vec::new());
        assert!(message.contains("workspace: `docs`"));
    }

    #[test]
    fn status_message_includes_pending_approval_summary() {
        let lane = LaneRecord {
            lane_id: "lane-1".to_owned(),
            chat_id: 1,
            thread_key: "dm".to_owned(),
            workspace_id: "main".to_owned(),
            mode: LaneMode::AwaitReply,
            state: LaneState::NeedsLocalApproval,
            codex_session_id: Some("session-1".to_owned()),
            extra_turn_budget: 0,
            waiting_since_ms: None,
        };

        let message =
            format_status_message(Some(lane), 3, vec!["req-1".to_owned(), "req-2".to_owned()]);
        assert!(message.contains("pending approval requests: 2"));
        assert!(message.contains("latest approval request: req-2"));
    }

    #[test]
    fn command_approval_notice_includes_buttons() {
        let notice = build_approval_notice_parts(
            "command approval",
            "approval-11111111-1111-1111-1111-111111111111",
            1_776_644_085_000,
            crate::store::ApprovalRequestKind::CommandExecution,
        );

        assert!(
            notice
                .text
                .contains("Request ID: `approval-11111111-1111-1111-1111-111111111111`")
        );
        assert_eq!(notice.buttons.as_ref().map(Vec::len), Some(2));
        assert_eq!(
            notice
                .buttons
                .as_ref()
                .and_then(|buttons| buttons.first())
                .map(|button| button.text.as_str()),
            Some("Approve")
        );
        assert_eq!(
            notice
                .buttons
                .as_ref()
                .and_then(|buttons| buttons.get(1))
                .map(|button| button.text.as_str()),
            Some("Deny")
        );
        assert_eq!(
            notice
                .buttons
                .as_ref()
                .and_then(|buttons| buttons.first())
                .map(|button| button.callback_data.as_str()),
            Some("approve:11111111-1111-1111-1111-111111111111:mo6g0ljc")
        );
    }

    #[test]
    fn tool_user_input_notice_skips_buttons() {
        let notice = build_approval_notice_parts(
            "need more input",
            "req-2",
            1_776_644_085_000,
            crate::store::ApprovalRequestKind::ToolUserInput,
        );

        assert!(notice.text.contains("cannot be answered from Telegram yet"));
        assert!(notice.buttons.is_none());
    }

    #[test]
    fn internal_control_path_does_not_consume_approval_request() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let mut config = test_config();
        config.codex.transport = crate::config::CodexTransport::AppServer;
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");

        store
            .insert_approval_request(crate::store::NewApprovalRequest {
                request_id: "req-1".to_owned(),
                transport_request_id: "transport-req-1".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: crate::store::ApprovalRequestTransport::AppServer,
                request_kind: crate::store::ApprovalRequestKind::CommandExecution,
                summary_text: "command".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: crate::store::ApprovalRequestStatus::Pending,
            })
            .expect("approval request should insert");

        let reply = handle_control_command(
            &store,
            &config,
            77,
            10,
            "dm",
            Some(&lane),
            TelegramControlCommand::Approve {
                request_id: "req-1".to_owned(),
            },
            3,
        )
        .expect("approve command should succeed");

        let request = store
            .find_approval_request("req-1")
            .expect("approval request should load")
            .expect("approval request should exist");
        assert_eq!(request.status, crate::store::ApprovalRequestStatus::Pending);
        assert_eq!(request.resolved_by_sender_id, None);
        assert!(reply.contains("This internal path does not handle approval decisions"));
    }

    #[test]
    fn internal_control_path_keeps_resolved_request_unchanged() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");

        store
            .insert_approval_request(crate::store::NewApprovalRequest {
                request_id: "req-1".to_owned(),
                transport_request_id: "transport-req-1".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: crate::store::ApprovalRequestTransport::AppServer,
                request_kind: crate::store::ApprovalRequestKind::CommandExecution,
                summary_text: "command".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: crate::store::ApprovalRequestStatus::Pending,
            })
            .expect("approval request should insert");
        store
            .resolve_approval_request("req-1", crate::store::ApprovalRequestStatus::Approved, 99)
            .expect("approval request should resolve");

        let reply = handle_control_command(
            &store,
            &config,
            77,
            10,
            "dm",
            Some(&lane),
            TelegramControlCommand::Deny {
                request_id: "req-1".to_owned(),
            },
            3,
        )
        .expect("deny command should succeed");

        let request = store
            .find_approval_request("req-1")
            .expect("approval request should load")
            .expect("approval request should exist");
        assert_eq!(
            request.status,
            crate::store::ApprovalRequestStatus::Approved
        );
        assert_eq!(request.resolved_by_sender_id, Some(99));
        assert!(reply.contains("This internal path does not handle approval decisions"));
    }

    #[test]
    fn internal_control_path_does_not_leak_lane_membership() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");

        store
            .insert_approval_request(crate::store::NewApprovalRequest {
                request_id: "req-1".to_owned(),
                transport_request_id: "transport-req-1".to_owned(),
                lane_id: "different-lane".to_owned(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: crate::store::ApprovalRequestTransport::AppServer,
                request_kind: crate::store::ApprovalRequestKind::CommandExecution,
                summary_text: "command".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: crate::store::ApprovalRequestStatus::Pending,
            })
            .expect("approval request should insert");

        let reply = handle_control_command(
            &store,
            &config,
            77,
            10,
            "dm",
            Some(&lane),
            TelegramControlCommand::Approve {
                request_id: "req-1".to_owned(),
            },
            3,
        )
        .expect("approve command should succeed");

        let request = store
            .find_approval_request("req-1")
            .expect("approval request should load")
            .expect("approval request should exist");
        assert_eq!(request.status, crate::store::ApprovalRequestStatus::Pending);
        assert!(reply.contains("This internal path does not handle approval decisions"));
    }

    #[test]
    fn format_sessions_message_includes_selection_command() {
        let message = format_sessions_message(&[CodexThreadSummary {
            thread_id: "thread-1".to_owned(),
            title: Some("Fix tests".to_owned()),
            cwd: None,
            model: None,
            updated_at: None,
        }]);

        assert!(message.contains("thread-1"));
        assert!(message.contains("Fix tests"));
        assert!(message.contains("/remotty-sessions <thread_id>"));
    }

    #[test]
    fn find_selected_thread_accepts_prefix_match() {
        let threads = vec![CodexThreadSummary {
            thread_id: "thread-abcdef".to_owned(),
            title: None,
            cwd: None,
            model: None,
            updated_at: None,
        }];

        let selected =
            find_selected_thread(&threads, "thread-abc").expect("prefix should select the thread");

        assert_eq!(selected.thread_id, "thread-abcdef");
    }

    #[test]
    fn format_thread_not_found_message_points_to_fresh_list() {
        let message = format_thread_not_found_message("thread-old");

        assert!(message.contains("thread-old"));
        assert!(message.contains("/remotty-sessions"));
    }

    #[test]
    fn selected_codex_session_id_prefers_live_lane_session() {
        let lane = LaneRecord {
            lane_id: "lane-1".to_owned(),
            chat_id: 10,
            thread_key: "dm".to_owned(),
            workspace_id: "main".to_owned(),
            mode: LaneMode::AwaitReply,
            state: LaneState::WaitingReply,
            codex_session_id: Some("live-thread".to_owned()),
            extra_turn_budget: 0,
            waiting_since_ms: None,
        };
        let binding = crate::store::CodexThreadBinding {
            chat_id: 10,
            thread_key: "dm".to_owned(),
            codex_thread_id: "saved-thread".to_owned(),
            workspace_id: "main".to_owned(),
            title: None,
            cwd: None,
            model: None,
            codex_updated_at: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };

        assert_eq!(
            selected_codex_session_id(&lane, Some(&binding)),
            Some("live-thread")
        );
    }

    #[test]
    fn selected_codex_session_id_uses_saved_thread_binding() {
        let lane = LaneRecord {
            lane_id: "lane-1".to_owned(),
            chat_id: 10,
            thread_key: "dm".to_owned(),
            workspace_id: "main".to_owned(),
            mode: LaneMode::AwaitReply,
            state: LaneState::WaitingReply,
            codex_session_id: None,
            extra_turn_budget: 0,
            waiting_since_ms: None,
        };
        let binding = crate::store::CodexThreadBinding {
            chat_id: 10,
            thread_key: "dm".to_owned(),
            codex_thread_id: "saved-thread".to_owned(),
            workspace_id: "main".to_owned(),
            title: None,
            cwd: None,
            model: None,
            codex_updated_at: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };

        assert_eq!(
            selected_codex_session_id(&lane, Some(&binding)),
            Some("saved-thread")
        );
    }

    #[tokio::test]
    async fn workspace_has_uncommitted_changes_detects_untracked_file() {
        let temp = tempdir().expect("tempdir should be created");
        let status = Command::new("git")
            .arg("init")
            .arg(temp.path())
            .status()
            .expect("git init should run");
        assert!(status.success());

        assert!(
            !workspace_has_uncommitted_changes(temp.path())
                .await
                .expect("clean worktree should inspect")
        );
        fs::write(temp.path().join("note.txt"), "draft").expect("file should write");

        assert!(
            workspace_has_uncommitted_changes(temp.path())
                .await
                .expect("dirty worktree should inspect")
        );
    }

    #[test]
    fn resolve_workspace_for_message_uses_binding_when_lane_is_missing() {
        let config = test_config();
        let binding = crate::store::CodexThreadBinding {
            chat_id: 10,
            thread_key: "dm".to_owned(),
            codex_thread_id: "saved-thread".to_owned(),
            workspace_id: "docs".to_owned(),
            title: None,
            cwd: None,
            model: None,
            codex_updated_at: None,
            created_at_ms: 1,
            updated_at_ms: 1,
        };

        let workspace = resolve_workspace_for_message(&config, None, Some(&binding))
            .expect("workspace should resolve");

        assert_eq!(workspace.id, "docs");
    }

    #[tokio::test]
    async fn running_app_server_lane_steers_followup_to_active_turn() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let mut config = test_config();
        config.codex.transport = CodexTransport::AppServer;
        let codex = CodexRunner::new(config.codex.clone());
        let telegram = MockTelegram::default();
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");
        store
            .update_lane_state(&lane.lane_id, LaneState::Running, Some("thread-1"))
            .expect("lane should update");
        let active_turns = ActiveTurnRegistry::default();
        let (_guard, mut receiver) = active_turns.register(&lane.lane_id);

        handle_message(
            &config,
            &store,
            &telegram,
            &codex,
            &active_turns,
            77,
            IncomingMessage {
                update_id: 1,
                chat_id: 10,
                chat_type: "private".to_owned(),
                sender_id: Some(77),
                text: "use the shorter path".to_owned(),
                attachments: Vec::new(),
                telegram_message_id: 501,
                thread_key: "dm".to_owned(),
                callback_query_id: None,
                control_command_override: None,
                payload_json: "{}".to_owned(),
            },
        )
        .await
        .expect("follow-up should steer");

        let followup = receiver.try_recv().expect("follow-up should be queued");
        assert_eq!(followup.prompt, "use the shorter path");
        let sent_messages = telegram.sent_messages();
        assert_eq!(sent_messages.len(), 1);
        assert!(sent_messages[0].text.contains("追加入力を送りました"));
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

    fn test_workspace(id: &str, path: &str) -> WorkspaceConfig {
        WorkspaceConfig {
            id: id.to_owned(),
            path: PathBuf::from(path),
            writable_roots: vec![PathBuf::from(path)],
            default_mode: LaneMode::AwaitReply,
            continue_prompt: "continue".to_owned(),
            checks_profile: "default".to_owned(),
        }
    }

    fn test_config() -> Config {
        Config {
            service: crate::config::ServiceConfig {
                run_mode: crate::config::RunMode::Console,
                poll_timeout_sec: 30,
                shutdown_grace_sec: 15,
            },
            telegram: crate::config::TelegramConfig {
                token_secret_ref: "token".to_owned(),
                allowed_chat_types: vec!["private".to_owned()],
                admin_sender_ids: vec![1],
                api_base_url: "https://api.telegram.org".to_owned(),
                file_base_url: "https://api.telegram.org/file".to_owned(),
            },
            codex: crate::config::CodexConfig {
                binary: "codex".to_owned(),
                model: "gpt-5.4".to_owned(),
                sandbox: "workspace-write".to_owned(),
                approval: "on-request".to_owned(),
                transport: crate::config::CodexTransport::Exec,
                profile: None,
            },
            storage: crate::config::StorageConfig {
                db_path: PathBuf::from("bridge.db"),
                state_dir: PathBuf::from("state"),
                temp_dir: PathBuf::from("temp"),
                log_dir: PathBuf::from("logs"),
            },
            policy: crate::config::PolicyConfig {
                default_mode: LaneMode::AwaitReply,
                progress_edit_interval_ms: 5000,
                max_output_chars: 12000,
                max_turns_limit: 3,
            },
            checks: crate::config::ChecksConfig::default(),
            workspaces: vec![
                test_workspace("main", "C:/workspace"),
                test_workspace("docs", "C:/docs"),
            ],
        }
    }

    fn insert_pending_approval_request(
        store: &Store,
        lane: &LaneRecord,
        run_id: &str,
        request_id: &str,
        transport: ApprovalRequestTransport,
        request_kind: ApprovalRequestKind,
    ) {
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: request_id.to_owned(),
                transport_request_id: format!("transport-{request_id}"),
                lane_id: lane.lane_id.clone(),
                run_id: run_id.to_owned(),
                thread_id: format!("thread-{request_id}"),
                turn_id: format!("turn-{request_id}"),
                item_id: format!("item-{request_id}"),
                transport,
                request_kind,
                summary_text: format!("summary for {request_id}"),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("approval request should insert");
    }

    #[tokio::test]
    async fn handle_message_reports_specific_callback_text_for_processed_request() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();
        let codex = CodexRunner::new(config.codex.clone());
        let telegram = MockTelegram::default();
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");

        insert_pending_approval_request(
            &store,
            &lane,
            "run-processed",
            "req-processed",
            ApprovalRequestTransport::Exec,
            ApprovalRequestKind::CommandExecution,
        );
        store
            .resolve_approval_request("req-processed", ApprovalRequestStatus::Approved, 90)
            .expect("approval should resolve");

        handle_message(
            &config,
            &store,
            &telegram,
            &codex,
            &ActiveTurnRegistry::default(),
            77,
            IncomingMessage {
                update_id: 1,
                chat_id: 10,
                chat_type: "private".to_owned(),
                sender_id: Some(77),
                text: String::new(),
                attachments: Vec::new(),
                telegram_message_id: 501,
                thread_key: "dm".to_owned(),
                callback_query_id: Some("callback-processed".to_owned()),
                control_command_override: Some(TelegramControlCommand::Approve {
                    request_id: "req-processed".to_owned(),
                }),
                payload_json: "{}".to_owned(),
            },
        )
        .await
        .expect("callback handling should succeed");

        let callback_answers = telegram.callback_answers();
        assert_eq!(callback_answers.len(), 1);
        assert_eq!(
            callback_answers[0].text.as_deref(),
            Some("Already `approved`.")
        );

        let sent_messages = telegram.sent_messages();
        assert_eq!(sent_messages.len(), 1);
        assert!(sent_messages[0].text.contains("already handled"));
    }

    #[tokio::test]
    async fn approval_decision_message_rejects_tool_user_input_requests() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let config = test_config();
        let codex = CodexRunner::new(config.codex.clone());
        let telegram = MockTelegram::default();
        let lane = store
            .get_or_create_lane(22, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");

        insert_pending_approval_request(
            &store,
            &lane,
            "run-tool-input",
            "req-tool-input",
            ApprovalRequestTransport::AppServer,
            ApprovalRequestKind::ToolUserInput,
        );

        let reply = handle_approval_decision_message(
            &config,
            &store,
            &telegram,
            &codex,
            77,
            22,
            "dm",
            Some(&lane),
            Some(700),
            "req-tool-input",
            true,
        )
        .await
        .expect("approval decision should return a reply");

        assert_eq!(
            reply.callback_text,
            "This request type cannot be answered from Telegram."
        );
        assert!(
            reply
                .reply_text
                .contains("cannot be handled from Telegram yet")
        );
        assert!(telegram.edited_messages().is_empty());
    }

    #[tokio::test]
    async fn restart_invalidation_edits_existing_notice_and_marks_request_invalidated() {
        let dir = tempdir().expect("tempdir should be created");
        let store = Store::open(dir.path().join("bridge.db")).expect("store should open");
        let telegram = MockTelegram::default();
        let lane = store
            .get_or_create_lane(31, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane should exist");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "restart".to_owned(),
            })
            .expect("run should insert");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-1"),
            )
            .expect("lane should update");
        insert_pending_approval_request(
            &store,
            &lane,
            &run.run_id,
            "req-restart",
            ApprovalRequestTransport::AppServer,
            ApprovalRequestKind::CommandExecution,
        );
        store
            .set_approval_request_message_id("req-restart", 88)
            .expect("message id should be stored");

        invalidate_pending_approval_notifications_for_restart(&store, &telegram).await;

        let request = store
            .find_approval_request("req-restart")
            .expect("approval should load")
            .expect("approval should exist");
        assert_eq!(request.status, ApprovalRequestStatus::Invalidated);

        let lane = store
            .find_lane(31, "dm")
            .expect("lane should load")
            .expect("lane should exist");
        assert_eq!(lane.state, LaneState::WaitingReply);
        assert_eq!(lane.codex_session_id, None);

        let edits = telegram.edited_messages();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].message_id, 88);
        assert!(edits[0].text.contains("invalidated by a bridge restart"));
        assert!(telegram.sent_messages().is_empty());
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
            turn_id: None,
            last_message: "続けます".to_owned(),
            exit_code: Some(0),
            approval_pending: false,
            approval_requests: Vec::new(),
            approval_request_count: 0,
            approval_resolved_count: 0,
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
            turn_id: None,
            last_message: "失敗しました".to_owned(),
            exit_code: Some(1),
            approval_pending: false,
            approval_requests: Vec::new(),
            approval_request_count: 0,
            approval_resolved_count: 0,
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
