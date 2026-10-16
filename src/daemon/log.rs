//! Tiny append-only debug log for the daemon.
//!
//! Worker failures and panics must be observable instead of silently dropping
//! prompt segments, so every warning/error is persisted to
//! `~/.cache/paneship/daemon.log` (and echoed to stderr when
//! `PANESHIP_DAEMON_DEBUG` is set).

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let dir = PathBuf::from(home).join(".cache").join("paneship");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("daemon.log"))
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn write_line(level: &str, message: &str) {
    let line = format!("{} [{level}] {message}", timestamp_ms());

    if std::env::var_os("PANESHIP_DAEMON_DEBUG").is_some() {
        eprintln!("{line}");
    }

    if let Some(path) = log_path() {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// Record an informational event.
pub fn info(message: impl AsRef<str>) {
    write_line("info", message.as_ref());
}

/// Record a recoverable problem.
pub fn warn(message: impl AsRef<str>) {
    write_line("warn", message.as_ref());
}

/// Record a failure (panic, worker crash).
pub fn error(message: impl AsRef<str>) {
    write_line("error", message.as_ref());
}
