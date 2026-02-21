use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool: String,
    pub title: Option<String>,
    pub input_summary: String,
    pub status: String,
    pub timestamp: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberToolHistory {
    pub name: String,
    pub team: String,
    pub backend: String,
    pub tool_calls: Vec<ToolCall>,
    pub stats: ToolStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStats {
    pub total: u32,
    pub reads: u32,
    pub writes: u32,
    pub edits: u32,
    pub bash: u32,
    pub other: u32,
}

impl ToolStats {
    pub fn from_calls(calls: &[ToolCall]) -> Self {
        let mut stats = Self::default();
        for call in calls {
            stats.total += 1;
            match call.tool.to_lowercase().as_str() {
                "read" => stats.reads += 1,
                "write" => stats.writes += 1,
                "edit" => stats.edits += 1,
                "bash" => stats.bash += 1,
                _ => stats.other += 1,
            }
        }
        stats
    }
}

fn summarize_input(tool: &str, input: &serde_json::Value) -> String {
    match tool.to_lowercase().as_str() {
        "read" => input
            .get("file_path")
            .or_else(|| input.get("filePath"))
            .and_then(|v| v.as_str())
            .unwrap_or(tool)
            .to_string(),
        "write" => input
            .get("file_path")
            .or_else(|| input.get("filePath"))
            .and_then(|v| v.as_str())
            .unwrap_or(tool)
            .to_string(),
        "edit" => input
            .get("file_path")
            .or_else(|| input.get("filePath"))
            .and_then(|v| v.as_str())
            .unwrap_or(tool)
            .to_string(),
        "bash" => {
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.len() > 80 {
                format!("{}...", &cmd[..77])
            } else {
                cmd.to_string()
            }
        }
        "glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or(tool)
            .to_string(),
        "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if path.is_empty() {
                pattern.to_string()
            } else {
                format!("{pattern} in {path}")
            }
        }
        "task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or(tool)
            .to_string(),
        _ => tool.to_string(),
    }
}

// --- OpenCode SQLite reader ---

pub fn read_opencode_tools(session_id: &str) -> Vec<ToolCall> {
    let db_path = opencode_db_path();
    let Some(db_path) = db_path else {
        return Vec::new();
    };
    if !db_path.exists() {
        return Vec::new();
    }

    let conn = match rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to open opencode db: {e}");
            return Vec::new();
        }
    };

    let mut stmt = match conn.prepare(
        "SELECT data, time_created FROM part \
         WHERE session_id = ?1 \
         AND json_extract(data, '$.type') = 'tool' \
         AND json_extract(data, '$.state.status') IN ('completed', 'error') \
         ORDER BY time_created",
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to prepare opencode query: {e}");
            return Vec::new();
        }
    };

    let rows = match stmt.query_map([session_id], |row| {
        let data: String = row.get(0)?;
        let time_created: u64 = row.get(1)?;
        Ok((data, time_created))
    }) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("failed to query opencode tools: {e}");
            return Vec::new();
        }
    };

    let mut calls = Vec::new();
    for row in rows {
        let Ok((data, time_created)) = row else {
            continue;
        };
        if let Some(call) = parse_opencode_part(&data, time_created) {
            calls.push(call);
        }
    }
    calls
}

#[derive(Deserialize)]
struct OpenCodePart {
    tool: Option<String>,
    state: Option<OpenCodeToolState>,
}

#[derive(Deserialize)]
struct OpenCodeToolState {
    status: String,
    input: Option<serde_json::Value>,
    title: Option<String>,
    time: Option<OpenCodeToolTime>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OpenCodeToolTime {
    start: Option<u64>,
    end: Option<u64>,
}

fn parse_opencode_part(data: &str, time_created: u64) -> Option<ToolCall> {
    let part: OpenCodePart = serde_json::from_str(data).ok()?;
    let tool = part.tool?;
    let state = part.state?;
    let input = state.input.as_ref().cloned().unwrap_or(serde_json::Value::Null);
    let summary = summarize_input(&tool, &input);

    let duration_ms = state.time.as_ref().and_then(|t| {
        let start = t.start?;
        let end = t.end?;
        Some(end.saturating_sub(start))
    });

    let timestamp = state
        .time
        .as_ref()
        .and_then(|t| t.start)
        .unwrap_or(time_created);

    let title = if state.status == "error" {
        state.error.or(state.title)
    } else {
        state.title
    };

    Some(ToolCall {
        tool,
        title,
        input_summary: summary,
        status: state.status,
        timestamp,
        duration_ms,
    })
}

fn opencode_db_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let path = if cfg!(target_os = "macos") {
        std::path::PathBuf::from(&home)
            .join("Library/Application Support/opencode/opencode.db")
    } else {
        let xdg_data = std::env::var("XDG_DATA_HOME")
            .unwrap_or_else(|_| format!("{home}/.local/share"));
        std::path::PathBuf::from(xdg_data).join("opencode/opencode.db")
    };
    Some(path)
}

// --- Claude Code JSONL reader ---

pub fn read_claude_tools(transcript_path: &Path) -> Vec<ToolCall> {
    if !transcript_path.exists() {
        return Vec::new();
    }

    let contents = match std::fs::read_to_string(transcript_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to read claude transcript {}: {e}", transcript_path.display());
            return Vec::new();
        }
    };

    let mut tool_uses: Vec<PendingToolUse> = Vec::new();
    let mut calls = Vec::new();

    for line in contents.lines() {
        if line.is_empty() {
            continue;
        }
        let entry: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let timestamp = entry
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(parse_iso_timestamp)
            .unwrap_or(0);

        match msg_type {
            "assistant" => {
                let content = entry
                    .pointer("/message/content")
                    .and_then(|v| v.as_array());
                if let Some(blocks) = content {
                    for block in blocks {
                        if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let input = block.get("input").cloned().unwrap_or(serde_json::Value::Null);
                            tool_uses.push(PendingToolUse {
                                id: id.to_string(),
                                tool: name.to_string(),
                                input,
                                timestamp,
                            });
                        }
                    }
                }
            }
            "user" => {
                let content = entry
                    .pointer("/message/content")
                    .and_then(|v| v.as_array());
                if let Some(blocks) = content {
                    for block in blocks {
                        if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            let tool_use_id = block
                                .get("tool_use_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let is_error = block
                                .get("is_error")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            if let Some(pos) = tool_uses.iter().position(|t| t.id == tool_use_id) {
                                let pending = tool_uses.remove(pos);
                                let summary = summarize_input(&pending.tool, &pending.input);
                                let duration_ms = if timestamp > pending.timestamp {
                                    Some(timestamp - pending.timestamp)
                                } else {
                                    None
                                };
                                calls.push(ToolCall {
                                    tool: pending.tool,
                                    title: None,
                                    input_summary: summary,
                                    status: if is_error {
                                        "error".to_string()
                                    } else {
                                        "completed".to_string()
                                    },
                                    timestamp: pending.timestamp,
                                    duration_ms,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // NOTE(victor): Pending tool_uses without results are still in-flight -- skip them
    calls
}

struct PendingToolUse {
    id: String,
    tool: String,
    input: serde_json::Value,
    timestamp: u64,
}

fn parse_iso_timestamp(s: &str) -> Option<u64> {
    // NOTE(victor): Timestamps are ISO 8601 like "2026-02-20T19:39:14.770Z".
    // Parse to epoch ms without pulling in chrono -- just use time crate already in deps.
    let format = time::format_description::well_known::Rfc3339;
    let dt = time::OffsetDateTime::parse(s, &format).ok()?;
    Some((dt.unix_timestamp() as u64) * 1000 + (dt.millisecond() as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_summarize_bash_input() {
        let input = serde_json::json!({"command": "cargo test --lib", "description": "Run tests"});
        assert_eq!(summarize_input("Bash", &input), "cargo test --lib");
    }

    #[test]
    fn should_truncate_long_bash_commands() {
        let long_cmd = "a".repeat(100);
        let input = serde_json::json!({"command": long_cmd});
        let summary = summarize_input("Bash", &input);
        assert!(summary.len() <= 80);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn should_summarize_file_path_tools() {
        let input = serde_json::json!({"file_path": "/src/main.rs"});
        assert_eq!(summarize_input("Read", &input), "/src/main.rs");
        assert_eq!(summarize_input("Write", &input), "/src/main.rs");
        assert_eq!(summarize_input("Edit", &input), "/src/main.rs");
    }

    #[test]
    fn should_summarize_grep_with_path() {
        let input = serde_json::json!({"pattern": "fn main", "path": "src/"});
        assert_eq!(summarize_input("Grep", &input), "fn main in src/");
    }

    #[test]
    fn should_summarize_grep_without_path() {
        let input = serde_json::json!({"pattern": "fn main"});
        assert_eq!(summarize_input("Grep", &input), "fn main");
    }

    #[test]
    fn should_compute_tool_stats() {
        let calls = vec![
            ToolCall { tool: "Read".into(), title: None, input_summary: "f".into(), status: "completed".into(), timestamp: 0, duration_ms: None },
            ToolCall { tool: "Read".into(), title: None, input_summary: "f".into(), status: "completed".into(), timestamp: 0, duration_ms: None },
            ToolCall { tool: "Edit".into(), title: None, input_summary: "f".into(), status: "completed".into(), timestamp: 0, duration_ms: None },
            ToolCall { tool: "Bash".into(), title: None, input_summary: "f".into(), status: "completed".into(), timestamp: 0, duration_ms: None },
            ToolCall { tool: "Glob".into(), title: None, input_summary: "f".into(), status: "completed".into(), timestamp: 0, duration_ms: None },
        ];
        let stats = ToolStats::from_calls(&calls);
        assert_eq!(stats.total, 5);
        assert_eq!(stats.reads, 2);
        assert_eq!(stats.edits, 1);
        assert_eq!(stats.bash, 1);
        assert_eq!(stats.other, 1);
    }

    #[test]
    fn should_parse_iso_timestamp() {
        let ts = parse_iso_timestamp("2026-02-20T19:39:14.770Z");
        assert!(ts.is_some());
        let ms = ts.unwrap();
        assert!(ms > 1771600000000);
    }

    #[test]
    fn should_parse_opencode_tool_part() {
        let data = r#"{"type":"tool","tool":"edit","callID":"call_1","state":{"status":"completed","input":{"file_path":"/src/main.rs"},"output":"ok","title":"Edit main.rs","time":{"start":1000,"end":1250}}}"#;
        let call = parse_opencode_part(data, 1000).unwrap();
        assert_eq!(call.tool, "edit");
        assert_eq!(call.title, Some("Edit main.rs".to_string()));
        assert_eq!(call.input_summary, "/src/main.rs");
        assert_eq!(call.status, "completed");
        assert_eq!(call.duration_ms, Some(250));
    }

    #[test]
    fn should_read_claude_tools_from_jsonl() {
        let jsonl = concat!(
            r#"{"type":"assistant","timestamp":"2026-02-20T19:39:14.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls"}}]},"uuid":"a1"}"#,
            "\n",
            r#"{"type":"user","timestamp":"2026-02-20T19:39:15.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"file1\nfile2"}]},"uuid":"a2"}"#,
            "\n",
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        std::fs::write(&path, jsonl).unwrap();

        let calls = read_claude_tools(&path);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "Bash");
        assert_eq!(calls[0].input_summary, "ls");
        assert_eq!(calls[0].status, "completed");
        assert_eq!(calls[0].duration_ms, Some(1000));
    }
}
