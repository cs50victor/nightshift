//! Fake opencode binary for integration tests.
//! Listens on the requested port, writes PID to a file if FAKE_OPENCODE_PID_FILE is set,
//! and exits on SIGTERM (default behavior).

use std::net::TcpListener;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .windows(2)
        .find(|w| w[0] == "--port")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(19276);

    if let Ok(path) = std::env::var("FAKE_OPENCODE_PID_FILE") {
        let _ = std::fs::write(&path, std::process::id().to_string());
    }

    let _listener =
        TcpListener::bind(("127.0.0.1", port)).expect("fake_opencode: failed to bind port");

    // Sleep until killed (SIGTERM/SIGKILL from daemon).
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
