use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    pub telegram_message_id: i64,
    pub thread_key: String,
    pub payload_json: String,
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
    chat: Chat,
    from: Option<User>,
    message_thread_id: Option<i64>,
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

        if !response.ok {
            bail!("telegram getUpdates returned ok=false");
        }

        let mut messages = Vec::new();
        for update in response.result {
            let message = update.message.or(update.edited_message);
            if let Some(message) = message {
                let text = match message.text.clone() {
                    Some(text) if !text.trim().is_empty() => text,
                    _ => continue,
                };
                let payload_json = serde_json::to_string(&message)
                    .context("failed to serialize telegram message")?;
                let thread_key = message
                    .message_thread_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "dm".to_owned());
                messages.push(IncomingMessage {
                    update_id: update.update_id,
                    chat_id: message.chat.id,
                    chat_type: message.chat.kind,
                    sender_id: message.from.map(|user| user.id),
                    text,
                    telegram_message_id: message.message_id,
                    thread_key,
                    payload_json,
                });
            }
        }
        Ok(messages)
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
}

#[derive(Debug, Clone)]
pub struct SendMessageResult {
    pub message_id: i64,
}
