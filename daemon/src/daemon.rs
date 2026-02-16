use anyhow::Result;
use std::time::Duration;
use tokio::signal;

use crate::update;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600);
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

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let data_dir = std::path::PathBuf::from(home).join(".nightshift");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::write(data_dir.join("opencode.json"), OPENCODE_CONFIG)?;

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

    let node_id = crate::nodes::register(PROXY_PORT).unwrap_or_else(|e| {
        tracing::warn!("failed to register node: {e}");
        String::new()
    });

    tokio::select! {
        status = child.wait() => {
            tracing::error!("opencode exited: {:?}, daemon will exit", status);
            std::process::exit(1);
        }
        result = crate::proxy::serve(OPENCODE_PORT, PROXY_PORT, data_dir.to_string_lossy().into_owned()) => {
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
    Ok(())
}
