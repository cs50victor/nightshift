//! Integration tests for execve self-restart on thaw detection.
//! Covers: child cleanup, port rebind, exec failure, pid file cleanup.

#[path = "support/mod.rs"]
mod support;

#[cfg(unix)]
mod tests {
    use super::support::*;
    use serial_test::serial;
    use std::time::Duration;

    /// After a forced thaw the daemon execve-restarts:
    /// - original opencode child is dead
    /// - second generation exits cleanly via SINGLE_RESTART
    #[test]
    #[serial]
    fn exec_restart_kills_child_and_daemon_exits_cleanly() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();
        let opencode_pid_file = home.path.join("opencode_child.pid");

        let mut daemon = spawn_daemon(
            &home,
            &[
                ("NIGHTSHIFT_TEST_FORCE_THAW", "1"),
                (
                    "FAKE_OPENCODE_PID_FILE",
                    opencode_pid_file.to_str().unwrap(),
                ),
            ],
        );

        // Wait for proxy port (first generation up).
        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "proxy port 19277 never came up in first generation"
        );

        let opencode_pid =
            read_pid_file(&opencode_pid_file).expect("opencode pid file not written");

        // Watchdog fires after ~100ms. Second generation exits after 200ms (SINGLE_RESTART).
        assert!(
            wait_for_child_exit(&mut daemon, Duration::from_secs(20)),
            "daemon did not exit within 20s after forced thaw + single restart"
        );

        // The original opencode child (grandchild) must be dead.
        assert!(
            wait_for_pid_exit(opencode_pid, Duration::from_secs(5)),
            "opencode child pid {} still alive after daemon restart",
            opencode_pid
        );

        // Wait for ports to be fully released before next test.
        wait_for_port_free(19276, Duration::from_secs(5));
        wait_for_port_free(19277, Duration::from_secs(5));
    }

    /// If exec target is invalid, daemon must exit with code 1 (not hang).
    #[test]
    #[serial]
    fn restart_fails_fast_if_exec_path_invalid() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();

        let mut daemon = spawn_daemon(
            &home,
            &[
                ("NIGHTSHIFT_TEST_FORCE_THAW", "1"),
                ("NIGHTSHIFT_TEST_EXEC_TARGET", "/nonexistent_binary_xyz"),
            ],
        );

        // Wait for proxy port (first generation up).
        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "proxy port never came up"
        );

        // Daemon should exit after exec failure.
        assert!(
            wait_for_child_exit(&mut daemon, Duration::from_secs(10)),
            "daemon did not exit after exec failure"
        );

        let status = daemon.wait().unwrap();
        assert_eq!(
            status.code(),
            Some(1),
            "expected exit code 1 on exec failure"
        );

        // Wait for ports to be fully released before next test.
        wait_for_port_free(19276, Duration::from_secs(5));
        wait_for_port_free(19277, Duration::from_secs(5));
    }
}
