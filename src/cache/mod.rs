//! State cache with an in-memory LRU store and hard TTL thresholds.
//!
//! Git metadata is the hot path, so it lives in [`lru::LruCache`] (default
//! capacity: 500 working directories). Lookups distinguish fresh, stale and
//! missing entries so callers can serve stale data immediately while a
//! background worker refreshes it.

use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const DEFAULT_CACHE_CAPACITY: usize = 500;
const DEFAULT_GIT_TTL_MS: u64 = 5_000;
const LANGUAGE_CACHE_TTL: Duration = Duration::from_secs(60);
const PACKAGE_CACHE_TTL: Duration = Duration::from_secs(120);

/// `(language, version)` pair surfaced in the prompt metadata area.
pub type LanguageSnapshot = (String, String);

/// Point-in-time Git state for a repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitSnapshot {
    /// Current branch name (or shortened commit for detached HEAD).
    pub branch: String,
    /// Full HEAD object id.
    pub head_id: String,
    /// Number of staged changes.
    pub staged: usize,
    /// Number of unstaged changes.
    pub unstaged: usize,
    /// Number of untracked files.
    pub untracked: usize,
}

/// Outcome of a cache lookup.
#[derive(Debug, Clone)]
pub enum GitLookup {
    /// Cached snapshot within its TTL.
    Fresh(GitSnapshot),
    /// Entry past its TTL, or a pending placeholder. `Some` carries the last
    /// known snapshot (may be stale), `None` means a refresh is in flight.
    Stale(Option<GitSnapshot>),
    /// No entry for this path.
    Missing,
}

#[derive(Debug, Clone)]
struct GitCacheEntry {
    snapshot: Option<GitSnapshot>,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct TimedCacheEntry<T> {
    expires_at: Instant,
    value: T,
    source_mtime: Option<SystemTime>,
}

type GitCache = LruCache<PathBuf, GitCacheEntry>;
type LanguageCache = LruCache<PathBuf, TimedCacheEntry<Option<LanguageSnapshot>>>;
type PackageCache = LruCache<PathBuf, TimedCacheEntry<Option<String>>>;
type RepoRootCache = LruCache<PathBuf, Option<PathBuf>>;

static GIT_CACHE: OnceLock<Mutex<GitCache>> = OnceLock::new();
static LANGUAGE_CACHE: OnceLock<Mutex<LanguageCache>> = OnceLock::new();
static PACKAGE_CACHE: OnceLock<Mutex<PackageCache>> = OnceLock::new();
static REPO_ROOT_CACHE: OnceLock<Mutex<RepoRootCache>> = OnceLock::new();

fn capacity() -> NonZeroUsize {
    let raw = std::env::var("PANESHIP_CACHE_CAPACITY")
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CACHE_CAPACITY);
    NonZeroUsize::new(raw).unwrap_or(NonZeroUsize::MIN)
}

fn git_ttl() -> Duration {
    let ms = std::env::var("PANESHIP_GIT_TTL_MS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_GIT_TTL_MS);
    Duration::from_millis(ms)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn new_lru<V>() -> LruCache<PathBuf, V> {
    LruCache::new(capacity())
}

fn git_cache() -> &'static Mutex<GitCache> {
    GIT_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

fn language_cache() -> &'static Mutex<LanguageCache> {
    LANGUAGE_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

fn package_cache() -> &'static Mutex<PackageCache> {
    PACKAGE_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

fn repo_root_cache() -> &'static Mutex<RepoRootCache> {
    REPO_ROOT_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

/// Look up the cached Git state for `path` (keyed by repository root).
pub fn git_lookup(path: &Path) -> GitLookup {
    let key = git_cache_key(path);
    let mut cache = lock(git_cache());
    let Some(entry) = cache.get(&key) else {
        return GitLookup::Missing;
    };

    if Instant::now() <= entry.expires_at {
        if let Some(snapshot) = &entry.snapshot {
            return GitLookup::Fresh(snapshot.clone());
        }
    }

    GitLookup::Stale(entry.snapshot.clone())
}

/// Store a freshly computed snapshot using the configured TTL.
pub fn set_git(path: &Path, snapshot: GitSnapshot) {
    set_git_with_ttl(path, snapshot, git_ttl());
}

/// Store a freshly computed snapshot with an explicit TTL (used by tests).
pub fn set_git_with_ttl(path: &Path, snapshot: GitSnapshot, ttl: Duration) {
    insert_git(path, Some(snapshot), ttl);
}

/// Record that a repository exists but its state is not yet known, so the
/// renderer emits an explicit placeholder instead of dropping the segment.
pub fn mark_git_pending(path: &Path) {
    insert_git(path, None, git_ttl());
}

fn insert_git(path: &Path, snapshot: Option<GitSnapshot>, ttl: Duration) {
    let key = git_cache_key(path);
    let entry = GitCacheEntry {
        snapshot,
        expires_at: Instant::now() + ttl,
    };
    lock(git_cache()).put(key, entry);
}

/// Drop the Git entry for `path`'s repository, forcing a recompute.
pub fn invalidate_git(path: &Path) {
    let key = git_cache_key(path);
    lock(git_cache()).pop(&key);
}

/// Compute and cache a language snapshot on miss/expiry.
pub fn get_or_compute_language<F>(path: &Path, compute: F) -> Option<LanguageSnapshot>
where
    F: FnOnce() -> Option<LanguageSnapshot>,
{
    if let Some(cached) = get_language(path) {
        return cached;
    }

    let fresh = compute();
    let entry = TimedCacheEntry {
        expires_at: Instant::now() + LANGUAGE_CACHE_TTL,
        value: fresh.clone(),
        source_mtime: file_mtime(path),
    };
    lock(language_cache()).put(path.to_path_buf(), entry);
    fresh
}

/// Read a non-expired language entry whose source file is unchanged.
pub fn get_language(path: &Path) -> Option<Option<LanguageSnapshot>> {
    let mut cache = lock(language_cache());
    let entry = cache.get(path)?.clone();
    if Instant::now() <= entry.expires_at && entry.source_mtime == file_mtime(path) {
        return Some(entry.value);
    }
    cache.pop(path);
    None
}

/// Compute and cache a package version on miss/expiry.
pub fn get_or_compute_package_version<F>(path: &Path, compute: F) -> Option<String>
where
    F: FnOnce() -> Option<String>,
{
    if let Some(cached) = get_package_version(path) {
        return cached;
    }

    let fresh = compute();
    let entry = TimedCacheEntry {
        expires_at: Instant::now() + PACKAGE_CACHE_TTL,
        value: fresh.clone(),
        source_mtime: file_mtime(path),
    };
    lock(package_cache()).put(path.to_path_buf(), entry);
    fresh
}

/// Read a non-expired package entry whose manifest is unchanged.
pub fn get_package_version(path: &Path) -> Option<Option<String>> {
    let mut cache = lock(package_cache());
    let entry = cache.get(path)?.clone();
    if Instant::now() <= entry.expires_at && entry.source_mtime == file_mtime(path) {
        return Some(entry.value);
    }
    cache.pop(path);
    None
}

/// Compute and cache the repository root for `path` on miss.
pub fn get_or_compute_repo_root<F>(path: &Path, compute: F) -> Option<PathBuf>
where
    F: FnOnce() -> Option<PathBuf>,
{
    if let Some(cached) = get_repo_root(path) {
        return cached;
    }

    let fresh = compute();
    lock(repo_root_cache()).put(path.to_path_buf(), fresh.clone());
    fresh
}

fn get_repo_root(path: &Path) -> Option<Option<PathBuf>> {
    let mut cache = lock(repo_root_cache());
    cache.get(path).cloned()
}

/// Resolve (and cache) the repository root containing `path`.
pub fn repo_root_for(path: &Path) -> Option<PathBuf> {
    get_or_compute_repo_root(path, || find_repo_root(path))
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if let Some(cached) = {
            let mut cache = lock(repo_root_cache());
            cache.get(dir).cloned()
        } {
            return cached;
        }

        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

fn git_cache_key(path: &Path) -> PathBuf {
    match repo_root_for(path) {
        // Canonicalize so symlinked spellings of the same repository
        // (e.g. `/tmp` vs `/private/tmp`) share one cache entry and the
        // filesystem watcher can invalidate entries created via the socket.
        Some(root) => std::fs::canonicalize(&root).unwrap_or(root),
        None => path.to_path_buf(),
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// Reset and resize the Git cache. Test-only.
#[cfg(test)]
pub fn reset_for_tests(new_capacity: usize) {
    let mut cache = lock(git_cache());
    cache.clear();
    cache.resize(NonZeroUsize::new(new_capacity.max(1)).unwrap());
}

/// Number of Git entries currently held. Test-only.
#[cfg(test)]
pub fn git_cache_len_for_tests() -> usize {
    lock(git_cache()).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    // Cache tests mutate global state, so they run one at a time.
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn guard() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn tmp_path(label: &str) -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "paneship-cache-test-{}-{label}-{id}",
            std::process::id()
        ))
    }

    fn snapshot(branch: &str) -> GitSnapshot {
        GitSnapshot {
            branch: branch.to_string(),
            head_id: "deadbeef".to_string(),
            staged: 0,
            unstaged: 0,
            untracked: 0,
        }
    }

    #[test]
    fn ttl_expiry_yields_stale_snapshot() {
        let _guard = guard();
        reset_for_tests(16);
        let path = tmp_path("ttl");

        set_git_with_ttl(&path, snapshot("main"), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(5));

        match git_lookup(&path) {
            GitLookup::Stale(Some(cached)) => assert_eq!(cached.branch, "main"),
            other => panic!("expected stale snapshot, got {other:?}"),
        }
    }

    #[test]
    fn fresh_entry_is_reported_fresh() {
        let _guard = guard();
        reset_for_tests(16);
        let path = tmp_path("fresh");

        set_git(&path, snapshot("main"));
        assert!(matches!(git_lookup(&path), GitLookup::Fresh(_)));
    }

    #[test]
    fn pending_marker_reports_stale_none() {
        let _guard = guard();
        reset_for_tests(16);
        let path = tmp_path("pending");

        mark_git_pending(&path);
        assert!(matches!(git_lookup(&path), GitLookup::Stale(None)));
    }

    #[test]
    fn lru_evicts_least_recently_used() {
        let _guard = guard();
        reset_for_tests(2);
        let a = tmp_path("a");
        let b = tmp_path("b");
        let c = tmp_path("c");

        set_git(&a, snapshot("a"));
        set_git(&b, snapshot("b"));
        set_git(&c, snapshot("c")); // evicts `a`

        assert_eq!(git_cache_len_for_tests(), 2);
        assert!(matches!(git_lookup(&a), GitLookup::Missing));
        assert!(matches!(git_lookup(&b), GitLookup::Fresh(_)));
        assert!(matches!(git_lookup(&c), GitLookup::Fresh(_)));
    }

    #[test]
    fn invalidation_after_push_clears_entry() {
        let _guard = guard();
        reset_for_tests(16);
        let repo = tmp_path("repo");

        set_git(&repo, snapshot("feature"));
        assert!(matches!(git_lookup(&repo), GitLookup::Fresh(_)));

        // Simulates the invalidation triggered by a `git push` event.
        invalidate_git(&repo);

        assert!(matches!(git_lookup(&repo), GitLookup::Missing));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_repo_paths_share_one_cache_entry() {
        let _guard = guard();
        reset_for_tests(8);

        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("paneship-cache-link-{}-{id}", std::process::id()));
        let real = base.join("real");
        let link = base.join("link");
        std::fs::create_dir_all(real.join(".git")).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();

        set_git(&real, snapshot("main"));
        assert!(
            matches!(git_lookup(&link), GitLookup::Fresh(_)),
            "symlinked spelling must hit the same cache entry"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
