//! State cache with an in-memory LRU store and hard TTL thresholds.
//!
//! Git metadata is the hot path, so it lives in [`lru::LruCache`] (default
//! capacity: 500 working directories). Lookups distinguish fresh, stale and
//! missing entries so callers can serve stale data immediately while a
//! background worker refreshes it.

use crate::daemon::telemetry::{self, CacheKind};
use crate::detectors::infra::EnvironmentSnapshot;
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
/// Infrastructure state changes slowly; a 30 s TTL keeps worker churn low.
const ENVIRONMENT_CACHE_TTL: Duration = Duration::from_secs(30);

/// Language/version metadata surfaced in the prompt metadata area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageSnapshot {
    /// Canonical language key (matches `[metadata.languages.<key>]`).
    pub language: String,
    /// Version pin, when one was extracted from a manifest.
    pub version: Option<String>,
}

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
type TimedCache<T> = LruCache<PathBuf, TimedCacheEntry<Option<T>>>;
type LanguageCache = TimedCache<LanguageSnapshot>;
type PackageCache = TimedCache<String>;
type EnvironmentCache = TimedCache<EnvironmentSnapshot>;
type RepoRootCache = LruCache<PathBuf, Option<PathBuf>>;

static GIT_CACHE: OnceLock<Mutex<GitCache>> = OnceLock::new();
static LANGUAGE_CACHE: OnceLock<Mutex<LanguageCache>> = OnceLock::new();
static PACKAGE_CACHE: OnceLock<Mutex<PackageCache>> = OnceLock::new();
static ENVIRONMENT_CACHE: OnceLock<Mutex<EnvironmentCache>> = OnceLock::new();
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
    parse_ttl(
        std::env::var("PANESHIP_GIT_TTL_MS").ok().as_deref(),
        Duration::from_millis(DEFAULT_GIT_TTL_MS),
    )
}

/// Resolve a TTL from an optional environment override, falling back to the
/// compiled-in default when the value is absent or unparseable.
///
/// Keeping this pure (rather than reading the process environment directly)
/// lets the parsing/fallback rules be unit-tested without mutating global state.
fn parse_ttl(raw: Option<&str>, default: Duration) -> Duration {
    raw.and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(default)
}

/// TTL for language metadata (`PANESHIP_LANG_TTL_MS`).
fn language_ttl() -> Duration {
    parse_ttl(
        std::env::var("PANESHIP_LANG_TTL_MS").ok().as_deref(),
        LANGUAGE_CACHE_TTL,
    )
}

/// TTL for package metadata (`PANESHIP_PACKAGE_TTL_MS`).
fn package_ttl() -> Duration {
    parse_ttl(
        std::env::var("PANESHIP_PACKAGE_TTL_MS").ok().as_deref(),
        PACKAGE_CACHE_TTL,
    )
}

/// TTL for infrastructure/environment metadata (`PANESHIP_ENV_TTL_MS`).
fn environment_ttl() -> Duration {
    parse_ttl(
        std::env::var("PANESHIP_ENV_TTL_MS").ok().as_deref(),
        ENVIRONMENT_CACHE_TTL,
    )
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

fn environment_cache() -> &'static Mutex<EnvironmentCache> {
    ENVIRONMENT_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

fn repo_root_cache() -> &'static Mutex<RepoRootCache> {
    REPO_ROOT_CACHE.get_or_init(|| Mutex::new(new_lru()))
}

/// Look up the cached Git state for `path` (keyed by repository root).
pub fn git_lookup(path: &Path) -> GitLookup {
    let started = Instant::now();
    let key = git_cache_key(path);
    let mut cache = lock(git_cache());
    let Some(entry) = cache.get(&key) else {
        telemetry::record_lookup(CacheKind::Git, false, started.elapsed());
        return GitLookup::Missing;
    };

    if Instant::now() <= entry.expires_at {
        if let Some(snapshot) = &entry.snapshot {
            telemetry::record_lookup(CacheKind::Git, true, started.elapsed());
            return GitLookup::Fresh(snapshot.clone());
        }
    }

    telemetry::record_lookup(CacheKind::Git, false, started.elapsed());
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

/// Insert into an LRU, counting only genuine capacity evictions (not
/// overwrites of an existing key).
fn put_counting<V>(cache: &mut LruCache<PathBuf, V>, key: PathBuf, value: V, kind: CacheKind) {
    let replacing = cache.contains(&key);
    let at_capacity = cache.len() >= cache.cap().get();
    cache.put(key, value);
    if at_capacity && !replacing {
        telemetry::record_eviction(kind);
    }
}

fn insert_git(path: &Path, snapshot: Option<GitSnapshot>, ttl: Duration) {
    let key = git_cache_key(path);
    let entry = GitCacheEntry {
        snapshot,
        expires_at: Instant::now() + ttl,
    };
    put_counting(&mut lock(git_cache()), key, entry, CacheKind::Git);
}

/// Drop the Git entry for `path`'s repository, forcing a recompute.
pub fn invalidate_git(path: &Path) {
    let key = git_cache_key(path);
    lock(git_cache()).pop(&key);
}

/// Outcome of a metadata cache lookup. Mirrors [`GitLookup`] so the render path
/// can distinguish fresh, stale/pending and unknown states.
#[derive(Debug, Clone)]
pub enum MetadataLookup<T> {
    /// Cached value within its TTL and with an unchanged source file.
    Fresh(T),
    /// Entry past its TTL, or a pending placeholder. `Some` carries the last
    /// known value (may be stale), `None` means a refresh is in flight.
    Stale(Option<T>),
    /// No entry for this source path.
    Missing,
}

fn lookup_metadata<T: Clone>(
    cache: &Mutex<TimedCache<T>>,
    path: &Path,
    kind: CacheKind,
) -> MetadataLookup<T> {
    let started = Instant::now();
    let mut guard = lock(cache);
    let Some(entry) = guard.get(path).cloned() else {
        telemetry::record_lookup(kind, false, started.elapsed());
        return MetadataLookup::Missing;
    };

    if Instant::now() > entry.expires_at {
        telemetry::record_lookup(kind, false, started.elapsed());
        return MetadataLookup::Stale(entry.value);
    }

    if entry.source_mtime != file_mtime(path) {
        guard.pop(path);
        telemetry::record_lookup(kind, false, started.elapsed());
        return MetadataLookup::Missing;
    }

    match entry.value {
        Some(value) => {
            telemetry::record_lookup(kind, true, started.elapsed());
            MetadataLookup::Fresh(value)
        }
        None => {
            telemetry::record_lookup(kind, false, started.elapsed());
            MetadataLookup::Stale(None)
        }
    }
}

fn store_metadata<T>(
    cache: &Mutex<TimedCache<T>>,
    path: &Path,
    value: Option<T>,
    ttl: Duration,
    kind: CacheKind,
) {
    let entry = TimedCacheEntry {
        expires_at: Instant::now() + ttl,
        value,
        source_mtime: file_mtime(path),
    };
    put_counting(&mut lock(cache), path.to_path_buf(), entry, kind);
}

/// Look up cached language metadata for a marker file.
pub fn language_lookup(marker: &Path) -> MetadataLookup<LanguageSnapshot> {
    lookup_metadata(language_cache(), marker, CacheKind::Language)
}

/// Record that a language marker exists but its metadata is not yet known.
pub fn mark_language_pending(marker: &Path) {
    store_metadata(
        language_cache(),
        marker,
        None,
        language_ttl(),
        CacheKind::Language,
    );
}

/// Store freshly computed language metadata (or a computed negative).
pub fn store_language(marker: &Path, value: Option<LanguageSnapshot>) {
    store_metadata(
        language_cache(),
        marker,
        value,
        language_ttl(),
        CacheKind::Language,
    );
}

/// Look up cached package metadata for a manifest file.
pub fn package_lookup(manifest: &Path) -> MetadataLookup<String> {
    lookup_metadata(package_cache(), manifest, CacheKind::Package)
}

/// Record that a package manifest exists but its version is not yet known.
pub fn mark_package_pending(manifest: &Path) {
    store_metadata(
        package_cache(),
        manifest,
        None,
        package_ttl(),
        CacheKind::Package,
    );
}

/// Store a freshly computed package version (or a computed negative).
pub fn store_package(manifest: &Path, value: Option<String>) {
    store_metadata(
        package_cache(),
        manifest,
        value,
        package_ttl(),
        CacheKind::Package,
    );
}

/// Look up cached environment metadata for a directory.
pub fn environment_lookup(path: &Path) -> MetadataLookup<EnvironmentSnapshot> {
    lookup_metadata(environment_cache(), path, CacheKind::Environment)
}

/// Record that a directory's environment is not yet known.
pub fn mark_environment_pending(path: &Path) {
    store_metadata(
        environment_cache(),
        path,
        None,
        environment_ttl(),
        CacheKind::Environment,
    );
}

/// Store freshly computed environment metadata (or a computed negative).
pub fn store_environment(path: &Path, value: Option<EnvironmentSnapshot>) {
    store_metadata(
        environment_cache(),
        path,
        value,
        environment_ttl(),
        CacheKind::Environment,
    );
}

/// Drop all language/package/environment entries. Called when config is
/// hot-reloaded so icon/theme changes and detector toggles take effect.
pub fn invalidate_metadata() {
    lock(language_cache()).clear();
    lock(package_cache()).clear();
    lock(environment_cache()).clear();
}

/// Occupancy of each tracked cache, for the diagnostics view.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheSizes {
    pub git: usize,
    pub language: usize,
    pub package: usize,
    pub environment: usize,
}

/// Snapshot the current occupancy of every tracked cache.
pub fn cache_sizes() -> CacheSizes {
    CacheSizes {
        git: lock(git_cache()).len(),
        language: lock(language_cache()).len(),
        package: lock(package_cache()).len(),
        environment: lock(environment_cache()).len(),
    }
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

/// Drop cached repository-root resolutions for `path` and every ancestor.
///
/// A negative lookup (a directory that was *not* inside a repository) is cached
/// forever for performance; without this, running `git init` — or deleting a
/// repository — would keep showing a stale decision until LRU eviction. The
/// daemon calls this on the post-`git` `INVALIDATE_CWD` signal, so the next
/// render re-discovers the repository state.
pub fn invalidate_repo_root(path: &Path) {
    let mut cache = lock(repo_root_cache());
    for ancestor in path.ancestors() {
        cache.pop(ancestor);
    }
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

    #[test]
    fn language_metadata_lifecycle_marks_pending_then_fresh() {
        let _guard = guard();
        let marker = tmp_path("lang-marker");

        assert!(matches!(language_lookup(&marker), MetadataLookup::Missing));

        mark_language_pending(&marker);
        assert!(
            matches!(language_lookup(&marker), MetadataLookup::Stale(None)),
            "pending marker must read as in-flight, not missing"
        );

        store_language(
            &marker,
            Some(LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            }),
        );
        match language_lookup(&marker) {
            MetadataLookup::Fresh(snapshot) => {
                assert_eq!(snapshot.language, "rust");
                assert_eq!(snapshot.version.as_deref(), Some("1.74"));
            }
            _ => panic!("expected a fresh language snapshot"),
        }
    }

    #[test]
    fn package_metadata_lifecycle_marks_pending_then_fresh() {
        let _guard = guard();
        let manifest = tmp_path("pkg-manifest");

        mark_package_pending(&manifest);
        assert!(matches!(
            package_lookup(&manifest),
            MetadataLookup::Stale(None)
        ));

        store_package(&manifest, Some("2.0.0".to_string()));
        match package_lookup(&manifest) {
            MetadataLookup::Fresh(version) => assert_eq!(version, "2.0.0"),
            _ => panic!("expected a fresh package version"),
        }
    }

    #[test]
    fn invalidate_metadata_clears_entries() {
        let _guard = guard();
        let marker = tmp_path("invalidate-meta");
        store_language(
            &marker,
            Some(LanguageSnapshot {
                language: "go".to_string(),
                version: None,
            }),
        );
        assert!(matches!(language_lookup(&marker), MetadataLookup::Fresh(_)));
        invalidate_metadata();
        assert!(matches!(language_lookup(&marker), MetadataLookup::Missing));
    }

    #[test]
    fn ttl_override_parses_and_falls_back_to_default() {
        let default = Duration::from_millis(30_000);
        assert_eq!(
            parse_ttl(Some("1500"), default),
            Duration::from_millis(1500)
        );
        assert_eq!(
            parse_ttl(Some("  250  "), default),
            Duration::from_millis(250)
        );
        // Absent, empty, non-numeric and negative values all keep the default.
        assert_eq!(parse_ttl(None, default), default);
        assert_eq!(parse_ttl(Some(""), default), default);
        assert_eq!(parse_ttl(Some("abc"), default), default);
        assert_eq!(parse_ttl(Some("-5"), default), default);
    }

    #[test]
    fn repo_root_invalidation_rediscovers_new_repository() {
        let _guard = guard();
        let dir = tmp_path("repo-root-invalidate");
        std::fs::create_dir_all(&dir).unwrap();

        // First lookup caches the negative result (no repository yet).
        assert!(repo_root_for(&dir).is_none());
        assert!(
            repo_root_for(&dir).is_none(),
            "negative result should be served from cache"
        );

        // `git init`: a repository now exists, but the cached negative wins
        // until the invalidation hook clears it.
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert!(repo_root_for(&dir).is_none());

        invalidate_repo_root(&dir);
        assert_eq!(
            repo_root_for(&dir).as_deref(),
            Some(dir.as_path()),
            "invalidation must force rediscovery of the new repository"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
