use anyhow::Result;
use std::time::Duration;
use tokio::signal;

use crate::update;

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const OPENCODE_CONFIG: &str = include_str!("../opencode.json");
const OPENCODE_PORT: u16 = 19276;
const PROXY_PORT: u16 = OPENCODE_PORT + 1;
const WATCHDOG_SLEEP: Duration = Duration::from_secs(5);
const WATCHDOG_THRESHOLD: Duration = Duration::from_secs(5);
const OPENCODE_PID_FILE: &str = "opencode.pid";
const READINESS_TIMEOUT: Duration = Duration::from_secs(8);

fn kill_stale_opencode(pid_path: &std::path::Path) {
    if let Ok(contents) = std::fs::read_to_string(pid_path) {
        if let Ok(pid) = contents.trim().parse::<i32>() {
            tracing::info!("killing stale opencode pid {}", pid);
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            std::thread::sleep(Duration::from_millis(500));
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
        let _ = std::fs::remove_file(pid_path);
    }
}

async fn wait_for_opencode(port: u16, timeout: Duration) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("opencode did not become ready within {:?}", timeout);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Spawn the OS-thread watchdog that detects sprite thaw via SystemTime vs Instant divergence.
///
/// After a sprite hibernates and thaws, CLOCK_MONOTONIC (Instant) does not advance during
/// suspend, but CLOCK_REALTIME (SystemTime) does. A large divergence between the two signals
/// a thaw. The watchdog runs on an OS thread (not a tokio task) so it is immune to epoll
/// corruption -- the same stale epoll that breaks listener.accept() would also break a tokio
/// interval task.
///
/// On detection: sends SIGTERM to the opencode child, then calls exit(0). The service manager
/// restarts the daemon, giving fresh FDs and a fresh tokio runtime.
fn spawn_watchdog(child_pid: Option<i32>) {
    std::thread::spawn(move || {
        let mut last_wall = std::time::SystemTime::now();
        let mut last_mono = std::time::Instant::now();
        loop {
            std::thread::sleep(WATCHDOG_SLEEP);
            let now_wall = std::time::SystemTime::now();
            let now_mono = std::time::Instant::now();
            let wall_elapsed = now_wall
                .duration_since(last_wall)
                .unwrap_or(Duration::ZERO);
            let mono_elapsed = now_mono.duration_since(last_mono);
            last_wall = now_wall;
            last_mono = now_mono;
            if wall_elapsed > mono_elapsed + WATCHDOG_THRESHOLD {
                tracing::info!(
                    "thaw detected (wall={:.1}s mono={:.1}s), restarting",
                    wall_elapsed.as_secs_f64(),
                    mono_elapsed.as_secs_f64()
                );
                if let Some(pid) = child_pid {
                    unsafe {
                        libc::kill(pid, libc::SIGTERM);
                    }
                }
                std::process::exit(0);
            }
        }
    });
}

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

    let pid_path = data_dir.join(OPENCODE_PID_FILE);
    kill_stale_opencode(&pid_path);

    // NOTE(victor): No supervisor. If opencode dies, the daemon exits.
    // The service manager (launchd/systemd/sprites supervisor) restarts the daemon,
    // which spawns a fresh opencode. This gives us fresh FDs and a clean epoll state.
    let mut child = tokio::process::Command::new("opencode")
        .args(["serve", "--port", &OPENCODE_PORT.to_string()])
        .current_dir(&data_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    let child_pid = child.id().map(|p| p as i32);

    tracing::info!(
        "spawned opencode on port {} (pid {})",
        OPENCODE_PORT,
        child.id().unwrap_or(0)
    );

    if let Some(pid) = child.id() {
        let _ = std::fs::write(&pid_path, pid.to_string());
    }

    wait_for_opencode(OPENCODE_PORT, READINESS_TIMEOUT).await?;
    tracing::info!("opencode ready on port {}", OPENCODE_PORT);

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

    // NOTE(victor): Must be an OS thread -- if epoll is stale after thaw, tokio tasks
    // also stop firing. OS thread uses nanosleep/futex, which resumes regardless.
    spawn_watchdog(child_pid);

    let start_time = std::time::Instant::now();

    tokio::select! {
        status = child.wait() => {
            tracing::error!("opencode exited: {:?}, daemon will exit", status);
            std::process::exit(1);
        }
        result = crate::proxy::serve(OPENCODE_PORT, proxy_port, data_dir.to_string_lossy().into_owned(), start_time) => {
            tracing::error!("proxy server failed: {:?}", result);
            child.kill().await.ok();
            std::process::exit(1);
        }
        _ = signal::ctrl_c() => {
            tracing::info!("received shutdown signal, killing opencode");
            child.kill().await.ok();
        }
    }

    let _ = std::fs::remove_file(&pid_path);
    crate::nodes::deregister(&node_id);
    if let Some(ref url) = server_url {
        crate::nodes::deregister_remote(url, &node_id).await.ok();
    }
    Ok(())
}
