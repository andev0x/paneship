//! Multi-path Git metadata watcher with a debounce/batch window.
//!
//! Watches `.git/HEAD`, `.git/index`, `.git/packed-refs` and `.git/refs/`
//! recursively (covering `heads/`, `remotes/` and `tags/`). Bursts of events
//! from operations like `git rebase` or `git checkout` are coalesced into a
//! single invalidation per repository after a short quiet period.

use super::{enqueue_git_refresh, log, UpdatingSet, WorkerTx};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Quiet period required before pending invalidations are flushed.
const DEBOUNCE: Duration = Duration::from_millis(50);

enum WatchCommand {
    /// Start watching a repository's `.git` metadata.
    Watch(PathBuf),
    /// A filesystem event mapped to a repository root.
    Changed(PathBuf),
}

/// Cloneable handle used by connection handlers to register repositories.
#[derive(Clone)]
pub struct GitWatcher {
    tx: Sender<WatchCommand>,
}

impl GitWatcher {
    /// Spawn the watcher thread and return a handle to it.
    pub fn start(worker_tx: WorkerTx, updating: UpdatingSet) -> Self {
        let (tx, rx) = mpsc::channel();
        let handle = Self { tx: tx.clone() };
        let _ = thread::Builder::new()
            .name("paneship-git-watch".to_string())
            .spawn(move || run(rx, tx, worker_tx, updating));
        handle
    }

    /// Register a repository root for watching (idempotent at the call site).
    pub fn watch_repo(&self, root: &Path) {
        let _ = self.tx.send(WatchCommand::Watch(root.to_path_buf()));
        crate::daemon::telemetry::record_watcher_subscription();
    }
}

fn run(
    rx: Receiver<WatchCommand>,
    tx: Sender<WatchCommand>,
    worker_tx: WorkerTx,
    updating: UpdatingSet,
) {
    let mut watcher =
        match notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            crate::daemon::telemetry::record_watcher_event();
            for path in event.paths {
                if let Some(root) = repo_root_from_git_path(&path) {
                    let _ = tx.send(WatchCommand::Changed(root));
                }
            }
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                log::error(format!("failed to start filesystem watcher: {err}"));
                return;
            }
        };

    let mut watched: HashSet<PathBuf> = HashSet::new();
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();

    loop {
        let command = if pending.is_empty() {
            rx.recv().ok()
        } else {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => {
                    flush(&mut pending, &worker_tx, &updating);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        };

        let Some(command) = command else { break };

        match command {
            WatchCommand::Watch(root) => watch_repo_paths(&mut watcher, &mut watched, &root),
            WatchCommand::Changed(root) => {
                pending.insert(root, Instant::now());
            }
        }
    }
}

fn flush(pending: &mut HashMap<PathBuf, Instant>, worker_tx: &WorkerTx, updating: &UpdatingSet) {
    for (root, _seen) in pending.drain() {
        log::info(format!("git metadata changed for {}", root.display()));
        crate::cache::invalidate_git(&root);
        enqueue_git_refresh(&root, worker_tx, updating);
    }
}

fn watch_repo_paths(watcher: &mut RecommendedWatcher, watched: &mut HashSet<PathBuf>, root: &Path) {
    let git = root.join(".git");
    if !git.exists() {
        return;
    }

    let git_dir = if git.is_file() {
        resolve_gitdir(&git).unwrap_or(git)
    } else {
        git
    };

    let files = [
        git_dir.join("HEAD"),
        git_dir.join("index"),
        git_dir.join("packed-refs"),
    ];
    for target in files {
        watch(watcher, watched, &target, RecursiveMode::NonRecursive);
    }

    watch(
        watcher,
        watched,
        &git_dir.join("refs"),
        RecursiveMode::Recursive,
    );
    // Catch metadata files that appear directly under `.git` (e.g. ORIG_HEAD).
    watch(watcher, watched, &git_dir, RecursiveMode::NonRecursive);
}

fn watch(
    watcher: &mut RecommendedWatcher,
    watched: &mut HashSet<PathBuf>,
    target: &Path,
    mode: RecursiveMode,
) {
    if !target.exists() || !watched.insert(target.to_path_buf()) {
        return;
    }
    if let Err(err) = watcher.watch(target, mode) {
        log::warn(format!("failed to watch {}: {err}", target.display()));
    }
}

/// Read the real git directory from a `.git` file (worktrees/submodules).
fn resolve_gitdir(git_file: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(git_file).ok()?;
    let raw = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("gitdir:"))?
        .trim();
    let raw = PathBuf::from(raw);
    if raw.is_absolute() {
        Some(raw)
    } else {
        git_file.parent().map(|parent| parent.join(raw))
    }
}

/// Map a filesystem event path back to the repository root that owns it.
pub fn repo_root_from_git_path(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if ancestor.file_name().and_then(|name| name.to_str()) == Some(".git") {
            return ancestor.parent().map(Path::to_path_buf);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::repo_root_from_git_path;
    use std::path::Path;

    #[test]
    fn maps_git_metadata_paths_to_repo_root() {
        let root = Some(Path::new("/repo").to_path_buf());

        assert_eq!(
            repo_root_from_git_path(Path::new("/repo/.git/refs/heads/main")),
            root
        );
        assert_eq!(
            repo_root_from_git_path(Path::new("/repo/.git/refs/remotes/origin/main")),
            root
        );
        assert_eq!(
            repo_root_from_git_path(Path::new("/repo/.git/packed-refs")),
            root
        );
        assert_eq!(repo_root_from_git_path(Path::new("/repo/.git/HEAD")), root);
        assert_eq!(repo_root_from_git_path(Path::new("/repo/.git/index")), root);
    }

    #[test]
    fn ignores_paths_outside_a_git_directory() {
        assert_eq!(repo_root_from_git_path(Path::new("/tmp/notes.md")), None);
        assert_eq!(repo_root_from_git_path(Path::new("relative/file")), None);
    }
}
