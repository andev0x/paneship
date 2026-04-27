use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const GIT_CACHE_TTL: Duration = Duration::from_millis(350);
const LANGUAGE_CACHE_TTL: Duration = Duration::from_secs(5);
const PACKAGE_CACHE_TTL: Duration = Duration::from_secs(5);

type LanguageSnapshot = (String, String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSnapshot {
    pub branch: String,
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
}

impl GitSnapshot {}

#[derive(Debug, Clone)]
struct GitCacheEntry {
    expires_at: Instant,
    value: Option<GitSnapshot>,
}

type GitCacheMap = HashMap<PathBuf, GitCacheEntry>;
type LanguageCacheMap = HashMap<PathBuf, TimedCacheEntry<Option<LanguageSnapshot>>>;
type PackageCacheMap = HashMap<PathBuf, TimedCacheEntry<Option<String>>>;
type RepoRootCacheMap = HashMap<PathBuf, Option<PathBuf>>;

static GIT_CACHE: OnceLock<Mutex<GitCacheMap>> = OnceLock::new();
static LANGUAGE_CACHE: OnceLock<Mutex<LanguageCacheMap>> = OnceLock::new();
static PACKAGE_CACHE: OnceLock<Mutex<PackageCacheMap>> = OnceLock::new();
static REPO_ROOT_CACHE: OnceLock<Mutex<RepoRootCacheMap>> = OnceLock::new();

fn git_cache() -> &'static Mutex<GitCacheMap> {
    GIT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
struct TimedCacheEntry<T> {
    expires_at: Instant,
    value: T,
}

fn language_cache() -> &'static Mutex<LanguageCacheMap> {
    LANGUAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn package_cache() -> &'static Mutex<PackageCacheMap> {
    PACKAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn repo_root_cache() -> &'static Mutex<RepoRootCacheMap> {
    REPO_ROOT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_or_compute_git<F>(path: &Path, compute: F) -> Option<GitSnapshot>
where
    F: FnOnce() -> Option<GitSnapshot>,
{
    if let Some(cached) = get_git(path) {
        return cached;
    }

    let fresh = compute();
    let entry = GitCacheEntry {
        expires_at: Instant::now() + GIT_CACHE_TTL,
        value: fresh.clone(),
    };

    if let Ok(mut cache) = git_cache().lock() {
        cache.insert(path.to_path_buf(), entry);
    }

    fresh
}

fn get_git(path: &Path) -> Option<Option<GitSnapshot>> {
    let mut cache = git_cache().lock().ok()?;
    let entry = cache.get(path)?.clone();
    if Instant::now() <= entry.expires_at {
        return Some(entry.value);
    }
    cache.remove(path);
    None
}

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
    };

    if let Ok(mut cache) = language_cache().lock() {
        cache.insert(path.to_path_buf(), entry);
    }

    fresh
}

fn get_language(path: &Path) -> Option<Option<LanguageSnapshot>> {
    let mut cache = language_cache().lock().ok()?;
    let entry = cache.get(path)?.clone();
    if Instant::now() <= entry.expires_at {
        return Some(entry.value);
    }
    cache.remove(path);
    None
}

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
    };

    if let Ok(mut cache) = package_cache().lock() {
        cache.insert(path.to_path_buf(), entry);
    }

    fresh
}

fn get_package_version(path: &Path) -> Option<Option<String>> {
    let mut cache = package_cache().lock().ok()?;
    let entry = cache.get(path)?.clone();
    if Instant::now() <= entry.expires_at {
        return Some(entry.value);
    }
    cache.remove(path);
    None
}

pub fn get_or_compute_repo_root<F>(path: &Path, compute: F) -> Option<PathBuf>
where
    F: FnOnce() -> Option<PathBuf>,
{
    if let Some(cached) = get_repo_root(path) {
        return cached;
    }

    let fresh = compute();
    if let Ok(mut cache) = repo_root_cache().lock() {
        cache.insert(path.to_path_buf(), fresh.clone());
    }
    fresh
}

fn get_repo_root(path: &Path) -> Option<Option<PathBuf>> {
    let cache = repo_root_cache().lock().ok()?;
    cache.get(path).cloned().map(Some)?
}
