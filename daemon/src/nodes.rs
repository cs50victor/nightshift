use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub name: String,
    pub url: String,
    pub started_at: String,
}

fn nodes_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".nightshift").join("nodes.json")
}

fn get_hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "localhost".to_string())
}

fn read_nodes() -> Vec<Node> {
    let path = nodes_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn write_nodes(nodes: &[Node]) -> Result<()> {
    let path = nodes_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(nodes)?;
    fs::write(&path, content)?;
    Ok(())
}

pub fn register(port: u16) -> Result<String> {
    let hostname = get_hostname();
    let url = format!("http://localhost:{port}");
    let id = format!("{hostname}-{port}");

    let now = time::OffsetDateTime::now_utc();
    let started_at = now
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".into());

    let node = Node {
        id: id.clone(),
        name: hostname,
        url: url.clone(),
        started_at,
    };

    let mut nodes = read_nodes();
    nodes.retain(|n| n.url != url);
    nodes.push(node);
    write_nodes(&nodes)?;

    tracing::info!("registered node {id} at {url}");
    Ok(id)
}

pub fn deregister(id: &str) {
    let mut nodes = read_nodes();
    let before = nodes.len();
    nodes.retain(|n| n.id != id);
    if nodes.len() < before {
        if let Err(e) = write_nodes(&nodes) {
            tracing::warn!("failed to deregister node: {e}");
        } else {
            tracing::info!("deregistered node {id}");
        }
    }
}
