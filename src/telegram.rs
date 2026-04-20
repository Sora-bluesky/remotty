use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct TelegramClient {
    http: Client,
    token: String,
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
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramControlCommand {
    Help,
    Status,
    Stop,
    Workspace {
        workspace_id: Option<String>,
    },
    Mode {
        mode: String,
        max_turns: Option<i64>,
    },
}

impl IncomingMessage {
    pub fn control_command(&self) -> Option<TelegramControlCommand> {
        parse_control_command(&self.text)
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Message {
    message_id: i64,
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

impl TelegramClient {
    pub fn new(token: String) -> Self {
        Self {
            http: Client::new(),
            token,
        }
    }

    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        timeout_sec: u64,
    ) -> Result<Vec<IncomingMessage>> {
        let url = format!("https://api.telegram.org/bot{}/getUpdates", self.token);
        let body = serde_json::json!({
            "offset": offset,
            "timeout": timeout_sec,
            "allowed_updates": ["message", "edited_message"],
        });

        let response: ApiResponse<Vec<Update>> = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .context("telegram getUpdates request failed")?
            .error_for_status()
            .context("telegram getUpdates returned error status")?
            .json()
            .await
            .context("failed to decode telegram getUpdates response")?;

        parse_updates(response)
    }

    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<SendMessageResult> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
        });
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

    pub async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str) -> Result<()> {
        let url = format!("https://api.telegram.org/bot{}/editMessageText", self.token);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "text": text,
        });
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
        let url = format!("https://api.telegram.org/bot{}/getFile", self.token);
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

        let url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.token, remote_file.file_path
        );
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
}

#[derive(Debug, Clone)]
pub struct SendMessageResult {
    pub message_id: i64,
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

fn normalize_update(update: Update) -> Result<Option<IncomingMessage>> {
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
    let mut parts = text.trim().split_whitespace();
    let command = parse_control_command_name(parts.next()?)?;

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
    if command.eq_ignore_ascii_case("workspace") {
        let workspace_id = parts.next().map(|value| value.trim().to_owned());
        if parts.next().is_some() {
            return None;
        }
        return Some(TelegramControlCommand::Workspace {
            workspace_id: workspace_id.filter(|value| !value.is_empty()),
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
            parse_control_command("/status@codex_channels_bot"),
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
    fn parses_stop_command() {
        assert_eq!(
            TelegramControlCommand::parse("/stop"),
            Some(TelegramControlCommand::Stop)
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
        assert_eq!(parse_control_command("/workspace docs extra"), None);
    }

    #[test]
    fn rejects_mode_command_with_extra_arguments() {
        assert_eq!(parse_control_command("/mode completion checks"), None);
        assert_eq!(parse_control_command("/mode max_turns 3 extra"), None);
        assert_eq!(parse_control_command("/mode max_turns 0"), None);
    }
}
