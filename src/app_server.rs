use std::collections::{HashMap, VecDeque};
use std::process::Stdio;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};
use tracing::warn;
use uuid::Uuid;

use crate::codex::{
    ActiveAppServerTurnPersistence, CodexFollowupRequest, CodexOutcome, CodexRequest,
};
use crate::config::{CodexConfig, WorkspaceConfig};
use crate::store::{ApprovalRequestKind, ApprovalRequestRecord};

const MIN_CODEX_APP_SERVER_VERSION: &str = "0.118.0";
const APP_SERVER_CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
const APP_SERVER_RESUME_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexApprovalRequest {
    pub request_id: String,
    pub transport_request_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub request_kind: ApprovalRequestKind,
    pub summary_text: String,
    pub raw_payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadSummary {
    pub thread_id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub updated_at: Option<String>,
}

pub struct AppServerClient {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    backlog: VecDeque<Value>,
    next_request_id: u64,
    initialized: bool,
}

impl AppServerClient {
    pub async fn spawn(config: &CodexConfig) -> Result<Self> {
        let mut command = Command::new(&config.binary);
        command
            .args(app_server_spawn_args(config))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = command
            .spawn()
            .context("failed to spawn codex app-server")?;
        let stdin = child.stdin.take().context("missing app-server stdin")?;
        let stdout = child.stdout.take().context("missing app-server stdout")?;
        let stderr = child.stderr.take().context("missing app-server stderr")?;
        tokio::spawn(log_app_server_stderr(stderr));

        Ok(Self {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: false,
        })
    }

    pub async fn start_turn(
        &mut self,
        config: &CodexConfig,
        workspace: &WorkspaceConfig,
        request: CodexRequest,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
        turn_persistence: Option<ActiveAppServerTurnPersistence>,
    ) -> Result<CodexOutcome> {
        self.ensure_initialized().await?;
        let mut params = json!({
            "cwd": workspace.path.display().to_string(),
            "approvalPolicy": config.approval,
            "approvalsReviewer": "user",
            "sandbox": config.sandbox,
            "experimentalRawEvents": false,
            "persistExtendedHistory": true,
            "serviceName": "remotty",
        });
        add_model_param(&mut params, config);
        let thread = self.call("thread/start", params).await?;
        let thread_id = thread_result_thread_id(&thread)?;
        self.start_turn_on_thread(
            config,
            workspace,
            &thread_id,
            true,
            request,
            followups,
            turn_persistence,
        )
        .await
    }

    pub async fn resume_turn(
        &mut self,
        config: &CodexConfig,
        workspace: &WorkspaceConfig,
        thread_id: &str,
        request: CodexRequest,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
        turn_persistence: Option<ActiveAppServerTurnPersistence>,
    ) -> Result<CodexOutcome> {
        self.ensure_initialized().await?;
        let params = json!({
                "threadId": thread_id,
                "cwd": workspace.path.display().to_string(),
                "approvalPolicy": config.approval,
                "approvalsReviewer": "user",
                "sandbox": config.sandbox,
                "persistExtendedHistory": true,
        });
        self.call_with_timeout("thread/resume", params, APP_SERVER_RESUME_TIMEOUT)
            .await?;
        self.start_turn_on_thread(
            config,
            workspace,
            thread_id,
            false,
            request,
            followups,
            turn_persistence,
        )
        .await
    }

    pub async fn resolve_approval(
        &mut self,
        request: &ApprovalRequestRecord,
        approved: bool,
    ) -> Result<CodexOutcome> {
        let response = approval_response(request, approved)?;
        self.resolve_server_request(request, response, None).await
    }

    pub async fn resolve_tool_user_input(
        &mut self,
        request: &ApprovalRequestRecord,
        answer_text: &str,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
    ) -> Result<CodexOutcome> {
        let response = tool_user_input_response(request, answer_text)?;
        self.resolve_server_request(request, response, followups)
            .await
    }

    async fn resolve_server_request(
        &mut self,
        request: &ApprovalRequestRecord,
        response: Value,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
    ) -> Result<CodexOutcome> {
        self.send_json_with_timeout(
            &json!({
            "id": request.transport_request_id,
            "response": response,
            "result": response,
            }),
            "approval response",
        )
        .await?;
        self.read_until_pause_or_completion(&request.thread_id, &request.turn_id, followups)
            .await
    }

    pub async fn list_threads(
        &mut self,
        limit: usize,
        filter: Option<&str>,
    ) -> Result<Vec<CodexThreadSummary>> {
        self.ensure_initialized().await?;
        let mut params = json!({ "limit": limit });
        if let Some(filter) = filter.map(str::trim).filter(|filter| !filter.is_empty()) {
            params["filter"] = Value::String(filter.to_owned());
        }
        let response = self.call("thread/list", params).await?;
        parse_thread_list_response(&response)
    }

    async fn start_turn_on_thread(
        &mut self,
        config: &CodexConfig,
        workspace: &WorkspaceConfig,
        thread_id: &str,
        include_model: bool,
        request: CodexRequest,
        followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
        turn_persistence: Option<ActiveAppServerTurnPersistence>,
    ) -> Result<CodexOutcome> {
        let mut params = json!({
            "threadId": thread_id,
            "input": request_to_user_inputs(request),
            "cwd": workspace.path.display().to_string(),
            "approvalPolicy": config.approval,
            "approvalsReviewer": "user",
            "sandboxPolicy": sandbox_policy_for_workspace(config, workspace),
        });
        if include_model {
            add_model_param(&mut params, config);
        }
        let turn = self.call("turn/start", params).await?;
        let turn_id = turn_result_turn_id(&turn)?;
        if let Some(persistence) = turn_persistence {
            if let Err(error) = persistence.persist(thread_id, &turn_id) {
                warn!("failed to persist active app-server turn: {error:#}");
            }
        }
        self.read_until_pause_or_completion(thread_id, &turn_id, followups)
            .await
    }

    async fn ensure_initialized(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        let response = self.call("initialize", initialize_params()).await?;
        assert_supported_app_server_version(&response)?;
        self.send_json_with_timeout(&json!({ "method": "initialized" }), "initialized")
            .await?;
        self.initialized = true;
        Ok(())
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.call_with_timeout(method, params, APP_SERVER_CONTROL_TIMEOUT)
            .await
    }

    async fn call_with_timeout(
        &mut self,
        method: &str,
        params: Value,
        timeout_duration: Duration,
    ) -> Result<Value> {
        let request_id = format!("client-{}", self.next_request_id);
        self.next_request_id += 1;
        self.send_json_with_timeout(
            &json!({
                "id": request_id,
                "method": method,
                "params": params,
            }),
            method,
        )
        .await?;

        timeout(
            timeout_duration,
            self.read_call_response(method, &request_id),
        )
        .await
        .map_err(|_| anyhow!("app-server `{method}` timed out"))?
    }

    async fn read_call_response(&mut self, method: &str, request_id: &str) -> Result<Value> {
        loop {
            let message = self.read_message().await?;
            if response_id(&message).as_deref() == Some(request_id) {
                if let Some(error) = message.get("error") {
                    bail!("app-server `{method}` failed: {error}");
                }
                return message
                    .get("result")
                    .or_else(|| message.get("response"))
                    .cloned()
                    .ok_or_else(|| anyhow!("app-server `{method}` missing result/response"));
            }
            self.backlog.push_back(message);
        }
    }

    async fn read_until_pause_or_completion(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        mut followups: Option<mpsc::UnboundedReceiver<CodexFollowupRequest>>,
    ) -> Result<CodexOutcome> {
        let mut delta_message = String::new();
        let mut approval_requests = Vec::new();
        let mut approval_resolved_count = 0_i64;

        loop {
            let message = if let Some(receiver) = followups.as_mut() {
                tokio::select! {
                    maybe_request = receiver.recv() => {
                        if let Some(followup) = maybe_request {
                            let result = self.steer_turn(thread_id, turn_id, followup.request).await;
                            let ok = result.is_ok();
                            let _ = followup.ack.send(result);
                            if !ok {
                                continue;
                            }
                        } else {
                            followups = None;
                        }
                        continue;
                    }
                    message = self.read_message() => message?,
                }
            } else {
                self.read_message().await?
            };
            let Some(method) = message.get("method").and_then(Value::as_str) else {
                self.backlog.push_back(message);
                continue;
            };

            let params = message.get("params").cloned().unwrap_or(Value::Null);
            if message.get("id").is_some() {
                if let Some(request) = parse_approval_request(&message)? {
                    approval_requests.push(request);
                    return Ok(CodexOutcome {
                        session_id: Some(thread_id.to_owned()),
                        turn_id: Some(turn_id.to_owned()),
                        last_message: delta_message,
                        exit_code: None,
                        approval_pending: true,
                        approval_requests,
                        approval_request_count: 1,
                        approval_resolved_count,
                    });
                }
                self.decline_unknown_server_request(&message).await?;
                continue;
            }

            match method {
                "item/agentMessage/delta" => {
                    if params.get("threadId").and_then(Value::as_str) == Some(thread_id)
                        && params.get("turnId").and_then(Value::as_str) == Some(turn_id)
                    {
                        if let Some(delta) = params.get("delta").and_then(Value::as_str) {
                            delta_message.push_str(delta);
                        }
                    }
                }
                "serverRequest/resolved" => {
                    if params.get("threadId").and_then(Value::as_str) == Some(thread_id) {
                        approval_resolved_count += 1;
                    }
                }
                "turn/completed" => {
                    let completed_turn = params
                        .get("turn")
                        .ok_or_else(|| anyhow!("turn/completed missing turn"))?;
                    if completed_turn.get("id").and_then(Value::as_str) != Some(turn_id) {
                        continue;
                    }
                    let status = completed_turn
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("failed");
                    let last_message = self
                        .read_last_agent_message(thread_id, turn_id)
                        .await
                        .unwrap_or(delta_message);
                    return Ok(CodexOutcome {
                        session_id: Some(thread_id.to_owned()),
                        turn_id: Some(turn_id.to_owned()),
                        last_message,
                        exit_code: Some(turn_status_exit_code(status)),
                        approval_pending: false,
                        approval_requests,
                        approval_request_count: 0,
                        approval_resolved_count,
                    });
                }
                _ => {}
            }
        }
    }

    async fn read_last_agent_message(&mut self, thread_id: &str, turn_id: &str) -> Result<String> {
        let response = self
            .call(
                "thread/read",
                json!({
                    "threadId": thread_id,
                    "includeTurns": true,
                }),
            )
            .await?;
        let turns = response
            .get("thread")
            .and_then(|thread| thread.get("turns"))
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("thread/read missing turns"))?;

        for turn in turns.iter().rev() {
            if turn.get("id").and_then(Value::as_str) != Some(turn_id) {
                continue;
            }
            if let Some(items) = turn.get("items").and_then(Value::as_array) {
                for item in items.iter().rev() {
                    if item.get("type").and_then(Value::as_str) == Some("agentMessage") {
                        return Ok(item
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned());
                    }
                }
            }
        }
        Ok(String::new())
    }

    pub async fn steer_turn(
        &mut self,
        thread_id: &str,
        turn_id: &str,
        request: CodexRequest,
    ) -> Result<()> {
        self.ensure_initialized().await?;
        self.ensure_turn_accepts_steer(thread_id, turn_id).await?;
        let response = self
            .call("turn/steer", turn_steer_params(thread_id, turn_id, request))
            .await?;
        assert_turn_steer_accepted(&response)?;
        Ok(())
    }

    async fn ensure_turn_accepts_steer(&mut self, thread_id: &str, turn_id: &str) -> Result<()> {
        let response = self
            .call(
                "thread/read",
                json!({
                    "threadId": thread_id,
                    "includeTurns": true,
                }),
            )
            .await?;
        let turns = response
            .get("thread")
            .and_then(|thread| thread.get("turns"))
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("thread/read missing turns"))?;
        let turn = turns
            .iter()
            .find(|turn| turn.get("id").and_then(Value::as_str) == Some(turn_id))
            .ok_or_else(|| anyhow!("turn `{turn_id}` was not found in thread `{thread_id}`"))?;
        let status = turn
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("turn `{turn_id}` has no status"))?;
        match status {
            "in_progress" | "in-progress" | "running" | "pending" => Ok(()),
            "completed" | "failed" | "cancelled" | "canceled" => {
                bail!("turn `{turn_id}` is already {status}")
            }
            other => bail!("turn `{turn_id}` has unsupported status `{other}`"),
        }
    }

    async fn decline_unknown_server_request(&mut self, message: &Value) -> Result<()> {
        let Some(response) = unknown_server_request_response(message) else {
            return Ok(());
        };
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        warn!("declining unsupported app-server request `{method}`");
        self.send_json_with_timeout(&response, "unsupported server request")
            .await
    }

    async fn read_message(&mut self) -> Result<Value> {
        if let Some(message) = self.backlog.pop_front() {
            return Ok(message);
        }

        loop {
            let mut line = String::new();
            let bytes_read = self
                .stdout
                .read_line(&mut line)
                .await
                .context("failed to read from app-server stdout")?;
            if bytes_read == 0 {
                bail!("app-server closed stdout");
            }
            if line.trim().is_empty() {
                continue;
            }
            return serde_json::from_str(line.trim())
                .context("failed to decode app-server JSON message");
        }
    }

    async fn send_json(&mut self, value: &Value) -> Result<()> {
        let payload =
            serde_json::to_string(value).context("failed to serialize app-server JSON")?;
        self.stdin
            .write_all(payload.as_bytes())
            .await
            .context("failed to write to app-server stdin")?;
        self.stdin
            .write_all(b"\n")
            .await
            .context("failed to terminate app-server JSON line")?;
        self.stdin
            .flush()
            .await
            .context("failed to flush app-server stdin")?;
        Ok(())
    }

    async fn send_json_with_timeout(&mut self, value: &Value, operation: &str) -> Result<()> {
        timeout(APP_SERVER_CONTROL_TIMEOUT, self.send_json(value))
            .await
            .map_err(|_| anyhow!("app-server `{operation}` write timed out"))?
    }
}

async fn log_app_server_stderr(stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => warn!("app-server stderr: {line}"),
            Ok(None) => break,
            Err(error) => {
                warn!("failed to read app-server stderr: {error:#}");
                break;
            }
        }
    }
}

fn sandbox_policy_for_workspace(config: &CodexConfig, workspace: &WorkspaceConfig) -> Value {
    let readable_roots = workspace_readable_roots(workspace);
    match config.sandbox.as_str() {
        "danger-full-access" => json!({ "type": "dangerFullAccess" }),
        "read-only" => json!({
            "type": "readOnly",
            "access": {
                "type": "restricted",
                "readableRoots": readable_roots,
            },
            "networkAccess": true,
        }),
        "workspace-write" => json!({
            "type": "workspaceWrite",
            "readOnlyAccess": {
                "type": "restricted",
                "readableRoots": readable_roots,
            },
            "writableRoots": workspace
                .writable_roots
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
            "networkAccess": true,
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
        }),
        other => json!({
            "type": "workspaceWrite",
            "readOnlyAccess": {
                "type": "restricted",
                "readableRoots": readable_roots,
            },
            "writableRoots": workspace
                .writable_roots
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>(),
            "networkAccess": true,
            "excludeTmpdirEnvVar": false,
            "excludeSlashTmp": false,
            "fallbackSandbox": other,
        }),
    }
}

fn initialize_params() -> Value {
    json!({
        "clientInfo": {
            "name": "remotty",
            "title": "remotty",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "experimentalApi": true,
            "optOutNotificationMethods": [],
        }
    })
}

fn assert_supported_app_server_version(response: &Value) -> Result<()> {
    let detected_version = response
        .get("userAgent")
        .and_then(Value::as_str)
        .and_then(read_codex_version_from_user_agent);
    let Some(detected_version) = detected_version else {
        bail!(
            "Codex app-server {MIN_CODEX_APP_SERVER_VERSION} or newer is required, but remotty could not determine the running Codex version"
        );
    };
    if compare_versions(detected_version, MIN_CODEX_APP_SERVER_VERSION).is_lt() {
        bail!(
            "Codex app-server {MIN_CODEX_APP_SERVER_VERSION} or newer is required, but detected {detected_version}"
        );
    }
    Ok(())
}

fn read_codex_version_from_user_agent(user_agent: &str) -> Option<&str> {
    let (_, version) = user_agent
        .split_whitespace()
        .find_map(|product| product.split_once('/'))?;
    let version = version
        .split([' ', '('])
        .next()
        .unwrap_or(version)
        .split(['+', '-'])
        .next()
        .unwrap_or(version);
    if version.split('.').take(3).count() == 3
        && version
            .split('.')
            .take(3)
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        Some(version)
    } else {
        None
    }
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_parts = numeric_version_parts(left);
    let right_parts = numeric_version_parts(right);
    left_parts.cmp(&right_parts)
}

fn numeric_version_parts(version: &str) -> [u64; 3] {
    let mut parts = [0_u64; 3];
    for (index, part) in version.split('.').take(3).enumerate() {
        parts[index] = part.parse::<u64>().unwrap_or(0);
    }
    parts
}

fn workspace_readable_roots(workspace: &WorkspaceConfig) -> Vec<String> {
    let mut roots = vec![workspace.path.display().to_string()];
    for path in &workspace.writable_roots {
        let rendered = path.display().to_string();
        if !roots.contains(&rendered) {
            roots.push(rendered);
        }
    }
    roots
}

fn app_server_spawn_args(config: &CodexConfig) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(profile) = config
        .profile
        .as_deref()
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
    {
        args.push("--profile".to_owned());
        args.push(profile.to_owned());
    }
    args.push("app-server".to_owned());
    args
}

fn configured_model(config: &CodexConfig) -> Option<&str> {
    let model = config.model.trim();
    if model.is_empty() { None } else { Some(model) }
}

fn add_model_param(params: &mut Value, config: &CodexConfig) {
    if let Some(model) = configured_model(config) {
        params["model"] = Value::String(model.to_owned());
    }
}

fn request_to_user_inputs(request: CodexRequest) -> Vec<Value> {
    let mut input = vec![json!({
        "type": "text",
        "text": request.prompt,
        "text_elements": [],
    })];
    for image_path in request.image_paths {
        input.push(json!({
            "type": "localImage",
            "path": image_path.display().to_string(),
        }));
    }
    input
}

fn turn_steer_params(thread_id: &str, turn_id: &str, request: CodexRequest) -> Value {
    json!({
        "threadId": thread_id,
        "expectedTurnId": turn_id,
        "input": request_to_user_inputs(request),
    })
}

fn assert_turn_steer_accepted(response: &Value) -> Result<()> {
    for field in ["accepted", "queued", "ok", "success"] {
        if response.get(field).and_then(Value::as_bool) == Some(false) {
            bail!("app-server `turn/steer` rejected follow-up");
        }
    }
    if let Some(status) = response.get("status").and_then(Value::as_str) {
        if matches!(
            status,
            "rejected" | "failed" | "error" | "cancelled" | "canceled"
        ) {
            bail!("app-server `turn/steer` returned status `{status}`");
        }
    }
    Ok(())
}

fn turn_result_turn_id(result: &Value) -> Result<String> {
    result
        .get("turn")
        .and_then(|turn| turn.get("id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("turn/start result missing turn.id"))
}

fn thread_result_thread_id(result: &Value) -> Result<String> {
    result
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("thread result missing thread.id"))
}

fn parse_thread_list_response(response: &Value) -> Result<Vec<CodexThreadSummary>> {
    let threads =
        thread_list_items(response).ok_or_else(|| anyhow!("thread/list result missing threads"))?;
    threads
        .iter()
        .map(parse_thread_summary)
        .collect::<Result<Vec<_>>>()
}

fn thread_list_items(response: &Value) -> Option<&Vec<Value>> {
    if let Some(items) = response.as_array() {
        return Some(items);
    }
    for key in ["threads", "items", "data"] {
        if let Some(items) = response.get(key).and_then(Value::as_array) {
            return Some(items);
        }
    }
    None
}

fn parse_thread_summary(thread: &Value) -> Result<CodexThreadSummary> {
    let thread_id = thread_string(thread, &["threadId", "id"])
        .ok_or_else(|| anyhow!("thread/list entry missing thread id"))?;
    Ok(CodexThreadSummary {
        thread_id,
        title: thread_string(thread, &["title", "name", "summary"]),
        cwd: thread_string(thread, &["cwd"]),
        model: thread_string(thread, &["model"]),
        updated_at: thread_string(thread, &["updatedAt", "lastUpdatedAt"]),
    })
}

fn thread_string(thread: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        thread
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn response_id(message: &Value) -> Option<String> {
    let id = message.get("id")?;
    match id {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn unknown_server_request_response(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let response = json!({
        "decision": "decline",
        "reason": "Unsupported app-server request type",
    });
    Some(json!({
        "id": id,
        "response": response,
        "result": response,
    }))
}

fn parse_approval_request(message: &Value) -> Result<Option<CodexApprovalRequest>> {
    let transport_request_id = match response_id(message) {
        Some(request_id) => request_id,
        None => return Ok(None),
    };
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("server request missing method"))?;
    let params = message
        .get("params")
        .cloned()
        .ok_or_else(|| anyhow!("server request missing params"))?;

    let (request_kind, summary_text) = match method {
        "item/commandExecution/requestApproval" | "execCommandApproval" => (
            ApprovalRequestKind::CommandExecution,
            summarize_command_approval(&params),
        ),
        "item/fileChange/requestApproval" | "applyPatchApproval" => (
            ApprovalRequestKind::FileChange,
            summarize_file_change_approval(&params),
        ),
        "item/permissions/requestApproval" => (
            ApprovalRequestKind::Permissions,
            summarize_permissions_approval(&params),
        ),
        "item/tool/requestUserInput" => (
            ApprovalRequestKind::ToolUserInput,
            summarize_tool_user_input(&params),
        ),
        _ => return Ok(None),
    };

    let thread_id = required_param_str(&params, "threadId")?.to_owned();
    let turn_id = required_param_str(&params, "turnId")?.to_owned();
    let item_id = required_param_str(&params, "itemId")?.to_owned();
    Ok(Some(CodexApprovalRequest {
        request_id: stable_approval_request_id(&thread_id, &turn_id, &item_id, request_kind),
        transport_request_id,
        thread_id,
        turn_id,
        item_id,
        request_kind,
        summary_text,
        raw_payload_json: serde_json::to_string(&params)
            .context("failed to serialize approval request params")?,
    }))
}

fn approval_response(request: &ApprovalRequestRecord, approved: bool) -> Result<Value> {
    let raw_payload: Value =
        serde_json::from_str(&request.raw_payload_json).context("invalid approval payload JSON")?;
    let response = match request.request_kind {
        ApprovalRequestKind::CommandExecution => json!({
            "decision": if approved { "accept" } else { "decline" }
        }),
        ApprovalRequestKind::FileChange => json!({
            "decision": if approved { "accept" } else { "decline" }
        }),
        ApprovalRequestKind::Permissions => {
            if approved {
                let granted = raw_payload
                    .get("permissions")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                json!({
                    "permissions": granted,
                    "scope": "turn",
                })
            } else {
                json!({
                    "permissions": {},
                    "scope": "turn",
                })
            }
        }
        ApprovalRequestKind::ToolUserInput => {
            if approved {
                bail!("tool user input approvals are not implemented yet");
            }
            json!({ "answers": {} })
        }
    };
    Ok(response)
}

fn tool_user_input_response(request: &ApprovalRequestRecord, answer_text: &str) -> Result<Value> {
    if request.request_kind != ApprovalRequestKind::ToolUserInput {
        bail!("approval request is not a tool user input request");
    }
    let raw_payload: Value = serde_json::from_str(&request.raw_payload_json)
        .context("invalid tool user input payload JSON")?;
    let questions = raw_payload
        .get("questions")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("tool user input payload missing questions"))?;
    if questions.is_empty() {
        bail!("tool user input payload has no questions");
    }

    let expected_ids = questions
        .iter()
        .filter_map(|question| question.get("id").and_then(Value::as_str))
        .filter(|id| !id.trim().is_empty())
        .collect::<Vec<_>>();
    let keyed_answers = parse_keyed_tool_user_input_answers(answer_text);
    let mut answers = serde_json::Map::new();
    for question in questions {
        let id = question
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("tool user input question missing id"))?;
        if question
            .get("isSecret")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            bail!("secret tool user input `{id}` cannot be answered from Telegram");
        }

        let answer = if questions.len() == 1 {
            answer_text.trim().to_owned()
        } else {
            keyed_answers.get(id).cloned().ok_or_else(|| {
                anyhow!(
                    "answer for question `{id}` is missing; use `id=value` on separate lines for: {}",
                    expected_ids.join(", ")
                )
            })?
        };
        if answer.trim().is_empty() {
            bail!("answer for question `{id}` is empty");
        }
        answers.insert(id.to_owned(), json!({ "answers": [answer] }));
    }

    Ok(json!({ "answers": Value::Object(answers) }))
}

pub(crate) fn validate_tool_user_input_answer(
    request: &ApprovalRequestRecord,
    answer_text: &str,
) -> Result<()> {
    tool_user_input_response(request, answer_text).map(|_| ())
}

fn parse_keyed_tool_user_input_answers(answer_text: &str) -> HashMap<String, String> {
    let mut answers = HashMap::new();
    for line in answer_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() && !value.is_empty() {
            answers.insert(key.to_owned(), value.to_owned());
        }
    }
    answers
}

fn summarize_command_approval(params: &Value) -> String {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("unknown command");
    let cwd = params.get("cwd").and_then(Value::as_str);
    match cwd {
        Some(cwd) if !cwd.is_empty() => {
            format!("Command approval requested: `{command}`\nWorking directory: `{cwd}`")
        }
        _ => format!("Command approval requested: `{command}`"),
    }
}

fn summarize_file_change_approval(params: &Value) -> String {
    if let Some(grant_root) = params.get("grantRoot").and_then(Value::as_str) {
        return format!("File change approval requested: write access to `{grant_root}`");
    }
    if let Some(reason) = params.get("reason").and_then(Value::as_str) {
        if !reason.trim().is_empty() {
            return format!("File change approval requested: {reason}");
        }
    }
    "File change approval requested".to_owned()
}

fn summarize_permissions_approval(params: &Value) -> String {
    let reason = params
        .get("reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or("Additional permissions are required.");
    format!("Permission approval requested: {reason}")
}

fn summarize_tool_user_input(params: &Value) -> String {
    let questions = params
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if questions.is_empty() {
        return "追加の入力待ち".to_owned();
    }

    let mut lines = vec![format!("追加の入力待ち: 質問数 {}", questions.len())];
    for (index, question) in questions.iter().take(3).enumerate() {
        let is_secret = question
            .get("isSecret")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let id = question
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty());
        let id_text = id
            .map(|value| format!("id `{value}`"))
            .unwrap_or_else(|| "id なし".to_owned());
        if is_secret {
            lines.push(format!(
                "{}. {}: 秘密入力（ローカル画面で入力）",
                index + 1,
                id_text
            ));
            continue;
        }

        let prompt = question
            .get("question")
            .and_then(Value::as_str)
            .or_else(|| question.get("header").and_then(Value::as_str))
            .map(|value| summarize_inline_text(value, 80));
        let option_count = question
            .get("options")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let option_labels = summarize_tool_user_input_options(question);
        let prompt_text = prompt.unwrap_or_else(|| "追加入力".to_owned());
        if option_count == 0 {
            lines.push(format!("{}. {}: {}", index + 1, id_text, prompt_text));
            continue;
        }

        lines.push(format!(
            "{}. {}: {}（選択肢 {} 件{}）",
            index + 1,
            id_text,
            prompt_text,
            option_count,
            option_labels
                .map(|labels| format!(": {labels}"))
                .unwrap_or_default()
        ));
    }
    summarize_multiline_text(&lines.join("\n"), 320)
}

fn summarize_tool_user_input_options(question: &Value) -> Option<String> {
    let labels = question
        .get("options")?
        .as_array()?
        .iter()
        .filter_map(|option| option.get("label").and_then(Value::as_str))
        .filter(|label| !label.trim().is_empty())
        .take(3)
        .map(|label| summarize_inline_text(label, 30))
        .collect::<Vec<_>>();
    (!labels.is_empty()).then(|| labels.join(", "))
}

fn summarize_inline_text(text: &str, max_chars: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    summarize_multiline_text(&text, max_chars)
}

fn summarize_multiline_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars == 0 {
        return String::new();
    }

    let prefix = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    format!("{prefix}…")
}

fn required_param_str<'a>(params: &'a Value, key: &str) -> Result<&'a str> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("approval params missing `{key}`"))
}

fn stable_approval_request_id(
    thread_id: &str,
    turn_id: &str,
    item_id: &str,
    request_kind: ApprovalRequestKind,
) -> String {
    let seed = format!(
        "{thread_id}\x1f{turn_id}\x1f{item_id}\x1f{}",
        approval_request_kind_key(request_kind)
    );
    format!(
        "approval-{}",
        Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes())
    )
}

fn approval_request_kind_key(request_kind: ApprovalRequestKind) -> &'static str {
    match request_kind {
        ApprovalRequestKind::CommandExecution => "command_execution",
        ApprovalRequestKind::FileChange => "file_change",
        ApprovalRequestKind::Permissions => "permissions",
        ApprovalRequestKind::ToolUserInput => "tool_user_input",
    }
}

fn turn_status_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "interrupted" => 130,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CodexConfig, CodexTransport, LaneMode, WorkspaceConfig};
    use crate::store::ApprovalRequestKind;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn parses_command_approval_request() {
        let request = parse_approval_request(&json!({
            "id": "req-1",
            "method": "item/commandExecution/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "command": "npm test",
                "cwd": "C:/workspace"
            }
        }))
        .expect("approval should parse")
        .expect("approval should exist");

        assert_eq!(request.request_kind, ApprovalRequestKind::CommandExecution);
        assert_eq!(
            request.request_id,
            stable_approval_request_id(
                "thread-1",
                "turn-1",
                "item-1",
                ApprovalRequestKind::CommandExecution,
            )
        );
        assert_eq!(request.transport_request_id, "req-1");
        let callback_target = format!(
            "{}:1",
            request
                .request_id
                .strip_prefix("approval-")
                .unwrap_or(&request.request_id)
        );
        assert!(format!("approve:{callback_target}").len() <= 64);
        assert!(format!("deny:{callback_target}").len() <= 64);
        assert!(request.summary_text.contains("npm test"));
        assert!(request.summary_text.contains("C:/workspace"));
    }

    #[test]
    fn parses_permissions_approval_request() {
        let request = parse_approval_request(&json!({
            "id": "req-2",
            "method": "item/permissions/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "reason": "network access required",
                "permissions": {
                    "network": { "kind": "full" },
                    "fileSystem": null
                }
            }
        }))
        .expect("approval should parse")
        .expect("approval should exist");

        assert_eq!(request.request_kind, ApprovalRequestKind::Permissions);
        assert_eq!(
            request.request_id,
            stable_approval_request_id(
                "thread-1",
                "turn-1",
                "item-1",
                ApprovalRequestKind::Permissions,
            )
        );
        assert_eq!(request.transport_request_id, "req-2");
        assert!(request.summary_text.contains("network access required"));
    }

    #[test]
    fn parses_tool_user_input_request_with_visible_summary() {
        let request = parse_approval_request(&json!({
            "id": "req-3",
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [
                    {
                        "id": "confirm_path",
                        "question": "Approve app tool call?",
                        "isSecret": false,
                        "options": [
                            { "label": "Allow once" },
                            { "label": "Allow for session" },
                            { "label": "Decline" }
                        ]
                    },
                    {
                        "id": "remember_choice",
                        "question": "Remember this choice?",
                        "isSecret": false,
                        "options": [
                            { "label": "Yes" },
                            { "label": "No" }
                        ]
                    }
                ]
            }
        }))
        .expect("approval should parse")
        .expect("approval should exist");

        assert_eq!(request.request_kind, ApprovalRequestKind::ToolUserInput);
        assert!(request.summary_text.contains("追加の入力待ち: 質問数 2"));
        assert!(request.summary_text.contains("id `confirm_path`"));
        assert!(request.summary_text.contains("Approve app tool call?"));
        assert!(request.summary_text.contains("Allow once"));
        assert!(request.summary_text.contains("id `remember_choice`"));
        assert!(request.summary_text.contains("Remember this choice?"));
        assert!(request.summary_text.contains("Yes"));
    }

    #[test]
    fn parses_tool_user_input_summary_keeps_full_question_id() {
        let long_id = "field_abcdefghijklmnopqrstuvwxyz_0123456789_extra";
        let request = parse_approval_request(&json!({
            "id": "req-long-id",
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [{
                    "id": long_id,
                    "question": "Target?",
                    "isSecret": false
                }]
            }
        }))
        .expect("approval should parse")
        .expect("approval should exist");

        assert!(request.summary_text.contains(&format!("id `{long_id}`")));
        assert!(!request.summary_text.contains('…'));
    }

    #[test]
    fn parses_secret_tool_user_input_without_leaking_prompt() {
        let request = parse_approval_request(&json!({
            "id": "req-4",
            "method": "item/tool/requestUserInput",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1",
                "questions": [{
                    "id": "token",
                    "question": "Paste the API token",
                    "isSecret": true
                }]
            }
        }))
        .expect("approval should parse")
        .expect("approval should exist");

        assert_eq!(request.request_kind, ApprovalRequestKind::ToolUserInput);
        assert!(request.summary_text.contains("id `token`"));
        assert!(request.summary_text.contains("秘密入力"));
        assert!(!request.summary_text.contains("Paste the API token"));
    }

    #[test]
    fn summarize_multiline_text_keeps_exact_limit_without_ellipsis() {
        let exact = "x".repeat(320);

        assert_eq!(summarize_multiline_text(&exact, 320), exact);
    }

    #[test]
    fn summarize_multiline_text_truncates_to_total_limit() {
        let over_limit = "x".repeat(321);
        let summary = summarize_multiline_text(&over_limit, 320);

        assert_eq!(summary.chars().count(), 320);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn builds_decline_response_for_command_execution() {
        let request = ApprovalRequestRecord {
            request_id: "req-1".to_owned(),
            transport_request_id: "transport-req-1".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::CommandExecution,
            summary_text: "command".to_owned(),
            raw_payload_json: "{}".to_owned(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let response = approval_response(&request, false).expect("response should build");
        assert_eq!(response, json!({ "decision": "decline" }));
    }

    #[test]
    fn builds_decline_response_for_unknown_server_request() {
        let response = unknown_server_request_response(&json!({
            "id": "transport-req-unknown",
            "method": "item/unknown/requestApproval",
            "params": {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "item-1"
            }
        }))
        .expect("response should build");

        assert_eq!(
            response["id"],
            Value::String("transport-req-unknown".to_owned())
        );
        assert_eq!(
            response["response"]["decision"],
            Value::String("decline".to_owned())
        );
        assert_eq!(
            response["result"]["decision"],
            Value::String("decline".to_owned())
        );
    }

    #[test]
    fn builds_accept_response_for_permissions_request() {
        let request = ApprovalRequestRecord {
            request_id: "req-2".to_owned(),
            transport_request_id: "transport-req-2".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::Permissions,
            summary_text: "permissions".to_owned(),
            raw_payload_json: json!({
                "permissions": {
                    "network": { "kind": "full" },
                    "fileSystem": null
                }
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let response = approval_response(&request, true).expect("response should build");
        assert_eq!(
            response,
            json!({
                "permissions": {
                    "network": { "kind": "full" },
                    "fileSystem": null
                },
                "scope": "turn",
            })
        );
    }

    #[test]
    fn builds_tool_user_input_response_for_single_question() {
        let request = ApprovalRequestRecord {
            request_id: "req-3".to_owned(),
            transport_request_id: "transport-req-3".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::ToolUserInput,
            summary_text: "input".to_owned(),
            raw_payload_json: json!({
                "questions": [{
                    "id": "confirm_path",
                    "header": "Path",
                    "question": "Use this path?",
                    "isSecret": false
                }]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let response = tool_user_input_response(&request, "yes").expect("response should build");
        assert_eq!(
            response,
            json!({
                "answers": {
                    "confirm_path": { "answers": ["yes"] }
                }
            })
        );
    }

    #[test]
    fn single_tool_user_input_preserves_key_value_text() {
        let request = ApprovalRequestRecord {
            request_id: "req-3".to_owned(),
            transport_request_id: "transport-req-3".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::ToolUserInput,
            summary_text: "input".to_owned(),
            raw_payload_json: json!({
                "questions": [{
                    "id": "token_text",
                    "question": "Token text?",
                    "isSecret": false
                }]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let response = tool_user_input_response(&request, "KEY=https://example.com:8443/path")
            .expect("response should build");
        assert_eq!(
            response,
            json!({
                "answers": {
                    "token_text": { "answers": ["KEY=https://example.com:8443/path"] }
                }
            })
        );
    }

    #[test]
    fn builds_tool_user_input_response_for_multiple_questions() {
        let request = ApprovalRequestRecord {
            request_id: "req-4".to_owned(),
            transport_request_id: "transport-req-4".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::ToolUserInput,
            summary_text: "input".to_owned(),
            raw_payload_json: json!({
                "questions": [
                    { "id": "target", "question": "Target?", "isSecret": false },
                    { "id": "mode", "question": "Mode?", "isSecret": false }
                ]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let response = tool_user_input_response(&request, "target=docs\nmode=review")
            .expect("response should build");
        assert_eq!(
            response,
            json!({
                "answers": {
                    "target": { "answers": ["docs"] },
                    "mode": { "answers": ["review"] }
                }
            })
        );
    }

    #[test]
    fn rejects_multiple_tool_user_input_missing_question_id_with_expected_ids() {
        let request = ApprovalRequestRecord {
            request_id: "req-4".to_owned(),
            transport_request_id: "transport-req-4".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::ToolUserInput,
            summary_text: "input".to_owned(),
            raw_payload_json: json!({
                "questions": [
                    { "id": "target", "question": "Target?", "isSecret": false },
                    { "id": "mode", "question": "Mode?", "isSecret": false }
                ]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let error = tool_user_input_response(&request, "target=docs")
            .expect_err("missing answer should be rejected");
        let message = error.to_string();
        assert!(message.contains("mode"));
        assert!(message.contains("target, mode"));
    }

    #[test]
    fn rejects_tool_user_input_response_for_wrong_request_kind() {
        let request = ApprovalRequestRecord {
            request_id: "req-6".to_owned(),
            transport_request_id: "transport-req-6".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::CommandExecution,
            summary_text: "command".to_owned(),
            raw_payload_json: json!({
                "questions": [{ "id": "target", "question": "Target?" }]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let error = tool_user_input_response(&request, "target=docs")
            .expect_err("wrong request kind should be rejected");
        assert!(error.to_string().contains("not a tool user input request"));
    }

    #[test]
    fn rejects_secret_tool_user_input_from_telegram() {
        let request = ApprovalRequestRecord {
            request_id: "req-5".to_owned(),
            transport_request_id: "transport-req-5".to_owned(),
            lane_id: "lane-1".to_owned(),
            run_id: "run-1".to_owned(),
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item_id: "item-1".to_owned(),
            transport: crate::store::ApprovalRequestTransport::AppServer,
            request_kind: ApprovalRequestKind::ToolUserInput,
            summary_text: "input".to_owned(),
            raw_payload_json: json!({
                "questions": [{
                    "id": "token",
                    "question": "Token?",
                    "isSecret": true
                }]
            })
            .to_string(),
            status: crate::store::ApprovalRequestStatus::Pending,
            requested_at_ms: 0,
            resolved_at_ms: None,
            resolved_by_sender_id: None,
            telegram_message_id: None,
        };

        let error = tool_user_input_response(&request, "secret")
            .expect_err("secret input should be rejected");
        assert!(
            error
                .to_string()
                .contains("cannot be answered from Telegram")
        );
    }

    #[test]
    fn stable_request_id_differs_by_approval_kind() {
        let command = stable_approval_request_id(
            "thread-1",
            "turn-1",
            "item-1",
            ApprovalRequestKind::CommandExecution,
        );
        let permissions = stable_approval_request_id(
            "thread-1",
            "turn-1",
            "item-1",
            ApprovalRequestKind::Permissions,
        );

        assert_ne!(command, permissions);
    }

    #[test]
    fn app_server_spawn_args_include_profile_when_present() {
        let args = app_server_spawn_args(&CodexConfig {
            binary: "codex".to_owned(),
            model: "gpt-5.4".to_owned(),
            sandbox: "workspace-write".to_owned(),
            approval: "on-request".to_owned(),
            transport: CodexTransport::AppServer,
            profile: Some("work".to_owned()),
        });

        assert_eq!(
            args,
            vec![
                "--profile".to_owned(),
                "work".to_owned(),
                "app-server".to_owned(),
            ]
        );
    }

    #[test]
    fn add_model_param_omits_blank_model() {
        let mut params = json!({});
        add_model_param(
            &mut params,
            &CodexConfig {
                binary: "codex".to_owned(),
                model: String::new(),
                sandbox: "workspace-write".to_owned(),
                approval: "on-request".to_owned(),
                transport: CodexTransport::AppServer,
                profile: None,
            },
        );

        assert!(params.get("model").is_none());
    }

    #[test]
    fn add_model_param_includes_configured_model() {
        let mut params = json!({});
        add_model_param(
            &mut params,
            &CodexConfig {
                binary: "codex".to_owned(),
                model: "gpt-5.4".to_owned(),
                sandbox: "workspace-write".to_owned(),
                approval: "on-request".to_owned(),
                transport: CodexTransport::AppServer,
                profile: None,
            },
        );

        assert_eq!(params["model"], Value::String("gpt-5.4".to_owned()));
    }

    #[test]
    fn workspace_write_sandbox_policy_enables_network_access() {
        let policy = sandbox_policy_for_workspace(
            &CodexConfig {
                binary: "codex".to_owned(),
                model: "gpt-5.4".to_owned(),
                sandbox: "workspace-write".to_owned(),
                approval: "on-request".to_owned(),
                transport: CodexTransport::AppServer,
                profile: None,
            },
            &WorkspaceConfig {
                id: "main".to_owned(),
                path: PathBuf::from("C:/workspace"),
                writable_roots: vec![PathBuf::from("C:/workspace")],
                default_mode: LaneMode::AwaitReply,
                continue_prompt: "continue".to_owned(),
                checks_profile: "default".to_owned(),
            },
        );

        assert_eq!(policy["type"], Value::String("workspaceWrite".to_owned()));
        assert_eq!(policy["networkAccess"], Value::Bool(true));
        assert_eq!(
            policy["readOnlyAccess"]["type"],
            Value::String("restricted".to_owned())
        );
        assert_eq!(
            policy["readOnlyAccess"]["readableRoots"],
            json!(["C:/workspace"])
        );
    }

    #[test]
    fn read_only_sandbox_policy_enables_network_access() {
        let policy = sandbox_policy_for_workspace(
            &CodexConfig {
                binary: "codex".to_owned(),
                model: "gpt-5.4".to_owned(),
                sandbox: "read-only".to_owned(),
                approval: "on-request".to_owned(),
                transport: CodexTransport::AppServer,
                profile: None,
            },
            &WorkspaceConfig {
                id: "main".to_owned(),
                path: PathBuf::from("C:/workspace"),
                writable_roots: vec![PathBuf::from("C:/workspace")],
                default_mode: LaneMode::AwaitReply,
                continue_prompt: "continue".to_owned(),
                checks_profile: "default".to_owned(),
            },
        );

        assert_eq!(policy["type"], Value::String("readOnly".to_owned()));
        assert_eq!(policy["networkAccess"], Value::Bool(true));
        assert_eq!(
            policy["access"]["type"],
            Value::String("restricted".to_owned())
        );
        assert_eq!(policy["access"]["readableRoots"], json!(["C:/workspace"]));
    }

    #[test]
    fn initialize_params_enable_experimental_api() {
        let params = initialize_params();
        assert_eq!(params["capabilities"]["experimentalApi"], Value::Bool(true));
    }

    #[test]
    fn read_codex_version_from_user_agent_accepts_codex_product_prefix() {
        assert_eq!(
            read_codex_version_from_user_agent("Codex/0.118.1 (windows)"),
            Some("0.118.1")
        );
    }

    #[test]
    fn read_codex_version_from_user_agent_accepts_desktop_origin_prefix() {
        assert_eq!(
            read_codex_version_from_user_agent("Codex Desktop/0.118.1 (windows)"),
            Some("0.118.1")
        );
    }

    #[test]
    fn assert_supported_app_server_version_rejects_old_versions() {
        let error = assert_supported_app_server_version(&json!({
            "userAgent": "Codex/0.117.9"
        }))
        .expect_err("old version should fail");

        assert!(error.to_string().contains("0.118.0 or newer"));
        assert!(error.to_string().contains("0.117.9"));
    }

    #[test]
    fn assert_supported_app_server_version_accepts_minimum_version() {
        assert_supported_app_server_version(&json!({
            "userAgent": "Codex/0.118.0"
        }))
        .expect("minimum version should pass");
    }

    #[test]
    fn parse_thread_list_response_accepts_threads_key() {
        let threads = parse_thread_list_response(&json!({
            "threads": [
                {
                    "threadId": "thread-1",
                    "title": "Fix tests",
                    "cwd": "C:/workspace",
                    "model": "gpt-5.4",
                    "updatedAt": "2026-04-22T00:00:00Z"
                }
            ]
        }))
        .expect("threads should parse");

        assert_eq!(
            threads,
            vec![CodexThreadSummary {
                thread_id: "thread-1".to_owned(),
                title: Some("Fix tests".to_owned()),
                cwd: Some("C:/workspace".to_owned()),
                model: Some("gpt-5.4".to_owned()),
                updated_at: Some("2026-04-22T00:00:00Z".to_owned()),
            }]
        );
    }

    #[test]
    fn parse_thread_list_response_accepts_items_key_and_id_fallback() {
        let threads = parse_thread_list_response(&json!({
            "items": [
                {
                    "id": "thread-2",
                    "summary": "Continue work",
                    "lastUpdatedAt": "2026-04-22T01:00:00Z"
                }
            ]
        }))
        .expect("threads should parse");

        assert_eq!(threads[0].thread_id, "thread-2");
        assert_eq!(threads[0].title.as_deref(), Some("Continue work"));
        assert_eq!(
            threads[0].updated_at.as_deref(),
            Some("2026-04-22T01:00:00Z")
        );
    }

    #[test]
    fn parse_thread_list_response_rejects_entries_without_id() {
        let error = parse_thread_list_response(&json!({
            "threads": [
                {
                    "title": "Missing id"
                }
            ]
        }))
        .expect_err("missing id should fail");

        assert!(error.to_string().contains("thread id"));
    }

    #[test]
    fn turn_steer_params_include_expected_turn_id_and_input() {
        let params = turn_steer_params("thread-1", "turn-1", CodexRequest::new("follow up"));

        assert_eq!(params["threadId"], Value::String("thread-1".to_owned()));
        assert_eq!(params["expectedTurnId"], Value::String("turn-1".to_owned()));
        assert_eq!(params["input"][0]["type"], Value::String("text".to_owned()));
        assert_eq!(
            params["input"][0]["text"],
            Value::String("follow up".to_owned())
        );
    }

    #[tokio::test]
    async fn steer_turn_writes_visible_followup_request() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; status = 'in_progress' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'turn/steer') {{
    @{{ id = $message.id; result = @{{ accepted = $true }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: true,
        };

        client
            .steer_turn("thread-1", "turn-1", CodexRequest::new("follow up"))
            .await
            .expect("turn steer should write");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let mut log = String::new();
        for _ in 0..60 {
            if let Ok(contents) = fs::read_to_string(&log_path) {
                log = contents;
                if log.contains(r#""method":"turn/steer""#) {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(log.contains(r#""method":"turn/steer""#));
        assert!(log.contains(r#""expectedTurnId":"turn-1""#));
        assert!(log.contains("follow up"));
    }

    #[tokio::test]
    async fn steer_turn_initializes_fresh_app_server_client() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'initialize') {{
    @{{ id = $message.id; result = @{{ userAgent = 'Codex/0.118.0' }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; status = 'in_progress' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'turn/steer') {{
    @{{ id = $message.id; result = @{{ accepted = $true }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: false,
        };

        client
            .steer_turn("thread-1", "turn-1", CodexRequest::new("follow up"))
            .await
            .expect("turn steer should initialize and write");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let log = fs::read_to_string(&log_path).expect("fake app-server log");
        assert!(log.contains(r#""method":"initialize""#));
        assert!(log.contains(r#""method":"initialized""#));
        assert!(log.contains(r#""method":"turn/steer""#));
    }

    #[tokio::test]
    async fn resume_turn_omits_model_for_existing_thread() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'initialize') {{
    @{{ id = $message.id; result = @{{ userAgent = 'Codex/0.118.0' }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'thread/resume') {{
    @{{ id = $message.id; result = @{{ thread = @{{ id = 'thread-1' }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'turn/start') {{
    @{{ id = $message.id; result = @{{ turn = @{{ id = 'turn-1' }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
    @{{ method = 'item/agentMessage/delta'; params = @{{ threadId = 'thread-1'; turnId = 'turn-1'; delta = 'done' }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
    @{{ method = 'turn/completed'; params = @{{ turn = @{{ id = 'turn-1'; status = 'completed'; items = @(@{{ type = 'agent_message'; text = 'done' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; items = @(@{{ type = 'agentMessage'; text = 'done' }}) }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: false,
        };

        let config = CodexConfig {
            binary: "codex".to_owned(),
            model: "gpt-5.4".to_owned(),
            sandbox: "workspace-write".to_owned(),
            approval: "on-request".to_owned(),
            transport: CodexTransport::AppServer,
            profile: None,
        };
        let workspace = WorkspaceConfig {
            id: "main".to_owned(),
            path: PathBuf::from("C:/workspace"),
            writable_roots: vec![PathBuf::from("C:/workspace")],
            default_mode: LaneMode::AwaitReply,
            continue_prompt: "continue".to_owned(),
            checks_profile: "default".to_owned(),
        };

        let outcome = client
            .resume_turn(
                &config,
                &workspace,
                "thread-1",
                CodexRequest::new("follow up"),
                None,
                None,
            )
            .await
            .expect("resume turn should succeed");
        assert_eq!(outcome.last_message, "done");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let log = fs::read_to_string(&log_path).expect("fake app-server log");
        assert!(log.contains(r#""method":"thread/resume""#));
        assert!(log.contains(r#""method":"turn/start""#));
        assert!(!log.contains(r#""model":"gpt-5.4""#));
    }

    #[tokio::test]
    async fn steer_turn_rejects_negative_server_response() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; status = 'in_progress' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }} elseif ($message.method -eq 'turn/steer') {{
    @{{ id = $message.id; result = @{{ accepted = $false }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: true,
        };

        client
            .steer_turn("thread-1", "turn-1", CodexRequest::new("follow up"))
            .await
            .expect_err("rejected turn steer should fail");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let log = fs::read_to_string(&log_path).expect("fake app-server log");
        assert!(log.contains(r#""method":"turn/steer""#));
    }

    #[tokio::test]
    async fn steer_turn_rejects_completed_turn_before_writing_followup() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; status = 'completed' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: true,
        };

        client
            .steer_turn("thread-1", "turn-1", CodexRequest::new("follow up"))
            .await
            .expect_err("completed turn should reject follow-up");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let log = fs::read_to_string(&log_path).expect("fake app-server log");
        assert!(log.contains(r#""method":"thread/read""#));
        assert!(!log.contains(r#""method":"turn/steer""#));
    }

    #[tokio::test]
    async fn steer_turn_rejects_unknown_turn_status_before_writing_followup() {
        let dir = tempdir().expect("tempdir should be created");
        let log_path = dir.path().join("app-server.log");
        let ps1_path = dir.path().join("fake-app-server.ps1");
        let escaped_log_path = log_path.display().to_string().replace('\'', "''");
        fs::write(
            &ps1_path,
            format!(
                r#"
$log = '{escaped_log_path}'
while ($null -ne ($line = [Console]::In.ReadLine())) {{
  Add-Content -LiteralPath $log -Value $line
  $message = $line | ConvertFrom-Json
  if ($message.method -eq 'thread/read') {{
    @{{ id = $message.id; result = @{{ thread = @{{ turns = @(@{{ id = 'turn-1'; status = 'mystery' }}) }} }} }} | ConvertTo-Json -Compress -Depth 10 -WarningAction SilentlyContinue
    [Console]::Out.Flush()
  }}
}}
"#
            ),
        )
        .expect("fake app-server script should write");
        let mut child = Command::new("pwsh")
            .args(["-NoProfile", "-File"])
            .arg(&ps1_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("fake app-server should spawn");
        let stdin = child.stdin.take().expect("stdin should exist");
        let stdout = child.stdout.take().expect("stdout should exist");
        let mut client = AppServerClient {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
            backlog: VecDeque::new(),
            next_request_id: 1,
            initialized: true,
        };

        client
            .steer_turn("thread-1", "turn-1", CodexRequest::new("follow up"))
            .await
            .expect_err("unknown turn status should reject follow-up");
        client
            .stdin
            .shutdown()
            .await
            .expect("fake app-server stdin should close");
        let _ = timeout(Duration::from_secs(2), client._child.wait()).await;

        let log = fs::read_to_string(&log_path).expect("fake app-server log");
        assert!(log.contains(r#""method":"thread/read""#));
        assert!(!log.contains(r#""method":"turn/steer""#));
    }
}
