CREATE TABLE teams (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT NOT NULL DEFAULT '',
    created_at_ms   INTEGER NOT NULL,
    archived_at_ms  INTEGER,
    lead_agent_id   TEXT NOT NULL,
    lead_session_id TEXT
);

CREATE TABLE members (
    id                  TEXT PRIMARY KEY,
    team_id             TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    agent_type          TEXT NOT NULL,
    model               TEXT NOT NULL,
    backend_type        TEXT NOT NULL,
    color               TEXT,
    cwd                 TEXT NOT NULL,
    plan_mode_required  INTEGER NOT NULL DEFAULT 0,
    tmux_target         TEXT,
    opencode_session_id TEXT,
    joined_at_ms        INTEGER NOT NULL,
    removed_at_ms       INTEGER,
    UNIQUE(team_id, name)
);

CREATE TABLE member_runs (
    id                  TEXT PRIMARY KEY,
    team_id             TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    member_id           TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    started_at_ms       INTEGER NOT NULL,
    ended_at_ms         INTEGER,
    end_reason          TEXT,
    spawn_command       TEXT,
    pid                 INTEGER,
    tmux_target         TEXT,
    backend_session_ref TEXT
);
CREATE INDEX idx_member_runs_lookup ON member_runs(team_id, member_id, started_at_ms);

CREATE TABLE tasks (
    id               TEXT PRIMARY KEY,
    team_id          TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    external_task_id TEXT NOT NULL,
    subject          TEXT NOT NULL,
    description      TEXT NOT NULL,
    active_form      TEXT NOT NULL DEFAULT '',
    status           TEXT NOT NULL,
    owner_member_id  TEXT REFERENCES members(id),
    metadata_json    TEXT,
    created_at_ms    INTEGER NOT NULL,
    updated_at_ms    INTEGER NOT NULL,
    UNIQUE(team_id, external_task_id)
);

CREATE TABLE task_dependencies (
    team_id            TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    task_id            TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    blocked_by_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY(team_id, task_id, blocked_by_task_id)
);

CREATE TABLE messages (
    id             TEXT PRIMARY KEY,
    team_id        TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    from_member_id TEXT REFERENCES members(id),
    to_member_id   TEXT REFERENCES members(id),
    message_type   TEXT NOT NULL,
    summary        TEXT,
    content_text   TEXT,
    payload_json   TEXT,
    created_at_ms  INTEGER NOT NULL
);

CREATE TABLE inbox_state (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    member_id  TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    read_at_ms INTEGER,
    PRIMARY KEY(message_id, member_id)
);

CREATE TABLE member_status_current (
    member_id                 TEXT PRIMARY KEY REFERENCES members(id) ON DELETE CASCADE,
    run_id                    TEXT REFERENCES member_runs(id),
    alive                     INTEGER NOT NULL DEFAULT 0,
    state                     TEXT NOT NULL,
    headline                  TEXT,
    active_tool_name          TEXT,
    active_tool_started_at_ms INTEGER,
    pending_from_count        INTEGER NOT NULL DEFAULT 0,
    their_unread_count        INTEGER NOT NULL DEFAULT 0,
    last_heartbeat_ms         INTEGER NOT NULL,
    last_error                TEXT
);

CREATE TABLE member_status_events (
    id            TEXT PRIMARY KEY,
    member_id     TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    run_id        TEXT REFERENCES member_runs(id),
    event_type    TEXT NOT NULL,
    state         TEXT,
    headline      TEXT,
    payload_json  TEXT,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_status_events_lookup ON member_status_events(member_id, created_at_ms);

CREATE TABLE tool_calls (
    id               TEXT PRIMARY KEY,
    team_id          TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    member_id        TEXT NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    run_id           TEXT REFERENCES member_runs(id),
    backend_type     TEXT NOT NULL,
    source           TEXT NOT NULL,
    external_call_id TEXT,
    tool_name        TEXT NOT NULL,
    tool_title       TEXT,
    input_summary    TEXT NOT NULL,
    input_json       TEXT,
    status           TEXT NOT NULL,
    error_text       TEXT,
    started_at_ms    INTEGER NOT NULL,
    ended_at_ms      INTEGER,
    duration_ms      INTEGER,
    ingested_at_ms   INTEGER NOT NULL
);
CREATE INDEX idx_tool_calls_member ON tool_calls(team_id, member_id, started_at_ms);
CREATE INDEX idx_tool_calls_team ON tool_calls(team_id, started_at_ms);
CREATE UNIQUE INDEX idx_tool_calls_dedup ON tool_calls(member_id, source, external_call_id, started_at_ms);

CREATE TABLE activity_log (
    id            TEXT PRIMARY KEY,
    team_id       TEXT NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    member_id     TEXT REFERENCES members(id),
    kind          TEXT NOT NULL,
    payload_json  TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_activity_log_lookup ON activity_log(team_id, created_at_ms);

CREATE TABLE ingest_cursors (
    id            TEXT PRIMARY KEY,
    cursor_type   TEXT NOT NULL,
    member_id     TEXT,
    cursor_value  TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
