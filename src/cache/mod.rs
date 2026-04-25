use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const GIT_CACHE_TTL: Duration = Duration::from_millis(350);

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

static GIT_CACHE: OnceLock<Mutex<HashMap<PathBuf, GitCacheEntry>>> = OnceLock::new();

fn git_cache() -> &'static Mutex<HashMap<PathBuf, GitCacheEntry>> {
    GIT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
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
