//! End-to-end daemon lifecycle test.
//!
//! Exercises the real binary: a stale socket file must be reclaimed, a fresh
//! daemon must come up and answer `ping`, and the client-driven
//! `invalidate` signal must be accepted.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_runtime_dir() -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("paneship-it-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn paneship() -> &'static str {
    env!("CARGO_BIN_EXE_paneship")
}

fn run_with_runtime(args: &[&str], runtime_dir: &Path) -> bool {
    Command::new(paneship())
        .args(args)
        .env("XDG_RUNTIME_DIR", runtime_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn daemon_heals_stale_socket_and_serves_clients() {
    let runtime_dir = unique_runtime_dir();
    let socket_path = runtime_dir.join("paneship.sock");

    // Simulate a crashed daemon: bind and drop, leaving the socket file behind.
    {
        let _stale = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
    }
    assert!(socket_path.exists(), "stale socket should exist");

    let mut daemon = Command::new(paneship())
        .arg("daemon")
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn daemon");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut ready = false;
    while Instant::now() < deadline {
        if run_with_runtime(&["daemon", "ping"], &runtime_dir) {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    if !ready {
        let _ = daemon.kill();
        let _ = daemon.wait();
        let _ = std::fs::remove_dir_all(&runtime_dir);
        panic!("daemon did not become ready within the timeout");
    }

    let invalidated = Command::new(paneship())
        .args(["invalidate", "--cwd"])
        .arg(&runtime_dir)
        .env("XDG_RUNTIME_DIR", &runtime_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    assert!(
        invalidated,
        "invalidate should succeed against a live daemon"
    );

    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = std::fs::remove_dir_all(&runtime_dir);
}
