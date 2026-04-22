use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter, types::Type};
use uuid::Uuid;

use crate::config::LaneMode;

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneState {
    Running,
    WaitingReply,
    Idle,
    NeedsLocalApproval,
    Failed,
}

impl LaneState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::WaitingReply => "waiting_reply",
            Self::Idle => "idle",
            Self::NeedsLocalApproval => "needs_local_approval",
            Self::Failed => "failed",
        }
    }

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        match value {
            "running" => Ok(Self::Running),
            "waiting_reply" => Ok(Self::WaitingReply),
            "idle" => Ok(Self::Idle),
            "needs_local_approval" => Ok(Self::NeedsLocalApproval),
            "failed" => Ok(Self::Failed),
            other => Err(format!("unknown lane state: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizedSender {
    pub sender_id: i64,
    pub platform: String,
    pub display_name: Option<String>,
    pub status: String,
    pub approved_at_ms: i64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAccessPairCode {
    pub code: String,
    pub sender_id: i64,
    pub chat_id: i64,
    pub chat_type: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct LaneRecord {
    pub lane_id: String,
    pub chat_id: i64,
    pub thread_key: String,
    pub workspace_id: String,
    pub mode: LaneMode,
    pub state: LaneState,
    pub codex_session_id: Option<String>,
    pub extra_turn_budget: i64,
    pub waiting_since_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexThreadBinding {
    pub chat_id: i64,
    pub thread_key: String,
    pub codex_thread_id: String,
    pub workspace_id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub codex_updated_at: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct NewCodexThreadBinding {
    pub chat_id: i64,
    pub thread_key: String,
    pub codex_thread_id: String,
    pub workspace_id: String,
    pub title: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub codex_updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewRun {
    pub lane_id: String,
    pub run_kind: String,
}

#[derive(Debug, Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub lane_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequestKind {
    CommandExecution,
    FileChange,
    Permissions,
    ToolUserInput,
}

impl ApprovalRequestKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::CommandExecution => "command_execution",
            Self::FileChange => "file_change",
            Self::Permissions => "permissions",
            Self::ToolUserInput => "tool_user_input",
        }
    }

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        match value {
            "command_execution" => Ok(Self::CommandExecution),
            "file_change" => Ok(Self::FileChange),
            "permissions" => Ok(Self::Permissions),
            "tool_user_input" => Ok(Self::ToolUserInput),
            other => Err(format!("unknown approval request kind: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequestStatus {
    Dispatching,
    Pending,
    Resolving,
    Invalidated,
    Approved,
    Declined,
    TimedOut,
}

impl ApprovalRequestStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Dispatching => "dispatching",
            Self::Pending => "pending",
            Self::Resolving => "resolving",
            Self::Invalidated => "invalidated",
            Self::Approved => "approved",
            Self::Declined => "declined",
            Self::TimedOut => "timed_out",
        }
    }

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        match value {
            "dispatching" => Ok(Self::Dispatching),
            "pending" => Ok(Self::Pending),
            "resolving" => Ok(Self::Resolving),
            "invalidated" => Ok(Self::Invalidated),
            "approved" => Ok(Self::Approved),
            "declined" => Ok(Self::Declined),
            "timed_out" => Ok(Self::TimedOut),
            other => Err(format!("unknown approval request status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalRequestTransport {
    AppServer,
    Exec,
}

impl ApprovalRequestTransport {
    fn as_str(self) -> &'static str {
        match self {
            Self::AppServer => "app_server",
            Self::Exec => "exec",
        }
    }

    fn from_str(value: &str) -> std::result::Result<Self, String> {
        match value {
            "app_server" => Ok(Self::AppServer),
            "exec" => Ok(Self::Exec),
            other => Err(format!("unknown approval request transport: {other}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalRequestRecord {
    pub request_id: String,
    pub transport_request_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub transport: ApprovalRequestTransport,
    pub request_kind: ApprovalRequestKind,
    pub summary_text: String,
    pub raw_payload_json: String,
    pub status: ApprovalRequestStatus,
    pub requested_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
    pub resolved_by_sender_id: Option<i64>,
    pub telegram_message_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PendingApprovalNotification {
    pub request: ApprovalRequestRecord,
    pub chat_id: i64,
}

#[derive(Debug, Clone)]
pub struct NewApprovalRequest {
    pub request_id: String,
    pub transport_request_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub transport: ApprovalRequestTransport,
    pub request_kind: ApprovalRequestKind,
    pub summary_text: String,
    pub raw_payload_json: String,
    pub status: ApprovalRequestStatus,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("failed to open sqlite database")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.migrate()?;
        store.expire_legacy_app_server_approval_requests()?;
        Ok(store)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open sqlite database read-only")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS authorized_senders (
                sender_id INTEGER PRIMARY KEY,
                platform TEXT NOT NULL,
                display_name TEXT,
                status TEXT NOT NULL,
                approved_at_ms INTEGER NOT NULL,
                source TEXT NOT NULL DEFAULT 'paired'
            );

            CREATE TABLE IF NOT EXISTS lanes (
                lane_id TEXT PRIMARY KEY,
                chat_id INTEGER NOT NULL,
                thread_key TEXT NOT NULL,
                workspace_id TEXT NOT NULL,
                mode TEXT NOT NULL,
                state TEXT NOT NULL,
                codex_session_id TEXT,
                extra_turn_budget INTEGER NOT NULL DEFAULT 0,
                waiting_since_ms INTEGER,
                UNIQUE(chat_id, thread_key)
            );

            CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                lane_id TEXT NOT NULL,
                run_kind TEXT NOT NULL,
                started_at_ms INTEGER NOT NULL,
                ended_at_ms INTEGER,
                exit_code INTEGER,
                completion_reason TEXT,
                approval_pending INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS codex_thread_bindings (
                chat_id INTEGER NOT NULL,
                thread_key TEXT NOT NULL,
                codex_thread_id TEXT NOT NULL,
                workspace_id TEXT NOT NULL,
                title TEXT,
                cwd TEXT,
                model TEXT,
                codex_updated_at TEXT,
                created_at_ms INTEGER NOT NULL,
                updated_at_ms INTEGER NOT NULL,
                PRIMARY KEY(chat_id, thread_key)
            );

            CREATE TABLE IF NOT EXISTS telegram_updates (
                update_id INTEGER PRIMARY KEY,
                chat_id INTEGER NOT NULL,
                sender_id INTEGER,
                update_kind TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS telegram_access_pair_codes (
                code TEXT PRIMARY KEY,
                sender_id INTEGER NOT NULL,
                chat_id INTEGER NOT NULL,
                chat_type TEXT NOT NULL,
                issued_at_ms INTEGER NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                consumed_at_ms INTEGER
            );

            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lane_id TEXT NOT NULL,
                run_id TEXT,
                direction TEXT NOT NULL,
                message_kind TEXT NOT NULL,
                telegram_message_id INTEGER,
                body_text TEXT,
                payload_json TEXT
            );

            INSERT OR IGNORE INTO schema_migrations(version, applied_at_ms)
                VALUES (1, unixepoch('subsec') * 1000);
        "#;
        self.with_conn(|conn| conn.execute_batch(sql))?;
        self.with_conn(|conn| {
            ensure_column_exists(
                conn,
                "runs",
                "approval_request_count",
                "ALTER TABLE runs ADD COLUMN approval_request_count INTEGER NOT NULL DEFAULT 0",
            )?;
            let added_authorized_sender_source = ensure_column_exists(
                conn,
                "authorized_senders",
                "source",
                "ALTER TABLE authorized_senders ADD COLUMN source TEXT NOT NULL DEFAULT 'paired'",
            )?;
            if added_authorized_sender_source {
                conn.execute(
                    "UPDATE authorized_senders SET source = 'config' WHERE source = 'paired'",
                    [],
                )?;
            }
            ensure_column_exists(
                conn,
                "runs",
                "approval_resolved_count",
                "ALTER TABLE runs ADD COLUMN approval_resolved_count INTEGER NOT NULL DEFAULT 0",
            )?;
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS approval_requests (
                    request_id TEXT PRIMARY KEY,
                    transport_request_id TEXT NOT NULL DEFAULT '',
                    lane_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    thread_id TEXT NOT NULL,
                    turn_id TEXT NOT NULL,
                    item_id TEXT NOT NULL,
                    transport TEXT NOT NULL DEFAULT 'app_server',
                    request_kind TEXT NOT NULL,
                    summary_text TEXT NOT NULL,
                    raw_payload_json TEXT NOT NULL,
                    status TEXT NOT NULL,
                    requested_at_ms INTEGER NOT NULL,
                    resolved_at_ms INTEGER,
                    resolved_by_sender_id INTEGER,
                    telegram_message_id INTEGER
                );
                "#,
            )?;
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS telegram_access_pair_codes (
                    code TEXT PRIMARY KEY,
                    sender_id INTEGER NOT NULL,
                    chat_id INTEGER NOT NULL,
                    chat_type TEXT NOT NULL,
                    issued_at_ms INTEGER NOT NULL,
                    expires_at_ms INTEGER NOT NULL,
                    consumed_at_ms INTEGER
                );
                "#,
            )?;
            ensure_column_exists(
                conn,
                "approval_requests",
                "transport",
                "ALTER TABLE approval_requests ADD COLUMN transport TEXT NOT NULL DEFAULT 'app_server'",
            )?;
            ensure_column_exists(
                conn,
                "approval_requests",
                "transport_request_id",
                "ALTER TABLE approval_requests ADD COLUMN transport_request_id TEXT NOT NULL DEFAULT ''",
            )
        })?;
        Ok(())
    }

    fn expire_legacy_app_server_approval_requests(&self) -> Result<()> {
        let legacy = self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT request_id, lane_id, run_id
                FROM approval_requests
                WHERE transport = 'app_server'
                  AND transport_request_id = ''
                  AND telegram_message_id IS NOT NULL
                  AND status IN ('pending', 'dispatching', 'resolving')
                "#,
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        for (request_id, lane_id, run_id) in legacy {
            let _ = self.expire_approval_request(&request_id, &lane_id, &run_id)?;
        }
        Ok(())
    }

    pub fn upsert_authorized_sender(&self, sender: AuthorizedSender) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO authorized_senders(sender_id, platform, display_name, status, approved_at_ms, source)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(sender_id) DO UPDATE SET
                    platform = excluded.platform,
                    display_name = excluded.display_name,
                    status = excluded.status,
                    approved_at_ms = excluded.approved_at_ms,
                    source = CASE
                        WHEN authorized_senders.source = 'paired' AND excluded.source = 'config'
                            THEN authorized_senders.source
                        ELSE excluded.source
                    END
                "#,
                params![
                    sender.sender_id,
                    sender.platform,
                    sender.display_name,
                    sender.status,
                    sender.approved_at_ms,
                    sender.source,
                ],
            )
        })?;
        Ok(())
    }

    pub fn sync_config_authorized_senders(&self, sender_ids: &[i64]) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        for sender_id in sender_ids {
            self.upsert_authorized_sender(AuthorizedSender {
                sender_id: *sender_id,
                platform: "telegram".to_owned(),
                display_name: None,
                status: "active".to_owned(),
                approved_at_ms: now,
                source: "config".to_owned(),
            })?;
        }

        self.with_conn(|conn| {
            if sender_ids.is_empty() {
                conn.execute(
                    "UPDATE authorized_senders SET status = 'inactive' WHERE source = 'config'",
                    [],
                )?;
            } else {
                let placeholders = (1..=sender_ids.len())
                    .map(|index| format!("?{index}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "UPDATE authorized_senders SET status = 'inactive' WHERE source = 'config' AND sender_id NOT IN ({placeholders})"
                );
                conn.execute(&sql, params_from_iter(sender_ids.iter()))?;
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn insert_access_pair_code(&self, code: &PendingAccessPairCode) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO telegram_access_pair_codes(
                    code, sender_id, chat_id, chat_type, issued_at_ms, expires_at_ms, consumed_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
                ON CONFLICT(code) DO UPDATE SET
                    sender_id = excluded.sender_id,
                    chat_id = excluded.chat_id,
                    chat_type = excluded.chat_type,
                    issued_at_ms = excluded.issued_at_ms,
                    expires_at_ms = excluded.expires_at_ms,
                    consumed_at_ms = NULL
                "#,
                params![
                    code.code,
                    code.sender_id,
                    code.chat_id,
                    code.chat_type,
                    code.issued_at_ms,
                    code.expires_at_ms,
                ],
            )
        })?;
        Ok(())
    }

    pub fn consume_access_pair_code(
        &self,
        code: &str,
        now_ms: i64,
    ) -> Result<Option<PendingAccessPairCode>> {
        self.with_conn(|conn| {
            let updated = conn.execute(
                r#"
                UPDATE telegram_access_pair_codes
                SET consumed_at_ms = ?2
                WHERE code = ?1
                  AND consumed_at_ms IS NULL
                  AND expires_at_ms >= ?2
                "#,
                params![code, now_ms],
            )?;
            if updated == 0 {
                return Ok(None);
            }

            conn.query_row(
                r#"
                SELECT code, sender_id, chat_id, chat_type, issued_at_ms, expires_at_ms
                FROM telegram_access_pair_codes
                WHERE code = ?1
                "#,
                params![code],
                |row| {
                    Ok(PendingAccessPairCode {
                        code: row.get(0)?,
                        sender_id: row.get(1)?,
                        chat_id: row.get(2)?,
                        chat_type: row.get(3)?,
                        issued_at_ms: row.get(4)?,
                        expires_at_ms: row.get(5)?,
                    })
                },
            )
            .optional()
        })
        .map_err(Into::into)
    }

    pub fn is_authorized_sender(&self, sender_id: i64) -> Result<bool> {
        let found: Option<i64> = self.with_conn(|conn| {
            conn.query_row(
                "SELECT sender_id FROM authorized_senders WHERE sender_id = ?1 AND status = 'active'",
                params![sender_id],
                |row| row.get(0),
            )
            .optional()
        })?;
        Ok(found.is_some())
    }

    pub fn active_authorized_sender(&self, sender_id: i64) -> Result<Option<AuthorizedSender>> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"
                SELECT sender_id, platform, display_name, status, approved_at_ms, source
                FROM authorized_senders
                WHERE sender_id = ?1 AND status = 'active'
                "#,
                params![sender_id],
                |row| {
                    Ok(AuthorizedSender {
                        sender_id: row.get(0)?,
                        platform: row.get(1)?,
                        display_name: row.get(2)?,
                        status: row.get(3)?,
                        approved_at_ms: row.get(4)?,
                        source: row.get(5)?,
                    })
                },
            )
            .optional()
        })
        .map_err(Into::into)
    }

    pub fn list_active_authorized_senders(&self) -> Result<Vec<AuthorizedSender>> {
        self.with_conn(|conn| {
            let source_expr = if column_exists(conn, "authorized_senders", "source")? {
                "source"
            } else {
                "'paired'"
            };
            let sql = format!(
                r#"
                SELECT sender_id, platform, display_name, status, approved_at_ms, {source_expr}
                FROM authorized_senders
                WHERE status = 'active'
                ORDER BY approved_at_ms ASC, sender_id ASC
                "#
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map([], |row| {
                Ok(AuthorizedSender {
                    sender_id: row.get(0)?,
                    platform: row.get(1)?,
                    display_name: row.get(2)?,
                    status: row.get(3)?,
                    approved_at_ms: row.get(4)?,
                    source: row.get(5)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(Into::into)
    }

    pub fn insert_seen_update(
        &self,
        update_id: i64,
        chat_id: i64,
        sender_id: Option<i64>,
        update_kind: &str,
        payload_json: &str,
    ) -> Result<bool> {
        let inserted = self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT OR IGNORE INTO telegram_updates(update_id, chat_id, sender_id, update_kind, payload_json)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![update_id, chat_id, sender_id, update_kind, payload_json],
            )
        })?;
        Ok(inserted > 0)
    }

    pub fn get_or_create_lane(
        &self,
        chat_id: i64,
        thread_key: &str,
        workspace_id: &str,
        mode: LaneMode,
        extra_turn_budget: i64,
    ) -> Result<LaneRecord> {
        if let Some(lane) = self.find_lane(chat_id, thread_key)? {
            return Ok(lane);
        }
        let lane = LaneRecord {
            lane_id: Uuid::new_v4().to_string(),
            chat_id,
            thread_key: thread_key.to_owned(),
            workspace_id: workspace_id.to_owned(),
            mode,
            state: LaneState::Idle,
            codex_session_id: None,
            extra_turn_budget,
            waiting_since_ms: None,
        };
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO lanes(
                    lane_id, chat_id, thread_key, workspace_id, mode, state,
                    codex_session_id, extra_turn_budget, waiting_since_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    lane.lane_id,
                    lane.chat_id,
                    lane.thread_key,
                    lane.workspace_id,
                    mode_to_str(lane.mode),
                    lane.state.as_str(),
                    lane.codex_session_id,
                    lane.extra_turn_budget,
                    lane.waiting_since_ms,
                ],
            )
        })?;
        Ok(lane)
    }

    pub fn find_lane(&self, chat_id: i64, thread_key: &str) -> Result<Option<LaneRecord>> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"
                SELECT lane_id, chat_id, thread_key, workspace_id, mode, state,
                       codex_session_id, extra_turn_budget, waiting_since_ms
                FROM lanes
                WHERE chat_id = ?1 AND thread_key = ?2
                "#,
                params![chat_id, thread_key],
                |row| {
                    Ok(LaneRecord {
                        lane_id: row.get(0)?,
                        chat_id: row.get(1)?,
                        thread_key: row.get(2)?,
                        workspace_id: row.get(3)?,
                        mode: mode_from_str(&row.get::<_, String>(4)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                4,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                        state: LaneState::from_str(&row.get::<_, String>(5)?).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                        codex_session_id: row.get(6)?,
                        extra_turn_budget: row.get(7)?,
                        waiting_since_ms: row.get(8)?,
                    })
                },
            )
            .optional()
        })
    }

    pub fn update_lane_state(
        &self,
        lane_id: &str,
        state: LaneState,
        codex_session_id: Option<&str>,
    ) -> Result<()> {
        let waiting_since_ms = if state == LaneState::WaitingReply {
            Some(Utc::now().timestamp_millis())
        } else {
            None
        };
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET state = ?2,
                    codex_session_id = COALESCE(?3, codex_session_id),
                    waiting_since_ms = ?4
                WHERE lane_id = ?1
                "#,
                params![lane_id, state.as_str(), codex_session_id, waiting_since_ms],
            )
        })?;
        Ok(())
    }

    pub fn clear_lane_session(&self, lane_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET state = 'idle',
                    codex_session_id = NULL,
                    waiting_since_ms = NULL
                WHERE lane_id = ?1
                "#,
                params![lane_id],
            )
        })?;
        Ok(())
    }

    pub fn fail_lane(&self, lane_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET state = 'failed',
                    codex_session_id = NULL,
                    waiting_since_ms = NULL
                WHERE lane_id = ?1
                "#,
                params![lane_id],
            )
        })?;
        Ok(())
    }

    pub fn update_lane_mode(
        &self,
        lane_id: &str,
        mode: LaneMode,
        extra_turn_budget: i64,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET mode = ?2,
                    extra_turn_budget = ?3
                WHERE lane_id = ?1
                "#,
                params![lane_id, mode_to_str(mode), extra_turn_budget],
            )
        })?;
        Ok(())
    }

    pub fn update_lane_workspace(&self, lane_id: &str, workspace_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET workspace_id = ?2,
                    state = 'idle',
                    codex_session_id = NULL,
                    waiting_since_ms = NULL
                WHERE lane_id = ?1
                "#,
                params![lane_id, workspace_id],
            )
        })?;
        Ok(())
    }

    pub fn insert_run(&self, new_run: NewRun) -> Result<RunRecord> {
        let run = RunRecord {
            run_id: Uuid::new_v4().to_string(),
            lane_id: new_run.lane_id,
        };
        let now = Utc::now().timestamp_millis();
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO runs(run_id, lane_id, run_kind, started_at_ms)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![run.run_id, run.lane_id, new_run.run_kind, now],
            )
        })?;
        Ok(run)
    }

    pub fn finish_run(
        &self,
        run_id: &str,
        exit_code: Option<i32>,
        completion_reason: &str,
        approval_pending: bool,
        approval_request_count: i64,
        approval_resolved_count: i64,
    ) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE runs
                SET ended_at_ms = ?2,
                    exit_code = ?3,
                    completion_reason = ?4,
                    approval_pending = ?5,
                    approval_request_count = ?6,
                    approval_resolved_count = ?7
                WHERE run_id = ?1
                "#,
                params![
                    run_id,
                    now,
                    exit_code,
                    completion_reason,
                    approval_pending as i32,
                    approval_request_count,
                    approval_resolved_count,
                ],
            )
        })?;
        Ok(())
    }

    pub fn insert_approval_request(&self, request: NewApprovalRequest) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO approval_requests(
                    request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                    transport, request_kind, summary_text, raw_payload_json, status, requested_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(request_id) DO NOTHING
                "#,
                params![
                    request.request_id,
                    request.transport_request_id,
                    request.lane_id,
                    request.run_id,
                    request.thread_id,
                    request.turn_id,
                    request.item_id,
                    request.transport.as_str(),
                    request.request_kind.as_str(),
                    request.summary_text,
                    request.raw_payload_json,
                    request.status.as_str(),
                    now,
                ],
            )
        })?;
        Ok(())
    }

    pub fn prepare_approval_request_for_dispatch(
        &self,
        request: NewApprovalRequest,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO approval_requests(
                    request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                    transport, request_kind, summary_text, raw_payload_json, status, requested_at_ms,
                    resolved_at_ms, resolved_by_sender_id, telegram_message_id
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'dispatching', ?12, NULL, NULL, NULL)
                ON CONFLICT(request_id) DO UPDATE SET
                    transport_request_id = excluded.transport_request_id,
                    lane_id = excluded.lane_id,
                    run_id = excluded.run_id,
                    thread_id = excluded.thread_id,
                    turn_id = excluded.turn_id,
                    item_id = excluded.item_id,
                    transport = excluded.transport,
                    request_kind = excluded.request_kind,
                    summary_text = excluded.summary_text,
                    raw_payload_json = excluded.raw_payload_json,
                    status = 'dispatching',
                    requested_at_ms = excluded.requested_at_ms,
                    resolved_at_ms = NULL,
                    resolved_by_sender_id = NULL,
                    telegram_message_id = NULL
                WHERE approval_requests.status IN ('invalidated', 'timed_out')
                "#,
                params![
                    request.request_id,
                    request.transport_request_id,
                    request.lane_id,
                    request.run_id,
                    request.thread_id,
                    request.turn_id,
                    request.item_id,
                    request.transport.as_str(),
                    request.request_kind.as_str(),
                    request.summary_text,
                    request.raw_payload_json,
                    now,
                ],
            )
        })?;
        Ok(updated > 0)
    }

    pub fn find_approval_request(&self, request_id: &str) -> Result<Option<ApprovalRequestRecord>> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"
                SELECT request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                       transport, request_kind, summary_text, raw_payload_json, status,
                       requested_at_ms, resolved_at_ms, resolved_by_sender_id, telegram_message_id
                FROM approval_requests
                WHERE request_id = ?1
                "#,
                params![request_id],
                |row| {
                    Ok(ApprovalRequestRecord {
                        request_id: row.get(0)?,
                        transport_request_id: row.get(1)?,
                        lane_id: row.get(2)?,
                        run_id: row.get(3)?,
                        thread_id: row.get(4)?,
                        turn_id: row.get(5)?,
                        item_id: row.get(6)?,
                        transport: ApprovalRequestTransport::from_str(&row.get::<_, String>(7)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    8,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        summary_text: row.get(9)?,
                        raw_payload_json: row.get(10)?,
                        status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    11,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        requested_at_ms: row.get(12)?,
                        resolved_at_ms: row.get(13)?,
                        resolved_by_sender_id: row.get(14)?,
                        telegram_message_id: row.get(15)?,
                    })
                },
            )
            .optional()
        })
    }

    pub fn resolve_approval_request(
        &self,
        request_id: &str,
        status: ApprovalRequestStatus,
        resolved_by_sender_id: i64,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = ?2,
                    resolved_at_ms = ?3,
                    resolved_by_sender_id = ?4
                WHERE request_id = ?1 AND status IN ('pending', 'resolving')
                "#,
                params![request_id, status.as_str(), now, resolved_by_sender_id],
            )
        })?;
        Ok(updated > 0)
    }

    pub fn begin_approval_resolution(
        &self,
        request_id: &str,
        resolved_by_sender_id: i64,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = 'resolving',
                    resolved_at_ms = ?2,
                    resolved_by_sender_id = ?3
                WHERE request_id = ?1 AND status = 'pending'
                "#,
                params![request_id, now, resolved_by_sender_id],
            )
        })?;
        Ok(updated > 0)
    }

    pub fn set_approval_request_message_id(
        &self,
        request_id: &str,
        telegram_message_id: i64,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE approval_requests SET telegram_message_id = ?2 WHERE request_id = ?1",
                params![request_id, telegram_message_id],
            )
        })?;
        Ok(())
    }

    pub fn mark_approval_request_pending(
        &self,
        request_id: &str,
        telegram_message_id: i64,
    ) -> Result<bool> {
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = 'pending',
                    resolved_at_ms = NULL,
                    resolved_by_sender_id = NULL,
                    telegram_message_id = ?2
                WHERE request_id = ?1 AND status IN ('dispatching', 'resolving')
                "#,
                params![request_id, telegram_message_id],
            )
        })?;
        Ok(updated > 0)
    }

    pub fn invalidate_approval_request(&self, request_id: &str) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = 'invalidated',
                    resolved_at_ms = ?2,
                    resolved_by_sender_id = NULL
                WHERE request_id = ?1 AND status IN ('dispatching', 'pending', 'resolving')
                "#,
                params![request_id, now],
            )
        })?;
        Ok(updated > 0)
    }

    pub fn delete_approval_request(&self, request_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM approval_requests WHERE request_id = ?1",
                params![request_id],
            )
        })?;
        Ok(())
    }

    pub fn list_pending_approval_requests_for_lane(
        &self,
        lane_id: &str,
    ) -> Result<Vec<ApprovalRequestRecord>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                       transport, request_kind, summary_text, raw_payload_json, status,
                       requested_at_ms, resolved_at_ms, resolved_by_sender_id, telegram_message_id
                FROM approval_requests
                WHERE lane_id = ?1 AND status = 'pending'
                ORDER BY requested_at_ms ASC
                "#,
            )?;
            let rows = statement.query_map(params![lane_id], |row| {
                Ok(ApprovalRequestRecord {
                    request_id: row.get(0)?,
                    transport_request_id: row.get(1)?,
                    lane_id: row.get(2)?,
                    run_id: row.get(3)?,
                    thread_id: row.get(4)?,
                    turn_id: row.get(5)?,
                    item_id: row.get(6)?,
                    transport: ApprovalRequestTransport::from_str(&row.get::<_, String>(7)?)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                    request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                8,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                    summary_text: row.get(9)?,
                    raw_payload_json: row.get(10)?,
                    status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?).map_err(
                        |err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                11,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        },
                    )?,
                    requested_at_ms: row.get(12)?,
                    resolved_at_ms: row.get(13)?,
                    resolved_by_sender_id: row.get(14)?,
                    telegram_message_id: row.get(15)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    pub fn list_unresolved_approval_requests_for_lane(
        &self,
        lane_id: &str,
    ) -> Result<Vec<ApprovalRequestRecord>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                       transport, request_kind, summary_text, raw_payload_json, status,
                       requested_at_ms, resolved_at_ms, resolved_by_sender_id, telegram_message_id
                FROM approval_requests
                WHERE lane_id = ?1 AND status IN ('pending', 'dispatching', 'resolving')
                ORDER BY requested_at_ms ASC
                "#,
            )?;
            let rows = statement.query_map(params![lane_id], |row| {
                Ok(ApprovalRequestRecord {
                    request_id: row.get(0)?,
                    transport_request_id: row.get(1)?,
                    lane_id: row.get(2)?,
                    run_id: row.get(3)?,
                    thread_id: row.get(4)?,
                    turn_id: row.get(5)?,
                    item_id: row.get(6)?,
                    transport: ApprovalRequestTransport::from_str(&row.get::<_, String>(7)?)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                7,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                    request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                        .map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                8,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        })?,
                    summary_text: row.get(9)?,
                    raw_payload_json: row.get(10)?,
                    status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?).map_err(
                        |err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                11,
                                Type::Text,
                                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
                            )
                        },
                    )?,
                    requested_at_ms: row.get(12)?,
                    resolved_at_ms: row.get(13)?,
                    resolved_by_sender_id: row.get(14)?,
                    telegram_message_id: row.get(15)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    pub fn list_pending_approval_notifications(&self) -> Result<Vec<PendingApprovalNotification>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT ar.request_id, ar.transport_request_id, ar.lane_id, ar.run_id, ar.thread_id, ar.turn_id, ar.item_id,
                       ar.transport, ar.request_kind, ar.summary_text, ar.raw_payload_json,
                       ar.status, ar.requested_at_ms, ar.resolved_at_ms,
                       ar.resolved_by_sender_id, ar.telegram_message_id, l.chat_id
                FROM approval_requests ar
                JOIN lanes l ON l.lane_id = ar.lane_id
                WHERE ar.status = 'pending'
                ORDER BY ar.requested_at_ms ASC
                "#,
            )?;
            let rows = statement.query_map([], |row| {
                Ok(PendingApprovalNotification {
                    request: ApprovalRequestRecord {
                        request_id: row.get(0)?,
                        transport_request_id: row.get(1)?,
                        lane_id: row.get(2)?,
                        run_id: row.get(3)?,
                        thread_id: row.get(4)?,
                        turn_id: row.get(5)?,
                        item_id: row.get(6)?,
                        transport: ApprovalRequestTransport::from_str(&row.get::<_, String>(7)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    8,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        summary_text: row.get(9)?,
                        raw_payload_json: row.get(10)?,
                        status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    11,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        requested_at_ms: row.get(12)?,
                        resolved_at_ms: row.get(13)?,
                        resolved_by_sender_id: row.get(14)?,
                        telegram_message_id: row.get(15)?,
                    },
                    chat_id: row.get(16)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    pub fn list_recent_resolving_approval_notifications(
        &self,
        transport: ApprovalRequestTransport,
        resolved_after_ms: i64,
    ) -> Result<Vec<PendingApprovalNotification>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT ar.request_id, ar.transport_request_id, ar.lane_id, ar.run_id, ar.thread_id, ar.turn_id, ar.item_id,
                       ar.transport, ar.request_kind, ar.summary_text, ar.raw_payload_json,
                       ar.status, ar.requested_at_ms, ar.resolved_at_ms,
                       ar.resolved_by_sender_id, ar.telegram_message_id, l.chat_id
                FROM approval_requests ar
                JOIN lanes l ON l.lane_id = ar.lane_id
                WHERE ar.transport = ?1
                  AND ar.status = 'resolving'
                  AND COALESCE(ar.resolved_at_ms, ar.requested_at_ms) > ?2
                ORDER BY COALESCE(ar.resolved_at_ms, ar.requested_at_ms) ASC
                "#,
            )?;
            let rows =
                statement.query_map(params![transport.as_str(), resolved_after_ms], |row| {
                    Ok(PendingApprovalNotification {
                        request: ApprovalRequestRecord {
                            request_id: row.get(0)?,
                            transport_request_id: row.get(1)?,
                            lane_id: row.get(2)?,
                            run_id: row.get(3)?,
                            thread_id: row.get(4)?,
                            turn_id: row.get(5)?,
                            item_id: row.get(6)?,
                            transport: ApprovalRequestTransport::from_str(
                                &row.get::<_, String>(7)?,
                            )
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                            request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        8,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            summary_text: row.get(9)?,
                            raw_payload_json: row.get(10)?,
                            status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        11,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            requested_at_ms: row.get(12)?,
                            resolved_at_ms: row.get(13)?,
                            resolved_by_sender_id: row.get(14)?,
                            telegram_message_id: row.get(15)?,
                        },
                        chat_id: row.get(16)?,
                    })
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    pub fn list_recent_dispatching_approval_notifications(
        &self,
        transport: ApprovalRequestTransport,
        requested_after_ms: i64,
    ) -> Result<Vec<PendingApprovalNotification>> {
        self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT ar.request_id, ar.transport_request_id, ar.lane_id, ar.run_id, ar.thread_id, ar.turn_id, ar.item_id,
                       ar.transport, ar.request_kind, ar.summary_text, ar.raw_payload_json,
                       ar.status, ar.requested_at_ms, ar.resolved_at_ms,
                       ar.resolved_by_sender_id, ar.telegram_message_id, l.chat_id
                FROM approval_requests ar
                JOIN lanes l ON l.lane_id = ar.lane_id
                WHERE ar.transport = ?1
                  AND ar.status = 'dispatching'
                  AND ar.requested_at_ms > ?2
                ORDER BY ar.requested_at_ms ASC
                "#,
            )?;
            let rows =
                statement.query_map(params![transport.as_str(), requested_after_ms], |row| {
                    Ok(PendingApprovalNotification {
                        request: ApprovalRequestRecord {
                            request_id: row.get(0)?,
                            transport_request_id: row.get(1)?,
                            lane_id: row.get(2)?,
                            run_id: row.get(3)?,
                            thread_id: row.get(4)?,
                            turn_id: row.get(5)?,
                            item_id: row.get(6)?,
                            transport: ApprovalRequestTransport::from_str(
                                &row.get::<_, String>(7)?,
                            )
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                            request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        8,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            summary_text: row.get(9)?,
                            raw_payload_json: row.get(10)?,
                            status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        11,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            requested_at_ms: row.get(12)?,
                            resolved_at_ms: row.get(13)?,
                            resolved_by_sender_id: row.get(14)?,
                            telegram_message_id: row.get(15)?,
                        },
                        chat_id: row.get(16)?,
                    })
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
    }

    pub fn expire_pending_approval_notifications(
        &self,
        transport: ApprovalRequestTransport,
        requested_before_ms: i64,
    ) -> Result<Vec<PendingApprovalNotification>> {
        let pending = self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT ar.request_id, ar.transport_request_id, ar.lane_id, ar.run_id, ar.thread_id, ar.turn_id, ar.item_id,
                       ar.transport, ar.request_kind, ar.summary_text, ar.raw_payload_json,
                       ar.status, ar.requested_at_ms, ar.resolved_at_ms,
                       ar.resolved_by_sender_id, ar.telegram_message_id, l.chat_id
                FROM approval_requests ar
                JOIN lanes l ON l.lane_id = ar.lane_id
                WHERE ar.transport = ?1
                  AND ar.status IN ('pending', 'dispatching')
                  AND ar.requested_at_ms <= ?2
                ORDER BY ar.requested_at_ms ASC
                "#,
            )?;
            let rows =
                statement.query_map(params![transport.as_str(), requested_before_ms], |row| {
                    Ok(PendingApprovalNotification {
                        request: ApprovalRequestRecord {
                            request_id: row.get(0)?,
                            transport_request_id: row.get(1)?,
                            lane_id: row.get(2)?,
                            run_id: row.get(3)?,
                            thread_id: row.get(4)?,
                            turn_id: row.get(5)?,
                            item_id: row.get(6)?,
                            transport: ApprovalRequestTransport::from_str(
                                &row.get::<_, String>(7)?,
                            )
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                            request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        8,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            summary_text: row.get(9)?,
                            raw_payload_json: row.get(10)?,
                            status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                                .map_err(|err| {
                                    rusqlite::Error::FromSqlConversionFailure(
                                        11,
                                        Type::Text,
                                        Box::new(std::io::Error::new(
                                            std::io::ErrorKind::InvalidData,
                                            err,
                                        )),
                                    )
                                })?,
                            requested_at_ms: row.get(12)?,
                            resolved_at_ms: row.get(13)?,
                            resolved_by_sender_id: row.get(14)?,
                            telegram_message_id: row.get(15)?,
                        },
                        chat_id: row.get(16)?,
                    })
                })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        for notification in &pending {
            let _ = self.expire_approval_request(
                &notification.request.request_id,
                &notification.request.lane_id,
                &notification.request.run_id,
            )?;
        }
        Ok(pending)
    }

    pub fn invalidate_pending_approval_notifications_for_restart(
        &self,
        transport: ApprovalRequestTransport,
    ) -> Result<Vec<PendingApprovalNotification>> {
        let pending = self.with_conn(|conn| {
            let mut statement = conn.prepare(
                r#"
                SELECT ar.request_id, ar.transport_request_id, ar.lane_id, ar.run_id, ar.thread_id, ar.turn_id, ar.item_id,
                       ar.transport, ar.request_kind, ar.summary_text, ar.raw_payload_json,
                       ar.status, ar.requested_at_ms, ar.resolved_at_ms,
                       ar.resolved_by_sender_id, ar.telegram_message_id, l.chat_id
                FROM approval_requests ar
                JOIN lanes l ON l.lane_id = ar.lane_id
                WHERE ar.transport = ?1
                  AND ar.status IN ('pending', 'dispatching')
                ORDER BY ar.requested_at_ms ASC
                "#,
            )?;
            let rows = statement.query_map(params![transport.as_str()], |row| {
                Ok(PendingApprovalNotification {
                    request: ApprovalRequestRecord {
                        request_id: row.get(0)?,
                        transport_request_id: row.get(1)?,
                        lane_id: row.get(2)?,
                        run_id: row.get(3)?,
                        thread_id: row.get(4)?,
                        turn_id: row.get(5)?,
                        item_id: row.get(6)?,
                        transport: ApprovalRequestTransport::from_str(&row.get::<_, String>(7)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        request_kind: ApprovalRequestKind::from_str(&row.get::<_, String>(8)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    8,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        summary_text: row.get(9)?,
                        raw_payload_json: row.get(10)?,
                        status: ApprovalRequestStatus::from_str(&row.get::<_, String>(11)?)
                            .map_err(|err| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    11,
                                    Type::Text,
                                    Box::new(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        err,
                                    )),
                                )
                            })?,
                        requested_at_ms: row.get(12)?,
                        resolved_at_ms: row.get(13)?,
                        resolved_by_sender_id: row.get(14)?,
                        telegram_message_id: row.get(15)?,
                    },
                    chat_id: row.get(16)?,
                })
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        for notification in &pending {
            let now = Utc::now().timestamp_millis();
            self.with_conn(|conn| {
                conn.execute(
                    r#"
                    UPDATE approval_requests
                    SET status = 'invalidated',
                        resolved_at_ms = ?2,
                        resolved_by_sender_id = NULL
                    WHERE request_id = ?1 AND status IN ('pending', 'dispatching')
                    "#,
                    params![notification.request.request_id, now],
                )
            })?;
            self.with_conn(|conn| {
                conn.execute(
                    r#"
                    UPDATE lanes
                    SET state = 'waiting_reply',
                        codex_session_id = NULL,
                        waiting_since_ms = NULL
                    WHERE lane_id = ?1
                      AND state != 'failed'
                    "#,
                    params![notification.request.lane_id],
                )
            })?;
            self.with_conn(|conn| {
                conn.execute(
                    r#"
                    UPDATE runs
                    SET ended_at_ms = COALESCE(ended_at_ms, ?2),
                        completion_reason = CASE
                            WHEN completion_reason = 'failed' THEN completion_reason
                            ELSE 'restarted'
                        END,
                        approval_pending = 0
                    WHERE run_id = ?1
                    "#,
                    params![notification.request.run_id, now],
                )
            })?;
        }
        Ok(pending)
    }

    pub fn fail_resolving_approval_request(
        &self,
        request_id: &str,
        lane_id: &str,
        run_id: &str,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = 'invalidated',
                    resolved_at_ms = ?2,
                    resolved_by_sender_id = NULL
                WHERE request_id = ?1 AND status = 'resolving'
                "#,
                params![request_id, now],
            )
        })?;
        if updated == 0 {
            return Ok(false);
        }
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET state = 'failed',
                    codex_session_id = NULL,
                    waiting_since_ms = NULL
                WHERE lane_id = ?1
                "#,
                params![lane_id],
            )
        })?;
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE runs
                SET ended_at_ms = COALESCE(ended_at_ms, ?2),
                    completion_reason = 'failed',
                    approval_pending = 0
                WHERE run_id = ?1
                "#,
                params![run_id, now],
            )
        })?;
        Ok(true)
    }

    pub fn expire_approval_request(
        &self,
        request_id: &str,
        lane_id: &str,
        run_id: &str,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        let updated = self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE approval_requests
                SET status = 'timed_out',
                    resolved_at_ms = ?2,
                    resolved_by_sender_id = NULL
                WHERE request_id = ?1 AND status IN ('pending', 'dispatching', 'resolving')
                "#,
                params![request_id, now],
            )
        })?;
        if updated == 0 {
            return Ok(false);
        }
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE lanes
                SET state = 'failed',
                    codex_session_id = NULL,
                    waiting_since_ms = NULL
                WHERE lane_id = ?1
                "#,
                params![lane_id],
            )
        })?;
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE runs
                SET ended_at_ms = COALESCE(ended_at_ms, ?2),
                    completion_reason = 'failed',
                    approval_pending = 0
                WHERE run_id = ?1
                "#,
                params![run_id, now],
            )
        })?;
        Ok(true)
    }

    pub fn upsert_codex_thread_binding(
        &self,
        binding: NewCodexThreadBinding,
    ) -> Result<CodexThreadBinding> {
        let now = Utc::now().timestamp_millis();
        let chat_id = binding.chat_id;
        let thread_key = binding.thread_key.clone();
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO codex_thread_bindings(
                    chat_id, thread_key, codex_thread_id, workspace_id, title, cwd, model,
                    codex_updated_at, created_at_ms, updated_at_ms
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                ON CONFLICT(chat_id, thread_key) DO UPDATE SET
                    codex_thread_id = excluded.codex_thread_id,
                    workspace_id = excluded.workspace_id,
                    title = excluded.title,
                    cwd = excluded.cwd,
                    model = excluded.model,
                    codex_updated_at = excluded.codex_updated_at,
                    updated_at_ms = excluded.updated_at_ms
                "#,
                params![
                    chat_id,
                    thread_key,
                    binding.codex_thread_id,
                    binding.workspace_id,
                    binding.title,
                    binding.cwd,
                    binding.model,
                    binding.codex_updated_at,
                    now,
                ],
            )
        })?;
        self.find_codex_thread_binding(chat_id, &thread_key)?
            .ok_or_else(|| anyhow!("codex thread binding was not saved"))
    }

    pub fn find_codex_thread_binding(
        &self,
        chat_id: i64,
        thread_key: &str,
    ) -> Result<Option<CodexThreadBinding>> {
        self.with_conn(|conn| {
            conn.query_row(
                r#"
                SELECT chat_id, thread_key, codex_thread_id, workspace_id, title, cwd, model,
                       codex_updated_at, created_at_ms, updated_at_ms
                FROM codex_thread_bindings
                WHERE chat_id = ?1 AND thread_key = ?2
                "#,
                params![chat_id, thread_key],
                |row| {
                    Ok(CodexThreadBinding {
                        chat_id: row.get(0)?,
                        thread_key: row.get(1)?,
                        codex_thread_id: row.get(2)?,
                        workspace_id: row.get(3)?,
                        title: row.get(4)?,
                        cwd: row.get(5)?,
                        model: row.get(6)?,
                        codex_updated_at: row.get(7)?,
                        created_at_ms: row.get(8)?,
                        updated_at_ms: row.get(9)?,
                    })
                },
            )
            .optional()
        })
    }

    pub fn insert_message(
        &self,
        lane_id: &str,
        run_id: Option<&str>,
        direction: &str,
        message_kind: &str,
        telegram_message_id: Option<i64>,
        body_text: Option<&str>,
        payload_json: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO messages(
                    lane_id, run_id, direction, message_kind, telegram_message_id, body_text, payload_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    lane_id,
                    run_id,
                    direction,
                    message_kind,
                    telegram_message_id,
                    body_text,
                    payload_json,
                ],
            )
        })?;
        Ok(())
    }

    fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> rusqlite::Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("sqlite mutex poisoned"))?;
        f(&guard).context("sqlite operation failed")
    }
}

fn ensure_column_exists(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    alter_sql: &str,
) -> rusqlite::Result<bool> {
    let exists = column_exists(conn, table_name, column_name)?;
    if !exists {
        conn.execute_batch(alter_sql)?;
        return Ok(true);
    }
    Ok(false)
}

fn column_exists(conn: &Connection, table_name: &str, column_name: &str) -> rusqlite::Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let exists = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .any(|existing| existing == column_name);
    Ok(exists)
}

fn mode_to_str(mode: LaneMode) -> &'static str {
    match mode {
        LaneMode::AwaitReply => "await_reply",
        LaneMode::Infinite => "infinite",
        LaneMode::CompletionChecks => "completion_checks",
        LaneMode::MaxTurns => "max_turns",
    }
}

fn mode_from_str(value: &str) -> std::result::Result<LaneMode, String> {
    match value {
        "await_reply" => Ok(LaneMode::AwaitReply),
        "infinite" => Ok(LaneMode::Infinite),
        "completion_checks" => Ok(LaneMode::CompletionChecks),
        "max_turns" => Ok(LaneMode::MaxTurns),
        other => Err(format!("unknown lane mode: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::{TempDir, tempdir};

    fn temp_store() -> (TempDir, Store) {
        let dir = tempdir().expect("temp dir");
        let store = Store::open(dir.path().join("store.db")).expect("store");
        (dir, store)
    }

    #[test]
    fn sync_config_authorized_senders_revokes_removed_config_sender() {
        let (_dir, store) = temp_store();

        store
            .sync_config_authorized_senders(&[11, 22])
            .expect("initial config sync");
        assert!(store.is_authorized_sender(11).expect("sender 11 lookup"));
        assert!(store.is_authorized_sender(22).expect("sender 22 lookup"));

        store
            .sync_config_authorized_senders(&[22])
            .expect("second config sync");
        assert!(!store.is_authorized_sender(11).expect("sender 11 lookup"));
        assert!(store.is_authorized_sender(22).expect("sender 22 lookup"));
    }

    #[test]
    fn sync_config_authorized_senders_keeps_sender_that_was_repaired_as_paired() {
        let (_dir, store) = temp_store();

        store
            .sync_config_authorized_senders(&[11])
            .expect("initial config sync");
        store
            .upsert_authorized_sender(AuthorizedSender {
                sender_id: 11,
                platform: "telegram".to_owned(),
                display_name: None,
                status: "active".to_owned(),
                approved_at_ms: Utc::now().timestamp_millis(),
                source: "paired".to_owned(),
            })
            .expect("pair sender");

        store
            .sync_config_authorized_senders(&[])
            .expect("remove config sender");
        assert!(store.is_authorized_sender(11).expect("sender 11 lookup"));
    }

    #[test]
    fn migrate_marks_legacy_authorized_senders_as_config_backed() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("legacy.db");
        let conn = Connection::open(&db_path).expect("legacy connection");
        conn.execute_batch(
            r#"
            CREATE TABLE authorized_senders (
                sender_id INTEGER PRIMARY KEY,
                platform TEXT NOT NULL,
                display_name TEXT,
                status TEXT NOT NULL,
                approved_at_ms INTEGER NOT NULL
            );
            INSERT INTO authorized_senders(sender_id, platform, display_name, status, approved_at_ms)
            VALUES (11, 'telegram', NULL, 'active', 1);
            "#,
        )
        .expect("seed legacy schema");
        drop(conn);

        let store = Store::open(&db_path).expect("migrated store");
        let sender = store
            .list_active_authorized_senders()
            .expect("list senders")
            .into_iter()
            .find(|sender| sender.sender_id == 11)
            .expect("legacy sender");
        assert_eq!(sender.source, "config");

        store
            .sync_config_authorized_senders(&[])
            .expect("sync empty config");
        assert!(!store.is_authorized_sender(11).expect("sender lookup"));
    }

    #[test]
    fn active_authorized_sender_returns_metadata_for_paired_sender() {
        let (_dir, store) = temp_store();
        let now = Utc::now().timestamp_millis();
        store
            .upsert_authorized_sender(AuthorizedSender {
                sender_id: 33,
                platform: "telegram".to_owned(),
                display_name: Some("paired".to_owned()),
                status: "active".to_owned(),
                approved_at_ms: now,
                source: "paired".to_owned(),
            })
            .expect("pair sender");

        let sender = store
            .active_authorized_sender(33)
            .expect("sender lookup")
            .expect("paired sender");
        assert_eq!(sender.sender_id, 33);
        assert_eq!(sender.source, "paired");
        assert_eq!(sender.display_name.as_deref(), Some("paired"));
    }

    #[test]
    fn store_open_expires_legacy_app_server_requests_without_transport_id() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("store.db");
        let store = Store::open(&db_path).expect("store");
        let lane = store
            .get_or_create_lane(90, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "legacy".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                1,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("legacy-session"),
            )
            .expect("lane state");
        store
            .with_conn(|conn| {
                conn.execute(
                    r#"
                    INSERT INTO approval_requests(
                        request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                        transport, request_kind, summary_text, raw_payload_json, status, requested_at_ms,
                        resolved_at_ms, resolved_by_sender_id, telegram_message_id
                    )
                    VALUES (?1, '', ?2, ?3, ?4, ?5, ?6, 'app_server', 'command_execution', ?7, '{}', 'pending', ?8, NULL, NULL, 55)
                    "#,
                    params![
                        "legacy-request",
                        lane.lane_id,
                        run.run_id,
                        "thread-legacy",
                        "turn-legacy",
                        "item-legacy",
                        "legacy",
                        Utc::now().timestamp_millis()
                    ],
                )
            })
            .expect("insert legacy approval");
        drop(store);

        let reopened = Store::open(&db_path).expect("reopen store");
        let request = reopened
            .find_approval_request("legacy-request")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::TimedOut);
        assert_eq!(request.telegram_message_id, Some(55));

        let lane = reopened
            .find_lane(90, "dm")
            .expect("find lane")
            .expect("lane");
        assert_eq!(lane.state, LaneState::Failed);
        assert!(lane.codex_session_id.is_none());
    }

    #[test]
    fn store_open_keeps_legacy_exec_requests_without_transport_id() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("store.db");
        let store = Store::open(&db_path).expect("store");
        let lane = store
            .get_or_create_lane(91, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "legacy-exec".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                1,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("legacy-exec-session"),
            )
            .expect("lane state");
        store
            .with_conn(|conn| {
                conn.execute(
                    r#"
                    INSERT INTO approval_requests(
                        request_id, transport_request_id, lane_id, run_id, thread_id, turn_id, item_id,
                        transport, request_kind, summary_text, raw_payload_json, status, requested_at_ms,
                        resolved_at_ms, resolved_by_sender_id, telegram_message_id
                    )
                    VALUES (?1, '', ?2, ?3, ?4, ?5, ?6, 'app_server', 'command_execution', ?7, '{}', 'pending', ?8, NULL, NULL, NULL)
                    "#,
                    params![
                        "legacy-exec-request",
                        lane.lane_id,
                        run.run_id,
                        "thread-legacy-exec",
                        "turn-legacy-exec",
                        "item-legacy-exec",
                        "legacy-exec",
                        Utc::now().timestamp_millis()
                    ],
                )
            })
            .expect("insert legacy exec approval");
        drop(store);

        let reopened = Store::open(&db_path).expect("reopen store");
        let request = reopened
            .find_approval_request("legacy-exec-request")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::Pending);

        let lane = reopened
            .find_lane(91, "dm")
            .expect("find lane")
            .expect("lane");
        assert_eq!(lane.state, LaneState::NeedsLocalApproval);
        assert_eq!(
            lane.codex_session_id.as_deref(),
            Some("legacy-exec-session")
        );
    }

    #[test]
    fn restart_invalidation_does_not_override_failed_lane_or_run() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(92, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "mixed".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                2,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("mixed-session"),
            )
            .expect("lane state");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-mixed-resolving".to_owned(),
                transport_request_id: "req-mixed-resolving".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: run.run_id.clone(),
                thread_id: "thread-mixed".to_owned(),
                turn_id: "turn-mixed".to_owned(),
                item_id: "item-resolving".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "resolving".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert resolving request");
        assert!(
            store
                .begin_approval_resolution("req-mixed-resolving", 99)
                .expect("move to resolving")
        );
        assert!(
            store
                .fail_resolving_approval_request("req-mixed-resolving", &lane.lane_id, &run.run_id,)
                .expect("fail resolving request")
        );

        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-mixed-pending".to_owned(),
                transport_request_id: "req-mixed-pending".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: run.run_id.clone(),
                thread_id: "thread-mixed".to_owned(),
                turn_id: "turn-mixed".to_owned(),
                item_id: "item-pending".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "pending".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert pending request");

        let invalidated = store
            .invalidate_pending_approval_notifications_for_restart(
                ApprovalRequestTransport::AppServer,
            )
            .expect("invalidate restart approvals");
        assert_eq!(invalidated.len(), 1);
        assert_eq!(invalidated[0].request.request_id, "req-mixed-pending");

        let lane = store.find_lane(92, "dm").expect("find lane").expect("lane");
        assert_eq!(lane.state, LaneState::Failed);

        let completion_reason: Option<String> = store
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT completion_reason FROM runs WHERE run_id = ?1",
                    params![run.run_id],
                    |row| row.get(0),
                )
            })
            .expect("query run");
        assert_eq!(completion_reason.as_deref(), Some("failed"));
    }

    #[test]
    fn find_lane_returns_lane_for_chat_and_thread() {
        let (_dir, store) = temp_store();
        let created = store
            .get_or_create_lane(42, "555", "workspace", LaneMode::AwaitReply, 0)
            .expect("lane");

        let fetched = store
            .find_lane(42, "555")
            .expect("query")
            .expect("lane exists");

        assert_eq!(fetched.lane_id, created.lane_id);
        assert_eq!(fetched.chat_id, 42);
        assert_eq!(fetched.thread_key, "555");
    }

    #[test]
    fn codex_thread_binding_round_trip_uses_chat_and_thread_key() {
        let (_dir, store) = temp_store();

        let saved = store
            .upsert_codex_thread_binding(NewCodexThreadBinding {
                chat_id: 42,
                thread_key: "dm".to_owned(),
                codex_thread_id: "thread-1".to_owned(),
                workspace_id: "main".to_owned(),
                title: Some("Fix tests".to_owned()),
                cwd: Some("C:/workspace".to_owned()),
                model: Some("gpt-5.4".to_owned()),
                codex_updated_at: Some("2026-04-22T00:00:00Z".to_owned()),
            })
            .expect("save binding");

        let fetched = store
            .find_codex_thread_binding(42, "dm")
            .expect("find binding")
            .expect("binding exists");

        assert_eq!(fetched, saved);
        assert_eq!(fetched.codex_thread_id, "thread-1");
        assert_eq!(fetched.workspace_id, "main");
        assert!(fetched.created_at_ms <= fetched.updated_at_ms);
    }

    #[test]
    fn codex_thread_binding_upsert_replaces_thread_for_same_chat() {
        let (_dir, store) = temp_store();

        store
            .upsert_codex_thread_binding(NewCodexThreadBinding {
                chat_id: 42,
                thread_key: "dm".to_owned(),
                codex_thread_id: "thread-1".to_owned(),
                workspace_id: "main".to_owned(),
                title: None,
                cwd: None,
                model: None,
                codex_updated_at: None,
            })
            .expect("save first binding");
        let updated = store
            .upsert_codex_thread_binding(NewCodexThreadBinding {
                chat_id: 42,
                thread_key: "dm".to_owned(),
                codex_thread_id: "thread-2".to_owned(),
                workspace_id: "other".to_owned(),
                title: Some("Continue".to_owned()),
                cwd: None,
                model: None,
                codex_updated_at: None,
            })
            .expect("update binding");

        assert_eq!(updated.codex_thread_id, "thread-2");
        assert_eq!(updated.workspace_id, "other");
        assert_eq!(updated.title.as_deref(), Some("Continue"));
        assert!(updated.created_at_ms <= updated.updated_at_ms);
    }

    #[test]
    fn clear_lane_session_resets_session_fields_and_state() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(42, "555", "workspace", LaneMode::MaxTurns, 2)
            .expect("lane");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("state update");
        store
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE lanes SET extra_turn_budget = 2 WHERE lane_id = ?1",
                    params![&lane.lane_id],
                )
            })
            .expect("budget update");

        store
            .clear_lane_session(&lane.lane_id)
            .expect("session clear");

        let lane = store
            .find_lane(42, "555")
            .expect("query")
            .expect("lane exists");
        assert_eq!(lane.state, LaneState::Idle);
        assert_eq!(lane.codex_session_id, None);
        assert_eq!(lane.extra_turn_budget, 2);
        assert_eq!(lane.waiting_since_ms, None);
        assert_eq!(lane.mode, LaneMode::MaxTurns);
    }

    #[test]
    fn fail_lane_clears_session_and_waiting_state() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(43, "556", "workspace", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-2"))
            .expect("state update");

        store.fail_lane(&lane.lane_id).expect("lane failure");

        let lane = store
            .find_lane(43, "556")
            .expect("query")
            .expect("lane exists");
        assert_eq!(lane.state, LaneState::Failed);
        assert_eq!(lane.codex_session_id, None);
        assert_eq!(lane.waiting_since_ms, None);
    }

    #[test]
    fn update_lane_mode_changes_mode_and_budget() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(42, "555", "workspace", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("state update");

        store
            .update_lane_mode(&lane.lane_id, LaneMode::MaxTurns, 5)
            .expect("mode update");

        let lane = store
            .find_lane(42, "555")
            .expect("query")
            .expect("lane exists");
        assert_eq!(lane.mode, LaneMode::MaxTurns);
        assert_eq!(lane.extra_turn_budget, 5);
        assert_eq!(lane.state, LaneState::WaitingReply);
        assert_eq!(lane.codex_session_id.as_deref(), Some("session-1"));
        assert!(lane.waiting_since_ms.is_some());
    }

    #[test]
    fn create_lane_uses_requested_budget_for_max_turns() {
        let (_dir, store) = temp_store();

        let lane = store
            .get_or_create_lane(42, "555", "workspace", LaneMode::MaxTurns, 4)
            .expect("lane");

        assert_eq!(lane.extra_turn_budget, 4);
    }

    #[test]
    fn update_lane_workspace_changes_workspace_and_clears_session() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(42, "555", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("state update");

        store
            .update_lane_workspace(&lane.lane_id, "docs")
            .expect("workspace update");

        let lane = store
            .find_lane(42, "555")
            .expect("query")
            .expect("lane exists");
        assert_eq!(lane.workspace_id, "docs");
        assert_eq!(lane.state, LaneState::Idle);
        assert_eq!(lane.codex_session_id, None);
        assert_eq!(lane.waiting_since_ms, None);
    }

    #[test]
    fn update_lane_state_to_needs_local_approval_keeps_session_and_clears_waiting() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(42, "555", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .update_lane_state(&lane.lane_id, LaneState::WaitingReply, Some("session-1"))
            .expect("state update");

        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-1"),
            )
            .expect("approval state update");

        let lane = store
            .find_lane(42, "555")
            .expect("query")
            .expect("lane exists");
        assert_eq!(lane.state, LaneState::NeedsLocalApproval);
        assert_eq!(lane.codex_session_id.as_deref(), Some("session-1"));
        assert_eq!(lane.waiting_since_ms, None);
    }

    #[test]
    fn finish_run_persists_approval_counts() {
        let (_dir, store) = temp_store();
        let run = store
            .insert_run(NewRun {
                lane_id: "lane-1".to_owned(),
                run_kind: "start".to_owned(),
            })
            .expect("run");

        store
            .finish_run(&run.run_id, None, "needs_local_approval", true, 2, 1)
            .expect("finish run");

        let (approval_pending, request_count, resolved_count): (i64, i64, i64) = store
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT approval_pending, approval_request_count, approval_resolved_count FROM runs WHERE run_id = ?1",
                    params![&run.run_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
            })
            .expect("read run");
        assert_eq!(approval_pending, 1);
        assert_eq!(request_count, 2);
        assert_eq!(resolved_count, 1);
    }

    #[test]
    fn approval_request_round_trip_supports_resolve_and_message_tracking() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(10, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-1".to_owned(),
                transport_request_id: "req-1".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "command approval".to_owned(),
                raw_payload_json: "{\"kind\":\"command\"}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert approval request");
        assert!(
            store
                .mark_approval_request_pending("req-1", 55)
                .expect("mark pending")
        );

        let pending = store
            .list_pending_approval_requests_for_lane(&lane.lane_id)
            .expect("pending approvals");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].status, ApprovalRequestStatus::Pending);
        assert!(
            store
                .begin_approval_resolution("req-1", 99)
                .expect("move to resolving")
        );
        let updated = store
            .resolve_approval_request("req-1", ApprovalRequestStatus::Approved, 99)
            .expect("resolve approval request");
        assert!(updated);

        let request = store
            .find_approval_request("req-1")
            .expect("find approval request")
            .expect("request exists");
        assert_eq!(request.status, ApprovalRequestStatus::Approved);
        assert_eq!(request.telegram_message_id, Some(55));
        assert_eq!(request.resolved_by_sender_id, Some(99));
        assert!(request.resolved_at_ms.is_some());

        let second_update = store
            .resolve_approval_request("req-1", ApprovalRequestStatus::Declined, 99)
            .expect("second resolve");
        assert!(!second_update);
    }

    #[test]
    fn resolve_approval_request_finalizes_resolving_state() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(11, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-resolve".to_owned(),
                transport_request_id: "req-resolve".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-resolve".to_owned(),
                thread_id: "thread-resolve".to_owned(),
                turn_id: "turn-resolve".to_owned(),
                item_id: "item-resolve".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "command approval".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert approval request");
        assert!(
            store
                .mark_approval_request_pending("req-resolve", 56)
                .expect("mark pending")
        );
        assert!(
            store
                .begin_approval_resolution("req-resolve", 77)
                .expect("move to resolving")
        );

        let finalized = store
            .resolve_approval_request("req-resolve", ApprovalRequestStatus::Approved, 77)
            .expect("finalize approval");
        assert!(finalized);

        let request = store
            .find_approval_request("req-resolve")
            .expect("find approval request")
            .expect("request exists");
        assert_eq!(request.status, ApprovalRequestStatus::Approved);
    }

    #[test]
    fn pending_approval_notifications_join_lane_chat_id() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(24, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-restore".to_owned(),
                transport_request_id: "req-restore".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "command approval".to_owned(),
                raw_payload_json: "{\"kind\":\"command\"}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert approval request");

        let notifications = store
            .list_pending_approval_notifications()
            .expect("notifications");
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].chat_id, 24);
        assert_eq!(notifications[0].request.request_id, "req-restore");
    }

    #[test]
    fn invalidate_approval_request_preserves_history_and_hides_from_pending_views() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(24, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-invalidated".to_owned(),
                transport_request_id: "req-invalidated".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-1".to_owned(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "command approval".to_owned(),
                raw_payload_json: "{\"kind\":\"command\"}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert approval request");
        assert!(
            store
                .mark_approval_request_pending("req-invalidated", 55)
                .expect("mark pending")
        );

        let invalidated = store
            .invalidate_approval_request("req-invalidated")
            .expect("invalidate approval request");
        assert!(invalidated);

        let request = store
            .find_approval_request("req-invalidated")
            .expect("find")
            .expect("approval request should remain");
        assert_eq!(request.status, ApprovalRequestStatus::Invalidated);
        assert_eq!(request.telegram_message_id, Some(55));

        let pending_for_lane = store
            .list_pending_approval_requests_for_lane(&lane.lane_id)
            .expect("pending approvals");
        assert!(pending_for_lane.is_empty());

        let notifications = store
            .list_pending_approval_notifications()
            .expect("pending notifications");
        assert!(notifications.is_empty());
    }

    #[test]
    fn prepare_approval_request_for_dispatch_reactivates_invalidated_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(26, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-redispatch".to_owned(),
                transport_request_id: "req-redispatch".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-old".to_owned(),
                thread_id: "thread-old".to_owned(),
                turn_id: "turn-old".to_owned(),
                item_id: "item-old".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "old".to_owned(),
                raw_payload_json: "{\"old\":true}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert request");
        assert!(
            store
                .mark_approval_request_pending("req-redispatch", 55)
                .expect("mark pending")
        );
        store
            .invalidate_approval_request("req-redispatch")
            .expect("invalidate");

        let updated = store
            .prepare_approval_request_for_dispatch(NewApprovalRequest {
                request_id: "req-redispatch".to_owned(),
                transport_request_id: "req-redispatch".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-new".to_owned(),
                thread_id: "thread-new".to_owned(),
                turn_id: "turn-new".to_owned(),
                item_id: "item-new".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::FileChange,
                summary_text: "new".to_owned(),
                raw_payload_json: "{\"new\":true}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("prepare dispatch");
        assert!(updated);

        let request = store
            .find_approval_request("req-redispatch")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::Dispatching);
        assert_eq!(request.run_id, "run-new");
        assert_eq!(request.thread_id, "thread-new");
        assert_eq!(request.turn_id, "turn-new");
        assert_eq!(request.item_id, "item-new");
        assert_eq!(request.request_kind, ApprovalRequestKind::FileChange);
        assert_eq!(request.summary_text, "new");
        assert_eq!(request.raw_payload_json, "{\"new\":true}");
        assert_eq!(request.resolved_at_ms, None);
        assert_eq!(request.resolved_by_sender_id, None);
        assert_eq!(request.telegram_message_id, None);
    }

    #[test]
    fn prepare_approval_request_for_dispatch_reactivates_timed_out_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(39, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "timed-out".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                1,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-timed-out"),
            )
            .expect("lane state");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-reopen-timeout".to_owned(),
                transport_request_id: "transport-timeout-old".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: run.run_id.clone(),
                thread_id: "thread-old".to_owned(),
                turn_id: "turn-old".to_owned(),
                item_id: "item-old".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "old".to_owned(),
                raw_payload_json: "{\"old\":true}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert request");
        assert!(
            store
                .expire_approval_request("req-reopen-timeout", &lane.lane_id, &run.run_id)
                .expect("expire request")
        );

        let updated = store
            .prepare_approval_request_for_dispatch(NewApprovalRequest {
                request_id: "req-reopen-timeout".to_owned(),
                transport_request_id: "transport-timeout-new".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-new".to_owned(),
                thread_id: "thread-new".to_owned(),
                turn_id: "turn-new".to_owned(),
                item_id: "item-new".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::Permissions,
                summary_text: "new".to_owned(),
                raw_payload_json: "{\"new\":true}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("prepare dispatch");
        assert!(updated);

        let request = store
            .find_approval_request("req-reopen-timeout")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::Dispatching);
        assert_eq!(request.transport_request_id, "transport-timeout-new");
        assert_eq!(request.run_id, "run-new");
        assert_eq!(request.request_kind, ApprovalRequestKind::Permissions);
    }

    #[test]
    fn prepare_approval_request_for_dispatch_skips_live_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(27, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-live".to_owned(),
                transport_request_id: "req-live".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-live".to_owned(),
                thread_id: "thread-live".to_owned(),
                turn_id: "turn-live".to_owned(),
                item_id: "item-live".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "live".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert request");

        let updated = store
            .prepare_approval_request_for_dispatch(NewApprovalRequest {
                request_id: "req-live".to_owned(),
                transport_request_id: "req-live".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-next".to_owned(),
                thread_id: "thread-next".to_owned(),
                turn_id: "turn-next".to_owned(),
                item_id: "item-next".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::Permissions,
                summary_text: "next".to_owned(),
                raw_payload_json: "{\"next\":true}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("prepare dispatch");
        assert!(!updated);

        let request = store
            .find_approval_request("req-live")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::Pending);
        assert_eq!(request.run_id, "run-live");
        assert_eq!(request.thread_id, "thread-live");
    }

    #[test]
    fn expire_pending_approval_notifications_marks_request_timed_out_and_lane_failed() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(24, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "primary".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                1,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-1"),
            )
            .expect("lane state");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-expire".to_owned(),
                transport_request_id: "req-expire".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: run.run_id.clone(),
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-1".to_owned(),
                item_id: "item-1".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "command approval".to_owned(),
                raw_payload_json: "{\"kind\":\"command\"}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert approval request");

        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, i64::MAX)
            .expect("expire approvals");
        assert_eq!(expired.len(), 1);

        let request = store
            .find_approval_request("req-expire")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::TimedOut);
        assert!(request.resolved_at_ms.is_some());

        let lane = store.find_lane(24, "dm").expect("find lane").expect("lane");
        assert_eq!(lane.state, LaneState::Failed);
        assert!(lane.codex_session_id.is_none());

        let (approval_pending, completion_reason): (i64, Option<String>) = store
            .with_conn(|conn| {
                conn.query_row(
                    "SELECT approval_pending, completion_reason FROM runs WHERE run_id = ?1",
                    params![run.run_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .expect("query run");
        assert_eq!(approval_pending, 0);
        assert_eq!(completion_reason.as_deref(), Some("failed"));
    }

    #[test]
    fn expire_pending_approval_notifications_keeps_recent_requests() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(25, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-recent".to_owned(),
                transport_request_id: "req-recent".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-recent".to_owned(),
                thread_id: "thread-recent".to_owned(),
                turn_id: "turn-recent".to_owned(),
                item_id: "item-recent".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "recent".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert approval request");

        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, 0)
            .expect("expire approvals");
        assert!(expired.is_empty());

        let request = store
            .find_approval_request("req-recent")
            .expect("find request")
            .expect("request");
        assert_eq!(request.status, ApprovalRequestStatus::Pending);
    }

    #[test]
    fn expire_pending_approval_notifications_only_targets_requested_transport() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(30, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-app".to_owned(),
                transport_request_id: "req-app".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-app".to_owned(),
                thread_id: "thread-app".to_owned(),
                turn_id: "turn-app".to_owned(),
                item_id: "item-app".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "app".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert app-server request");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-exec".to_owned(),
                transport_request_id: "req-exec".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-exec".to_owned(),
                thread_id: "thread-exec".to_owned(),
                turn_id: "turn-exec".to_owned(),
                item_id: "item-exec".to_owned(),
                transport: ApprovalRequestTransport::Exec,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "exec".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert exec request");

        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, i64::MAX)
            .expect("expire app-server approvals");
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].request.request_id, "req-app");

        let app_request = store
            .find_approval_request("req-app")
            .expect("find app request")
            .expect("app request");
        assert_eq!(app_request.status, ApprovalRequestStatus::TimedOut);

        let exec_request = store
            .find_approval_request("req-exec")
            .expect("find exec request")
            .expect("exec request");
        assert_eq!(exec_request.status, ApprovalRequestStatus::Pending);
    }

    #[test]
    fn expire_pending_approval_notifications_also_cleans_dispatching_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(31, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-dispatching"),
            )
            .expect("lane state");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-dispatching".to_owned(),
                transport_request_id: "req-dispatching".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-dispatching".to_owned(),
                thread_id: "thread-dispatching".to_owned(),
                turn_id: "turn-dispatching".to_owned(),
                item_id: "item-dispatching".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "dispatching".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert dispatching request");

        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, i64::MAX)
            .expect("expire app-server approvals");
        assert_eq!(expired.len(), 1);
        assert_eq!(
            expired[0].request.status,
            ApprovalRequestStatus::Dispatching
        );

        let request = store
            .find_approval_request("req-dispatching")
            .expect("find dispatching request")
            .expect("dispatching request");
        assert_eq!(request.status, ApprovalRequestStatus::TimedOut);
    }

    #[test]
    fn fail_resolving_approval_request_invalidates_row_and_fails_lane() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(32, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        let run = store
            .insert_run(NewRun {
                lane_id: lane.lane_id.clone(),
                run_kind: "resolving".to_owned(),
            })
            .expect("run");
        store
            .finish_run(
                &run.run_id,
                None,
                LaneState::NeedsLocalApproval.as_str(),
                true,
                1,
                0,
            )
            .expect("finish run");
        store
            .update_lane_state(
                &lane.lane_id,
                LaneState::NeedsLocalApproval,
                Some("session-resolving"),
            )
            .expect("lane state");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-resolving".to_owned(),
                transport_request_id: "req-resolving".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: run.run_id.clone(),
                thread_id: "thread-resolving".to_owned(),
                turn_id: "turn-resolving".to_owned(),
                item_id: "item-resolving".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "resolving".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert resolving request");
        assert!(
            store
                .begin_approval_resolution("req-resolving", 90)
                .expect("move to resolving")
        );

        let failed = store
            .fail_resolving_approval_request("req-resolving", &lane.lane_id, &run.run_id)
            .expect("fail resolving approval");
        assert!(failed);

        let request = store
            .find_approval_request("req-resolving")
            .expect("find resolving request")
            .expect("resolving request");
        assert_eq!(request.status, ApprovalRequestStatus::Invalidated);

        let lane = store.find_lane(32, "dm").expect("find lane").expect("lane");
        assert_eq!(lane.state, LaneState::Failed);
        assert!(lane.codex_session_id.is_none());
    }

    #[test]
    fn expire_pending_approval_notifications_keeps_recently_resolving_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(33, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-resolving-recent".to_owned(),
                transport_request_id: "req-resolving-recent".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-resolving-recent".to_owned(),
                thread_id: "thread-resolving-recent".to_owned(),
                turn_id: "turn-resolving-recent".to_owned(),
                item_id: "item-resolving-recent".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "resolving".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert resolving request");
        assert!(
            store
                .begin_approval_resolution("req-resolving-recent", 91)
                .expect("move to resolving")
        );
        store
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE approval_requests SET requested_at_ms = 0, resolved_at_ms = ?2 WHERE request_id = ?1",
                    params![
                        "req-resolving-recent",
                        Utc::now().timestamp_millis() + 60_000
                    ],
                )
            })
            .expect("age original request");

        let cutoff = Utc::now().timestamp_millis() - 1;
        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, cutoff)
            .expect("expire approvals");
        assert!(expired.is_empty());

        let request = store
            .find_approval_request("req-resolving-recent")
            .expect("find resolving request")
            .expect("resolving request");
        assert_eq!(request.status, ApprovalRequestStatus::Resolving);
    }

    #[test]
    fn list_recent_resolving_approval_notifications_keeps_rows_resolving() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(34, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-requeue".to_owned(),
                transport_request_id: "req-requeue".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-requeue".to_owned(),
                thread_id: "thread-requeue".to_owned(),
                turn_id: "turn-requeue".to_owned(),
                item_id: "item-requeue".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "requeue".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert request");
        assert!(
            store
                .begin_approval_resolution("req-requeue", 92)
                .expect("move to resolving")
        );

        let restored = store
            .list_recent_resolving_approval_notifications(ApprovalRequestTransport::AppServer, 0)
            .expect("list approvals");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].request.status, ApprovalRequestStatus::Resolving);

        let request = store
            .find_approval_request("req-requeue")
            .expect("find request")
            .expect("request exists");
        assert_eq!(request.status, ApprovalRequestStatus::Resolving);
        assert!(request.resolved_at_ms.is_some());
        assert_eq!(request.resolved_by_sender_id, Some(92));
    }

    #[test]
    fn list_recent_dispatching_approval_notifications_returns_recent_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(37, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-dispatch-recover".to_owned(),
                transport_request_id: "transport-dispatch-recover".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-dispatch-recover".to_owned(),
                thread_id: "thread-dispatch-recover".to_owned(),
                turn_id: "turn-dispatch-recover".to_owned(),
                item_id: "item-dispatch-recover".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "recover".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert request");

        let restored = store
            .list_recent_dispatching_approval_notifications(ApprovalRequestTransport::AppServer, 0)
            .expect("list approvals");
        assert_eq!(restored.len(), 1);
        assert_eq!(
            restored[0].request.status,
            ApprovalRequestStatus::Dispatching
        );
        assert_eq!(
            restored[0].request.transport_request_id,
            "transport-dispatch-recover"
        );
    }

    #[test]
    fn list_recent_dispatching_approval_notifications_skips_old_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(38, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-dispatch-old".to_owned(),
                transport_request_id: "transport-dispatch-old".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-dispatch-old".to_owned(),
                thread_id: "thread-dispatch-old".to_owned(),
                turn_id: "turn-dispatch-old".to_owned(),
                item_id: "item-dispatch-old".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "old".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Dispatching,
            })
            .expect("insert request");
        store
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE approval_requests SET requested_at_ms = 0 WHERE request_id = ?1",
                    params!["req-dispatch-old"],
                )
            })
            .expect("age request");

        let restored = store
            .list_recent_dispatching_approval_notifications(ApprovalRequestTransport::AppServer, 1)
            .expect("list approvals");
        assert!(restored.is_empty());
    }

    #[test]
    fn mark_approval_request_pending_from_resolving_preserves_dispatch_version() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(35, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-requeue-expiry".to_owned(),
                transport_request_id: "req-requeue-expiry".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-requeue-expiry".to_owned(),
                thread_id: "thread-requeue-expiry".to_owned(),
                turn_id: "turn-requeue-expiry".to_owned(),
                item_id: "item-requeue-expiry".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "requeue".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert request");
        assert!(
            store
                .begin_approval_resolution("req-requeue-expiry", 93)
                .expect("move to resolving")
        );
        store
            .with_conn(|conn| {
                conn.execute(
                    "UPDATE approval_requests SET requested_at_ms = 0 WHERE request_id = ?1",
                    params!["req-requeue-expiry"],
                )
            })
            .expect("age request");

        let restored = store
            .list_recent_resolving_approval_notifications(ApprovalRequestTransport::AppServer, 0)
            .expect("list approvals");
        assert_eq!(restored.len(), 1);
        assert!(
            store
                .mark_approval_request_pending("req-requeue-expiry", 105)
                .expect("mark pending")
        );

        let refreshed = store
            .find_approval_request("req-requeue-expiry")
            .expect("find request")
            .expect("request exists");
        let cutoff = refreshed.requested_at_ms - 1;
        let expired = store
            .expire_pending_approval_notifications(ApprovalRequestTransport::AppServer, cutoff)
            .expect("expire approvals");
        assert!(expired.is_empty());

        let request = store
            .find_approval_request("req-requeue-expiry")
            .expect("find request")
            .expect("request exists");
        assert_eq!(request.status, ApprovalRequestStatus::Pending);
        assert_eq!(request.requested_at_ms, 0);
        assert_eq!(request.telegram_message_id, Some(105));
    }

    #[test]
    fn mark_approval_request_pending_does_not_revive_resolved_rows() {
        let (_dir, store) = temp_store();
        let lane = store
            .get_or_create_lane(36, "dm", "main", LaneMode::AwaitReply, 0)
            .expect("lane");
        store
            .insert_approval_request(NewApprovalRequest {
                request_id: "req-stale".to_owned(),
                transport_request_id: "req-stale".to_owned(),
                lane_id: lane.lane_id.clone(),
                run_id: "run-stale".to_owned(),
                thread_id: "thread-stale".to_owned(),
                turn_id: "turn-stale".to_owned(),
                item_id: "item-stale".to_owned(),
                transport: ApprovalRequestTransport::AppServer,
                request_kind: ApprovalRequestKind::CommandExecution,
                summary_text: "stale".to_owned(),
                raw_payload_json: "{}".to_owned(),
                status: ApprovalRequestStatus::Pending,
            })
            .expect("insert request");
        assert!(
            store
                .begin_approval_resolution("req-stale", 94)
                .expect("move to resolving")
        );
        assert!(
            store
                .resolve_approval_request("req-stale", ApprovalRequestStatus::Approved, 94)
                .expect("resolve request")
        );

        let updated = store
            .mark_approval_request_pending("req-stale", 106)
            .expect("mark pending");
        assert!(!updated);

        let request = store
            .find_approval_request("req-stale")
            .expect("find request")
            .expect("request exists");
        assert_eq!(request.status, ApprovalRequestStatus::Approved);
        assert_eq!(request.telegram_message_id, None);
    }
}
