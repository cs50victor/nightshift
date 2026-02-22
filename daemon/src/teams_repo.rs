use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, SqlitePool};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::db::now_ms;
use crate::toolcalls::{MemberToolHistory, ToolCall, ToolStats};

#[derive(FromRow)]
struct TeamRow {
    id: String,
    name: String,
    description: String,
    created_at_ms: i64,
    archived_at_ms: Option<i64>,
}

#[derive(FromRow)]
struct MemberRow {
    name: String,
    agent_type: String,
    model: String,
    cwd: String,
    color: Option<String>,
    removed_at_ms: Option<i64>,
}

#[derive(FromRow)]
struct TaskRow {
    external_task_id: String,
    subject: String,
    status: String,
    owner_member_id: Option<String>,
}

#[derive(FromRow)]
struct StatusRow {
    member_name: String,
    alive: i64,
    state: String,
    headline: Option<String>,
    last_heartbeat_ms: i64,
    last_error: Option<String>,
}

#[derive(FromRow)]
struct ActivityRow {
    id: String,
    kind: String,
    payload_json: String,
    created_at_ms: i64,
}

#[derive(FromRow)]
struct TimelineRow {
    id: String,
    event_type: String,
    state: Option<String>,
    headline: Option<String>,
    payload_json: Option<String>,
    created_at_ms: i64,
}

#[derive(FromRow)]
struct MemberToolLookupRow {
    member_id: String,
    backend_type: String,
    team_id: String,
}

#[derive(FromRow)]
struct ToolCallRow {
    tool_name: String,
    tool_title: Option<String>,
    input_summary: String,
    status: String,
    started_at_ms: i64,
    duration_ms: Option<i64>,
}

#[derive(FromRow)]
struct MemberDiffRow {
    cwd: String,
    baseline_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TeamSummary {
    pub name: String,
    pub description: String,
    pub created_at: u64,
    pub archived: bool,
    pub members: Vec<MemberSummary>,
    pub tasks: Vec<TaskSummary>,
    pub conflicts: Vec<ConflictInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberSummary {
    pub name: String,
    pub agent_type: String,
    pub model: String,
    pub cwd: String,
    pub is_active: bool,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub subject: String,
    pub status: String,
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub path: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TeamSnapshot {
    pub team: TeamSummary,
    pub statuses: Vec<MemberStatusCurrent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberStatusCurrent {
    pub member_name: String,
    pub alive: bool,
    pub state: String,
    pub headline: Option<String>,
    pub last_heartbeat_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: String,
    pub kind: String,
    pub payload_json: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub id: String,
    pub event_type: String,
    pub state: Option<String>,
    pub headline: Option<String>,
    pub payload_json: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTeamRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_lead_name")]
    pub lead_name: String,
    #[serde(default = "default_lead_type")]
    pub lead_agent_type: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_backend")]
    pub backend_type: String,
    pub cwd: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteTeamRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpawnTeammateRequest {
    pub team: String,
    pub name: String,
    #[serde(default = "default_agent_type")]
    pub agent_type: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_backend")]
    pub backend_type: String,
    pub cwd: String,
    #[serde(default)]
    pub plan_mode_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct KillTeammateRequest {
    pub team: String,
    pub name: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub team: String,
    pub external_task_id: String,
    pub subject: String,
    pub description: String,
    #[serde(default)]
    pub active_form: String,
    #[serde(default = "default_task_status")]
    pub status: String,
    #[serde(default)]
    pub owner_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    pub team: String,
    pub external_task_id: String,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub active_form: Option<String>,
    pub status: Option<String>,
    pub owner_name: Option<Option<String>>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub team: String,
    pub from_name: Option<String>,
    pub to_name: Option<String>,
    pub message_type: String,
    pub summary: Option<String>,
    pub content_text: Option<String>,
    pub payload_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadInboxRequest {
    pub team: String,
    pub member_name: String,
}

fn default_lead_name() -> String {
    "team-lead".to_string()
}
fn default_lead_type() -> String {
    "team-lead".to_string()
}
fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}
fn default_backend() -> String {
    "claude".to_string()
}
fn default_agent_type() -> String {
    "general".to_string()
}
fn default_task_status() -> String {
    "pending".to_string()
}

async fn append_activity(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    team_id: &str,
    member_id: Option<&str>,
    kind: &str,
    payload: serde_json::Value,
) -> Result<()> {
    let activity_id = Uuid::new_v4().to_string();
    let payload_json = payload.to_string();
    let created_at_ms = now_ms();
    sqlx::query!(
        "INSERT INTO activity_log (id, team_id, member_id, kind, payload_json, created_at_ms) VALUES (?, ?, ?, ?, ?, ?)",
        activity_id,
        team_id,
        member_id,
        kind,
        payload_json,
        created_at_ms,
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn resolve_team_id(executor: &SqlitePool, team_name: &str) -> Result<Option<String>> {
    let row = sqlx::query_scalar!(
        "SELECT id as \"id!: String\" FROM teams WHERE name = ?",
        team_name,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

async fn resolve_member_id(
    executor: &SqlitePool,
    team_id: &str,
    member_name: &str,
) -> Result<Option<String>> {
    let row = sqlx::query_scalar!(
        "SELECT id as \"id!: String\" FROM members WHERE team_id = ? AND name = ? AND removed_at_ms IS NULL",
        team_id,
        member_name,
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}

pub async fn create_team(pool: &SqlitePool, req: CreateTeamRequest) -> Result<ActionResponse> {
    let now = now_ms();
    let team_id = Uuid::new_v4().to_string();
    let lead_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "INSERT INTO teams (id, name, description, created_at_ms, lead_agent_id) VALUES (?, ?, ?, ?, ?)",
        team_id,
        req.name,
        req.description,
        now,
        lead_id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO members (id, team_id, name, agent_type, model, backend_type, cwd, joined_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        lead_id,
        team_id,
        req.lead_name,
        req.lead_agent_type,
        req.model,
        req.backend_type,
        req.cwd,
        now,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO member_status_current (member_id, alive, state, last_heartbeat_ms) VALUES (?, 1, 'idle', ?)",
        lead_id,
        now,
    )
    .execute(&mut *tx)
    .await?;

    append_activity(
        &mut tx,
        &team_id,
        Some(&lead_id),
        "team_created",
        json!({"team": req.name}),
    )
    .await?;

    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn delete_team(pool: &SqlitePool, req: DeleteTeamRequest) -> Result<ActionResponse> {
    let team_id = resolve_team_id(pool, &req.name)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;

    let count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM members WHERE team_id = ? AND removed_at_ms IS NULL AND id != (SELECT lead_agent_id FROM teams WHERE id = ?)",
        team_id,
        team_id,
    )
    .fetch_one(pool)
    .await?;
    if count > 0 {
        return Err(anyhow!("cannot archive team with active non-lead members"));
    }

    let mut tx = pool.begin().await?;
    let archived_at_ms = now_ms();
    sqlx::query!(
        "UPDATE teams SET archived_at_ms = ? WHERE id = ?",
        archived_at_ms,
        team_id,
    )
    .execute(&mut *tx)
    .await?;
    append_activity(
        &mut tx,
        &team_id,
        None,
        "team_archived",
        json!({"team": req.name}),
    )
    .await?;
    tx.commit().await?;

    Ok(ActionResponse { ok: true })
}

pub async fn spawn_teammate(
    pool: &SqlitePool,
    req: SpawnTeammateRequest,
) -> Result<ActionResponse> {
    let now = now_ms();
    let team_id = resolve_team_id(pool, &req.team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let member_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let baseline_commit = git_head(&req.cwd).await;
    let plan_mode_required = if req.plan_mode_required { 1 } else { 0 };

    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO members (id, team_id, name, agent_type, model, backend_type, cwd, plan_mode_required, joined_at_ms, baseline_commit) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        member_id,
        team_id,
        req.name,
        req.agent_type,
        req.model,
        req.backend_type,
        req.cwd,
        plan_mode_required,
        now,
        baseline_commit,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO member_runs (id, team_id, member_id, started_at_ms) VALUES (?, ?, ?, ?)",
        run_id,
        team_id,
        member_id,
        now,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO member_status_current (member_id, run_id, alive, state, last_heartbeat_ms) VALUES (?, ?, 1, 'idle', ?)",
        member_id,
        run_id,
        now,
    )
    .execute(&mut *tx)
    .await?;

    append_activity(
        &mut tx,
        &team_id,
        Some(&member_id),
        "member_spawned",
        json!({"member": req.name}),
    )
    .await?;

    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn kill_teammate(pool: &SqlitePool, req: KillTeammateRequest) -> Result<ActionResponse> {
    let now = now_ms();
    let team_id = resolve_team_id(pool, &req.team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let member_id = resolve_member_id(pool, &team_id, &req.name)
        .await?
        .ok_or_else(|| anyhow!("member not found"))?;

    let mut tx = pool.begin().await?;
    sqlx::query!(
        "UPDATE member_runs SET ended_at_ms = ?, end_reason = COALESCE(?, 'killed') WHERE team_id = ? AND member_id = ? AND ended_at_ms IS NULL",
        now,
        req.reason,
        team_id,
        member_id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE members SET removed_at_ms = ? WHERE id = ?",
        now,
        member_id
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE member_status_current SET alive = 0, state = 'offline', run_id = NULL, last_heartbeat_ms = ? WHERE member_id = ?",
        now,
        member_id,
    )
    .execute(&mut *tx)
    .await?;

    append_activity(
        &mut tx,
        &team_id,
        Some(&member_id),
        "member_killed",
        json!({"member": req.name}),
    )
    .await?;

    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn create_task(pool: &SqlitePool, req: CreateTaskRequest) -> Result<ActionResponse> {
    let now = now_ms();
    let team_id = resolve_team_id(pool, &req.team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let owner_id = match req.owner_name.as_deref() {
        Some(name) => resolve_member_id(pool, &team_id, name).await?,
        None => None,
    };
    let task_id = Uuid::new_v4().to_string();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO tasks (id, team_id, external_task_id, subject, description, active_form, status, owner_member_id, created_at_ms, updated_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        task_id,
        team_id,
        req.external_task_id,
        req.subject,
        req.description,
        req.active_form,
        req.status,
        owner_id,
        now,
        now,
    )
    .execute(&mut *tx)
    .await?;
    append_activity(
        &mut tx,
        &team_id,
        None,
        "task_created",
        json!({"externalTaskId": req.external_task_id}),
    )
    .await?;
    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn update_task(pool: &SqlitePool, req: UpdateTaskRequest) -> Result<ActionResponse> {
    let UpdateTaskRequest {
        team,
        external_task_id,
        subject,
        description,
        active_form,
        status,
        owner_name,
        metadata_json,
    } = req;
    let now = now_ms();
    let team_id = resolve_team_id(pool, &team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let existing_meta: Option<Option<String>> = sqlx::query_scalar!(
        "SELECT metadata_json FROM tasks WHERE team_id = ? AND external_task_id = ?",
        team_id,
        external_task_id,
    )
    .fetch_optional(pool)
    .await?;
    let existing_meta = existing_meta.flatten();

    let merged_meta = merge_metadata(existing_meta, metadata_json);

    let mut tx = pool.begin().await?;
    let rows_affected = if let Some(owner_name) = owner_name {
        let owner_member_id: Option<String> = match owner_name {
            Some(name) => Some(
                resolve_member_id(pool, &team_id, &name)
                    .await?
                    .ok_or_else(|| anyhow!("owner member not found"))?,
            ),
            None => None,
        };
        sqlx::query!(
            "UPDATE tasks SET subject = COALESCE(?, subject), description = COALESCE(?, description), active_form = COALESCE(?, active_form), status = COALESCE(?, status), owner_member_id = ?, metadata_json = ?, updated_at_ms = ? WHERE team_id = ? AND external_task_id = ?",
            subject,
            description,
            active_form,
            status,
            owner_member_id,
            merged_meta,
            now,
            team_id,
            external_task_id,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
    } else {
        sqlx::query!(
            "UPDATE tasks SET subject = COALESCE(?, subject), description = COALESCE(?, description), active_form = COALESCE(?, active_form), status = COALESCE(?, status), metadata_json = ?, updated_at_ms = ? WHERE team_id = ? AND external_task_id = ?",
            subject,
            description,
            active_form,
            status,
            merged_meta,
            now,
            team_id,
            external_task_id,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
    };
    if rows_affected == 0 {
        return Err(anyhow!("task not found"));
    }
    append_activity(
        &mut tx,
        &team_id,
        None,
        "task_updated",
        json!({"externalTaskId": external_task_id}),
    )
    .await?;
    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

fn merge_metadata(existing: Option<String>, patch: Option<serde_json::Value>) -> Option<String> {
    let Some(patch) = patch else { return existing };
    let mut base = existing
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .unwrap_or_else(|| json!({}));
    if let (Some(base_obj), Some(patch_obj)) = (base.as_object_mut(), patch.as_object()) {
        for (k, v) in patch_obj {
            base_obj.insert(k.clone(), v.clone());
        }
        Some(base.to_string())
    } else {
        Some(patch.to_string())
    }
}

pub async fn send_message(pool: &SqlitePool, req: SendMessageRequest) -> Result<ActionResponse> {
    let team_id = resolve_team_id(pool, &req.team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let from_id = match req.from_name.as_deref() {
        Some(name) => Some(
            resolve_member_id(pool, &team_id, name)
                .await?
                .ok_or_else(|| anyhow!("from member not found"))?,
        ),
        None => None,
    };
    let to_id = match req.to_name.as_deref() {
        Some(name) => Some(
            resolve_member_id(pool, &team_id, name)
                .await?
                .ok_or_else(|| anyhow!("to member not found"))?,
        ),
        None => None,
    };
    let msg_id = Uuid::new_v4().to_string();
    let now = now_ms();
    let payload_json = req.payload_json.map(|v| v.to_string());

    let mut tx = pool.begin().await?;
    sqlx::query!(
        "INSERT INTO messages (id, team_id, from_member_id, to_member_id, message_type, summary, content_text, payload_json, created_at_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        msg_id,
        team_id,
        from_id,
        to_id,
        req.message_type,
        req.summary,
        req.content_text,
        payload_json,
        now,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(ref to_member_id) = to_id {
        sqlx::query!(
            "INSERT INTO inbox_state (message_id, member_id) VALUES (?, ?)",
            msg_id,
            to_member_id,
        )
        .execute(&mut *tx)
        .await?;
    } else {
        let recipients = sqlx::query_scalar!(
            "SELECT id FROM members WHERE team_id = ? AND removed_at_ms IS NULL AND (? IS NULL OR id != ?)",
            team_id,
            from_id,
            from_id,
        )
        .fetch_all(&mut *tx)
        .await?;

        for member_id in recipients {
            sqlx::query!(
                "INSERT INTO inbox_state (message_id, member_id) VALUES (?, ?)",
                msg_id,
                member_id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    append_activity(
        &mut tx,
        &team_id,
        from_id.as_deref(),
        "message_sent",
        json!({"messageType": req.message_type}),
    )
    .await?;
    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn mark_read(pool: &SqlitePool, req: ReadInboxRequest) -> Result<ActionResponse> {
    let team_id = resolve_team_id(pool, &req.team)
        .await?
        .ok_or_else(|| anyhow!("team not found"))?;
    let member_id = resolve_member_id(pool, &team_id, &req.member_name)
        .await?
        .ok_or_else(|| anyhow!("member not found"))?;
    let now = now_ms();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "UPDATE inbox_state SET read_at_ms = ? WHERE member_id = ? AND read_at_ms IS NULL",
        now,
        member_id,
    )
    .execute(&mut *tx)
    .await?;
    append_activity(
        &mut tx,
        &team_id,
        Some(&member_id),
        "inbox_read",
        json!({"member": req.member_name}),
    )
    .await?;
    tx.commit().await?;
    Ok(ActionResponse { ok: true })
}

pub async fn list_teams(pool: &SqlitePool) -> Result<Vec<TeamSummary>> {
    let teams = sqlx::query_as!(
        TeamRow,
        "SELECT id as \"id!: String\", name as \"name!: String\", description as \"description!: String\", created_at_ms as \"created_at_ms!: i64\", archived_at_ms FROM teams ORDER BY created_at_ms DESC",
    )
        .fetch_all(pool)
        .await?;

    let mut out = Vec::new();
    for row in teams {
        let members = list_members(pool, &row.id).await?;
        let tasks = list_tasks(pool, &row.id).await?;
        out.push(TeamSummary {
            name: row.name,
            description: row.description,
            created_at: row.created_at_ms.max(0) as u64,
            archived: row.archived_at_ms.is_some(),
            members,
            tasks,
            conflicts: Vec::new(),
        });
    }
    Ok(out)
}

async fn list_members(pool: &SqlitePool, team_id: &str) -> Result<Vec<MemberSummary>> {
    let rows = sqlx::query_as!(
        MemberRow,
        "SELECT name, agent_type, model, cwd, color, removed_at_ms FROM members WHERE team_id = ? ORDER BY joined_at_ms ASC",
        team_id,
    )
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| MemberSummary {
            name: r.name,
            agent_type: r.agent_type,
            model: r.model,
            cwd: r.cwd,
            color: r.color,
            is_active: r.removed_at_ms.is_none(),
        })
        .collect())
}

async fn list_tasks(pool: &SqlitePool, team_id: &str) -> Result<Vec<TaskSummary>> {
    let rows = sqlx::query_as!(
        TaskRow,
        "SELECT external_task_id, subject, status, owner_member_id FROM tasks WHERE team_id = ? AND status != 'deleted' ORDER BY updated_at_ms DESC",
        team_id,
    )
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| TaskSummary {
            id: r.external_task_id,
            subject: r.subject,
            status: r.status,
            owner: r.owner_member_id,
        })
        .collect())
}

pub async fn get_team_snapshot(pool: &SqlitePool, team: &str) -> Result<Option<TeamSnapshot>> {
    let teams = list_teams(pool).await?;
    let Some(team_summary) = teams.into_iter().find(|t| t.name == team) else {
        return Ok(None);
    };

    let rows = sqlx::query_as!(
        StatusRow,
        "SELECT m.name as member_name, s.alive, s.state, s.headline, s.last_heartbeat_ms, s.last_error FROM members m JOIN member_status_current s ON s.member_id = m.id WHERE m.team_id = (SELECT id FROM teams WHERE name = ?)",
        team,
    )
    .fetch_all(pool)
    .await?;

    let statuses = rows
        .into_iter()
        .map(|r| MemberStatusCurrent {
            member_name: r.member_name,
            alive: r.alive == 1,
            state: r.state,
            headline: r.headline,
            last_heartbeat_ms: r.last_heartbeat_ms.max(0) as u64,
            last_error: r.last_error,
        })
        .collect();
    Ok(Some(TeamSnapshot {
        team: team_summary,
        statuses,
    }))
}

pub async fn get_activity(
    pool: &SqlitePool,
    team: &str,
    since_ms: Option<u64>,
    limit: Option<u32>,
) -> Result<Vec<ActivityEntry>> {
    let since = since_ms.unwrap_or(0) as i64;
    let lim = limit.unwrap_or(100) as i64;
    let rows = sqlx::query_as!(
        ActivityRow,
        "SELECT id as \"id!: String\", kind as \"kind!: String\", payload_json as \"payload_json!: String\", created_at_ms as \"created_at_ms!: i64\" FROM activity_log WHERE team_id = (SELECT id FROM teams WHERE name = ?) AND created_at_ms >= ? ORDER BY created_at_ms DESC LIMIT ?",
        team,
        since,
        lim,
    )
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| ActivityEntry {
            id: r.id,
            kind: r.kind,
            payload_json: r.payload_json,
            created_at_ms: r.created_at_ms.max(0) as u64,
        })
        .collect())
}

pub async fn get_member_timeline(
    pool: &SqlitePool,
    team: &str,
    name: &str,
    since_ms: Option<u64>,
    limit: Option<u32>,
) -> Result<Vec<TimelineEntry>> {
    let since = since_ms.unwrap_or(0) as i64;
    let lim = limit.unwrap_or(100) as i64;
    let rows = sqlx::query_as!(
        TimelineRow,
        "SELECT e.id as \"id!: String\", e.event_type as \"event_type!: String\", e.state, e.headline, e.payload_json, e.created_at_ms as \"created_at_ms!: i64\" FROM member_status_events e JOIN members m ON m.id = e.member_id WHERE m.team_id = (SELECT id FROM teams WHERE name = ?) AND m.name = ? AND e.created_at_ms >= ? ORDER BY e.created_at_ms DESC LIMIT ?",
        team,
        name,
        since,
        lim,
    )
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| TimelineEntry {
            id: r.id,
            event_type: r.event_type,
            state: r.state,
            headline: r.headline,
            payload_json: r.payload_json,
            created_at_ms: r.created_at_ms.max(0) as u64,
        })
        .collect())
}

pub async fn get_member_tools(
    pool: &SqlitePool,
    team: &str,
    name: &str,
) -> Result<Option<MemberToolHistory>> {
    let member = sqlx::query_as!(
        MemberToolLookupRow,
        "SELECT m.id as \"member_id!: String\", m.backend_type as \"backend_type!: String\", t.id as \"team_id!: String\" FROM members m JOIN teams t ON t.id = m.team_id WHERE t.name = ? AND m.name = ?",
        team,
        name,
    )
        .fetch_optional(pool)
        .await?;
    let Some(member) = member else {
        return Ok(None);
    };

    let member_id = member.member_id;
    let backend = member.backend_type;
    let team_id = member.team_id;

    let rows = sqlx::query_as!(
        ToolCallRow,
        "SELECT tool_name, tool_title, input_summary, status, started_at_ms, duration_ms FROM tool_calls WHERE team_id = ? AND member_id = ? ORDER BY started_at_ms DESC LIMIT 500",
        team_id,
        member_id,
    )
        .fetch_all(pool)
        .await?;

    let calls: Vec<ToolCall> = rows
        .into_iter()
        .map(|r| ToolCall {
            tool: r.tool_name,
            title: r.tool_title,
            input_summary: r.input_summary,
            status: r.status,
            timestamp: r.started_at_ms.max(0) as u64,
            duration_ms: r.duration_ms.map(|v| v.max(0) as u64),
        })
        .collect();
    let stats = ToolStats::from_calls(&calls);
    Ok(Some(MemberToolHistory {
        name: name.to_string(),
        team: team.to_string(),
        backend,
        tool_calls: calls,
        stats,
    }))
}

pub async fn get_member_diff_cwd(
    pool: &SqlitePool,
    team: &str,
    name: &str,
) -> Result<Option<(String, Option<String>)>> {
    let row = sqlx::query_as!(
        MemberDiffRow,
        "SELECT m.cwd, m.baseline_commit FROM members m JOIN teams t ON t.id = m.team_id WHERE t.name = ? AND m.name = ?",
        team,
        name,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.cwd, r.baseline_commit)))
}

async fn git_head(cwd: &str) -> Option<String> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_create_team_and_append_activity() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("teams.db");
        let pool = crate::db::open(&db_path).await.unwrap();

        create_team(
            &pool,
            CreateTeamRequest {
                name: "alpha".to_string(),
                description: "desc".to_string(),
                lead_name: "team-lead".to_string(),
                lead_agent_type: "team-lead".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                backend_type: "claude".to_string(),
                cwd: "/tmp".to_string(),
            },
        )
        .await
        .unwrap();

        let teams = list_teams(&pool).await.unwrap();
        assert_eq!(teams.len(), 1);
        assert_eq!(teams[0].name, "alpha");

        let activity_count: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM activity_log WHERE kind = 'team_created'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(activity_count, 1);
    }
}
