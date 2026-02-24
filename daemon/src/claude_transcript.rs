use anyhow::{Context, Result};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
pub enum ClaudeEvent {
    ToolUse(ClaudeToolUse),
    ToolResult(ClaudeToolResult),
    MemberStatus(ClaudeMemberStatus),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeToolUse {
    pub external_call_id: Option<String>,
    pub tool_name: String,
    pub input_json: Option<String>,
    pub started_at_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeToolResult {
    pub external_call_id: Option<String>,
    pub tool_name: Option<String>,
    pub output_json: Option<String>,
    pub error_text: Option<String>,
    pub ended_at_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeMemberStatus {
    pub state: String,
    pub headline: Option<String>,
    pub payload_json: Option<String>,
    pub created_at_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

pub fn parse_jsonl_line(line: &str) -> Result<Vec<ClaudeEvent>> {
    let v: Value = serde_json::from_str(line).context("failed to parse transcript json line")?;
    let mut events = Vec::new();

    let ts = timestamp_ms(v.get("timestamp").and_then(Value::as_str));
    let session_id = v
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let agent_id = v.get("agentId").and_then(Value::as_str).map(str::to_string);

    match v.get("type").and_then(Value::as_str) {
        Some("tool_use") => {
            if let Some(tool_name) = v.get("tool_name").and_then(Value::as_str) {
                events.push(ClaudeEvent::ToolUse(ClaudeToolUse {
                    external_call_id: None,
                    tool_name: tool_name.to_string(),
                    input_json: stringify_json(v.get("tool_input")),
                    started_at_ms: ts,
                    session_id,
                    agent_id,
                }));
            }
        }
        Some("tool_result") => {
            let tool_name = v
                .get("tool_name")
                .and_then(Value::as_str)
                .map(str::to_string);
            events.push(ClaudeEvent::ToolResult(ClaudeToolResult {
                external_call_id: None,
                tool_name,
                output_json: stringify_json(v.get("tool_output")),
                error_text: tool_error_text(v.get("tool_output")),
                ended_at_ms: ts,
                session_id,
                agent_id,
            }));
        }
        Some("assistant") => {
            parse_assistant_tool_uses(&v, ts, &session_id, &agent_id, &mut events);
        }
        Some("user") => {
            parse_user_tool_results(&v, ts, &session_id, &agent_id, &mut events);
        }
        Some("progress") => {
            parse_progress_status(&v, ts, &session_id, &agent_id, &mut events);
        }
        Some("system") => {
            parse_system_status(&v, ts, &session_id, &agent_id, &mut events);
        }
        _ => {}
    }

    Ok(events)
}

fn parse_assistant_tool_uses(
    v: &Value,
    ts: i64,
    session_id: &Option<String>,
    agent_id: &Option<String>,
    out: &mut Vec<ClaudeEvent>,
) {
    let Some(content) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return;
    };

    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let Some(tool_name) = part.get("name").and_then(Value::as_str) else {
            continue;
        };
        let external_call_id = part.get("id").and_then(Value::as_str).map(str::to_string);
        out.push(ClaudeEvent::ToolUse(ClaudeToolUse {
            external_call_id,
            tool_name: tool_name.to_string(),
            input_json: stringify_json(part.get("input")),
            started_at_ms: ts,
            session_id: session_id.clone(),
            agent_id: agent_id.clone(),
        }));
    }
}

fn parse_user_tool_results(
    v: &Value,
    ts: i64,
    session_id: &Option<String>,
    agent_id: &Option<String>,
    out: &mut Vec<ClaudeEvent>,
) {
    let content = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array);

    let mut saw_tool_result_part = false;
    if let Some(content) = content {
        for part in content {
            if part.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            saw_tool_result_part = true;
            let external_call_id = part
                .get("tool_use_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            out.push(ClaudeEvent::ToolResult(ClaudeToolResult {
                external_call_id,
                tool_name: None,
                output_json: stringify_json(part.get("content")),
                error_text: None,
                ended_at_ms: ts,
                session_id: session_id.clone(),
                agent_id: agent_id.clone(),
            }));
        }
    }

    if !saw_tool_result_part {
        if let Some(tool_use_result) = v.get("toolUseResult") {
            let external_call_id = first_non_empty_str(&[
                v.get("toolUseID").and_then(Value::as_str),
                v.get("parentToolUseID").and_then(Value::as_str),
            ])
            .map(str::to_string);
            out.push(ClaudeEvent::ToolResult(ClaudeToolResult {
                external_call_id,
                tool_name: None,
                output_json: Some(tool_use_result.to_string()),
                error_text: None,
                ended_at_ms: ts,
                session_id: session_id.clone(),
                agent_id: agent_id.clone(),
            }));
        }
    }
}

fn parse_progress_status(
    v: &Value,
    ts: i64,
    session_id: &Option<String>,
    agent_id: &Option<String>,
    out: &mut Vec<ClaudeEvent>,
) {
    let data = v.get("data");
    let Some(data) = data else {
        return;
    };
    let state = match data.get("type").and_then(Value::as_str) {
        Some("waiting_for_task") => "waiting_for_task",
        Some("hook_progress") => "hook_progress",
        Some(other) => other,
        None => return,
    };

    out.push(ClaudeEvent::MemberStatus(ClaudeMemberStatus {
        state: state.to_string(),
        headline: data
            .get("taskDescription")
            .and_then(Value::as_str)
            .map(str::to_string),
        payload_json: Some(data.to_string()),
        created_at_ms: ts,
        session_id: session_id.clone(),
        agent_id: agent_id.clone(),
    }));
}

fn parse_system_status(
    v: &Value,
    ts: i64,
    session_id: &Option<String>,
    agent_id: &Option<String>,
    out: &mut Vec<ClaudeEvent>,
) {
    if v.get("subtype").and_then(Value::as_str) != Some("api_error") {
        return;
    }

    let headline = v
        .get("error")
        .and_then(|e| e.get("error"))
        .and_then(|e| e.get("error"))
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            v.get("error")
                .and_then(|e| e.get("error"))
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(str::to_string)
        });

    out.push(ClaudeEvent::MemberStatus(ClaudeMemberStatus {
        state: "api_error".to_string(),
        headline,
        payload_json: Some(v.to_string()),
        created_at_ms: ts,
        session_id: session_id.clone(),
        agent_id: agent_id.clone(),
    }));
}

fn timestamp_ms(ts: Option<&str>) -> i64 {
    let Some(ts) = ts else { return 0 };
    OffsetDateTime::parse(ts, &Rfc3339)
        .map(|dt| dt.unix_timestamp_nanos() / 1_000_000)
        .unwrap_or(0) as i64
}

fn stringify_json(v: Option<&Value>) -> Option<String> {
    v.map(ToString::to_string)
}

fn tool_error_text(tool_output: Option<&Value>) -> Option<String> {
    let output = tool_output?;
    if let Some(exit) = output.get("exit").and_then(Value::as_i64) {
        if exit != 0 {
            return Some(format!("tool exited with status {exit}"));
        }
    }
    output
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn first_non_empty_str<'a>(values: &[Option<&'a str>]) -> Option<&'a str> {
    values.iter().flatten().find(|s| !s.is_empty()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_tool_use() {
        let line = r#"{"type":"tool_use","timestamp":"2026-01-03T07:21:59.801Z","tool_name":"bash","tool_input":{"command":"bd ready"}}"#;
        let events = parse_jsonl_line(line).unwrap();
        assert_eq!(events.len(), 1);

        let ClaudeEvent::ToolUse(ev) = &events[0] else {
            panic!("expected tool use");
        };
        assert_eq!(ev.tool_name, "bash");
        assert!(ev.external_call_id.is_none());
        assert!(ev
            .input_json
            .as_deref()
            .unwrap_or_default()
            .contains("bd ready"));
        assert!(ev.started_at_ms > 0);
    }

    #[test]
    fn parses_rich_assistant_tool_use() {
        let line = r#"{"type":"assistant","timestamp":"2026-02-21T22:02:52.670Z","sessionId":"sess-1","agentId":"agent-1","message":{"content":[{"type":"tool_use","id":"toolu_123","name":"Grep","input":{"pattern":"rusqlite"}}]}}"#;
        let events = parse_jsonl_line(line).unwrap();
        assert_eq!(events.len(), 1);

        let ClaudeEvent::ToolUse(ev) = &events[0] else {
            panic!("expected tool use");
        };
        assert_eq!(ev.tool_name, "Grep");
        assert_eq!(ev.external_call_id.as_deref(), Some("toolu_123"));
        assert_eq!(ev.session_id.as_deref(), Some("sess-1"));
        assert_eq!(ev.agent_id.as_deref(), Some("agent-1"));
    }

    #[test]
    fn parses_rich_user_tool_result() {
        let line = r#"{"type":"user","timestamp":"2026-02-21T22:02:52.789Z","sessionId":"sess-1","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"ok"}]},"toolUseResult":{"mode":"content","content":"ok"}}"#;
        let events = parse_jsonl_line(line).unwrap();
        assert_eq!(events.len(), 1);

        let ClaudeEvent::ToolResult(first) = &events[0] else {
            panic!("expected tool result");
        };
        assert_eq!(first.external_call_id.as_deref(), Some("toolu_123"));
        assert_eq!(first.session_id.as_deref(), Some("sess-1"));

        assert!(first
            .output_json
            .as_deref()
            .unwrap_or_default()
            .contains("ok"));
    }

    #[test]
    fn parses_waiting_for_task_status() {
        let line = r#"{"type":"progress","timestamp":"2026-02-21T22:08:45.979Z","sessionId":"sess-1","agentId":"agent-1","data":{"type":"waiting_for_task","taskDescription":"Search lightweight SQLite Rust crates"}}"#;
        let events = parse_jsonl_line(line).unwrap();
        assert_eq!(events.len(), 1);

        let ClaudeEvent::MemberStatus(status) = &events[0] else {
            panic!("expected member status");
        };
        assert_eq!(status.state, "waiting_for_task");
        assert_eq!(
            status.headline.as_deref(),
            Some("Search lightweight SQLite Rust crates")
        );
    }

    #[test]
    fn parses_api_error_status() {
        let line = r#"{"type":"system","subtype":"api_error","timestamp":"2026-02-21T22:06:37.546Z","error":{"error":{"error":{"message":"Overloaded"}}}}"#;
        let events = parse_jsonl_line(line).unwrap();
        assert_eq!(events.len(), 1);

        let ClaudeEvent::MemberStatus(status) = &events[0] else {
            panic!("expected member status");
        };
        assert_eq!(status.state, "api_error");
        assert_eq!(status.headline.as_deref(), Some("Overloaded"));
    }
}
