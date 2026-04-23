use anyhow::{Context, Result, bail};
use reqwest::Client;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::telegram_poller_guard::TelegramPollerGuard;

#[derive(Clone)]
pub struct TelegramClient {
    http: Client,
    token: String,
    api_base_url: String,
    file_base_url: String,
}

#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub update_id: i64,
    pub chat_id: i64,
    pub chat_type: String,
    pub sender_id: Option<i64>,
    pub text: String,
    pub attachments: Vec<TelegramAttachment>,
    pub telegram_message_id: i64,
    pub thread_key: String,
    pub callback_query_id: Option<String>,
    pub control_command_override: Option<TelegramControlCommand>,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingUpdate {
    pub update_id: i64,
    pub message: PairingMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingMessage {
    pub chat_id: i64,
    pub chat_type: String,
    pub sender_id: i64,
    pub text: String,
    pub sent_at_s: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramControlCommand {
    Help,
    Status,
    Stop,
    Approve {
        request_id: String,
    },
    Deny {
        request_id: String,
    },
    Workspace {
        workspace_id: Option<String>,
    },
    Sessions {
        thread_id: Option<String>,
    },
    Mode {
        mode: String,
        max_turns: Option<i64>,
    },
}

impl IncomingMessage {
    pub fn control_command(&self) -> Option<TelegramControlCommand> {
        self.control_command_override
            .clone()
            .or_else(|| parse_control_command(&self.text))
    }
}

impl TelegramControlCommand {
    pub fn parse(text: &str) -> Option<Self> {
        parse_control_command(text)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramAttachment {
    pub kind: TelegramAttachmentKind,
    pub file_id: String,
    pub file_unique_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelegramAttachmentKind {
    Photo,
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelegramRemoteFile {
    pub file_id: String,
    pub file_unique_id: String,
    pub file_path: String,
    pub file_size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedTelegramAttachment {
    pub attachment: TelegramAttachment,
    pub remote_file: TelegramRemoteFile,
    pub local_path: PathBuf,
    pub bytes_written: usize,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    result: T,
}

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
    edited_message: Option<Message>,
    callback_query: Option<CallbackQuery>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    message_id: i64,
    date: Option<i64>,
    text: Option<String>,
    caption: Option<String>,
    chat: Chat,
    from: Option<User>,
    message_thread_id: Option<i64>,
    photo: Option<Vec<PhotoSize>>,
    document: Option<Document>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct User {
    id: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct CallbackQuery {
    id: String,
    from: User,
    data: Option<String>,
    message: Option<Message>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct PhotoSize {
    file_id: String,
    file_unique_id: String,
    width: i64,
    height: i64,
    file_size: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Document {
    file_id: String,
    file_unique_id: String,
    file_name: Option<String>,
    mime_type: Option<String>,
    file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TelegramFileResult {
    file_id: String,
    file_unique_id: String,
    file_size: Option<i64>,
    file_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct TelegramBotInfo {
    pub id: i64,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct TelegramWebhookInfo {
    pub url: String,
}

pub struct TelegramPoller {
    client: TelegramClient,
    bot: TelegramBotInfo,
    _guard: TelegramPollerGuard,
}

impl TelegramPoller {
    pub async fn acquire(client: TelegramClient) -> Result<Self> {
        let bot = client.get_me().await?;
        let guard = TelegramPollerGuard::acquire(bot.id)?;
        Ok(Self {
            client,
            bot,
            _guard: guard,
        })
    }

    pub fn bot(&self) -> &TelegramBotInfo {
        &self.bot
    }

    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        timeout_sec: u64,
    ) -> Result<Vec<IncomingMessage>> {
        let response = self
            .client
            .request_updates(
                offset,
                timeout_sec,
                &["message", "edited_message", "callback_query"],
            )
            .await?;
        parse_updates(response)
    }

    pub async fn get_pairing_updates(
        &self,
        offset: Option<i64>,
        timeout_sec: u64,
    ) -> Result<Vec<PairingUpdate>> {
        let response = self
            .client
            .request_updates(offset, timeout_sec, &["message"])
            .await?;
        parse_pairing_updates(response)
    }

    pub async fn drain_pending_updates(&self) -> Result<()> {
        let mut offset = None;
        loop {
            let response = self
                .client
                .request_updates(offset, 0, &["message", "edited_message", "callback_query"])
                .await?;
            if !response.ok {
                bail!("telegram getUpdates returned ok=false");
            }
            let Some(last_update_id) = response.result.iter().map(|update| update.update_id).max()
            else {
                return Ok(());
            };
            offset = Some(last_update_id + 1);
        }
    }
}

impl TelegramClient {
    pub fn new(token: String) -> Self {
        Self::with_base_urls(
            token,
            "https://api.telegram.org".to_owned(),
            "https://api.telegram.org/file".to_owned(),
        )
    }

    pub fn with_base_urls(token: String, api_base_url: String, file_base_url: String) -> Self {
        Self {
            http: Client::new(),
            token,
            api_base_url: api_base_url.trim_end_matches('/').to_owned(),
            file_base_url: file_base_url.trim_end_matches('/').to_owned(),
        }
    }

    pub async fn get_me(&self) -> Result<TelegramBotInfo> {
        let url = self.api_url("getMe");
        let response: ApiResponse<TelegramBotInfo> = self
            .http
            .get(url)
            .send()
            .await
            .context("telegram getMe request failed")?
            .error_for_status()
            .context("telegram getMe returned error status")?
            .json()
            .await
            .context("failed to decode telegram getMe response")?;
        if !response.ok {
            bail!("telegram getMe returned ok=false");
        }
        Ok(response.result)
    }

    pub async fn get_webhook_info(&self) -> Result<TelegramWebhookInfo> {
        let url = self.api_url("getWebhookInfo");
        let response: ApiResponse<TelegramWebhookInfo> = self
            .http
            .post(url)
            .send()
            .await
            .context("telegram getWebhookInfo request failed")?
            .error_for_status()
            .context("telegram getWebhookInfo returned error status")?
            .json()
            .await
            .context("failed to decode telegram getWebhookInfo response")?;
        if !response.ok {
            bail!("telegram getWebhookInfo returned ok=false");
        }
        Ok(response.result)
    }

    pub async fn delete_webhook(&self, drop_pending_updates: bool) -> Result<()> {
        let url = self.api_url("deleteWebhook");
        let body = serde_json::json!({
            "drop_pending_updates": drop_pending_updates,
        });
        let response: ApiResponse<bool> = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram deleteWebhook request failed")?
            .error_for_status()
            .context("telegram deleteWebhook returned error status")?
            .json()
            .await
            .context("failed to decode telegram deleteWebhook response")?;
        if !response.ok || !response.result {
            bail!("telegram deleteWebhook returned ok=false");
        }
        Ok(())
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<SendMessageResult> {
        self.send_message_with_markup(chat_id, text, None).await
    }

    pub async fn send_message_with_inline_keyboard(
        &self,
        chat_id: i64,
        text: &str,
        buttons: &[InlineKeyboardButton],
    ) -> Result<SendMessageResult> {
        let keyboard = serde_json::json!({
            "inline_keyboard": [
                buttons
                    .iter()
                    .map(|button| serde_json::json!({
                        "text": button.text,
                        "callback_data": button.callback_data,
                    }))
                    .collect::<Vec<_>>()
            ]
        });
        self.send_message_with_markup(chat_id, text, Some(keyboard))
            .await
    }

    async fn send_message_with_markup(
        &self,
        chat_id: i64,
        text: &str,
        reply_markup: Option<Value>,
    ) -> Result<SendMessageResult> {
        let url = self.api_url("sendMessage");
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
        });
        if let Some(reply_markup) = reply_markup {
            body["reply_markup"] = reply_markup;
        }
        let response: ApiResponse<Value> = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram sendMessage request failed")?
            .error_for_status()
            .context("telegram sendMessage returned error status")?
            .json()
            .await
            .context("failed to decode telegram sendMessage response")?;
        if !response.ok {
            bail!("telegram sendMessage returned ok=false");
        }
        let message_id = response.result["message_id"]
            .as_i64()
            .context("telegram sendMessage response missing message_id")?;
        Ok(SendMessageResult { message_id })
    }

    pub async fn answer_callback_query(
        &self,
        callback_query_id: &str,
        text: Option<&str>,
    ) -> Result<()> {
        let url = self.api_url("answerCallbackQuery");
        let mut body = serde_json::json!({
            "callback_query_id": callback_query_id,
        });
        if let Some(text) = text {
            body["text"] = Value::String(text.to_owned());
        }
        self.http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram answerCallbackQuery request failed")?
            .error_for_status()
            .context("telegram answerCallbackQuery returned error status")?;
        Ok(())
    }

    pub async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<()> {
        self.edit_message_with_markup(chat_id, message_id, text, None)
            .await
    }

    pub async fn edit_message_clearing_inline_keyboard(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
    ) -> Result<()> {
        self.edit_message_with_markup(
            chat_id,
            message_id,
            text,
            Some(serde_json::json!({ "inline_keyboard": [] })),
        )
        .await
    }

    async fn edit_message_with_markup(
        &self,
        chat_id: i64,
        message_id: i64,
        text: &str,
        reply_markup: Option<Value>,
    ) -> Result<()> {
        let url = self.api_url("editMessageText");
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
        });
        if let Some(reply_markup) = reply_markup {
            body["reply_markup"] = reply_markup;
        }
        self.http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram editMessageText request failed")?
            .error_for_status()
            .context("telegram editMessageText returned error status")?;
        Ok(())
    }

    pub async fn resolve_attachment_file(
        &self,
        attachment: &TelegramAttachment,
    ) -> Result<TelegramRemoteFile> {
        let url = self.api_url("getFile");
        let body = serde_json::json!({
            "file_id": attachment.file_id,
        });
        let response: ApiResponse<TelegramFileResult> = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram getFile request failed")?
            .error_for_status()
            .context("telegram getFile returned error status")?
            .json()
            .await
            .context("failed to decode telegram getFile response")?;
        if !response.ok {
            bail!("telegram getFile returned ok=false");
        }

        let file_path = response
            .result
            .file_path
            .context("telegram getFile response missing file_path")?;

        Ok(TelegramRemoteFile {
            file_id: response.result.file_id,
            file_unique_id: response.result.file_unique_id,
            file_path,
            file_size: response.result.file_size,
        })
    }

    pub async fn download_attachment_bytes(
        &self,
        attachment: &TelegramAttachment,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        let downloaded = self.download_attachment(attachment, max_bytes).await?;
        Ok(downloaded.bytes)
    }

    pub async fn save_attachment<P: AsRef<Path>>(
        &self,
        attachment: &TelegramAttachment,
        directory: P,
        max_bytes: usize,
    ) -> Result<SavedTelegramAttachment> {
        let directory = directory.as_ref().to_path_buf();
        let downloaded = self.download_attachment(attachment, max_bytes).await?;
        write_downloaded_attachment(&directory, attachment, downloaded)
    }

    pub async fn save_attachments<P: AsRef<Path>>(
        &self,
        attachments: &[TelegramAttachment],
        directory: P,
        max_bytes: usize,
    ) -> Result<Vec<SavedTelegramAttachment>> {
        let directory = directory.as_ref().to_path_buf();
        let mut saved = Vec::with_capacity(attachments.len());
        for attachment in attachments {
            let downloaded = self.download_attachment(attachment, max_bytes).await?;
            saved.push(write_downloaded_attachment(
                &directory, attachment, downloaded,
            )?);
        }
        Ok(saved)
    }

    async fn download_attachment(
        &self,
        attachment: &TelegramAttachment,
        max_bytes: usize,
    ) -> Result<DownloadedTelegramAttachment> {
        validate_download_limit(max_bytes)?;

        let remote_file = self.resolve_attachment_file(attachment).await?;
        let reported_size = remote_file.file_size.or(attachment.file_size);
        ensure_within_limit(reported_size, max_bytes, &attachment.file_id)?;

        let url = self.file_url(&remote_file.file_path);
        let mut response = self
            .http
            .get(url)
            .send()
            .await
            .context("telegram file download request failed")?
            .error_for_status()
            .context("telegram file download returned error status")?;

        if let Some(content_length) = response.content_length() {
            if content_length > max_bytes as u64 {
                bail!(
                    "telegram file {} exceeds download limit of {} bytes",
                    attachment.file_id,
                    max_bytes
                );
            }
        }

        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .context("telegram file download stream failed")?
        {
            if bytes.len().saturating_add(chunk.len()) > max_bytes {
                bail!(
                    "telegram file {} exceeds download limit of {} bytes",
                    attachment.file_id,
                    max_bytes
                );
            }
            bytes.extend_from_slice(&chunk);
        }

        Ok(DownloadedTelegramAttachment { remote_file, bytes })
    }

    fn api_url(&self, method: &str) -> String {
        format!("{}/bot{}/{}", self.api_base_url, self.token, method)
    }

    fn file_url(&self, file_path: &str) -> String {
        format!(
            "{}/bot{}/{}",
            self.file_base_url,
            self.token,
            file_path.trim_start_matches('/')
        )
    }

    async fn request_updates(
        &self,
        offset: Option<i64>,
        timeout_sec: u64,
        allowed_updates: &[&str],
    ) -> Result<ApiResponse<Vec<Update>>> {
        let url = self.api_url("getUpdates");
        let body = serde_json::json!({
            "offset": offset,
            "timeout": timeout_sec,
            "allowed_updates": allowed_updates,
        });

        self.http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram getUpdates request failed")
            .and_then(|response| {
                if response.status() == StatusCode::CONFLICT {
                    bail!(
                        "telegram getUpdates returned 409 Conflict. Stop any other bridge, live smoke, or bot worker that is reading updates for this bot, then retry.\n{}",
                        polling_conflict_hint()
                    );
                }
                response
                    .error_for_status()
                    .context("telegram getUpdates returned error status")
            })?
            .json()
            .await
            .context("failed to decode telegram getUpdates response")
    }
}

fn polling_conflict_hint() -> &'static str {
    "Windows では `Get-Process remotty, codex -ErrorAction SilentlyContinue | Select-Object Id,ProcessName,Path` で候補を確認できます。対象が分かる場合は `Stop-Process -Id <PID>` で止めてください。"
}

#[derive(Debug, Clone)]
pub struct SendMessageResult {
    pub message_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineKeyboardButton {
    pub text: String,
    pub callback_data: String,
}

#[derive(Debug)]
struct DownloadedTelegramAttachment {
    remote_file: TelegramRemoteFile,
    bytes: Vec<u8>,
}

fn parse_updates(response: ApiResponse<Vec<Update>>) -> Result<Vec<IncomingMessage>> {
    if !response.ok {
        bail!("telegram getUpdates returned ok=false");
    }

    let mut messages = Vec::new();
    for update in response.result {
        if let Some(message) = normalize_update(update)? {
            messages.push(message);
        }
    }
    Ok(messages)
}

fn parse_pairing_updates(response: ApiResponse<Vec<Update>>) -> Result<Vec<PairingUpdate>> {
    if !response.ok {
        bail!("telegram getUpdates returned ok=false");
    }

    let mut messages = Vec::new();
    for update in response.result {
        let Some(message) = update.message else {
            continue;
        };
        let Some(sender_id) = message.from.as_ref().map(|user| user.id) else {
            continue;
        };
        let text = normalized_message_text(&message, &collect_attachments(&message));
        if text.trim().is_empty() {
            continue;
        }
        messages.push(PairingUpdate {
            update_id: update.update_id,
            message: PairingMessage {
                chat_id: message.chat.id,
                chat_type: message.chat.kind,
                sender_id,
                text,
                sent_at_s: message.date.unwrap_or_default(),
            },
        });
    }
    Ok(messages)
}

fn normalize_update(update: Update) -> Result<Option<IncomingMessage>> {
    if let Some(callback_query) = update.callback_query {
        return normalize_callback_query(update.update_id, callback_query);
    }

    let message = match update.message.or(update.edited_message) {
        Some(message) => message,
        None => return Ok(None),
    };

    let attachments = collect_attachments(&message);
    let text = normalized_message_text(&message, &attachments);
    if text.is_empty() && attachments.is_empty() {
        return Ok(None);
    }

    let payload_json =
        serde_json::to_string(&message).context("failed to serialize telegram message")?;
    let thread_key = message
        .message_thread_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "dm".to_owned());

    Ok(Some(IncomingMessage {
        update_id: update.update_id,
        chat_id: message.chat.id,
        chat_type: message.chat.kind,
        sender_id: message.from.map(|user| user.id),
        text,
        attachments,
        telegram_message_id: message.message_id,
        thread_key,
        callback_query_id: None,
        control_command_override: None,
        payload_json,
    }))
}

fn normalize_callback_query(
    update_id: i64,
    callback_query: CallbackQuery,
) -> Result<Option<IncomingMessage>> {
    let Some(message) = callback_query.message.as_ref() else {
        return Ok(None);
    };
    let text = callback_query.data.clone().unwrap_or_default();
    let control_command_override = parse_callback_command(&text);
    if text.trim().is_empty() && control_command_override.is_none() {
        return Ok(None);
    }

    let payload_json =
        serde_json::to_string(&callback_query).context("failed to serialize callback query")?;
    let thread_key = message
        .message_thread_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "dm".to_owned());

    Ok(Some(IncomingMessage {
        update_id,
        chat_id: message.chat.id,
        chat_type: message.chat.kind.clone(),
        sender_id: Some(callback_query.from.id),
        text,
        attachments: Vec::new(),
        telegram_message_id: message.message_id,
        thread_key,
        callback_query_id: Some(callback_query.id),
        control_command_override,
        payload_json,
    }))
}

fn collect_attachments(message: &Message) -> Vec<TelegramAttachment> {
    let mut attachments = Vec::new();

    if let Some(photo) = select_largest_photo(message.photo.as_deref()) {
        attachments.push(TelegramAttachment {
            kind: TelegramAttachmentKind::Photo,
            file_id: photo.file_id.clone(),
            file_unique_id: photo.file_unique_id.clone(),
            file_name: None,
            mime_type: None,
            file_size: photo.file_size,
            width: Some(photo.width),
            height: Some(photo.height),
        });
    }

    if let Some(document) = message.document.as_ref() {
        attachments.push(TelegramAttachment {
            kind: TelegramAttachmentKind::Document,
            file_id: document.file_id.clone(),
            file_unique_id: document.file_unique_id.clone(),
            file_name: document.file_name.clone(),
            mime_type: document.mime_type.clone(),
            file_size: document.file_size,
            width: None,
            height: None,
        });
    }

    attachments
}

fn select_largest_photo(photos: Option<&[PhotoSize]>) -> Option<&PhotoSize> {
    photos.and_then(|photos| {
        photos.iter().max_by_key(|photo| {
            (
                photo.file_size.unwrap_or_default(),
                photo.width.saturating_mul(photo.height),
                photo.width,
                photo.height,
            )
        })
    })
}

fn normalized_message_text(message: &Message, attachments: &[TelegramAttachment]) -> String {
    first_non_empty([message.text.as_deref(), message.caption.as_deref()])
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| render_attachment_summary(attachments))
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<&'a str> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
}

fn render_attachment_summary(attachments: &[TelegramAttachment]) -> String {
    attachments
        .iter()
        .map(|attachment| match attachment.kind {
            TelegramAttachmentKind::Photo => "[Photo]".to_owned(),
            TelegramAttachmentKind::Document => attachment
                .file_name
                .as_deref()
                .map(|name| format!("[Document] {name}"))
                .unwrap_or_else(|| "[Document]".to_owned()),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn parse_control_command(text: &str) -> Option<TelegramControlCommand> {
    let text = text.trim();
    let command_token = text.split_whitespace().next()?;
    let command = parse_control_command_name(command_token)?;
    let rest = text[command_token.len()..].trim();
    let mut parts = rest.split_whitespace();

    if command.eq_ignore_ascii_case("help") {
        return parts
            .next()
            .is_none()
            .then_some(TelegramControlCommand::Help);
    }
    if command.eq_ignore_ascii_case("status") {
        return parts
            .next()
            .is_none()
            .then_some(TelegramControlCommand::Status);
    }
    if command.eq_ignore_ascii_case("stop") {
        return parts
            .next()
            .is_none()
            .then_some(TelegramControlCommand::Stop);
    }
    if command.eq_ignore_ascii_case("approve") {
        let request_id = parts.next()?.trim().to_owned();
        if request_id.is_empty() || parts.next().is_some() {
            return None;
        }
        return Some(TelegramControlCommand::Approve { request_id });
    }
    if command.eq_ignore_ascii_case("deny") {
        let request_id = parts.next()?.trim().to_owned();
        if request_id.is_empty() || parts.next().is_some() {
            return None;
        }
        return Some(TelegramControlCommand::Deny { request_id });
    }
    if command.eq_ignore_ascii_case("workspace") {
        let workspace_id = parts.next().map(|value| value.trim().to_owned());
        if parts.next().is_some() {
            return None;
        }
        return Some(TelegramControlCommand::Workspace {
            workspace_id: workspace_id.filter(|value| !value.is_empty()),
        });
    }
    if command.eq_ignore_ascii_case("sessions") || command.eq_ignore_ascii_case("remotty-sessions")
    {
        return Some(TelegramControlCommand::Sessions {
            thread_id: (!rest.is_empty()).then_some(rest.to_owned()),
        });
    }
    if command.eq_ignore_ascii_case("mode") {
        let mode = parts.next()?.trim().to_ascii_lowercase();
        if mode.is_empty() {
            return None;
        }
        let max_turns = match parts.next() {
            Some(raw_budget) => {
                if !mode.eq_ignore_ascii_case("max_turns") || parts.next().is_some() {
                    return None;
                }
                let parsed = raw_budget.parse::<i64>().ok()?;
                if parsed <= 0 {
                    return None;
                }
                Some(parsed)
            }
            None => None,
        };
        return Some(TelegramControlCommand::Mode { mode, max_turns });
    }

    None
}

fn parse_callback_command(text: &str) -> Option<TelegramControlCommand> {
    parse_control_command(text).or_else(|| {
        let (command, request_id) = text.split_once(':')?;
        let request_id = request_id.trim();
        if request_id.is_empty() {
            return None;
        }
        match command.trim().to_ascii_lowercase().as_str() {
            "approve" => Some(TelegramControlCommand::Approve {
                request_id: request_id.to_owned(),
            }),
            "deny" => Some(TelegramControlCommand::Deny {
                request_id: request_id.to_owned(),
            }),
            _ => None,
        }
    })
}

fn parse_control_command_name(token: &str) -> Option<&str> {
    let token = token.strip_prefix('/')?;
    let (command, _) = token.split_once('@').unwrap_or((token, ""));
    if command.is_empty() {
        None
    } else {
        Some(command)
    }
}

fn write_downloaded_attachment(
    directory: &Path,
    attachment: &TelegramAttachment,
    downloaded: DownloadedTelegramAttachment,
) -> Result<SavedTelegramAttachment> {
    fs::create_dir_all(directory).with_context(|| {
        format!(
            "failed to create telegram attachment directory {}",
            directory.display()
        )
    })?;

    let file_name = safe_attachment_filename(attachment, &downloaded.remote_file);
    let local_path = directory.join(&file_name);
    fs::write(&local_path, &downloaded.bytes).with_context(|| {
        format!(
            "failed to write telegram attachment to {}",
            local_path.display()
        )
    })?;

    Ok(SavedTelegramAttachment {
        attachment: attachment.clone(),
        remote_file: downloaded.remote_file,
        local_path,
        bytes_written: downloaded.bytes.len(),
    })
}

fn validate_download_limit(max_bytes: usize) -> Result<()> {
    if max_bytes == 0 {
        bail!("telegram attachment download limit must be greater than zero");
    }
    Ok(())
}

fn ensure_within_limit(file_size: Option<i64>, max_bytes: usize, file_id: &str) -> Result<()> {
    let Some(file_size) = file_size else {
        return Ok(());
    };
    let file_size = usize::try_from(file_size)
        .with_context(|| format!("telegram file {} returned invalid negative size", file_id))?;
    if file_size > max_bytes {
        bail!(
            "telegram file {} exceeds download limit of {} bytes",
            file_id,
            max_bytes
        );
    }
    Ok(())
}

fn safe_attachment_filename(
    attachment: &TelegramAttachment,
    remote_file: &TelegramRemoteFile,
) -> String {
    let stem = sanitize_file_component(
        attachment
            .file_name
            .as_deref()
            .and_then(file_stem_from_name)
            .unwrap_or(default_attachment_stem(attachment.kind)),
        default_attachment_stem(attachment.kind),
        80,
    );
    let unique = sanitize_file_component(&attachment.file_unique_id, "file", 40);
    let extension = attachment
        .file_name
        .as_deref()
        .and_then(file_extension_from_name)
        .or_else(|| file_name_from_path(&remote_file.file_path).and_then(file_extension_from_name))
        .or_else(|| default_attachment_extension(attachment.kind))
        .map(sanitize_extension)
        .filter(|value| !value.is_empty());

    let mut file_name = format!("{stem}_{unique}");
    if let Some(extension) = extension {
        file_name.push('.');
        file_name.push_str(&extension);
    }
    file_name
}

fn default_attachment_stem(kind: TelegramAttachmentKind) -> &'static str {
    match kind {
        TelegramAttachmentKind::Photo => "photo",
        TelegramAttachmentKind::Document => "document",
    }
}

fn default_attachment_extension(kind: TelegramAttachmentKind) -> Option<&'static str> {
    match kind {
        TelegramAttachmentKind::Photo => Some("jpg"),
        TelegramAttachmentKind::Document => None,
    }
}

fn sanitize_file_component(value: &str, fallback: &str, max_len: usize) -> String {
    let last_segment = file_name_from_path(value).unwrap_or(value);
    let mut sanitized = String::new();
    let mut previous_was_separator = false;

    for character in last_segment.chars() {
        let mapped = if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) {
            '_'
        } else if character.is_whitespace() {
            '_'
        } else {
            character
        };

        if mapped == '_' {
            if previous_was_separator {
                continue;
            }
            previous_was_separator = true;
        } else {
            previous_was_separator = false;
        }
        sanitized.push(mapped);
    }

    let mut trimmed = sanitized
        .trim_matches(|character| matches!(character, '.' | ' ' | '_'))
        .chars()
        .take(max_len)
        .collect::<String>();

    if trimmed.is_empty() {
        trimmed = fallback.to_owned();
    }
    if is_windows_reserved_name(&trimmed) {
        trimmed.insert(0, '_');
    }
    trimmed
}

fn sanitize_extension(extension: &str) -> String {
    extension
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned()
}

fn file_name_from_path(path: &str) -> Option<&str> {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.trim().is_empty())
}

fn file_stem_from_name(name: &str) -> Option<&str> {
    let name = file_name_from_path(name)?;
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => Some(stem),
        _ if !name.is_empty() => Some(name),
        _ => None,
    }
}

fn file_extension_from_name(name: &str) -> Option<&str> {
    let name = file_name_from_path(name)?;
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        None
    } else {
        Some(extension)
    }
}

fn is_windows_reserved_name(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
fn parse_updates_response(body: &str) -> Result<Vec<IncomingMessage>> {
    let response: ApiResponse<Vec<Update>> =
        serde_json::from_str(body).context("failed to decode telegram test response")?;
    parse_updates(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_plain_text_message_without_attachments() {
        let messages = parse_updates_response(
            r#"{
                "ok": true,
                "result": [
                    {
                        "update_id": 101,
                        "message": {
                            "message_id": 7,
                            "text": "hello from telegram",
                            "chat": { "id": 42, "type": "private" },
                            "from": { "id": 9 }
                        }
                    }
                ]
            }"#,
        )
        .expect("plain text update should parse");

        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.update_id, 101);
        assert_eq!(message.chat_id, 42);
        assert_eq!(message.chat_type, "private");
        assert_eq!(message.sender_id, Some(9));
        assert_eq!(message.text, "hello from telegram");
        assert_eq!(message.telegram_message_id, 7);
        assert_eq!(message.thread_key, "dm");
        assert!(message.attachments.is_empty());
    }

    #[test]
    fn parses_photo_attachment_with_caption() {
        let messages = parse_updates_response(
            r#"{
                "ok": true,
                "result": [
                    {
                        "update_id": 202,
                        "message": {
                            "message_id": 11,
                            "caption": "see attached",
                            "chat": { "id": -100, "type": "supergroup" },
                            "from": { "id": 12 },
                            "message_thread_id": 555,
                            "photo": [
                                {
                                    "file_id": "small-file",
                                    "file_unique_id": "photo-1",
                                    "width": 90,
                                    "height": 90,
                                    "file_size": 1200
                                },
                                {
                                    "file_id": "large-file",
                                    "file_unique_id": "photo-1",
                                    "width": 1280,
                                    "height": 720,
                                    "file_size": 4800
                                }
                            ]
                        }
                    }
                ]
            }"#,
        )
        .expect("photo update should parse");

        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.text, "see attached");
        assert_eq!(message.thread_key, "555");
        assert_eq!(message.attachments.len(), 1);

        let attachment = &message.attachments[0];
        assert_eq!(attachment.kind, TelegramAttachmentKind::Photo);
        assert_eq!(attachment.file_id, "large-file");
        assert_eq!(attachment.file_unique_id, "photo-1");
        assert_eq!(attachment.file_size, Some(4800));
        assert_eq!(attachment.width, Some(1280));
        assert_eq!(attachment.height, Some(720));
        assert_eq!(attachment.file_name, None);
        assert_eq!(attachment.mime_type, None);
    }

    #[test]
    fn parses_document_attachment_without_caption() {
        let messages = parse_updates_response(
            r#"{
                "ok": true,
                "result": [
                    {
                        "update_id": 303,
                        "message": {
                            "message_id": 19,
                            "chat": { "id": 77, "type": "private" },
                            "from": { "id": 21 },
                            "document": {
                                "file_id": "doc-file",
                                "file_unique_id": "doc-1",
                                "file_name": "report.pdf",
                                "mime_type": "application/pdf",
                                "file_size": 2048
                            }
                        }
                    }
                ]
            }"#,
        )
        .expect("document update should parse");

        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.text, "[Document] report.pdf");
        assert_eq!(message.attachments.len(), 1);

        let attachment = &message.attachments[0];
        assert_eq!(attachment.kind, TelegramAttachmentKind::Document);
        assert_eq!(attachment.file_id, "doc-file");
        assert_eq!(attachment.file_unique_id, "doc-1");
        assert_eq!(attachment.file_name.as_deref(), Some("report.pdf"));
        assert_eq!(attachment.mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(attachment.file_size, Some(2048));
        assert_eq!(attachment.width, None);
        assert_eq!(attachment.height, None);
    }

    #[test]
    fn select_largest_photo_prefers_file_size_then_dimensions() {
        let photos = vec![
            PhotoSize {
                file_id: "smaller".to_owned(),
                file_unique_id: "photo-1".to_owned(),
                width: 1024,
                height: 1024,
                file_size: Some(2_000),
            },
            PhotoSize {
                file_id: "larger".to_owned(),
                file_unique_id: "photo-1".to_owned(),
                width: 800,
                height: 800,
                file_size: Some(4_000),
            },
        ];

        let selected = select_largest_photo(Some(&photos)).expect("photo should be selected");
        assert_eq!(selected.file_id, "larger");
    }

    #[test]
    fn safe_attachment_filename_sanitizes_reserved_document_name() {
        let attachment = TelegramAttachment {
            kind: TelegramAttachmentKind::Document,
            file_id: "doc-file".to_owned(),
            file_unique_id: "doc:1".to_owned(),
            file_name: Some("../CON?.pdf".to_owned()),
            mime_type: Some("application/pdf".to_owned()),
            file_size: Some(2048),
            width: None,
            height: None,
        };
        let remote_file = TelegramRemoteFile {
            file_id: "doc-file".to_owned(),
            file_unique_id: "doc:1".to_owned(),
            file_path: "documents/report.bin".to_owned(),
            file_size: Some(2048),
        };

        let file_name = safe_attachment_filename(&attachment, &remote_file);
        assert_eq!(file_name, "_CON_doc_1.pdf");
    }

    #[test]
    fn write_downloaded_attachment_creates_safe_file() {
        let directory = tempdir().expect("tempdir should be created");
        let attachment = TelegramAttachment {
            kind: TelegramAttachmentKind::Photo,
            file_id: "photo-file".to_owned(),
            file_unique_id: "photo-1".to_owned(),
            file_name: None,
            mime_type: None,
            file_size: Some(5),
            width: Some(100),
            height: Some(100),
        };
        let downloaded = DownloadedTelegramAttachment {
            remote_file: TelegramRemoteFile {
                file_id: "photo-file".to_owned(),
                file_unique_id: "photo-1".to_owned(),
                file_path: "photos/file_10.JPG".to_owned(),
                file_size: Some(5),
            },
            bytes: b"hello".to_vec(),
        };

        let saved = write_downloaded_attachment(directory.path(), &attachment, downloaded)
            .expect("attachment should be written");

        assert_eq!(
            saved
                .local_path
                .file_name()
                .and_then(|value| value.to_str()),
            Some("photo_photo-1.jpg")
        );
        assert_eq!(
            fs::read(&saved.local_path).expect("file should exist"),
            b"hello"
        );
        assert_eq!(saved.bytes_written, 5);
    }

    #[test]
    fn parses_help_command() {
        assert_eq!(
            parse_control_command(" /help "),
            Some(TelegramControlCommand::Help)
        );
    }

    #[test]
    fn parses_status_command_with_bot_suffix() {
        assert_eq!(
            parse_control_command("/status@remotty_bot"),
            Some(TelegramControlCommand::Status)
        );
    }

    #[test]
    fn parses_mode_command_argument() {
        assert_eq!(
            parse_control_command("/mode completion_checks"),
            Some(TelegramControlCommand::Mode {
                mode: "completion_checks".to_owned(),
                max_turns: None,
            })
        );
    }

    #[test]
    fn parses_mode_command_with_max_turns_budget() {
        assert_eq!(
            parse_control_command("/mode max_turns 5"),
            Some(TelegramControlCommand::Mode {
                mode: "max_turns".to_owned(),
                max_turns: Some(5),
            })
        );
    }

    #[test]
    fn ignores_mode_command_without_argument() {
        assert_eq!(parse_control_command("/mode"), None);
    }

    #[test]
    fn parses_workspace_command_without_argument() {
        assert_eq!(
            parse_control_command("/workspace"),
            Some(TelegramControlCommand::Workspace { workspace_id: None })
        );
    }

    #[test]
    fn parses_workspace_command_with_id() {
        assert_eq!(
            parse_control_command("/workspace docs"),
            Some(TelegramControlCommand::Workspace {
                workspace_id: Some("docs".to_owned())
            })
        );
    }

    #[test]
    fn parses_sessions_command_without_id() {
        assert_eq!(
            parse_control_command("/remotty-sessions"),
            Some(TelegramControlCommand::Sessions { thread_id: None })
        );
    }

    #[test]
    fn parses_sessions_command_with_id() {
        assert_eq!(
            parse_control_command("/sessions thread-1"),
            Some(TelegramControlCommand::Sessions {
                thread_id: Some("thread-1".to_owned())
            })
        );
    }

    #[test]
    fn parses_sessions_command_with_title() {
        assert_eq!(
            parse_control_command("/remotty-sessions Start workspace session"),
            Some(TelegramControlCommand::Sessions {
                thread_id: Some("Start workspace session".to_owned())
            })
        );
    }

    #[test]
    fn parses_sessions_command_preserves_inner_title_whitespace() {
        assert_eq!(
            parse_control_command("/remotty-sessions Start  workspace  session"),
            Some(TelegramControlCommand::Sessions {
                thread_id: Some("Start  workspace  session".to_owned())
            })
        );
    }

    #[test]
    fn parses_stop_command() {
        assert_eq!(
            TelegramControlCommand::parse("/stop"),
            Some(TelegramControlCommand::Stop)
        );
    }

    #[test]
    fn parses_approve_command() {
        assert_eq!(
            parse_control_command("/approve req-1"),
            Some(TelegramControlCommand::Approve {
                request_id: "req-1".to_owned(),
            })
        );
    }

    #[test]
    fn parses_deny_command() {
        assert_eq!(
            parse_control_command("/deny req-2"),
            Some(TelegramControlCommand::Deny {
                request_id: "req-2".to_owned(),
            })
        );
    }

    #[test]
    fn parses_callback_query_as_control_command() {
        let messages = parse_updates_response(
            r#"{
                "ok": true,
                "result": [
                    {
                        "update_id": 303,
                        "callback_query": {
                            "id": "callback-1",
                            "from": { "id": 12 },
                            "data": "approve:req-9",
                            "message": {
                                "message_id": 44,
                                "chat": { "id": 42, "type": "private" }
                            }
                        }
                    }
                ]
            }"#,
        )
        .expect("callback update should parse");

        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.callback_query_id.as_deref(), Some("callback-1"));
        assert_eq!(
            message.control_command(),
            Some(TelegramControlCommand::Approve {
                request_id: "req-9".to_owned(),
            })
        );
    }

    #[test]
    fn reads_control_command_from_incoming_message_text() {
        let message = IncomingMessage {
            update_id: 1,
            chat_id: 10,
            chat_type: "private".to_owned(),
            sender_id: Some(20),
            text: " /status ".to_owned(),
            attachments: Vec::new(),
            telegram_message_id: 30,
            thread_key: "dm".to_owned(),
            callback_query_id: None,
            control_command_override: None,
            payload_json: "{}".to_owned(),
        };

        assert_eq!(
            message.control_command(),
            Some(TelegramControlCommand::Status)
        );
    }

    #[test]
    fn rejects_extra_arguments_for_simple_commands() {
        assert_eq!(parse_control_command("/help now"), None);
        assert_eq!(parse_control_command("/status now"), None);
        assert_eq!(parse_control_command("/stop now"), None);
        assert_eq!(parse_control_command("/approve req extra"), None);
        assert_eq!(parse_control_command("/deny req extra"), None);
        assert_eq!(parse_control_command("/workspace docs extra"), None);
    }

    #[test]
    fn rejects_mode_command_with_extra_arguments() {
        assert_eq!(parse_control_command("/mode completion checks"), None);
        assert_eq!(parse_control_command("/mode max_turns 3 extra"), None);
        assert_eq!(parse_control_command("/mode max_turns 0"), None);
    }
}
