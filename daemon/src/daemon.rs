use anyhow::Result;
use std::time::Duration;
use tokio::signal;

use crate::update;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600);

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

    tracing::info!("daemon running, waiting for signals...");

    signal::ctrl_c().await?;
    tracing::info!("received shutdown signal, exiting");

    Ok(())
}
