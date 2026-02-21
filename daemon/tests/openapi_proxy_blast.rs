#[path = "support/mod.rs"]
mod support;

#[cfg(unix)]
mod tests {
    use super::support::*;
    use serial_test::serial;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    const TEST_PROXY_PORT: u16 = 19377;

    fn http_get_status(port: u16, path: &str) -> Result<u16, String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;

        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;

        let mut buf = Vec::with_capacity(1024);
        stream.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&buf);
        let status = text
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| "failed to parse status line".to_string())?;
        Ok(status)
    }

    #[test]
    #[serial]
    fn real_opencode_parallel_blast_endpoints_succeed() {
        if !has_real_opencode() {
            eprintln!("skipping: real opencode not found on PATH");
            return;
        }

        kill_stale_port_holders(19276);
        kill_stale_port_holders(TEST_PROXY_PORT);

        let home = TestHome::new();
        write_test_config(&home, TEST_PROXY_PORT);
        let mut daemon = spawn_daemon_real_opencode(&home, &[]);

        assert!(
            wait_for_port(TEST_PROXY_PORT, Duration::from_secs(40)),
            "proxy port never came up"
        );

        let endpoints = Arc::new(vec![
            "/doc",
            "/openapi.json",
            "/project/absolute_path",
            "/teams",
            "/path",
            "/vcs",
            "/session",
            "/command",
        ]);

        for path in endpoints.iter() {
            let status = http_get_status(TEST_PROXY_PORT, path).unwrap_or(0);
            assert_eq!(status, 200, "warmup failed for {path}: status {status}");
        }

        let failures: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let workers = 24;
        let rounds_per_worker = 20;

        let mut handles = Vec::new();
        for _ in 0..workers {
            let endpoints = Arc::clone(&endpoints);
            let failures = Arc::clone(&failures);
            handles.push(thread::spawn(move || {
                for _ in 0..rounds_per_worker {
                    for path in endpoints.iter() {
                        match http_get_status(TEST_PROXY_PORT, path) {
                            Ok(200) => {}
                            Ok(code) => failures
                                .lock()
                                .unwrap()
                                .push(format!("{path} returned status {code}")),
                            Err(err) => failures
                                .lock()
                                .unwrap()
                                .push(format!("{path} request error: {err}")),
                        }
                    }
                }
            }));
        }

        for h in handles {
            let _ = h.join();
        }

        kill_and_wait(&mut daemon);

        let failures = failures.lock().unwrap();
        assert!(
            failures.is_empty(),
            "blast test found endpoint failures: {}",
            failures
                .iter()
                .take(10)
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
}
