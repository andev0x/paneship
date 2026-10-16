//! Modular runtime/language detector registry (Phase 2, Module B).
//!
//! Every language lives behind the [`Detector`] trait in its own file. Adding a
//! new language is a two-step change: implement the trait, then append the
//! detector to [`build_registry`]. Nothing in the rendering engine changes.
//!
//! Detection is intentionally **manifest-first**: version pins are read from
//! `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml`, … with lightweight
//! in-process parsers. No external subprocess (`node -v`, `python --version`)
//! is ever spawned on the prompt path.

use async_trait::async_trait;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;
use std::task::{Context, Poll, Waker};

pub mod bun;
pub mod deno;
pub mod go;
pub mod infra;
pub mod java;
pub mod node;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;

mod manifest;

/// Result of a successful detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInfo {
    /// Canonical language key (matches `[metadata.languages.<key>]`).
    pub language: String,
    /// Version pin/toolchain extracted from a manifest, when available.
    pub version: Option<String>,
    /// Manifest that produced the match.
    pub source: PathBuf,
}

/// Uniform interface implemented by every environment/language detector.
#[async_trait]
pub trait Detector: Send + Sync {
    /// Canonical language key (stable; used as the config/icon lookup key).
    fn name(&self) -> &'static str;

    /// Marker file names probed within a single directory, in priority order.
    fn marker_files(&self) -> &'static [&'static str];

    /// Locate this detector's marker inside `dir` (cheap `stat`s only).
    fn marker_in(&self, dir: &Path) -> Option<PathBuf> {
        self.marker_files()
            .iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file())
    }

    /// Extract runtime metadata for the project rooted at/above `cwd`.
    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo>;
}

static REGISTRY: OnceLock<Vec<Box<dyn Detector>>> = OnceLock::new();

/// The core detector list. Register a new detector here.
fn build_registry() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(rust::RustDetector),
        Box::new(node::NodeDetector),
        Box::new(bun::BunDetector),
        Box::new(deno::DenoDetector),
        Box::new(go::GoDetector),
        Box::new(python::PythonDetector),
        Box::new(ruby::RubyDetector),
        Box::new(php::PhpDetector),
        Box::new(java::JavaDetector),
    ]
}

/// The static registry of boxed detectors.
pub fn registry() -> &'static [Box<dyn Detector>] {
    REGISTRY.get_or_init(build_registry).as_slice()
}

/// A registry-wide marker hit.
#[derive(Debug, Clone)]
pub struct MarkerMatch {
    /// Index into [`registry`].
    pub detector_index: usize,
    /// The matched manifest file.
    pub marker: PathBuf,
}

/// Nearest marker across the whole registry, walking `cwd` upwards.
///
/// This is the cheap probe used on the hot render path: it performs only
/// directory-entry existence checks and never parses a manifest.
pub fn find_marker(cwd: &Path) -> Option<MarkerMatch> {
    for dir in cwd.ancestors() {
        for (detector_index, detector) in registry().iter().enumerate() {
            if let Some(marker) = detector.marker_in(dir) {
                return Some(MarkerMatch {
                    detector_index,
                    marker,
                });
            }
        }
        if is_boundary(dir) {
            break;
        }
    }
    None
}

/// Fully resolve runtime metadata for `cwd` using the registry.
pub fn detect_runtime(cwd: &Path) -> Option<RuntimeInfo> {
    let matched = find_marker(cwd)?;
    let detector = registry().get(matched.detector_index)?;
    let mut info = block_on(detector.detect(cwd))?;
    // The registry owns the canonical language key; keep the payload in sync.
    info.language = detector.name().to_string();
    Some(info)
}

/// Locate `detector`'s nearest marker, walking up to the repository boundary.
pub(crate) fn find_marker_for(detector: &dyn Detector, cwd: &Path) -> Option<(PathBuf, PathBuf)> {
    for dir in cwd.ancestors() {
        if let Some(marker) = detector.marker_in(dir) {
            return Some((dir.to_path_buf(), marker));
        }
        if is_boundary(dir) {
            break;
        }
    }
    None
}

/// Stop walking once we reach a repository root or the filesystem root.
fn is_boundary(dir: &Path) -> bool {
    dir.join(".git").exists() || dir.parent().is_none()
}

/// Drive an immediately-ready future to completion.
///
/// Detector bodies are synchronous file reads dressed in the `async` trait, so
/// the future never yields; a no-op waker is therefore sufficient and a
/// `Pending` result simply spins. This keeps the worker thread free of an async
/// runtime dependency.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut context = Context::from_waker(Waker::noop());
    let mut future: Pin<Box<F>> = Box::pin(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paneship-det-{label}-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn registry_has_unique_language_keys() {
        let mut names: Vec<_> = registry().iter().map(|d| d.name()).collect();
        names.sort_unstable();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped, "detector names must be unique");
        assert!(names.len() >= 9);
    }

    #[test]
    fn marker_probe_finds_nearest_and_stops_at_repo_root() {
        let root = temp_dir("probe");
        let nested = root.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("package.json"), "{}").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();

        let matched = find_marker(&nested).expect("marker should be found");
        assert_eq!(registry()[matched.detector_index].name(), "node");
        assert_eq!(matched.marker, root.join("package.json"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn block_on_resolves_ready_future() {
        assert_eq!(block_on(async { 41 + 1 }), 42);
    }
}
