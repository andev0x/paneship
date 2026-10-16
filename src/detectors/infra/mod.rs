//! Infrastructure & environment detector suite (Phase 3, Module B).
//!
//! Extends the Phase 2 detector model with low-overhead system and cloud
//! extractors. Unlike language detectors, infrastructure detectors are
//! **best-effort and non-fatal**: every one runs inside a detached worker with
//! a hard [`DETECTOR_TIMEOUT`] budget, so a stalled network mount or a huge
//! `~/.kube/config` can never block the daemon worker or the prompt.
//!
//! Nothing here is ever invoked on the hot render path: the daemon resolves a
//! snapshot in the background and serves cached data (or an empty state).

pub mod cloud;
pub mod container;
pub mod health;
pub mod kubernetes;
pub mod nix;

use crate::core::config::EnvironmentConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Maximum bytes read from a single configuration file (256 KiB).
pub(crate) const MAX_CONFIG_BYTES: u64 = 256 * 1024;

/// Hard per-detector execution budget. A detector that exceeds it yields no
/// result (the worker keeps running detached and is discarded).
pub const DETECTOR_TIMEOUT: Duration = Duration::from_millis(10);

/// Looser budget for opt-in health checks, which may shell out to a platform
/// helper (`pmset` on macOS). Health is off by default and always resolved in
/// the background, so it never touches the render path.
pub const HEALTH_TIMEOUT: Duration = Duration::from_millis(250);

/// Canonical key for an environment signal (icon/colour lookup key).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnvironmentKind {
    Kubernetes,
    Docker,
    Aws,
    Gcp,
    Azure,
    Nix,
    Direnv,
    Load,
    Battery,
}

impl EnvironmentKind {
    /// Stable config/icon key.
    pub fn key(self) -> &'static str {
        match self {
            Self::Kubernetes => "kubernetes",
            Self::Docker => "docker",
            Self::Aws => "aws",
            Self::Gcp => "gcp",
            Self::Azure => "azure",
            Self::Nix => "nix",
            Self::Direnv => "direnv",
            Self::Load => "load",
            Self::Battery => "battery",
        }
    }
}

/// A single active environment signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Which detector produced this signal.
    pub kind: EnvironmentKind,
    /// Primary text (e.g. `minikube/kube-system`, `default`, `pure`).
    pub label: String,
}

impl EnvironmentInfo {
    fn new(kind: EnvironmentKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
        }
    }
}

/// Resolved environment state for a working directory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Active signals, in stable display order.
    pub entries: Vec<EnvironmentInfo>,
}

impl EnvironmentSnapshot {
    /// True when no infrastructure signal is active.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Resolve every enabled environment signal for `cwd`.
///
/// Each provider is isolated behind [`bounded`]: a timeout, panic, or I/O
/// failure removes only that provider's signal.
pub fn detect_environment(cwd: &Path, config: &EnvironmentConfig) -> EnvironmentSnapshot {
    if !config.enabled {
        return EnvironmentSnapshot::default();
    }

    let mut entries = Vec::new();

    if config.kubernetes {
        if let Some(info) = bounded(kubernetes::detect).flatten() {
            entries.push(info);
        }
    }
    if config.docker {
        if let Some(info) = bounded(container::detect).flatten() {
            entries.push(info);
        }
    }
    if config.cloud {
        entries.extend(bounded(cloud::detect).unwrap_or_default());
    }
    if config.nix_direnv {
        let cwd = cwd.to_path_buf();
        entries.extend(bounded(move || nix::detect(&cwd)).unwrap_or_default());
    }
    if config.health {
        entries.extend(bounded_with_timeout(HEALTH_TIMEOUT, health::detect).unwrap_or_default());
    }

    EnvironmentSnapshot { entries }
}

/// User home directory, cross-platform.
pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Read a configuration file, refusing anything larger than [`MAX_CONFIG_BYTES`].
pub(crate) fn read_bounded(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Run `f` on a detached worker, returning `None` if it panics or exceeds
/// [`DETECTOR_TIMEOUT`].
fn bounded<T, F>(f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    bounded_with_timeout(DETECTOR_TIMEOUT, f)
}

/// Run `f` on a detached worker with an explicit timeout budget.
fn bounded_with_timeout<T, F>(timeout: Duration, f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("paneship-env".to_string())
        .spawn(move || {
            let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok();
            let _ = tx.send(value);
        })
        .ok()?;

    match rx.recv_timeout(timeout) {
        Ok(Some(value)) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_config_yields_no_signals() {
        let config = EnvironmentConfig {
            enabled: false,
            ..EnvironmentConfig::default()
        };
        let snapshot = detect_environment(Path::new("."), &config);
        assert!(snapshot.is_empty());
    }

    #[test]
    fn bounded_returns_value_when_fast() {
        assert_eq!(bounded(|| 7), Some(7));
    }

    #[test]
    fn bounded_with_timeout_allows_slower_work() {
        let result = bounded_with_timeout(HEALTH_TIMEOUT, || {
            std::thread::sleep(Duration::from_millis(20));
            7
        });
        assert_eq!(result, Some(7));
    }

    #[test]
    fn bounded_drops_panicking_detector() {
        let result: Option<u32> = bounded(|| panic!("boom"));
        assert_eq!(result, None);
    }

    #[test]
    fn bounded_enforces_timeout() {
        let result: Option<u32> = bounded(|| {
            std::thread::sleep(DETECTOR_TIMEOUT * 20);
            1
        });
        assert_eq!(result, None);
    }
}
