#[path = "support/fake_telegram.rs"]
mod fake_telegram;

use anyhow::Result;
use chrono::Utc;
use remotty::config::{Config, LaneMode};
use remotty::store::{
    ApprovalRequestKind, ApprovalRequestStatus, ApprovalRequestTransport, NewApprovalRequest,
    NewRun, PendingAccessPairCode, Store,
};
use remotty::telegram::{
    TelegramAttachment, TelegramAttachmentKind, TelegramClient, TelegramControlCommand,
    TelegramPoller,
};
use remotty::telegram_cli::{access_pair, pair_with_code};
use serial_test::serial;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use tokio::time::{Duration, Instant, sleep, timeout};
use tokio_util::sync::CancellationToken;

const TEST_TOKEN: &str = "test-token";

#[tokio::test]
#[serial]
async fn telegram_client_routes_updates_and_callback_operations_to_fake_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    server
        .enqueue_message(42, 9, "hello from fake telegram")
        .await?;
    server
        .enqueue_callback_query(42, 9, 7, "approve:req-9")
        .await?;

    let client = TelegramClient::with_base_urls(
        TEST_TOKEN.to_owned(),
        server.api_base_url(),
        server.file_base_url(),
    );

    let bot = client.get_me().await?;
    assert_eq!(bot.id, 77_000);
    assert_eq!(bot.username.as_deref(), Some("remotty_test_bot"));

    let poller = TelegramPoller::acquire(client.clone()).await?;
    let updates = poller.get_updates(None, 0).await?;
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].text, "hello from fake telegram");
    assert_eq!(
        updates[1].control_command(),
        Some(TelegramControlCommand::Approve {
            request_id: "req-9".to_owned(),
        })
    );

    client.send_message(42, "bridge reply").await?;
    client.edit_message(42, 1, "edited reply").await?;
    client
        .answer_callback_query("callback-2", Some("processed"))
        .await?;
    drop(poller);

    let sent = server.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chat_id, 42);
    assert_eq!(sent[0].text, "bridge reply");

    let edited = server.edited_messages().await;
    assert_eq!(edited.len(), 1);
    assert_eq!(edited[0].chat_id, 42);
    assert_eq!(edited[0].message_id, 1);
    assert_eq!(edited[0].text, "edited reply");

    let callbacks = server.callback_answers().await;
    assert_eq!(callbacks.len(), 1);
    assert_eq!(callbacks[0].callback_query_id, "callback-2");
    assert_eq!(callbacks[0].text.as_deref(), Some("processed"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn telegram_client_handles_webhook_and_attachment_download_against_fake_server() -> Result<()>
{
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    server.set_webhook_url("https://example.com/hook").await;
    server
        .register_file("file-1", "unique-1", "docs/report.txt", b"hello")
        .await;

    let client = TelegramClient::with_base_urls(
        TEST_TOKEN.to_owned(),
        server.api_base_url(),
        server.file_base_url(),
    );
    let temp = tempdir()?;

    let webhook = client.get_webhook_info().await?;
    assert_eq!(webhook.url, "https://example.com/hook");

    client.delete_webhook(false).await?;
    let delete_calls = server.delete_webhook_calls().await;
    assert_eq!(delete_calls.len(), 1);
    assert!(!delete_calls[0].drop_pending_updates);

    let poller = TelegramPoller::acquire(client.clone()).await?;
    let pairing = poller.get_pairing_updates(None, 0).await?;
    assert!(pairing.is_empty());
    drop(poller);

    let saved = client
        .save_attachment(
            &TelegramAttachment {
                kind: TelegramAttachmentKind::Document,
                file_id: "file-1".to_owned(),
                file_unique_id: "unique-1".to_owned(),
                file_name: Some("report.txt".to_owned()),
                mime_type: Some("text/plain".to_owned()),
                file_size: Some(5),
                width: None,
                height: None,
            },
            temp.path(),
            1024,
        )
        .await?;
    assert_eq!(std::fs::read_to_string(saved.local_path)?, "hello");

    Ok(())
}

#[tokio::test]
#[serial]
async fn telegram_pair_flow_authorizes_sender_against_fake_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    let temp = tempdir()?;
    let db_path = temp.path().join("state/bridge.db");
    let state_dir = temp.path().join("state");
    let temp_dir = temp.path().join("tmp");
    let log_dir = temp.path().join("logs");
    let config_path = temp.path().join("bridge.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "test-secret"
allowed_chat_types = ["private"]
admin_sender_ids = []
api_base_url = "{}"
file_base_url = "{}"

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
            server.api_base_url(),
            server.file_base_url(),
            db_path.display().to_string().replace('\\', "/"),
            state_dir.display().to_string().replace('\\', "/"),
            temp_dir.display().to_string().replace('\\', "/"),
            log_dir.display().to_string().replace('\\', "/"),
        ),
    )?;
    unsafe {
        std::env::set_var("TELEGRAM_BOT_TOKEN", TEST_TOKEN);
    }
    let pair_code = "PAIRCODE1234";
    server
        .enqueue_message(42, 9, &format!("/pair {pair_code}"))
        .await?;

    let result = pair_with_code(&config_path, pair_code, Utc::now().timestamp() - 1).await?;
    assert!(result.contains("`9`"));

    let store = remotty::store::Store::open(&db_path)?;
    let senders = store.list_active_authorized_senders()?;
    assert_eq!(senders.len(), 1);
    assert_eq!(senders[0].sender_id, 9);
    assert_eq!(senders[0].source, "paired");

    Ok(())
}

#[tokio::test]
#[serial]
async fn telegram_access_pair_flow_authorizes_sender_against_fake_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    let temp = tempdir()?;
    let db_path = temp.path().join("state/bridge.db");
    let state_dir = temp.path().join("state");
    let temp_dir = temp.path().join("tmp");
    let log_dir = temp.path().join("logs");
    let config_path = temp.path().join("bridge.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "test-secret"
allowed_chat_types = ["private"]
admin_sender_ids = []
api_base_url = "{}"
file_base_url = "{}"

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
            server.api_base_url(),
            server.file_base_url(),
            db_path.display().to_string().replace('\\', "/"),
            state_dir.display().to_string().replace('\\', "/"),
            temp_dir.display().to_string().replace('\\', "/"),
            log_dir.display().to_string().replace('\\', "/"),
        ),
    )?;
    let pair_code = "ACCESS1234";
    fs::create_dir_all(&state_dir)?;
    let store = Store::open(&db_path)?;
    let issued_at_ms = Utc::now().timestamp_millis();
    store.insert_access_pair_code(&PendingAccessPairCode {
        code: pair_code.to_owned(),
        sender_id: 9,
        chat_id: 42,
        chat_type: "private".to_owned(),
        issued_at_ms,
        expires_at_ms: issued_at_ms + 180_000,
    })?;

    let result = access_pair(&config_path, pair_code).await?;
    assert!(result.contains("`9`"));

    let senders = store.list_active_authorized_senders()?;
    assert_eq!(senders.len(), 1);
    assert_eq!(senders[0].sender_id, 9);
    assert_eq!(senders[0].source, "paired");

    Ok(())
}

#[tokio::test]
#[serial]
async fn live_env_check_reports_webhook_state_against_fake_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    server.set_webhook_url("https://example.com/hook").await;
    let temp = tempdir()?;
    let fake_codex = write_fake_codex(temp.path(), "unused fake codex reply")?;
    let (_, _) = write_bridge_config(temp.path(), &server, &fake_codex)?;
    let config_path = temp.path().join("bridge.toml");
    unsafe {
        std::env::set_var("TELEGRAM_BOT_TOKEN", TEST_TOKEN);
    }

    let report = remotty::telegram_cli::live_env_check(&config_path).await?;

    assert!(report.contains("- Telegram webhook: webhook-configured"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn bridge_round_trip_replies_against_fake_telegram_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    let temp = tempdir()?;
    let reply_text = "fake codex reply from integration";
    let fake_codex = write_fake_codex(temp.path(), reply_text)?;
    let (config, _) = write_bridge_config(temp.path(), &server, &fake_codex)?;
    unsafe {
        std::env::set_var("TELEGRAM_BOT_TOKEN", TEST_TOKEN);
    }

    server
        .enqueue_message(42, 9, "hello from fake bridge test")
        .await?;

    let shutdown = CancellationToken::new();
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let mut bridge = tokio::task::spawn_local(remotty::engine::run_with_shutdown(
                config,
                shutdown.clone(),
            ));

            let sent = tokio::select! {
                result = wait_for_sent_message_containing(&server, "処理を開始") => result?,
                result = &mut bridge => {
                    result??;
                    anyhow::bail!("bridge stopped before sending progress message");
                }
            };
            assert_eq!(sent.chat_id, 42);

            let edited = tokio::select! {
                result = wait_for_edited_message_containing(&server, reply_text) => result?,
                result = &mut bridge => {
                    result??;
                    anyhow::bail!("bridge stopped before editing the final reply");
                }
            };
            assert_eq!(edited.chat_id, 42);

            shutdown.cancel();
            timeout(Duration::from_secs(5), bridge).await???;
            Ok::<_, anyhow::Error>(())
        })
        .await?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn bridge_records_approval_accept_and_decline_against_fake_telegram_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start(TEST_TOKEN).await?;
    let temp = tempdir()?;
    let fake_codex = write_fake_codex(temp.path(), "unused fake codex reply")?;
    let (config, db_path) = write_bridge_config(temp.path(), &server, &fake_codex)?;
    unsafe {
        std::env::set_var("TELEGRAM_BOT_TOKEN", TEST_TOKEN);
    }

    let store = Store::open(&db_path)?;
    seed_exec_approval_request(&store, "approval-accept-fake", 101)?;
    seed_exec_approval_request(&store, "approval-decline-fake", 102)?;
    drop(store);

    server
        .enqueue_callback_query(42, 9, 101, "approve:approval-accept-fake")
        .await?;
    server
        .enqueue_callback_query(42, 9, 102, "deny:approval-decline-fake")
        .await?;

    let shutdown = CancellationToken::new();
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let mut bridge = tokio::task::spawn_local(remotty::engine::run_with_shutdown(
                config,
                shutdown.clone(),
            ));

            let callbacks = tokio::select! {
                result = wait_for_callback_answers(&server, 2) => result?,
                result = &mut bridge => {
                    result??;
                    anyhow::bail!("bridge stopped before answering callbacks");
                }
            };
            assert!(callbacks.iter().any(|answer| {
                answer.callback_query_id == "callback-1"
                    && answer.text.as_deref() == Some("承認を記録しました。")
            }));
            assert!(callbacks.iter().any(|answer| {
                answer.callback_query_id == "callback-2"
                    && answer.text.as_deref() == Some("非承認を記録しました。")
            }));

            let edited = tokio::select! {
                result = wait_for_edited_messages(&server, 2) => result?,
                result = &mut bridge => {
                    result??;
                    anyhow::bail!("bridge stopped before editing approval messages");
                }
            };
            assert!(
                edited.iter().any(|message| {
                    message.message_id == 101
                        && message.text.contains(
                            "承認要求 `approval-accept-fake` を 承認 として受け付けました。",
                        )
                }),
                "edited messages: {edited:?}"
            );
            assert!(
                edited.iter().any(|message| {
                    message.message_id == 102
                        && message.text.contains(
                            "承認要求 `approval-decline-fake` を 非承認 として受け付けました。",
                        )
                }),
                "edited messages: {edited:?}"
            );

            shutdown.cancel();
            timeout(Duration::from_secs(5), bridge).await???;
            Ok::<_, anyhow::Error>(())
        })
        .await?;

    let store = Store::open(&db_path)?;
    let accepted = store
        .find_approval_request("approval-accept-fake")?
        .expect("accept request should exist");
    assert_eq!(accepted.status, ApprovalRequestStatus::Approved);
    assert_eq!(accepted.resolved_by_sender_id, Some(9));

    let declined = store
        .find_approval_request("approval-decline-fake")?
        .expect("decline request should exist");
    assert_eq!(declined.status, ApprovalRequestStatus::Declined);
    assert_eq!(declined.resolved_by_sender_id, Some(9));

    Ok(())
}

fn write_bridge_config(
    root: &Path,
    server: &fake_telegram::FakeTelegramServer,
    fake_codex: &Path,
) -> Result<(Config, PathBuf)> {
    let db_path = root.join("state").join("bridge.db");
    let state_dir = root.join("state");
    let temp_dir = root.join("tmp");
    let log_dir = root.join("logs");
    let workspace_dir = root.join("workspace");
    let config_path = root.join("bridge.toml");

    fs::create_dir_all(&state_dir)?;
    fs::create_dir_all(&temp_dir)?;
    fs::create_dir_all(&log_dir)?;
    fs::create_dir_all(&workspace_dir)?;
    fs::write(
        &config_path,
        format!(
            r#"
[service]
run_mode = "console"
poll_timeout_sec = 0
shutdown_grace_sec = 1

[telegram]
token_secret_ref = "test-secret"
allowed_chat_types = ["private"]
admin_sender_ids = [9]
api_base_url = "{}"
file_base_url = "{}"

[codex]
binary = "{}"
model = "gpt-5.4"
sandbox = "workspace-write"
approval = "on-request"
transport = "exec"

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
path = "{}"
writable_roots = ["{}"]
default_mode = "await_reply"
continue_prompt = "continue"
checks_profile = "default"
"#,
            server.api_base_url(),
            server.file_base_url(),
            toml_path(fake_codex),
            toml_path(&db_path),
            toml_path(&state_dir),
            toml_path(&temp_dir),
            toml_path(&log_dir),
            toml_path(&workspace_dir),
            toml_path(&workspace_dir),
        ),
    )?;

    Ok((Config::load(&config_path)?, db_path))
}

fn write_fake_codex(root: &Path, reply_text: &str) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let path = root.join("fake-codex.cmd");
        fs::write(
            &path,
            format!(
                "@echo off\r\n\
echo {{^\"type^\":^\"thread.started^\",^\"thread_id^\":^\"thread-fake^\"}}\r\n\
echo {{^\"type^\":^\"item.completed^\",^\"item^\":{{^\"type^\":^\"agent_message^\",^\"text^\":^\"{}^\"}}}}\r\n\
exit /b 0\r\n",
                reply_text
            ),
        )?;
        Ok(path)
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;

        let path = root.join("fake-codex");
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n\
printf '%s\n' '{{\"type\":\"thread.started\",\"thread_id\":\"thread-fake\"}}'\n\
printf '%s\n' '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"{}\"}}}}'\n",
                reply_text
            ),
        )?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(path)
    }
}

fn seed_exec_approval_request(
    store: &Store,
    request_id: &str,
    telegram_message_id: i64,
) -> Result<()> {
    let lane = store.get_or_create_lane(42, "dm", "main", LaneMode::AwaitReply, 0)?;
    let run = store.insert_run(NewRun {
        lane_id: lane.lane_id.clone(),
        run_kind: "start".to_owned(),
    })?;
    store.insert_approval_request(NewApprovalRequest {
        request_id: request_id.to_owned(),
        transport_request_id: String::new(),
        lane_id: lane.lane_id,
        run_id: run.run_id,
        thread_id: "thread-fake".to_owned(),
        turn_id: "turn-fake".to_owned(),
        item_id: "item-fake".to_owned(),
        transport: ApprovalRequestTransport::Exec,
        request_kind: ApprovalRequestKind::CommandExecution,
        summary_text: "Run fake command".to_owned(),
        raw_payload_json: "{}".to_owned(),
        status: ApprovalRequestStatus::Pending,
    })?;
    store.set_approval_request_message_id(request_id, telegram_message_id)?;
    Ok(())
}

async fn wait_for_sent_message_containing(
    server: &fake_telegram::FakeTelegramServer,
    text: &str,
) -> Result<fake_telegram::SentMessageRecord> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(message) = server
            .sent_messages()
            .await
            .into_iter()
            .find(|message| message.text.contains(text))
        {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for sent message containing {text:?}; sent={:?}; edited={:?}; callbacks={:?}",
                server.sent_messages().await,
                server.edited_messages().await,
                server.callback_answers().await
            );
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_edited_message_containing(
    server: &fake_telegram::FakeTelegramServer,
    text: &str,
) -> Result<fake_telegram::EditedMessageRecord> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(message) = server
            .edited_messages()
            .await
            .into_iter()
            .find(|message| message.text.contains(text))
        {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for edited message containing {text:?}; sent={:?}; edited={:?}; callbacks={:?}",
                server.sent_messages().await,
                server.edited_messages().await,
                server.callback_answers().await
            );
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_edited_messages(
    server: &fake_telegram::FakeTelegramServer,
    count: usize,
) -> Result<Vec<fake_telegram::EditedMessageRecord>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let messages = server.edited_messages().await;
        if messages.len() >= count {
            return Ok(messages);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for {count} edited messages; sent={:?}; edited={:?}; callbacks={:?}",
                server.sent_messages().await,
                server.edited_messages().await,
                server.callback_answers().await
            );
        }
        sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_callback_answers(
    server: &fake_telegram::FakeTelegramServer,
    count: usize,
) -> Result<Vec<fake_telegram::CallbackAnswerRecord>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let answers = server.callback_answers().await;
        if answers.len() >= count {
            return Ok(answers);
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for {count} callback answers; sent={:?}; edited={:?}; callbacks={:?}",
                server.sent_messages().await,
                server.edited_messages().await,
                server.callback_answers().await
            );
        }
        sleep(Duration::from_millis(50)).await;
    }
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
