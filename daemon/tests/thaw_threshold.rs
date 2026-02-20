//! Tests that sub-threshold clock divergence does NOT trigger a restart (no false positives).

#[path = "support/mod.rs"]
mod support;

#[cfg(unix)]
mod tests {
    use super::support::*;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    use serial_test::serial;
    use std::time::Duration;

    /// Without NIGHTSHIFT_TEST_FORCE_THAW, the daemon must NOT restart spontaneously
    /// within a short window.
    #[test]
    #[serial]
    fn no_spurious_restart_without_thaw_trigger() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();

        let mut daemon = spawn_daemon(&home, &[]);

        let daemon_pid = daemon.id() as i32;

        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "proxy port never came up"
        );

        // Run for 3 seconds. Daemon must still be alive.
        std::thread::sleep(Duration::from_secs(3));

        let still_alive = kill(Pid::from_raw(daemon_pid), None).is_ok();
        assert!(
            still_alive,
            "daemon exited spontaneously -- possible false-positive thaw detection"
        );

        kill_and_wait(&mut daemon);
    }
}
