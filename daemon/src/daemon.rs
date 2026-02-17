use anyhow::Result;
use std::time::Duration;
use tokio::signal;

use crate::update;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const OPENCODE_CONFIG: &str = include_str!("../opencode.json");
const OPENCODE_PORT: u16 = 19276;
const PROXY_PORT: u16 = OPENCODE_PORT + 1;

pub async fn run() -> Result<()> {
    tracing::info!("starting nightshift daemon v{}", env!("CARGO_PKG_VERSION"));

    if update::is_enabled() {
        match update::check_and_apply().await {
            Ok(true) => {
                tracing::info!("binary updated, exiting for restart");
                std::process::exit(0);
            }
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("startup update check failed: {}", e);
            }
        }
    } else {
        tracing::info!("self-update disabled via NIGHTSHIFT_NO_UPDATE");
    }

    if update::is_enabled() {
        tokio::spawn(async {
            let mut interval = tokio::time::interval(UPDATE_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                match update::check_and_apply().await {
                    Ok(true) => {
                        tracing::info!("binary updated, exiting for restart");
                        std::process::exit(0);
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!("periodic update check failed: {}", e);
                    }
                }
            }
        });
    }

    let cfg = crate::config::load();

    let proxy_port = cfg.as_ref().map(|c| c.proxy_port).unwrap_or(PROXY_PORT);
    let url_override = cfg.as_ref().map(|c| c.public_url.as_str());

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let data_dir = std::path::PathBuf::from(home).join(".nightshift");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::write(data_dir.join("opencode.json"), OPENCODE_CONFIG)?;

    if let Ok(bun_install) = std::env::var("BUN_INSTALL") {
        let bun_bin = format!("{bun_install}/bin");
        let path = std::env::var("PATH").unwrap_or_default();
        if !path.contains(&bun_bin) {
            std::env::set_var("PATH", format!("{bun_bin}:{path}"));
        }
    }

    // NOTE(victor): No supervisor. If opencode dies, the daemon exits.
    // launchd/systemd restarts the daemon, which restarts opencode.
    let mut child = tokio::process::Command::new("opencode")
        .args(["serve", "--port", &OPENCODE_PORT.to_string()])
        .current_dir(&data_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    tracing::info!(
        "spawned opencode on port {} (pid {})",
        OPENCODE_PORT,
        child.id().unwrap_or(0)
    );

    let (node_id, node) = crate::nodes::register(proxy_port, url_override)?;

    let server_url = cfg.as_ref().map(|c| c.server_url.clone());

    if let Some(ref url) = server_url {
        if let Err(e) = crate::nodes::register_remote(url, &node).await {
            tracing::warn!("remote registration failed: {e}");
        }
    }

    if let Some(ref url) = server_url {
        let heartbeat_url = url.clone();
        let heartbeat_id = node_id.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Err(e) =
                    crate::nodes::heartbeat_remote(&heartbeat_url, &heartbeat_id).await
                {
                    tracing::warn!("heartbeat failed: {e}");
                }
            }
        });
    }

    tokio::select! {
        status = child.wait() => {
            tracing::error!("opencode exited: {:?}, daemon will exit", status);
            std::process::exit(1);
        }
        result = crate::proxy::serve(OPENCODE_PORT, proxy_port, data_dir.to_string_lossy().into_owned()) => {
            tracing::error!("proxy server failed: {:?}", result);
            child.kill().await.ok();
            std::process::exit(1);
        }
        _ = signal::ctrl_c() => {
            tracing::info!("received shutdown signal, killing opencode");
            child.kill().await.ok();
        }
    }

    crate::nodes::deregister(&node_id);
    if let Some(ref url) = server_url {
        crate::nodes::deregister_remote(url, &node_id).await.ok();
    }
    Ok(())
}
