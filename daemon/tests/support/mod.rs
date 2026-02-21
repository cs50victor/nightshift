//! Shared test helpers for nightshift-daemon integration tests.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Temporary home directory for a test. Cleaned up on drop.
pub struct TestHome {
    #[allow(dead_code)]
    pub dir: tempfile::TempDir,
    pub path: PathBuf,
}

impl TestHome {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().to_path_buf();
        Self { dir, path }
    }

    pub fn nightshift_dir(&self) -> PathBuf {
        self.path.join(".nightshift")
    }

    pub fn pid_file(&self) -> PathBuf {
        self.nightshift_dir().join("opencode.pid")
    }
}

pub fn write_test_config(home: &TestHome, proxy_port: u16) {
    let dir = home.nightshift_dir();
    std::fs::create_dir_all(&dir).expect("create .nightshift dir");
    let cfg = serde_json::json!({
        "version": 1,
        "serverUrl": "http://localhost:4001",
        "publicUrl": format!("http://localhost:{proxy_port}"),
        "proxyPort": proxy_port
    });
    std::fs::write(
        dir.join("config.json"),
        serde_json::to_string(&cfg).expect("serialize config"),
    )
    .expect("write config");
}

/// Path to the fake_opencode binary (built by cargo as part of this package).
pub fn fake_opencode_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("fake_opencode").to_path_buf()
}

/// Path to the nightshift-daemon binary.
pub fn daemon_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("nightshift-daemon").to_path_buf()
}

/// Spawn the daemon with test-safe environment.
/// - HOME=<temp_home>
/// - PATH prepended with dir containing fake_opencode (renamed to "opencode")
/// - NIGHTSHIFT_NO_UPDATE=1
/// - Any extra env vars from `extra_env`
pub fn spawn_daemon(home: &TestHome, extra_env: &[(&str, &str)]) -> Child {
    let bin_dir = home.path.join("test_bin");
    std::fs::create_dir_all(&bin_dir).unwrap();

    let fake_bin = fake_opencode_bin();
    let opencode_link = bin_dir.join("opencode");
    std::fs::copy(&fake_bin, &opencode_link).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&opencode_link).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&opencode_link, perms).unwrap();
    }

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let mut cmd = Command::new(daemon_bin());
    cmd.arg("daemon")
        .env("HOME", &home.path)
        .env("XDG_CONFIG_HOME", home.path.join(".config"))
        .env("XDG_DATA_HOME", home.path.join(".local/share"))
        .env("XDG_CACHE_HOME", home.path.join(".cache"))
        .env("PATH", &new_path)
        .env("NIGHTSHIFT_NO_UPDATE", "1")
        .env("RUST_LOG", "nightshift_daemon=debug");

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    cmd.spawn().expect("failed to spawn daemon")
}

/// Spawn the daemon using the real `opencode` from PATH.
pub fn spawn_daemon_real_opencode(home: &TestHome, extra_env: &[(&str, &str)]) -> Child {
    let mut cmd = Command::new(daemon_bin());
    cmd.arg("daemon")
        .env("HOME", &home.path)
        .env("XDG_CONFIG_HOME", home.path.join(".config"))
        .env("XDG_DATA_HOME", home.path.join(".local/share"))
        .env("XDG_CACHE_HOME", home.path.join(".cache"))
        .env("NIGHTSHIFT_NO_UPDATE", "1")
        .env("RUST_LOG", "nightshift_daemon=debug");

    for (k, v) in extra_env {
        cmd.env(k, v);
    }

    cmd.spawn().expect("failed to spawn daemon")
}

/// Returns true if the real `opencode` binary is available on PATH.
pub fn has_real_opencode() -> bool {
    Command::new("opencode")
        .arg("--help")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Wait until a TCP port is accepting connections, up to `timeout`.
pub fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Kill any processes holding the given port (macOS/Linux). Best-effort, ignores errors.
#[cfg(unix)]
pub fn kill_stale_port_holders(port: u16) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let output = std::process::Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output();
    if let Ok(out) = output {
        let pids = String::from_utf8_lossy(&out.stdout);
        for pid_str in pids.split_whitespace() {
            if let Ok(pid) = pid_str.parse::<i32>() {
                let _ = kill(Pid::from_raw(pid), Signal::SIGKILL);
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Wait until a TCP port stops accepting connections (is free), up to `timeout`.
pub fn wait_for_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Wait until a PID is no longer alive, up to `timeout`.
#[cfg(unix)]
pub fn wait_for_pid_exit(pid: i32, timeout: Duration) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let deadline = Instant::now() + timeout;
    loop {
        let alive = kill(Pid::from_raw(pid), None).is_ok();
        if !alive {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait for a direct child process to exit, up to `timeout`.
/// Uses try_wait() instead of kill(pid, 0) to correctly detect zombies.
pub fn wait_for_child_exit(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => {}
            Err(_) => return true,
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Read a PID from a file. Returns None if missing or unparseable.
pub fn read_pid_file(path: &Path) -> Option<i32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Kill a Child process and wait for it to exit. Ignores errors.
pub fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
