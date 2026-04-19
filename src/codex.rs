use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::warn;

use crate::config::{CodexConfig, WorkspaceConfig};

#[derive(Debug, Clone)]
pub struct CodexOutcome {
    pub session_id: Option<String>,
    pub last_message: String,
    pub exit_code: Option<i32>,
    pub approval_pending: bool,
}

#[derive(Debug, Clone)]
pub struct CodexRequest {
    pub prompt: String,
    pub image_paths: Vec<PathBuf>,
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
}

impl CodexRunner {
    pub fn new(config: CodexConfig) -> Self {
        Self { config }
    }

    pub async fn start(
        &self,
        workspace: &WorkspaceConfig,
        request: impl Into<CodexRequest>,
    ) -> Result<CodexOutcome> {
        let mut args = self.base_args(workspace);
        args.extend(request.into().into_args());
        self.run_command(args, &workspace.path).await
    }

    pub async fn resume(
        &self,
        workspace: &WorkspaceConfig,
        session_id: &str,
        request: impl Into<CodexRequest>,
    ) -> Result<CodexOutcome> {
        let mut args = vec![
            "exec".to_owned(),
            "resume".to_owned(),
            session_id.to_owned(),
        ];
        args.extend(request.into().into_args());
        args.push("--json".to_owned());
        self.run_command(args, &workspace.path).await
    }

    fn base_args(&self, workspace: &WorkspaceConfig) -> Vec<String> {
        vec![
            "exec".to_owned(),
            "--json".to_owned(),
            "--skip-git-repo-check".to_owned(),
            "--sandbox".to_owned(),
            self.config.sandbox.clone(),
            "--model".to_owned(),
            self.config.model.clone(),
            "--profile".to_owned(),
            self.config.profile.clone(),
            "--config".to_owned(),
            format!("approval_policy=\"{}\"", self.config.approval),
            "--cd".to_owned(),
            workspace.path.display().to_string(),
        ]
    }

    async fn run_command(&self, args: Vec<String>, cwd: &std::path::Path) -> Result<CodexOutcome> {
        let mut child = Command::new(&self.config.binary)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

                if session_id.is_none() {
                    session_id = parsed
                        .get("session_id")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                }

                if let Some(msg) = parsed.get("msg") {
                    if msg.get("type").and_then(Value::as_str) == Some("task_complete") {
                        if let Some(text) = msg.get("last_agent_message").and_then(Value::as_str) {
                            last_message = text.to_owned();
                        }
                    }
                    if let Some(kind) = msg.get("type").and_then(Value::as_str) {
                        if matches!(
                            kind,
                            "exec_approval_request" | "apply_patch_approval_request"
                        ) {
                            approval_pending = true;
                        }
                    }
                }
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
            last_message,
            exit_code: status.code(),
            approval_pending,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CodexRequest;
    use std::path::PathBuf;

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
}
