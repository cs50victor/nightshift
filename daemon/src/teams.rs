use crate::toolcalls::{self, MemberToolHistory, ToolCall, ToolStats};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::ToSchema;

const TEAMS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
const DIFF_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
const FULL_RESCAN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
const DEBOUNCE_DURATION: std::time::Duration = std::time::Duration::from_millis(100);

// --- MCP config types (read-only, deserialized from ~/.claude/) ---

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TeamConfig {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    created_at: u64,
    #[serde(default)]
    #[allow(dead_code)]
    lead_agent_id: String,
    members: Vec<MemberConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MemberConfig {
    #[serde(default)]
    #[allow(dead_code)]
    agent_id: String,
    name: String,
    #[serde(default)]
    agent_type: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    is_active: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    tmux_pane_id: Option<String>,
    #[serde(default)]
    backend_type: Option<String>,
    #[serde(default)]
    opencode_session_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TaskFile {
    id: serde_json::Value,
    subject: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: String,
    status: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    blocks: Vec<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    blocked_by: Vec<serde_json::Value>,
}

// --- Claude Code active-sessions.json ---

#[derive(Deserialize, Debug)]
struct ActiveSessions {
    sessions: HashMap<String, ActiveSession>,
}

#[derive(Deserialize, Debug)]
struct ActiveSession {
    transcript_path: String,
    #[serde(default)]
    tmux: Option<TmuxInfo>,
}

#[derive(Deserialize, Debug)]
struct TmuxInfo {
    pane_id: String,
}

// --- Internal state ---

struct MemberState {
    config: MemberConfig,
    baseline_commit: Option<String>,
    cached_summary: Option<DiffSummary>,
    session_path: Option<PathBuf>,
}

struct TeamState {
    config: TeamConfig,
    members: HashMap<String, MemberState>,
    tasks: HashMap<String, TaskFile>,
}

pub struct TeamsData {
    active: HashMap<String, TeamState>,
    archived: HashMap<String, TeamArchive>,
}

pub type TeamsHandle = Arc<RwLock<TeamsData>>;

// --- Archive types ---

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TeamArchive {
    name: String,
    archived_at: u64,
    final_state: TeamSummary,
    member_diffs: HashMap<String, String>,
    member_tools: HashMap<String, Vec<ToolCall>>,
}

// --- API response types ---

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
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

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberSummary {
    pub name: String,
    pub agent_type: String,
    pub model: String,
    pub cwd: String,
    pub is_active: bool,
    pub color: Option<String>,
    pub diff_summary: Option<DiffSummary>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    pub files: Vec<FileStat>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub path: String,
    pub additions: u32,
    pub deletions: u32,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub id: String,
    pub subject: String,
    pub status: String,
    pub owner: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub path: String,
    pub members: Vec<String>,
}

#[derive(Serialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberDiffDetail {
    pub name: String,
    pub team: String,
    pub cwd: String,
    pub baseline_commit: Option<String>,
    pub current_commit: Option<String>,
    pub diff: String,
}

// --- Public API ---

pub fn new_handle() -> TeamsHandle {
    let archived = load_archives();
    Arc::new(RwLock::new(TeamsData {
        active: HashMap::new(),
        archived,
    }))
}

pub async fn get_teams_summary(handle: &TeamsHandle) -> Vec<TeamSummary> {
    let data = handle.read().await;
    let mut result = Vec::new();

    for team in data.active.values() {
        result.push(build_team_summary(team, false));
    }

    for archive in data.archived.values() {
        result.push(archive.final_state.clone());
    }

    result
}

pub async fn get_member_diff(
    handle: &TeamsHandle,
    team_name: &str,
    member_name: &str,
) -> Option<MemberDiffDetail> {
    let member_info = {
        let data = handle.read().await;
        if let Some(team) = data.active.get(team_name) {
            team.members
                .get(member_name)
                .map(|m| (m.config.cwd.clone(), m.baseline_commit.clone()))
        } else if let Some(archive) = data.archived.get(team_name) {
            if let Some(diff) = archive.member_diffs.get(member_name) {
                return Some(MemberDiffDetail {
                    name: member_name.to_string(),
                    team: team_name.to_string(),
                    cwd: String::new(),
                    baseline_commit: None,
                    current_commit: None,
                    diff: diff.clone(),
                });
            }
            return None;
        } else {
            None
        }
    };

    let (cwd, baseline) = member_info?;
    let diff = if let Some(ref b) = baseline {
        compute_diff_full(&cwd, b).await.unwrap_or_default()
    } else {
        String::new()
    };
    let current = git_head(&cwd).await;

    Some(MemberDiffDetail {
        name: member_name.to_string(),
        team: team_name.to_string(),
        cwd,
        baseline_commit: baseline,
        current_commit: current,
        diff,
    })
}

pub async fn get_member_tools(
    handle: &TeamsHandle,
    team_name: &str,
    member_name: &str,
) -> Option<MemberToolHistory> {
    let member_info = {
        let data = handle.read().await;
        if let Some(team) = data.active.get(team_name) {
            team.members.get(member_name).map(|m| {
                let backend = m
                    .config
                    .backend_type
                    .as_deref()
                    .unwrap_or("claude")
                    .to_string();
                let session_path = m.session_path.clone();
                let opencode_session_id = m.config.opencode_session_id.clone();
                (backend, session_path, opencode_session_id)
            })
        } else if let Some(archive) = data.archived.get(team_name) {
            if let Some(calls) = archive.member_tools.get(member_name) {
                let stats = ToolStats::from_calls(calls);
                return Some(MemberToolHistory {
                    name: member_name.to_string(),
                    team: team_name.to_string(),
                    backend: "archived".to_string(),
                    tool_calls: calls.clone(),
                    stats,
                });
            }
            return None;
        } else {
            None
        }
    };

    let (backend, session_path, opencode_session_id) = member_info?;
    let calls = match backend.as_str() {
        "claude" => {
            if let Some(ref path) = session_path {
                crate::toolcalls::read_claude_tools(path)
            } else {
                Vec::new()
            }
        }
        "opencode" => {
            if let Some(ref sid) = opencode_session_id {
                crate::toolcalls::read_opencode_tools(sid).await
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    };
    let stats = ToolStats::from_calls(&calls);

    Some(MemberToolHistory {
        name: member_name.to_string(),
        team: team_name.to_string(),
        backend,
        tool_calls: calls,
        stats,
    })
}

async fn read_member_tools(member: &MemberState, backend: &str) -> Vec<ToolCall> {
    match backend {
        "opencode" => {
            if let Some(ref sid) = member.config.opencode_session_id {
                toolcalls::read_opencode_tools(sid).await
            } else {
                Vec::new()
            }
        }
        _ => {
            if let Some(ref path) = member.session_path {
                toolcalls::read_claude_tools(path)
            } else {
                Vec::new()
            }
        }
    }
}

// --- Watcher ---

pub async fn spawn_watcher(handle: TeamsHandle) {
    let teams_dir = teams_dir();
    let tasks_dir = tasks_dir();

    loop {
        if teams_dir.exists() {
            break;
        }
        tracing::debug!("~/.claude/teams/ not found, polling...");
        tokio::time::sleep(TEAMS_POLL_INTERVAL).await;
    }

    tracing::info!("discovered ~/.claude/teams/, starting team watcher");
    initial_scan(&handle).await;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(16);

    let tx_clone = tx.clone();
    let mut watcher = match notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(_event) = res {
            let _ = tx_clone.try_send(());
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            tracing::warn!("failed to create filesystem watcher: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(&teams_dir, RecursiveMode::Recursive) {
        tracing::warn!("failed to watch teams dir: {e}");
    }
    if tasks_dir.exists() {
        if let Err(e) = watcher.watch(&tasks_dir, RecursiveMode::Recursive) {
            tracing::warn!("failed to watch tasks dir: {e}");
        }
    }

    let diff_handle = handle.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(DIFF_REFRESH_INTERVAL);
        loop {
            interval.tick().await;
            refresh_diff_summaries(&diff_handle).await;
        }
    });

    let rescan_handle = handle.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(FULL_RESCAN_INTERVAL);
        loop {
            interval.tick().await;
            rescan(&rescan_handle).await;
        }
    });

    loop {
        if rx.recv().await.is_none() {
            break;
        }
        tokio::time::sleep(DEBOUNCE_DURATION).await;
        while rx.try_recv().is_ok() {}
        rescan(&handle).await;
    }
}

async fn initial_scan(handle: &TeamsHandle) {
    let teams_dir = teams_dir();
    let entries = match std::fs::read_dir(&teams_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("failed to read teams dir: {e}");
            return;
        }
    };

    let mut data = handle.write().await;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let team_name = path.file_name().unwrap().to_string_lossy().to_string();
        let config_path = path.join("config.json");
        if !config_path.exists() {
            continue;
        }
        if let Some(state) = load_team_state(&team_name).await {
            data.active.insert(team_name, state);
        }
    }

    let active_count = data.active.len();
    let archived_count = data.archived.len();
    tracing::info!("initial scan: {active_count} active teams, {archived_count} archived");
}

async fn rescan(handle: &TeamsHandle) {
    let teams_dir = teams_dir();
    let current_dirs: Vec<String> = match std::fs::read_dir(&teams_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir() && e.path().join("config.json").exists())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => Vec::new(),
    };

    let mut data = handle.write().await;

    let removed: Vec<String> = data
        .active
        .keys()
        .filter(|k| !current_dirs.contains(k))
        .cloned()
        .collect();
    for name in removed {
        tracing::info!("team '{name}' deleted, archiving");
        if let Some(state) = data.active.remove(&name) {
            let archive = archive_team(&state).await;
            save_archive(&name, &archive);
            data.archived.insert(name, archive);
        }
    }

    drop(data);

    for name in &current_dirs {
        let needs_update = {
            let data = handle.read().await;
            !data.active.contains_key(name)
        };
        if needs_update {
            if let Some(state) = load_team_state(name).await {
                let mut data = handle.write().await;
                data.active.insert(name.clone(), state);
            }
        } else {
            update_team_state(handle, name).await;
        }
    }
}

async fn update_team_state(handle: &TeamsHandle, team_name: &str) {
    let teams_dir = teams_dir();
    let config_path = teams_dir.join(team_name).join("config.json");
    let config: TeamConfig = match read_json(&config_path) {
        Some(c) => c,
        None => return,
    };

    let tasks = load_tasks(team_name);

    let mut data = handle.write().await;
    if let Some(team) = data.active.get_mut(team_name) {
        team.config = config.clone();
        team.tasks = tasks;

        let existing_names: Vec<String> = team.members.keys().cloned().collect();
        let new_names: Vec<String> = config.members.iter().map(|m| m.name.clone()).collect();

        for name in &existing_names {
            if !new_names.contains(name) {
                team.members.remove(name);
            }
        }

        for mc in &config.members {
            if !team.members.contains_key(&mc.name) {
                let baseline = capture_baseline(&mc.cwd).await;
                let session_path = resolve_session_path(mc);
                team.members.insert(
                    mc.name.clone(),
                    MemberState {
                        config: mc.clone(),
                        baseline_commit: baseline,
                        cached_summary: None,
                        session_path,
                    },
                );
            } else if let Some(ms) = team.members.get_mut(&mc.name) {
                ms.config = mc.clone();
                if ms.session_path.is_none() {
                    ms.session_path = resolve_session_path(mc);
                }
            }
        }
    }
}

async fn load_team_state(team_name: &str) -> Option<TeamState> {
    let teams_dir = teams_dir();
    let config_path = teams_dir.join(team_name).join("config.json");
    let config: TeamConfig = read_json(&config_path)?;

    let mut members = HashMap::new();
    for mc in &config.members {
        let baseline = capture_baseline(&mc.cwd).await;
        let session_path = resolve_session_path(mc);
        members.insert(
            mc.name.clone(),
            MemberState {
                config: mc.clone(),
                baseline_commit: baseline,
                cached_summary: None,
                session_path,
            },
        );
    }

    let tasks = load_tasks(team_name);

    Some(TeamState {
        config,
        members,
        tasks,
    })
}

fn load_tasks(team_name: &str) -> HashMap<String, TaskFile> {
    let tasks_dir = tasks_dir().join(team_name);
    let mut tasks = HashMap::new();
    if !tasks_dir.exists() {
        return tasks;
    }
    let entries = match std::fs::read_dir(&tasks_dir) {
        Ok(e) => e,
        Err(_) => return tasks,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false)
            && path.file_stem().map(|s| s != ".lock").unwrap_or(true)
        {
            if let Some(task) = read_json::<TaskFile>(&path) {
                let id = match &task.id {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.clone(),
                    _ => continue,
                };
                tasks.insert(id, task);
            }
        }
    }
    tasks
}

async fn refresh_diff_summaries(handle: &TeamsHandle) {
    let members_to_refresh: Vec<(String, String, String, Option<String>)> = {
        let data = handle.read().await;
        let mut v = Vec::new();
        for (team_name, team) in &data.active {
            for (member_name, member) in &team.members {
                v.push((
                    team_name.clone(),
                    member_name.clone(),
                    member.config.cwd.clone(),
                    member.baseline_commit.clone(),
                ));
            }
        }
        v
    };

    for (team_name, member_name, cwd, baseline) in members_to_refresh {
        if let Some(ref baseline) = baseline {
            let summary = compute_diff_summary(&cwd, baseline).await;
            let mut data = handle.write().await;
            if let Some(team) = data.active.get_mut(&team_name) {
                if let Some(member) = team.members.get_mut(&member_name) {
                    member.cached_summary = summary;
                }
            }
        }
    }
}

// --- Git operations ---

async fn capture_baseline(cwd: &str) -> Option<String> {
    if cwd.is_empty() || !Path::new(cwd).exists() {
        return None;
    }
    git_head(cwd).await
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

async fn compute_diff_summary(cwd: &str, baseline: &str) -> Option<DiffSummary> {
    if !Path::new(cwd).exists() {
        return None;
    }

    let numstat = tokio::process::Command::new("git")
        .args(["diff", "--numstat", baseline])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    let name_status = tokio::process::Command::new("git")
        .args(["diff", "--name-status", baseline])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    if !numstat.status.success() {
        return None;
    }

    let numstat_out = String::from_utf8_lossy(&numstat.stdout);
    let name_status_out = String::from_utf8_lossy(&name_status.stdout);

    let statuses = parse_name_status(&name_status_out);
    let files = parse_numstat(&numstat_out, &statuses);

    let mut total_additions = 0u32;
    let mut total_deletions = 0u32;
    for f in &files {
        total_additions += f.additions;
        total_deletions += f.deletions;
    }

    Some(DiffSummary {
        files_changed: files.len() as u32,
        additions: total_additions,
        deletions: total_deletions,
        files,
    })
}

async fn compute_diff_full(cwd: &str, baseline: &str) -> Option<String> {
    if !Path::new(cwd).exists() {
        return None;
    }
    let output = tokio::process::Command::new("git")
        .args(["diff", baseline])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_numstat(output: &str, statuses: &HashMap<String, String>) -> Vec<FileStat> {
    let mut files = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let additions = parts[0].parse::<u32>().unwrap_or(0);
        let deletions = parts[1].parse::<u32>().unwrap_or(0);
        let path = parts[2].to_string();
        let status = statuses
            .get(&path)
            .cloned()
            .unwrap_or_else(|| "modified".to_string());
        files.push(FileStat {
            path,
            additions,
            deletions,
            status,
        });
    }
    files
}

fn parse_name_status(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let status = match parts[0].chars().next() {
            Some('A') => "added",
            Some('D') => "deleted",
            Some('M') => "modified",
            Some('R') => "modified",
            Some('C') => "added",
            _ => "modified",
        };
        let path = parts.last().unwrap().to_string();
        map.insert(path, status.to_string());
    }
    map
}

// --- Session resolution ---

fn resolve_session_path(member: &MemberConfig) -> Option<PathBuf> {
    let backend = member.backend_type.as_deref().unwrap_or("claude");
    if backend != "claude" {
        return None;
    }
    let pane_id = member.tmux_pane_id.as_deref()?;
    let active_sessions_path = claude_dir().join("active-sessions.json");
    let sessions: ActiveSessions = read_json(&active_sessions_path)?;

    for session in sessions.sessions.values() {
        if let Some(ref tmux) = session.tmux {
            if tmux.pane_id == pane_id {
                let path = PathBuf::from(&session.transcript_path);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }
    None
}

// --- Archival ---

async fn archive_team(team: &TeamState) -> TeamArchive {
    let summary = build_team_summary(team, true);

    let mut member_diffs = HashMap::new();
    let mut member_tools = HashMap::new();

    for (name, member) in &team.members {
        if let Some(ref baseline) = member.baseline_commit {
            if let Some(diff) = compute_diff_full(&member.config.cwd, baseline).await {
                member_diffs.insert(name.clone(), diff);
            }
        }

        let backend = member.config.backend_type.as_deref().unwrap_or("claude");
        let calls = read_member_tools(member, backend).await;
        if !calls.is_empty() {
            member_tools.insert(name.clone(), calls);
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    TeamArchive {
        name: team.config.name.clone(),
        archived_at: now,
        final_state: summary,
        member_diffs,
        member_tools,
    }
}

fn save_archive(name: &str, archive: &TeamArchive) {
    let dir = archive_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!("failed to create archive dir: {e}");
        return;
    }
    let path = dir.join(format!("{name}.json"));
    match serde_json::to_string_pretty(archive) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                tracing::warn!("failed to write archive {}: {e}", path.display());
            }
        }
        Err(e) => tracing::warn!("failed to serialize archive: {e}"),
    }
}

fn load_archives() -> HashMap<String, TeamArchive> {
    let dir = archive_dir();
    let mut archives = HashMap::new();
    if !dir.exists() {
        return archives;
    }
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return archives,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Some(archive) = read_json::<TeamArchive>(&path) {
                archives.insert(archive.name.clone(), archive);
            }
        }
    }
    archives
}

// --- Helpers ---

fn build_team_summary(team: &TeamState, archived: bool) -> TeamSummary {
    let members: Vec<MemberSummary> = team
        .members
        .values()
        .map(|m| MemberSummary {
            name: m.config.name.clone(),
            agent_type: m.config.agent_type.clone(),
            model: m.config.model.clone(),
            cwd: m.config.cwd.clone(),
            is_active: m.config.is_active.unwrap_or(false),
            color: m.config.color.clone(),
            diff_summary: m.cached_summary.clone(),
        })
        .collect();

    let tasks: Vec<TaskSummary> = team
        .tasks
        .values()
        .map(|t| {
            let id = match &t.id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => String::new(),
            };
            TaskSummary {
                id,
                subject: t.subject.clone(),
                status: t.status.clone(),
                owner: t.owner.clone(),
            }
        })
        .collect();

    let conflicts = detect_conflicts(&members);

    TeamSummary {
        name: team.config.name.clone(),
        description: team.config.description.clone(),
        created_at: team.config.created_at,
        archived,
        members,
        tasks,
        conflicts,
    }
}

fn detect_conflicts(members: &[MemberSummary]) -> Vec<ConflictInfo> {
    let mut file_owners: HashMap<String, Vec<String>> = HashMap::new();
    for member in members {
        if let Some(ref summary) = member.diff_summary {
            for file in &summary.files {
                file_owners
                    .entry(file.path.clone())
                    .or_default()
                    .push(member.name.clone());
            }
        }
    }
    file_owners
        .into_iter()
        .filter(|(_, owners)| owners.len() > 1)
        .map(|(path, members)| ConflictInfo { path, members })
        .collect()
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn claude_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".claude")
}

fn teams_dir() -> PathBuf {
    claude_dir().join("teams")
}

fn tasks_dir() -> PathBuf {
    claude_dir().join("tasks")
}

fn archive_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".nightshift/team-archives")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_parse_numstat_output() {
        let numstat = "10\t2\tsrc/foo.rs\n3\t0\tsrc/bar.rs\n";
        let mut statuses = HashMap::new();
        statuses.insert("src/foo.rs".to_string(), "modified".to_string());
        statuses.insert("src/bar.rs".to_string(), "added".to_string());

        let files = parse_numstat(numstat, &statuses);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/foo.rs");
        assert_eq!(files[0].additions, 10);
        assert_eq!(files[0].deletions, 2);
        assert_eq!(files[0].status, "modified");
        assert_eq!(files[1].path, "src/bar.rs");
        assert_eq!(files[1].additions, 3);
        assert_eq!(files[1].status, "added");
    }

    #[test]
    fn should_parse_name_status_output() {
        let output = "A\tsrc/new.rs\nM\tsrc/main.rs\nD\tsrc/old.rs\nR100\tsrc/renamed.rs\n";
        let map = parse_name_status(output);
        assert_eq!(map.get("src/new.rs").unwrap(), "added");
        assert_eq!(map.get("src/main.rs").unwrap(), "modified");
        assert_eq!(map.get("src/old.rs").unwrap(), "deleted");
        assert_eq!(map.get("src/renamed.rs").unwrap(), "modified");
    }

    #[test]
    fn should_detect_conflicts() {
        let members = vec![
            MemberSummary {
                name: "agent-a".into(),
                agent_type: String::new(),
                model: String::new(),
                cwd: String::new(),
                is_active: true,
                color: None,
                diff_summary: Some(DiffSummary {
                    files_changed: 1,
                    additions: 5,
                    deletions: 0,
                    files: vec![FileStat {
                        path: "shared.rs".into(),
                        additions: 5,
                        deletions: 0,
                        status: "modified".into(),
                    }],
                }),
            },
            MemberSummary {
                name: "agent-b".into(),
                agent_type: String::new(),
                model: String::new(),
                cwd: String::new(),
                is_active: true,
                color: None,
                diff_summary: Some(DiffSummary {
                    files_changed: 1,
                    additions: 3,
                    deletions: 0,
                    files: vec![FileStat {
                        path: "shared.rs".into(),
                        additions: 3,
                        deletions: 0,
                        status: "modified".into(),
                    }],
                }),
            },
        ];
        let conflicts = detect_conflicts(&members);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "shared.rs");
        assert_eq!(conflicts[0].members.len(), 2);
    }

    #[test]
    fn should_detect_no_conflicts_when_files_differ() {
        let members = vec![
            MemberSummary {
                name: "a".into(),
                agent_type: String::new(),
                model: String::new(),
                cwd: String::new(),
                is_active: true,
                color: None,
                diff_summary: Some(DiffSummary {
                    files_changed: 1,
                    additions: 1,
                    deletions: 0,
                    files: vec![FileStat {
                        path: "a.rs".into(),
                        additions: 1,
                        deletions: 0,
                        status: "added".into(),
                    }],
                }),
            },
            MemberSummary {
                name: "b".into(),
                agent_type: String::new(),
                model: String::new(),
                cwd: String::new(),
                is_active: true,
                color: None,
                diff_summary: Some(DiffSummary {
                    files_changed: 1,
                    additions: 1,
                    deletions: 0,
                    files: vec![FileStat {
                        path: "b.rs".into(),
                        additions: 1,
                        deletions: 0,
                        status: "added".into(),
                    }],
                }),
            },
        ];
        let conflicts = detect_conflicts(&members);
        assert_eq!(conflicts.len(), 0);
    }

    #[test]
    fn should_deserialize_team_config() {
        let json = r#"{
            "name": "test-team",
            "description": "A test team",
            "createdAt": 1000,
            "leadAgentId": "lead@test",
            "members": [
                {
                    "agentId": "lead@test",
                    "name": "team-lead",
                    "agentType": "team-lead",
                    "model": "claude-opus-4-6",
                    "cwd": "/tmp/test",
                    "isActive": true,
                    "tmuxPaneId": "%5"
                }
            ]
        }"#;
        let config: TeamConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.name, "test-team");
        assert_eq!(config.members.len(), 1);
        assert_eq!(config.members[0].name, "team-lead");
        assert_eq!(config.members[0].tmux_pane_id, Some("%5".to_string()));
    }

    #[test]
    fn should_deserialize_task_file() {
        let json = r#"{
            "id": 1,
            "subject": "Fix the bug",
            "description": "Details here",
            "status": "in_progress",
            "owner": "implementer",
            "blocks": [],
            "blockedBy": []
        }"#;
        let task: TaskFile = serde_json::from_str(json).unwrap();
        assert_eq!(task.subject, "Fix the bug");
        assert_eq!(task.status, "in_progress");
        assert_eq!(task.owner, Some("implementer".to_string()));
    }
}
