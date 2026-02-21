use anyhow::Result;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::time::Duration;
use tokio::signal;

use crate::update;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set to true by the watchdog before it kills the opencode child and execve-restarts.
/// Prevents the child.wait() select arm from calling exit(1) during a planned restart.
#[cfg(unix)]
static RESTARTING: AtomicBool = AtomicBool::new(false);

const UPDATE_INTERVAL: Duration = Duration::from_secs(3600);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);
const OPENCODE_CONFIG: &str = include_str!("../opencode.json");
const PLANNER_PROMPT: &str = include_str!("../planner-system-prompt.txt");
const OPENCODE_PORT: u16 = 19276;
const PROXY_PORT: u16 = OPENCODE_PORT + 1;
const WATCHDOG_SLEEP: Duration = Duration::from_secs(5);
const WATCHDOG_THRESHOLD: Duration = Duration::from_secs(5);
const OPENCODE_PID_FILE: &str = "opencode.pid";
const READINESS_TIMEOUT: Duration = Duration::from_secs(8);

#[cfg(unix)]
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

#[cfg(not(unix))]
fn kill_stale_opencode(pid_path: &std::path::Path) {
    let _ = std::fs::remove_file(pid_path);
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

/// Returns true if wall_elapsed exceeds mono_elapsed + threshold, indicating a thaw.
/// Uses saturating arithmetic: if wall_elapsed < mono_elapsed (clock went backwards), returns false.
fn thaw_detected(wall_elapsed: Duration, mono_elapsed: Duration, threshold: Duration) -> bool {
    wall_elapsed > mono_elapsed.saturating_add(threshold)
}

#[cfg(unix)]
#[cfg(any(debug_assertions, test))]
fn check_fd_cloexec() {
    #[cfg(target_os = "linux")]
    {
        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if let Ok(fd) = name.parse::<i32>() {
                        if fd <= 2 {
                            continue;
                        }
                        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                        if flags >= 0 && (flags & libc::FD_CLOEXEC) == 0 {
                            tracing::warn!("fd {} missing FD_CLOEXEC -- will leak across exec", fd);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
fn restart_self_for_thaw(child_pid: Option<i32>) -> ! {
    RESTARTING.store(true, Ordering::SeqCst);
    if let Some(pid) = child_pid {
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        std::thread::sleep(Duration::from_millis(300));
        let still_alive = unsafe { libc::kill(pid, 0) } == 0;
        if still_alive {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }

    #[cfg(any(debug_assertions, test))]
    check_fd_cloexec();

    // NOTE(victor): execve replaces process image. All FD_CLOEXEC fds close automatically.
    // Same PID, so service manager stays happy.
    // NIGHTSHIFT_TEST_EXEC_TARGET lets tests inject a fake binary.
    let exe = if let Ok(target) = std::env::var("NIGHTSHIFT_TEST_EXEC_TARGET") {
        std::path::PathBuf::from(target)
    } else {
        match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("nightshift: current_exe() failed: {e}");
                unsafe { libc::_exit(1) }
            }
        }
    };

    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let err = std::process::Command::new(&exe)
        .args(&args)
        .env_remove("NIGHTSHIFT_TEST_FORCE_THAW") // prevent infinite restart loop in tests
        .env("NIGHTSHIFT_TEST_IS_RESTART", "1")
        .exec();
    eprintln!("nightshift: execve failed: {err}");
    unsafe { libc::_exit(1) }
}

/// Spawn the OS-thread watchdog that detects sprite thaw via SystemTime vs Instant divergence.
///
/// After a sprite hibernates and thaws, CLOCK_MONOTONIC (Instant) does not advance during
/// suspend, but CLOCK_REALTIME (SystemTime) does. A large divergence between the two signals
/// a thaw. The watchdog runs on an OS thread (not a tokio task) so it is immune to epoll
/// corruption -- the same stale epoll that breaks listener.accept() would also break a tokio
/// interval task.
///
/// On detection: sends SIGTERM to the opencode child, then calls execve to replace the process
/// image. Same PID, fresh FDs and a fresh tokio runtime.
#[cfg(unix)]
fn spawn_watchdog(child_pid: Option<i32>) {
    std::thread::spawn(move || {
        let mut last_wall = std::time::SystemTime::now();
        let mut last_mono = std::time::Instant::now();
        loop {
            let sleep_dur = if std::env::var("NIGHTSHIFT_TEST_FORCE_THAW").is_ok() {
                Duration::from_millis(1000)
            } else {
                WATCHDOG_SLEEP
            };
            std::thread::sleep(sleep_dur);

            if std::env::var("NIGHTSHIFT_TEST_FORCE_THAW").is_ok() {
                tracing::info!("NIGHTSHIFT_TEST_FORCE_THAW: triggering synthetic thaw");
                restart_self_for_thaw(child_pid);
            }

            let now_wall = std::time::SystemTime::now();
            let now_mono = std::time::Instant::now();
            let wall_elapsed = now_wall.duration_since(last_wall).unwrap_or(Duration::ZERO);
            let mono_elapsed = now_mono.duration_since(last_mono);
            last_wall = now_wall;
            last_mono = now_mono;
            if thaw_detected(wall_elapsed, mono_elapsed, WATCHDOG_THRESHOLD) {
                tracing::info!(
                    "thaw detected (wall={:.1}s mono={:.1}s), restarting",
                    wall_elapsed.as_secs_f64(),
                    mono_elapsed.as_secs_f64()
                );
                restart_self_for_thaw(child_pid);
            }
        }
    });
}

#[cfg(not(unix))]
fn spawn_watchdog(_child_pid: Option<i32>) {}

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

    if std::env::var("NIGHTSHIFT_TEST_IS_RESTART").is_ok() {
        tracing::info!("NIGHTSHIFT_TEST_IS_RESTART set: second generation exiting cleanly");
        std::thread::sleep(Duration::from_millis(200));
        std::process::exit(0);
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
    let data_dir = std::path::PathBuf::from(&home).join(".nightshift");
    std::fs::create_dir_all(&data_dir)?;
    std::fs::write(data_dir.join("opencode.json"), OPENCODE_CONFIG)?;

    let prompts_dir = std::path::PathBuf::from(&home).join(".agents/prompts");
    std::fs::create_dir_all(&prompts_dir)?;
    std::fs::write(
        prompts_dir.join("planner-system-prompt.txt"),
        PLANNER_PROMPT,
    )?;

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
        .args([
            "serve",
            "--log-level",
            "DEBUG",
            "--print-logs",
            "--port",
            &OPENCODE_PORT.to_string(),
        ])
        .current_dir(&data_dir)
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
        let reg_url = url.clone();
        let reg_node = node.clone();
        tokio::spawn(async move {
            let delays = [
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
            ];
            for (attempt, delay) in delays.iter().enumerate() {
                match crate::nodes::register_remote(&reg_url, &reg_node).await {
                    Ok(()) => return,
                    Err(e) => {
                        tracing::warn!(
                            "remote registration attempt {}/{} failed: {e}",
                            attempt + 1,
                            delays.len()
                        );
                        if attempt < delays.len() - 1 {
                            tokio::time::sleep(*delay).await;
                        }
                    }
                }
            }
            tracing::warn!("remote registration failed after {} attempts", delays.len());
        });
    }

    if let Some(ref url) = server_url {
        let heartbeat_url = url.clone();
        let heartbeat_id = node_id.clone();
        let heartbeat_node = node.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(HEARTBEAT_INTERVAL);
            interval.tick().await;
            loop {
                interval.tick().await;
                match crate::nodes::heartbeat_remote(&heartbeat_url, &heartbeat_id).await {
                    Ok(crate::nodes::HeartbeatResult::Ok) => {}
                    Ok(crate::nodes::HeartbeatResult::NodeExpired) => {
                        tracing::warn!("node expired from server, re-registering");
                        if let Err(e) =
                            crate::nodes::register_remote(&heartbeat_url, &heartbeat_node).await
                        {
                            tracing::warn!("re-registration failed: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("heartbeat failed: {e}");
                    }
                }
            }
        });
    }

    // NOTE(victor): Must be an OS thread -- if epoll is stale after thaw, tokio tasks
    // also stop firing. OS thread uses nanosleep/futex, which resumes regardless.
    spawn_watchdog(child_pid);

    let teams_handle = crate::teams::new_handle();
    tokio::spawn(crate::teams::spawn_watcher(teams_handle.clone()));

    let start_time = std::time::Instant::now();

    tokio::select! {
        status = child.wait() => {
            #[cfg(unix)]
            if RESTARTING.load(Ordering::SeqCst) {
                // Watchdog is about to execve-replace us. Block here -- execve will take over.
                tracing::info!("opencode exited during planned restart, waiting for execve");
                loop { std::thread::sleep(std::time::Duration::from_secs(60)); }
            }
            tracing::error!("opencode exited: {:?}, daemon will exit", status);
            std::process::exit(1);
        }
        result = crate::proxy::serve(OPENCODE_PORT, proxy_port, data_dir.to_string_lossy().into_owned(), start_time, teams_handle.clone()) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn thaw_detected_true_when_wall_exceeds_mono_plus_threshold() {
        assert!(thaw_detected(
            Duration::from_secs(15),
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
    }

    #[test]
    fn thaw_detected_false_at_exact_threshold_boundary() {
        assert!(!thaw_detected(
            Duration::from_secs(10),
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
    }

    #[test]
    fn thaw_detected_false_when_wall_below_threshold() {
        assert!(!thaw_detected(
            Duration::from_secs(6),
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
    }

    #[test]
    fn thaw_detected_false_when_wall_elapsed_zero() {
        assert!(!thaw_detected(
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(5),
        ));
    }

    #[cfg(unix)]
    #[test]
    fn kill_stale_opencode_removes_pidfile_with_dead_pid() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("opencode.pid");
        std::fs::write(&pid_path, i32::MAX.to_string()).unwrap();
        kill_stale_opencode(&pid_path);
        assert!(
            !pid_path.exists(),
            "pid file must be removed even if pid is dead/invalid"
        );
    }
}
