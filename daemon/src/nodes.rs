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
    pub os: String,
    pub arch: String,
    pub daemon_version: String,
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

pub fn register(port: u16, url_override: Option<&str>) -> Result<(String, Node)> {
    let hostname = get_hostname();
    let url = url_override
        .map(String::from)
        .unwrap_or_else(|| format!("http://localhost:{port}"));
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
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    let mut nodes = read_nodes();
    nodes.retain(|n| n.url != url);
    nodes.push(node.clone());
    write_nodes(&nodes)?;

    tracing::info!("registered node {id} at {url}");
    Ok((id, node))
}

pub async fn register_remote(server_url: &str, node: &Node) -> Result<()> {
    reqwest::Client::new()
        .post(format!("{server_url}/nodes"))
        .json(node)
        .send()
        .await?
        .error_for_status()?;
    tracing::info!("registered node {} with server", node.id);
    Ok(())
}

pub async fn deregister_remote(server_url: &str, id: &str) -> Result<()> {
    reqwest::Client::new()
        .delete(format!("{server_url}/nodes/{id}"))
        .send()
        .await?
        .error_for_status()?;
    tracing::info!("deregistered node {id} from server");
    Ok(())
}

pub enum HeartbeatResult {
    Ok,
    NodeExpired,
}

pub async fn heartbeat_remote(server_url: &str, id: &str) -> Result<HeartbeatResult> {
    let resp = reqwest::Client::new()
        .put(format!("{server_url}/nodes/{id}/heartbeat"))
        .send()
        .await?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(HeartbeatResult::NodeExpired);
    }
    resp.error_for_status()?;
    Ok(HeartbeatResult::Ok)
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
