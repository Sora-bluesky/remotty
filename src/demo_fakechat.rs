use std::path::PathBuf;
use std::sync::Arc;

#[cfg(test)]
use std::net::SocketAddr;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
#[cfg(test)]
use tokio::task::JoinHandle;

use crate::cli::FakechatOptions;
use crate::codex::{CodexRequest, CodexRunner};
use crate::config::{CodexConfig, CodexTransport, LaneMode, WorkspaceConfig};

#[derive(Clone)]
struct AppState {
    runner: CodexRunner,
    workspace: WorkspaceConfig,
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageRequest {
    message: String,
}

#[derive(Debug, Serialize)]
struct MessageResponse {
    reply: String,
    exit_code: Option<i32>,
    approval_pending: bool,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(serde_json::json!({
            "error": self.0.to_string()
        }));
        (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self(value.into())
    }
}

pub async fn run_fakechat(options: FakechatOptions) -> Result<()> {
    let listener = bind_listener(&options).await?;
    let address = listener
        .local_addr()
        .context("failed to read fakechat local address")?;
    println!("remotty fakechat running at http://{address}");
    println!("Open this URL in your browser. Press Ctrl+C to stop.");

    axum::serve(listener, app(options)?)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .context("fakechat server failed")
}

#[cfg(test)]
pub async fn spawn_fakechat(options: FakechatOptions) -> Result<(SocketAddr, JoinHandle<()>)> {
    let listener = bind_listener(&options).await?;
    let address = listener
        .local_addr()
        .context("failed to read fakechat local address")?;
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app(options).expect("fakechat app should build")).await;
    });
    Ok((address, handle))
}

async fn bind_listener(options: &FakechatOptions) -> Result<TcpListener> {
    TcpListener::bind((options.host.as_str(), options.port))
        .await
        .with_context(|| {
            format!(
                "failed to bind fakechat server on {}:{}",
                options.host, options.port
            )
        })
}

fn app(options: FakechatOptions) -> Result<Router> {
    let workspace_path = normalize_workspace_path(options.workspace)?;
    let use_app_server = options.thread_id.is_some();
    let state = AppState {
        runner: CodexRunner::new(CodexConfig {
            binary: options.codex_binary,
            model: options.model,
            sandbox: "read-only".to_owned(),
            approval: "never".to_owned(),
            transport: if use_app_server {
                CodexTransport::AppServer
            } else {
                CodexTransport::Exec
            },
            profile: None,
        }),
        workspace: WorkspaceConfig {
            id: "fakechat".to_owned(),
            path: workspace_path.clone(),
            writable_roots: vec![workspace_path],
            default_mode: LaneMode::AwaitReply,
            continue_prompt: "Continue only if needed.".to_owned(),
            checks_profile: "default".to_owned(),
        },
        thread_id: options.thread_id,
    };

    Ok(Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/message", post(message))
        .with_state(Arc::new(state)))
}

fn normalize_workspace_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .context("failed to resolve current directory")
        .map(|current| current.join(path))
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn message(
    State(state): State<Arc<AppState>>,
    Json(request): Json<MessageRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let prompt = request.message.trim();
    if prompt.is_empty() {
        return Err(anyhow::anyhow!("message must not be empty").into());
    }

    let request = CodexRequest::new(prompt);
    let outcome = if let Some(thread_id) = state.thread_id.as_deref() {
        state
            .runner
            .resume(&state.workspace, thread_id, request)
            .await?
    } else {
        state.runner.start(&state.workspace, request).await?
    };
    let reply = if outcome.last_message.trim().is_empty() && outcome.approval_pending {
        "Codex is waiting for local approval in the terminal.".to_owned()
    } else {
        outcome.last_message
    };

    Ok(Json(MessageResponse {
        reply,
        exit_code: outcome.exit_code,
        approval_pending: outcome.approval_pending,
    }))
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>remotty fakechat</title>
  <style>
    :root {
      color-scheme: light dark;
      font-family: "Segoe UI", system-ui, sans-serif;
      line-height: 1.4;
    }
    body {
      margin: 0;
      min-height: 100vh;
      background: Canvas;
      color: CanvasText;
      display: grid;
      grid-template-rows: auto 1fr auto;
    }
    header {
      border-bottom: 1px solid color-mix(in srgb, CanvasText 20%, transparent);
      padding: 14px 18px;
    }
    h1 {
      font-size: 18px;
      margin: 0;
      font-weight: 650;
    }
    main {
      overflow-y: auto;
      padding: 18px;
    }
    .message {
      max-width: 860px;
      margin: 0 0 12px;
      padding: 10px 12px;
      border: 1px solid color-mix(in srgb, CanvasText 18%, transparent);
      border-radius: 8px;
      white-space: pre-wrap;
    }
    .user {
      margin-left: auto;
      background: color-mix(in srgb, Highlight 16%, Canvas);
    }
    .assistant {
      margin-right: auto;
      background: color-mix(in srgb, CanvasText 6%, Canvas);
    }
    form {
      border-top: 1px solid color-mix(in srgb, CanvasText 20%, transparent);
      display: grid;
      grid-template-columns: 1fr auto;
      gap: 10px;
      padding: 12px;
    }
    textarea {
      resize: vertical;
      min-height: 46px;
      max-height: 180px;
      padding: 10px;
      font: inherit;
      border-radius: 8px;
      border: 1px solid color-mix(in srgb, CanvasText 30%, transparent);
      background: Canvas;
      color: CanvasText;
    }
    button {
      min-width: 88px;
      border: 0;
      border-radius: 8px;
      padding: 0 16px;
      font: inherit;
      font-weight: 650;
      background: Highlight;
      color: HighlightText;
      cursor: pointer;
    }
    button:disabled {
      cursor: wait;
      opacity: .65;
    }
  </style>
</head>
<body>
  <header><h1>remotty fakechat</h1></header>
  <main id="messages"></main>
  <form id="form">
    <textarea id="message" placeholder="Ask about the current workspace" autofocus></textarea>
    <button id="send" type="submit">Send</button>
  </form>
  <script>
    const form = document.getElementById("form");
    const textarea = document.getElementById("message");
    const send = document.getElementById("send");
    const messages = document.getElementById("messages");

    function addMessage(kind, text) {
      const element = document.createElement("div");
      element.className = `message ${kind}`;
      element.textContent = text;
      messages.appendChild(element);
      messages.scrollTop = messages.scrollHeight;
    }

    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const text = textarea.value.trim();
      if (!text) return;
      addMessage("user", text);
      textarea.value = "";
      send.disabled = true;
      try {
        const response = await fetch("/api/message", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ message: text })
        });
        const body = await response.json();
        if (!response.ok) {
          throw new Error(body.error || "request failed");
        }
        addMessage("assistant", body.reply || "(no reply)");
      } catch (error) {
        addMessage("assistant", `Error: ${error.message}`);
      } finally {
        send.disabled = false;
        textarea.focus();
      }
    });
  </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use anyhow::Result;
    use reqwest::StatusCode;
    use serde_json::json;

    use crate::cli::FakechatOptions;

    use super::spawn_fakechat;

    #[tokio::test]
    async fn fakechat_replies_through_fake_codex() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let fake_codex = write_fake_codex(temp.path(), "fakechat reply")?;
        let (address, handle) = spawn_fakechat(FakechatOptions {
            host: "127.0.0.1".to_owned(),
            port: 0,
            workspace: temp.path().to_path_buf(),
            codex_binary: fake_codex.display().to_string(),
            model: "gpt-5.4-mini".to_owned(),
            thread_id: None,
        })
        .await?;

        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/message"))
            .json(&json!({ "message": "hello" }))
            .send()
            .await?;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.json::<serde_json::Value>().await?;
        assert_eq!(body["reply"], "fakechat reply");
        assert_eq!(body["exit_code"], 0);
        assert_eq!(body["approval_pending"], false);

        handle.abort();
        Ok(())
    }

    #[tokio::test]
    async fn fakechat_resumes_selected_thread_through_fake_app_server() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let fake_codex = write_fake_app_server_codex(temp.path(), "saved thread reply")?;
        let (address, handle) = spawn_fakechat(FakechatOptions {
            host: "127.0.0.1".to_owned(),
            port: 0,
            workspace: temp.path().to_path_buf(),
            codex_binary: fake_codex.display().to_string(),
            model: "gpt-5.4-mini".to_owned(),
            thread_id: Some("thread-saved".to_owned()),
        })
        .await?;

        let response = reqwest::Client::new()
            .post(format!("http://{address}/api/message"))
            .json(&json!({ "message": "hello" }))
            .send()
            .await?;
        let status = response.status();
        let body = response.json::<serde_json::Value>().await?;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["reply"], "saved thread reply");
        assert_eq!(body["exit_code"], 0);
        assert_eq!(body["approval_pending"], false);

        handle.abort();
        Ok(())
    }

    fn write_fake_codex(root: &Path, reply_text: &str) -> Result<PathBuf> {
        #[cfg(windows)]
        {
            let path = root.join("fake-codex.cmd");
            let escaped_reply = reply_text.replace('"', "\\\"");
            fs::write(
                &path,
                format!(
                    "@echo off\r\n\
echo {{^\"type^\":^\"thread.started^\",^\"thread_id^\":^\"thread-fakechat^\"}}\r\n\
echo {{^\"type^\":^\"item.completed^\",^\"item^\":{{^\"type^\":^\"agent_message^\",^\"text^\":^\"{}^\"}}}}\r\n",
                    escaped_reply
                ),
            )?;
            Ok(path)
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = root.join("fake-codex");
            let escaped_reply = reply_text.replace('\'', "'\\''");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\n\
printf '%s\n' '{{\"type\":\"thread.started\",\"thread_id\":\"thread-fakechat\"}}'\n\
printf '%s\n' '{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"{}\"}}}}'\n",
                    escaped_reply
                ),
            )?;
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)?;
            Ok(path)
        }
    }

    fn write_fake_app_server_codex(root: &Path, reply_text: &str) -> Result<PathBuf> {
        #[cfg(windows)]
        {
            let path = root.join("fake-app-server-codex.cmd");
            let script_path = root.join("fake-app-server-codex.ps1");
            fs::write(
                &path,
                "@echo off\r\npwsh -NoProfile -ExecutionPolicy Bypass -File \"%~dp0fake-app-server-codex.ps1\"\r\n",
            )?;
            let escaped_reply = reply_text.replace('\'', "''");
            let script = r#"
while ($null -ne ($line = [Console]::In.ReadLine())) {
    if ($line.Contains('"id":"client-1"')) {
        Write-Output '{"id":"client-1","result":{"userAgent":"Codex/0.118.0"}}'
        continue
    }
    if ($line.Contains('"id":"client-2"')) {
        Write-Output '{"id":"client-2","result":{}}'
        continue
    }
    if ($line.Contains('"id":"client-3"')) {
        Write-Output '{"id":"client-3","result":{"turn":{"id":"turn-fakechat"}}}'
        Write-Output '{"method":"turn/completed","params":{"turn":{"id":"turn-fakechat","status":"completed"}}}'
        continue
    }
    if ($line.Contains('"id":"client-4"')) {
        Write-Output '{"id":"client-4","result":{"thread":{"turns":[{"id":"turn-fakechat","items":[{"type":"agentMessage","text":"__REPLY__"}]}]}}}'
        continue
    }
}
"#
            .replace("__REPLY__", &escaped_reply);
            fs::write(&script_path, script)?;
            Ok(path)
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = root.join("fake-app-server-codex");
            let escaped_reply = reply_text.replace('\'', "'\\''");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\n\
while IFS= read -r line; do\n\
  case \"$line\" in\n\
    *client-1*) printf '%s\\n' '{{\"id\":\"client-1\",\"result\":{{\"userAgent\":\"Codex/0.118.0\"}}}}' ;;\n\
    *client-2*) printf '%s\\n' '{{\"id\":\"client-2\",\"result\":{{}}}}' ;;\n\
    *client-3*) printf '%s\\n' '{{\"id\":\"client-3\",\"result\":{{\"turn\":{{\"id\":\"turn-fakechat\"}}}}}}'; printf '%s\\n' '{{\"method\":\"turn/completed\",\"params\":{{\"turn\":{{\"id\":\"turn-fakechat\",\"status\":\"completed\"}}}}}}' ;;\n\
    *client-4*) printf '%s\\n' '{{\"id\":\"client-4\",\"result\":{{\"thread\":{{\"turns\":[{{\"id\":\"turn-fakechat\",\"items\":[{{\"type\":\"agentMessage\",\"text\":\"{}\"}}]}}]}}}}}}' ;;\n\
  esac\n\
done\n",
                    escaped_reply
                ),
            )?;
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)?;
            Ok(path)
        }
    }
}
