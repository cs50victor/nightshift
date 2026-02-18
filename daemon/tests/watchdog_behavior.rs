//! Tests for watchdog OS-thread behavior.
//! Verifies the watchdog fires independently of the tokio runtime.

#[path = "support/mod.rs"]
mod support;

#[cfg(unix)]
mod tests {
    use super::support::*;
    use serial_test::serial;
    use std::time::Duration;

    /// Watchdog must trigger even when the tokio runtime is busy.
    /// NIGHTSHIFT_TEST_FORCE_THAW causes a restart; SINGLE_RESTART makes the
    /// second generation exit cleanly. If the watchdog were a tokio task instead
    /// of an OS thread, a stalled runtime could prevent it from firing.
    #[test]
    #[serial]
    fn watchdog_triggers_and_restarts_daemon() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();

        let mut daemon = spawn_daemon(
            &home,
            &[
                ("NIGHTSHIFT_TEST_FORCE_THAW", "1"),
            ],
        );

        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "proxy port never came up"
        );

        // Watchdog fires ~100ms, second generation exits ~200ms later.
        assert!(
            wait_for_child_exit(&mut daemon, Duration::from_secs(15)),
            "daemon did not restart+exit within 15s -- watchdog may not have fired"
        );
    }

    /// Second generation must exit cleanly (code 0) via SINGLE_RESTART,
    /// not enter a tight exec loop.
    #[test]
    #[serial]
    fn second_generation_exits_cleanly_no_exec_loop() {
        kill_stale_port_holders(19276);
        kill_stale_port_holders(19277);
        let home = TestHome::new();

        let mut daemon = spawn_daemon(
            &home,
            &[
                ("NIGHTSHIFT_TEST_FORCE_THAW", "1"),
            ],
        );

        assert!(
            wait_for_port(19277, Duration::from_secs(15)),
            "proxy port never came up"
        );

        assert!(
            wait_for_child_exit(&mut daemon, Duration::from_secs(15)),
            "daemon did not exit"
        );

        let status = daemon.wait().unwrap();
        assert_eq!(
            status.code(),
            Some(0),
            "expected clean exit from second generation, got {:?}",
            status.code()
        );
    }
}
