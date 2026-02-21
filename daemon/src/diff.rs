use serde::{Deserialize, Serialize};
use std::path::Path;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberDiffDetail {
    pub name: String,
    pub team: String,
    pub cwd: String,
    pub baseline_commit: Option<String>,
    pub current_commit: Option<String>,
    pub diff: String,
}

pub async fn git_head(cwd: &str) -> Option<String> {
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

pub async fn compute_diff_full(cwd: &str, baseline: &str) -> Option<String> {
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
