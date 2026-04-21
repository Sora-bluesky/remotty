#[path = "support/fake_telegram.rs"]
mod fake_telegram;

use anyhow::Result;
use chrono::Utc;
use remotty::store::{PendingAccessPairCode, Store};
use remotty::telegram::{
    TelegramAttachment, TelegramAttachmentKind, TelegramClient, TelegramControlCommand,
};
use remotty::telegram_cli::{access_pair, pair_with_code};
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn telegram_client_routes_updates_and_callback_operations_to_fake_server() -> Result<()> {
    let server = fake_telegram::FakeTelegramServer::start("test-token").await?;
    server
        .enqueue_message(42, 9, "hello from fake telegram")
        .await?;
    server
        .enqueue_callback_query(42, 9, 7, "approve:req-9")
        .await?;

    let client = TelegramClient::with_base_urls(
        "test-token".to_owned(),
        server.api_base_url(),
        server.file_base_url(),
    );

    let bot = client.get_me().await?;
    assert_eq!(bot.id, 77_000);
    assert_eq!(bot.username.as_deref(), Some("remotty_test_bot"));

    let updates = client.get_updates(None, 0).await?;
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
async fn telegram_client_handles_webhook_and_attachment_download_against_fake_server() -> Result<()>
{
    let server = fake_telegram::FakeTelegramServer::start("test-token").await?;
    server.set_webhook_url("https://example.com/hook").await;
    server
        .register_file("file-1", "unique-1", "docs/report.txt", b"hello")
        .await;

    let client = TelegramClient::with_base_urls(
        "test-token".to_owned(),
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

    let pairing = client.get_pairing_updates(None, 0).await?;
    assert!(pairing.is_empty());

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
    let server = fake_telegram::FakeTelegramServer::start("test-token").await?;
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
        std::env::set_var("TELEGRAM_BOT_TOKEN", "test-token");
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
    let server = fake_telegram::FakeTelegramServer::start("test-token").await?;
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
