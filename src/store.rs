use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params, types::Type};
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

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("failed to open sqlite database")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.migrate()?;
        Ok(store)
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
                approved_at_ms INTEGER NOT NULL
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

            CREATE TABLE IF NOT EXISTS telegram_updates (
                update_id INTEGER PRIMARY KEY,
                chat_id INTEGER NOT NULL,
                sender_id INTEGER,
                update_kind TEXT NOT NULL,
                payload_json TEXT NOT NULL
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
        Ok(())
    }

    pub fn upsert_authorized_sender(&self, sender: AuthorizedSender) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO authorized_senders(sender_id, platform, display_name, status, approved_at_ms)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(sender_id) DO UPDATE SET
                    platform = excluded.platform,
                    display_name = excluded.display_name,
                    status = excluded.status,
                    approved_at_ms = excluded.approved_at_ms
                "#,
                params![
                    sender.sender_id,
                    sender.platform,
                    sender.display_name,
                    sender.status,
                    sender.approved_at_ms,
                ],
            )
        })?;
        Ok(())
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
            extra_turn_budget: 0,
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
    ) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.with_conn(|conn| {
            conn.execute(
                r#"
                UPDATE runs
                SET ended_at_ms = ?2,
                    exit_code = ?3,
                    completion_reason = ?4,
                    approval_pending = ?5
                WHERE run_id = ?1
                "#,
                params![
                    run_id,
                    now,
                    exit_code,
                    completion_reason,
                    approval_pending as i32
                ],
            )
        })?;
        Ok(())
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
