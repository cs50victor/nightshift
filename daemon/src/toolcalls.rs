use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub tool: String,
    pub title: Option<String>,
    pub input_summary: String,
    pub status: String,
    pub timestamp: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberToolHistory {
    pub name: String,
    pub team: String,
    pub backend: String,
    pub tool_calls: Vec<ToolCall>,
    pub stats: ToolStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_compute_tool_stats() {
        let calls = vec![
            ToolCall {
                tool: "Read".into(),
                title: None,
                input_summary: "f".into(),
                status: "completed".into(),
                timestamp: 0,
                duration_ms: None,
            },
            ToolCall {
                tool: "Read".into(),
                title: None,
                input_summary: "f".into(),
                status: "completed".into(),
                timestamp: 0,
                duration_ms: None,
            },
            ToolCall {
                tool: "Edit".into(),
                title: None,
                input_summary: "f".into(),
                status: "completed".into(),
                timestamp: 0,
                duration_ms: None,
            },
            ToolCall {
                tool: "Bash".into(),
                title: None,
                input_summary: "f".into(),
                status: "completed".into(),
                timestamp: 0,
                duration_ms: None,
            },
            ToolCall {
                tool: "Glob".into(),
                title: None,
                input_summary: "f".into(),
                status: "completed".into(),
                timestamp: 0,
                duration_ms: None,
            },
        ];
        let stats = ToolStats::from_calls(&calls);
        assert_eq!(stats.total, 5);
        assert_eq!(stats.reads, 2);
        assert_eq!(stats.edits, 1);
        assert_eq!(stats.bash, 1);
        assert_eq!(stats.other, 1);
    }
}
