//! Tests that no listener FDs leak across execve.
//! If a listener FD leaked, the new generation would fail to rebind with EADDRINUSE.

#[path = "support/mod.rs"]
mod support;

#[cfg(unix)]
mod tests {
    use super::support::*;
    use serial_test::serial;
    use std::time::Duration;

    /// After execve restart, the new generation must rebind the proxy port.
    /// FD leak would cause EADDRINUSE and the daemon would exit with an error.
    #[test]
    #[serial]
    fn new_generation_rebinds_proxy_port_after_exec() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();

        let mut daemon = spawn_daemon(
            &home,
            &[
                ("NIGHTSHIFT_TEST_FORCE_THAW", "1"),
                // No SINGLE_RESTART: let second generation run normally.
            ],
        );

        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "first generation proxy port never came up"
        );

        // Watchdog fires after ~100ms. Give second generation time to come up.
        std::thread::sleep(Duration::from_millis(500));

        // Port should still be serving (second generation rebound it).
        assert!(
            wait_for_port(19277, Duration::from_secs(10)),
            "proxy port not available after execve restart -- possible FD leak"
        );

        kill_and_wait(&mut daemon);
    }
}
