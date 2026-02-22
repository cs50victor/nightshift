use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::claude_transcript::{self, ClaudeEvent};
use crate::db::now_ms;

const CURSOR_TYPE: &str = "claude_jsonl_offset";
const SOURCE: &str = "claude_transcript";
const POLL_INTERVAL: Duration = Duration::from_secs(2);
static LIVE_INGEST_SUPERVISOR: OnceLock<Arc<IngestSupervisor>> = OnceLock::new();

#[derive(Debug, Clone)]
struct ClaudeMember {
    id: String,
    team_id: String,
    cwd: String,
}

#[derive(Default)]
struct IngestSupervisor {
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    wake: tokio::sync::Notify,
}

impl IngestSupervisor {
    fn new() -> Self {
        Self {
            task: Mutex::new(None),
            wake: tokio::sync::Notify::new(),
        }
    }

    async fn start_if_needed(self: &Arc<Self>, pool: SqlitePool) -> Result<()> {
        let has_members = has_active_claude_members(&pool).await?;
        if !has_members {
            return Ok(());
        }

        let mut guard = self
            .task
            .lock()
            .map_err(|_| anyhow!("claude ingest task mutex poisoned"))?;
        if let Some(handle) = guard.as_ref() {
            if !handle.is_finished() {
                return Ok(());
            }
        }

        let supervisor = Arc::clone(self);
        *guard = Some(tokio::spawn(async move {
            supervisor.run(pool).await;
        }));
        Ok(())
    }

    fn notify_member_set_changed(&self) {
        self.wake.notify_waiters();
    }

    #[cfg(test)]
    fn is_running(&self) -> bool {
        let Ok(guard) = self.task.lock() else {
            return false;
        };
        guard.as_ref().is_some_and(|handle| !handle.is_finished())
    }

    async fn run(self: Arc<Self>, pool: SqlitePool) {
        loop {
            let has_members = match has_active_claude_members(&pool).await {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!("claude transcript member check failed: {err:#}");
                    tokio::time::sleep(POLL_INTERVAL).await;
                    continue;
                }
            };
            if !has_members {
                break;
            }
            if let Err(err) = ingest_once(&pool).await {
                tracing::warn!("claude transcript ingest failed: {err:#}");
            }
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = self.wake.notified() => {}
            }
        }

        if let Ok(mut guard) = self.task.lock() {
            *guard = None;
        }
    }
}

fn global_supervisor() -> &'static Arc<IngestSupervisor> {
    LIVE_INGEST_SUPERVISOR.get_or_init(|| Arc::new(IngestSupervisor::new()))
}

pub async fn ensure_live_ingest(pool: SqlitePool) -> Result<()> {
    global_supervisor().start_if_needed(pool).await
}

pub fn notify_member_set_changed() {
    global_supervisor().notify_member_set_changed();
}

pub async fn ingest_once(pool: &SqlitePool) -> Result<()> {
    let members = active_claude_members(pool).await?;
    if members.is_empty() {
        return Ok(());
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("."));
    let projects_root = Path::new(&home).join(".claude").join("projects");
    if !projects_root.exists() {
        return Ok(());
    }

    for member in members {
        let dir = projects_root.join(cwd_to_projects_dir(&member.cwd));
        if !dir.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_jsonl_files(&dir, &mut files)?;
        for file in files {
            ingest_member_file(pool, &member, &file).await?;
        }
    }
    Ok(())
}

async fn active_claude_members(pool: &SqlitePool) -> Result<Vec<ClaudeMember>> {
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT m.id, m.team_id, m.cwd
         FROM members m
         JOIN teams t ON t.id = m.team_id
         WHERE m.backend_type = 'claude' AND m.removed_at_ms IS NULL AND t.archived_at_ms IS NULL",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, team_id, cwd)| ClaudeMember { id, team_id, cwd })
        .collect())
}

async fn has_active_claude_members(pool: &SqlitePool) -> Result<bool> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM members m
         JOIN teams t ON t.id = m.team_id
         WHERE m.backend_type = 'claude' AND m.removed_at_ms IS NULL AND t.archived_at_ms IS NULL",
    )
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

async fn ingest_member_file(pool: &SqlitePool, member: &ClaudeMember, file: &Path) -> Result<()> {
    let cursor_id = cursor_id(&member.id, file);
    let mut offset = read_cursor(pool, &cursor_id).await?.unwrap_or(0);

    let f = match fs::File::open(file) {
        Ok(f) => f,
        Err(_) => return Ok(()),
    };
    let size = f.metadata().map(|m| m.len()).unwrap_or(0);
    if offset > size {
        offset = 0;
    }

    let mut reader = BufReader::new(f);
    reader.seek(SeekFrom::Start(offset))?;

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        offset += n as u64;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match claude_transcript::parse_jsonl_line(trimmed) {
            Ok(events) => {
                for event in events {
                    apply_event(pool, member, event).await?;
                }
            }
            Err(err) => {
                tracing::debug!(
                    "skipping unparsable transcript line ({}): {err}",
                    file.display()
                );
            }
        }
    }

    write_cursor(pool, &cursor_id, &member.id, offset).await?;
    Ok(())
}

async fn apply_event(pool: &SqlitePool, member: &ClaudeMember, event: ClaudeEvent) -> Result<()> {
    match event {
        ClaudeEvent::ToolUse(e) => {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO tool_calls (id, team_id, member_id, backend_type, source, external_call_id, tool_name, input_summary, input_json, status, started_at_ms, ingested_at_ms)
                 VALUES (?, ?, ?, 'claude', ?, ?, ?, ?, ?, 'started', ?, ?)",
            )
            .bind(id)
            .bind(&member.team_id)
            .bind(&member.id)
            .bind(SOURCE)
            .bind(e.external_call_id)
            .bind(&e.tool_name)
            .bind(summarize_input(e.input_json.as_deref()))
            .bind(e.input_json)
            .bind(e.started_at_ms)
            .bind(now_ms())
            .execute(pool)
            .await
            .ok();
        }
        ClaudeEvent::ToolResult(e) => {
            let ended = e.ended_at_ms;
            if let Some(call_id) = e.external_call_id.as_deref() {
                let updated = sqlx::query(
                    "UPDATE tool_calls
                     SET status = CASE WHEN ? IS NULL THEN 'completed' ELSE 'error' END,
                         error_text = ?,
                         ended_at_ms = ?,
                         duration_ms = CASE WHEN started_at_ms <= ? THEN ? - started_at_ms ELSE 0 END
                     WHERE member_id = ? AND source = ? AND external_call_id = ? AND ended_at_ms IS NULL",
                )
                .bind(e.error_text.as_deref())
                .bind(e.error_text.as_deref())
                .bind(ended)
                .bind(ended)
                .bind(ended)
                .bind(&member.id)
                .bind(SOURCE)
                .bind(call_id)
                .execute(pool)
                .await?
                .rows_affected();
                if updated > 0 {
                    return Ok(());
                }
            }

            let id = Uuid::new_v4().to_string();
            let tool_name = e.tool_name.unwrap_or_else(|| String::from("unknown"));
            sqlx::query(
                "INSERT INTO tool_calls (id, team_id, member_id, backend_type, source, external_call_id, tool_name, input_summary, input_json, status, error_text, started_at_ms, ended_at_ms, duration_ms, ingested_at_ms)
                 VALUES (?, ?, ?, 'claude', ?, ?, ?, '', NULL, CASE WHEN ? IS NULL THEN 'completed' ELSE 'error' END, ?, ?, ?, 0, ?)",
            )
            .bind(id)
            .bind(&member.team_id)
            .bind(&member.id)
            .bind(SOURCE)
            .bind(e.external_call_id)
            .bind(tool_name)
            .bind(e.error_text.as_deref())
            .bind(e.error_text.as_deref())
            .bind(ended)
            .bind(ended)
            .bind(now_ms())
            .execute(pool)
            .await
            .ok();
        }
        ClaudeEvent::MemberStatus(e) => {
            let event_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO member_status_events (id, member_id, event_type, state, headline, payload_json, created_at_ms)
                 VALUES (?, ?, 'claude_transcript', ?, ?, ?, ?)",
            )
            .bind(event_id)
            .bind(&member.id)
            .bind(&e.state)
            .bind(e.headline.as_deref())
            .bind(e.payload_json.as_deref())
            .bind(e.created_at_ms)
            .execute(pool)
            .await?;

            let last_error = if e.state == "api_error" {
                e.headline.clone()
            } else {
                None
            };
            sqlx::query(
                "UPDATE member_status_current
                 SET state = ?, headline = ?, last_heartbeat_ms = ?, last_error = COALESCE(?, last_error)
                 WHERE member_id = ?",
            )
            .bind(&e.state)
            .bind(e.headline)
            .bind(e.created_at_ms)
            .bind(last_error)
            .bind(&member.id)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

fn summarize_input(input_json: Option<&str>) -> String {
    let Some(input_json) = input_json else {
        return String::new();
    };
    if input_json.len() <= 256 {
        return input_json.to_string();
    }
    input_json[..256].to_string()
}

fn cwd_to_projects_dir(cwd: &str) -> String {
    cwd.replace('/', "-")
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out)?;
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
    Ok(())
}

fn cursor_id(member_id: &str, path: &Path) -> String {
    format!("{CURSOR_TYPE}:{member_id}:{}", path.display())
}

async fn read_cursor(pool: &SqlitePool, id: &str) -> Result<Option<u64>> {
    let row =
        sqlx::query_scalar::<_, String>("SELECT cursor_value FROM ingest_cursors WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|s| s.parse::<u64>().ok()))
}

async fn write_cursor(pool: &SqlitePool, id: &str, member_id: &str, offset: u64) -> Result<()> {
    sqlx::query(
        "INSERT INTO ingest_cursors (id, cursor_type, member_id, cursor_value, updated_at_ms)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET cursor_value = excluded.cursor_value, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(id)
    .bind(CURSOR_TYPE)
    .bind(member_id)
    .bind(offset.to_string())
    .bind(now_ms())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn wait_until(condition: impl Fn() -> bool) -> bool {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while tokio::time::Instant::now() < deadline {
            if condition() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    #[test]
    fn maps_cwd_to_projects_directory_name() {
        assert_eq!(
            cwd_to_projects_dir("/Users/johndoe/dev/nightshift"),
            "-Users-johndoe-dev-nightshift"
        );
    }

    #[tokio::test]
    async fn supervisor_starts_and_stops_with_member_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("teams.db");
        let pool = crate::db::open(&db_path).await.unwrap();
        let now = now_ms();

        sqlx::query(
            "INSERT INTO teams (id, name, description, created_at_ms, lead_agent_id) VALUES (?, ?, '', ?, ?)",
        )
        .bind("t1")
        .bind("team-1")
        .bind(now)
        .bind("m1")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO members (id, team_id, name, agent_type, model, backend_type, cwd, joined_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("m1")
        .bind("t1")
        .bind("lead")
        .bind("team-lead")
        .bind("claude-sonnet-4-6")
        .bind("claude")
        .bind("/tmp")
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let supervisor = Arc::new(IngestSupervisor::new());
        supervisor.start_if_needed(pool.clone()).await.unwrap();
        assert!(wait_until(|| supervisor.is_running()).await);

        sqlx::query("UPDATE members SET removed_at_ms = ? WHERE id = ?")
            .bind(now_ms())
            .bind("m1")
            .execute(&pool)
            .await
            .unwrap();

        supervisor.notify_member_set_changed();
        assert!(wait_until(|| !supervisor.is_running()).await);
    }
}
