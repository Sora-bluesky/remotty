use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;

use crate::app_server::{AppServerClient, CodexApprovalRequest, CodexThreadSummary};
use crate::config::{CodexConfig, CodexTransport, WorkspaceConfig};
use crate::store::{ApprovalRequestRecord, Store};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone)]
pub struct CodexOutcome {
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub last_message: String,
    pub exit_code: Option<i32>,
    pub approval_pending: bool,
    pub approval_requests: Vec<CodexApprovalRequest>,
    pub approval_request_count: i64,
    pub approval_resolved_count: i64,
}

#[derive(Debug, Clone)]
pub struct CodexRequest {
    pub prompt: String,
    pub image_paths: Vec<PathBuf>,
}

pub struct CodexFollowupRequest {
    pub request: CodexRequest,
    pub ack: oneshot::Sender<Result<()>>,
}

#[derive(Clone)]
pub struct ActiveAppServerTurnPersistence {
    store: Store,
    lane_id: String,
    run_id: String,
}

impl ActiveAppServerTurnPersistence {
    pub fn new(store: Store, lane_id: String, run_id: String) -> Self {
        Self {
            store,
            lane_id,
            run_id,
        }
    }

    pub fn persist(&self, thread_id: &str, turn_id: &str) -> Result<()> {
        self.store
            .update_lane_active_turn(&self.lane_id, &self.run_id, thread_id, turn_id)
    }
}

impl CodexRequest {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            image_paths: Vec::new(),
        }
    }

    pub fn with_images(
        prompt: impl Into<String>,
        image_paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            image_paths: image_paths.into_iter().collect(),
        }
    }

    fn into_args(self) -> Vec<String> {
        let mut args = vec![self.prompt];
        for image_path in self.image_paths {
            args.push("--image".to_owned());
            args.push(image_path.display().to_string());
        }
        args
    }
}

impl From<&str> for CodexRequest {
    fn from(prompt: &str) -> Self {
        Self::new(prompt)
    }
}

impl From<&String> for CodexRequest {
    fn from(prompt: &String) -> Self {
        Self::new(prompt.clone())
    }
}

impl From<String> for CodexRequest {
    fn from(prompt: String) -> Self {
        Self::new(prompt)
    }
}

#[derive(Clone)]
pub struct CodexRunner {
    config: CodexConfig,
    app_server: Arc<Mutex<Option<AppServerClient>>>,
}

impl CodexRunner {
    pub fn new(config: CodexConfig) -> Self {
        Self {
            config,
            app_server: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn start(
        &self,
        workspace: &WorkspaceConfig,
        request: impl Into<CodexRequest>,
    ) -> Result<CodexOutcome> {
        self.start_with_followups(workspace, request, None).await
    }

    pub async fn start_with_followups(
        &self,
        workspace: &WorkspaceConfig,
        request: impl Into<CodexRequest>,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
    ) -> Result<CodexOutcome> {
        self.start_with_followups_and_turn_sender(workspace, request, followups, None)
            .await
    }

    pub async fn start_with_followups_and_turn_sender(
        &self,
        workspace: &WorkspaceConfig,
        request: impl Into<CodexRequest>,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
        turn_persistence: Option<ActiveAppServerTurnPersistence>,
    ) -> Result<CodexOutcome> {
        match self.config.transport {
            CodexTransport::Exec => {
                let mut args = self.base_args(workspace);
                args.extend(request.into().into_args());
                self.run_command(args, &workspace.path).await
            }
            CodexTransport::AppServer => {
                let mut client = AppServerClient::spawn(&self.config).await?;
                client
                    .start_turn(
                        &self.config,
                        workspace,
                        request.into(),
                        followups,
                        turn_persistence,
                    )
                    .await
            }
        }
    }

    pub async fn resume(
        &self,
        workspace: &WorkspaceConfig,
        session_id: &str,
        request: impl Into<CodexRequest>,
    ) -> Result<CodexOutcome> {
        self.resume_with_followups(workspace, session_id, request, None)
            .await
    }

    pub async fn resume_with_followups(
        &self,
        workspace: &WorkspaceConfig,
        session_id: &str,
        request: impl Into<CodexRequest>,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
    ) -> Result<CodexOutcome> {
        self.resume_with_followups_and_turn_sender(workspace, session_id, request, followups, None)
            .await
    }

    pub async fn resume_with_followups_and_turn_sender(
        &self,
        workspace: &WorkspaceConfig,
        session_id: &str,
        request: impl Into<CodexRequest>,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
        turn_persistence: Option<ActiveAppServerTurnPersistence>,
    ) -> Result<CodexOutcome> {
        match self.config.transport {
            CodexTransport::Exec => {
                let mut args = vec![
                    "exec".to_owned(),
                    "resume".to_owned(),
                    session_id.to_owned(),
                ];
                args.extend(request.into().into_args());
                args.push("--json".to_owned());
                self.run_command(args, &workspace.path).await
            }
            CodexTransport::AppServer => {
                let mut client = AppServerClient::spawn(&self.config).await?;
                client
                    .resume_turn(
                        &self.config,
                        workspace,
                        session_id,
                        request.into(),
                        followups,
                        turn_persistence,
                    )
                    .await
            }
        }
    }

    pub async fn resolve_approval(
        &self,
        request: &ApprovalRequestRecord,
        approved: bool,
    ) -> Result<CodexOutcome> {
        let mut client = self.ensure_app_server().await?;
        client
            .as_mut()
            .expect("app-server should exist")
            .resolve_approval(request, approved)
            .await
    }

    pub async fn list_threads(
        &self,
        limit: usize,
        filter: Option<&str>,
    ) -> Result<Vec<CodexThreadSummary>> {
        let mut client = self.ensure_app_server().await?;
        client
            .as_mut()
            .expect("app-server should exist")
            .list_threads(limit, filter)
            .await
    }

    fn base_args(&self, workspace: &WorkspaceConfig) -> Vec<String> {
        let mut args = vec![
            "exec".to_owned(),
            "--json".to_owned(),
            "--skip-git-repo-check".to_owned(),
            "--sandbox".to_owned(),
            self.config.sandbox.clone(),
            "--config".to_owned(),
            format!("approval_policy=\"{}\"", self.config.approval),
            "--cd".to_owned(),
            workspace.path.display().to_string(),
        ];
        let model = self.config.model.trim();
        if !model.is_empty() {
            args.push("--model".to_owned());
            args.push(model.to_owned());
        }
        if let Some(profile) = self
            .config
            .profile
            .as_deref()
            .map(str::trim)
            .filter(|profile| !profile.is_empty())
        {
            args.push("--profile".to_owned());
            args.push(profile.to_owned());
        }
        args
    }

    async fn run_command(&self, args: Vec<String>, cwd: &std::path::Path) -> Result<CodexOutcome> {
        let mut command = Command::new(&self.config.binary);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_child_window(&mut command);

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn codex in {}", cwd.display()))?;

        let stdout = child.stdout.take().context("missing codex stdout")?;
        let stderr = child.stderr.take().context("missing codex stderr")?;

        let stdout_task = tokio::spawn(async move {
            let mut session_id = None;
            let mut last_message = String::new();
            let mut approval_pending = false;
            let mut lines = BufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if line.trim().is_empty() {
                    continue;
                }
                let parsed: Value = match serde_json::from_str(&line) {
                    Ok(value) => value,
                    Err(_) => {
                        warn!("ignored non-json codex line");
                        continue;
                    }
                };

                parse_exec_stdout_event(
                    &parsed,
                    &mut session_id,
                    &mut last_message,
                    &mut approval_pending,
                );
            }
            Ok::<_, anyhow::Error>((session_id, last_message, approval_pending))
        });

        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await? {
                warn!("codex stderr: {line}");
            }
            Ok::<_, anyhow::Error>(())
        });

        let status = child.wait().await.context("failed to wait for codex")?;
        let (session_id, last_message, approval_pending) = stdout_task.await??;
        stderr_task.await??;

        Ok(CodexOutcome {
            session_id,
            turn_id: None,
            last_message,
            exit_code: status.code(),
            approval_pending,
            approval_requests: Vec::new(),
            approval_request_count: 0,
            approval_resolved_count: 0,
        })
    }

    async fn ensure_app_server(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<AppServerClient>>> {
        let mut guard = self.app_server.lock().await;
        if guard.is_none() {
            *guard = Some(AppServerClient::spawn(&self.config).await?);
        }
        Ok(guard)
    }
}

fn parse_exec_stdout_event(
    parsed: &Value,
    session_id: &mut Option<String>,
    last_message: &mut String,
    approval_pending: &mut bool,
) {
    if session_id.is_none() {
        *session_id = parsed
            .get("session_id")
            .or_else(|| parsed.get("thread_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
    }

    if parsed.get("type").and_then(Value::as_str) == Some("item.completed") {
        if let Some(item) = parsed.get("item") {
            if item.get("type").and_then(Value::as_str) == Some("agent_message") {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    *last_message = text.to_owned();
                }
            }
        }
    }

    if let Some(msg) = parsed.get("msg") {
        if msg.get("type").and_then(Value::as_str) == Some("task_complete") {
            if let Some(text) = msg.get("last_agent_message").and_then(Value::as_str) {
                *last_message = text.to_owned();
            }
        }
        if let Some(kind) = msg.get("type").and_then(Value::as_str) {
            if matches!(
                kind,
                "exec_approval_request" | "apply_patch_approval_request"
            ) {
                *approval_pending = true;
            }
        }
    }
}

#[cfg(windows)]
fn hide_child_window(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_child_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::{CodexRequest, CodexRunner, parse_exec_stdout_event};
    use serde_json::json;
    use std::path::PathBuf;

    use crate::config::{CodexConfig, CodexTransport, LaneMode, WorkspaceConfig};

    #[test]
    fn request_without_images_keeps_prompt_only() {
        let args = CodexRequest::new("hello").into_args();

        assert_eq!(args, vec!["hello".to_owned()]);
    }

    #[test]
    fn request_with_images_appends_image_flags() {
        let args = CodexRequest::with_images(
            "hello",
            [
                PathBuf::from("C:/tmp/one.png"),
                PathBuf::from("C:/tmp/two.png"),
            ],
        )
        .into_args();

        assert_eq!(
            args,
            vec![
                "hello".to_owned(),
                "--image".to_owned(),
                "C:/tmp/one.png".to_owned(),
                "--image".to_owned(),
                "C:/tmp/two.png".to_owned(),
            ]
        );
    }

    #[test]
    fn base_args_omit_profile_when_not_configured() {
        let runner = CodexRunner::new(CodexConfig {
            binary: "codex".to_owned(),
            model: "gpt-5.4".to_owned(),
            sandbox: "read-only".to_owned(),
            approval: "never".to_owned(),
            transport: CodexTransport::Exec,
            profile: None,
        });

        let args = runner.base_args(&workspace());

        assert!(!args.iter().any(|arg| arg == "--profile"));
    }

    #[test]
    fn base_args_omit_model_when_not_configured() {
        let runner = CodexRunner::new(CodexConfig {
            binary: "codex".to_owned(),
            model: String::new(),
            sandbox: "read-only".to_owned(),
            approval: "never".to_owned(),
            transport: CodexTransport::Exec,
            profile: None,
        });

        let args = runner.base_args(&workspace());

        assert!(!args.iter().any(|arg| arg == "--model"));
    }

    #[test]
    fn base_args_include_model_when_configured() {
        let runner = CodexRunner::new(CodexConfig {
            binary: "codex".to_owned(),
            model: "gpt-5.4".to_owned(),
            sandbox: "read-only".to_owned(),
            approval: "never".to_owned(),
            transport: CodexTransport::Exec,
            profile: None,
        });

        let args = runner.base_args(&workspace());
        let model_index = args
            .iter()
            .position(|arg| arg == "--model")
            .expect("missing --model");

        assert_eq!(args.get(model_index + 1), Some(&"gpt-5.4".to_owned()));
    }

    #[test]
    fn base_args_include_profile_when_configured() {
        let runner = CodexRunner::new(CodexConfig {
            binary: "codex".to_owned(),
            model: "gpt-5.4".to_owned(),
            sandbox: "read-only".to_owned(),
            approval: "never".to_owned(),
            transport: CodexTransport::Exec,
            profile: Some("work".to_owned()),
        });

        let args = runner.base_args(&workspace());
        let profile_index = args
            .iter()
            .position(|arg| arg == "--profile")
            .expect("missing --profile");

        assert_eq!(args.get(profile_index + 1), Some(&"work".to_owned()));
    }

    #[test]
    fn parses_current_exec_json_agent_message() {
        let mut session_id = None;
        let mut last_message = String::new();
        let mut approval_pending = false;

        parse_exec_stdout_event(
            &json!({
                "type": "thread.started",
                "thread_id": "thread-1"
            }),
            &mut session_id,
            &mut last_message,
            &mut approval_pending,
        );
        parse_exec_stdout_event(
            &json!({
                "type": "item.completed",
                "item": {
                    "type": "agent_message",
                    "text": "こんにちは。何から始めますか？"
                }
            }),
            &mut session_id,
            &mut last_message,
            &mut approval_pending,
        );

        assert_eq!(session_id.as_deref(), Some("thread-1"));
        assert_eq!(last_message, "こんにちは。何から始めますか？");
        assert!(!approval_pending);
    }

    fn workspace() -> WorkspaceConfig {
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
