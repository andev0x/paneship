//! Daemon telemetry (Phase 3, Module D).
//!
//! Process-wide counters feeding the `paneship top` diagnostics view. Every
//! counter is a relaxed atomic increment, so instrumentation is effectively
//! free on the render path and safe from any thread. The snapshot is the wire
//! payload for [`crate::daemon::Request::Stats`].

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use Ordering::Relaxed;

/// Cache families tracked by the diagnostics view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheKind {
    Git,
    Language,
    Package,
    Environment,
}

/// Hit/miss/eviction/occupancy counters for one cache.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: u64,
    /// Mean lookup latency (hit or miss), in microseconds.
    pub avg_lookup_us: f64,
}

/// Filesystem watcher counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WatchStats {
    /// Repositories currently subscribed for change notifications.
    pub subscriptions: u64,
    /// Raw filesystem events forwarded into the debounce window.
    pub events: u64,
}

/// Point-in-time daemon diagnostics payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetrySnapshot {
    pub uptime_ms: u64,
    pub render_requests: u64,
    pub git_refreshes: u64,
    pub metadata_refreshes: u64,
    pub environment_refreshes: u64,
    /// Total time the worker thread spent processing refresh jobs.
    pub worker_busy_ms: u64,
    /// `worker_busy_ms / uptime_ms`, clamped to `1.0`.
    pub worker_utilization: f64,
    pub active_connections: u64,
    pub git_cache: CacheStats,
    pub language_cache: CacheStats,
    pub package_cache: CacheStats,
    pub environment_cache: CacheStats,
    pub watcher: WatchStats,
}

#[derive(Default)]
struct CacheCounters {
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
    /// Accumulated lookup time in nanoseconds across hits and misses.
    lookup_ns: AtomicU64,
}

struct Telemetry {
    start: Instant,
    render_requests: AtomicU64,
    git_refreshes: AtomicU64,
    metadata_refreshes: AtomicU64,
    environment_refreshes: AtomicU64,
    worker_busy_ns: AtomicU64,
    active_connections: AtomicU64,
    watcher_subscriptions: AtomicU64,
    watcher_events: AtomicU64,
    git: CacheCounters,
    language: CacheCounters,
    package: CacheCounters,
    environment: CacheCounters,
}

static TELEMETRY: OnceLock<Telemetry> = OnceLock::new();

fn telemetry() -> &'static Telemetry {
    TELEMETRY.get_or_init(|| Telemetry {
        start: Instant::now(),
        render_requests: AtomicU64::new(0),
        git_refreshes: AtomicU64::new(0),
        metadata_refreshes: AtomicU64::new(0),
        environment_refreshes: AtomicU64::new(0),
        worker_busy_ns: AtomicU64::new(0),
        active_connections: AtomicU64::new(0),
        watcher_subscriptions: AtomicU64::new(0),
        watcher_events: AtomicU64::new(0),
        git: CacheCounters::default(),
        language: CacheCounters::default(),
        package: CacheCounters::default(),
        environment: CacheCounters::default(),
    })
}

fn counters(telemetry: &Telemetry, kind: CacheKind) -> &CacheCounters {
    match kind {
        CacheKind::Git => &telemetry.git,
        CacheKind::Language => &telemetry.language,
        CacheKind::Package => &telemetry.package,
        CacheKind::Environment => &telemetry.environment,
    }
}

/// Count a rendered prompt request.
pub fn record_render_request() {
    telemetry().render_requests.fetch_add(1, Relaxed);
}

/// Count a completed Git refresh.
pub fn record_git_refresh() {
    telemetry().git_refreshes.fetch_add(1, Relaxed);
}

/// Count a completed language/package metadata refresh.
pub fn record_metadata_refresh() {
    telemetry().metadata_refreshes.fetch_add(1, Relaxed);
}

/// Count a completed environment refresh.
pub fn record_environment_refresh() {
    telemetry().environment_refreshes.fetch_add(1, Relaxed);
}

/// Accumulate worker thread busy time.
pub fn record_worker_busy(elapsed: Duration) {
    telemetry()
        .worker_busy_ns
        .fetch_add(elapsed.as_nanos() as u64, Relaxed);
}

/// Record a cache lookup outcome and its latency.
pub fn record_lookup(kind: CacheKind, hit: bool, elapsed: Duration) {
    let counters = counters(telemetry(), kind);
    if hit {
        counters.hits.fetch_add(1, Relaxed);
    } else {
        counters.misses.fetch_add(1, Relaxed);
    }
    counters
        .lookup_ns
        .fetch_add(elapsed.as_nanos() as u64, Relaxed);
}

/// Record an LRU eviction.
pub fn record_eviction(kind: CacheKind) {
    counters(telemetry(), kind).evictions.fetch_add(1, Relaxed);
}

/// Record a newly watched repository.
pub fn record_watcher_subscription() {
    telemetry().watcher_subscriptions.fetch_add(1, Relaxed);
}

/// Record a raw filesystem event.
pub fn record_watcher_event() {
    telemetry().watcher_events.fetch_add(1, Relaxed);
}

/// Build the current diagnostics snapshot.
pub fn snapshot() -> TelemetrySnapshot {
    let telemetry = telemetry();
    let uptime = telemetry.start.elapsed();
    let busy_ns = telemetry.worker_busy_ns.load(Relaxed);
    let worker_utilization = if uptime.as_nanos() == 0 {
        0.0
    } else {
        (busy_ns as f64 / uptime.as_nanos() as f64).clamp(0.0, 1.0)
    };

    let sizes = crate::cache::cache_sizes();

    let cache_stats = |kind: CacheKind, entries: usize| {
        let counters = counters(telemetry, kind);
        let hits = counters.hits.load(Relaxed);
        let misses = counters.misses.load(Relaxed);
        let lookups = hits + misses;
        let avg_lookup_us = if lookups == 0 {
            0.0
        } else {
            counters.lookup_ns.load(Relaxed) as f64 / lookups as f64 / 1_000.0
        };
        CacheStats {
            hits,
            misses,
            evictions: counters.evictions.load(Relaxed),
            entries: entries as u64,
            avg_lookup_us,
        }
    };

    TelemetrySnapshot {
        uptime_ms: uptime.as_millis() as u64,
        render_requests: telemetry.render_requests.load(Relaxed),
        git_refreshes: telemetry.git_refreshes.load(Relaxed),
        metadata_refreshes: telemetry.metadata_refreshes.load(Relaxed),
        environment_refreshes: telemetry.environment_refreshes.load(Relaxed),
        worker_busy_ms: busy_ns / 1_000_000,
        worker_utilization,
        active_connections: telemetry.active_connections.load(Relaxed),
        git_cache: cache_stats(CacheKind::Git, sizes.git),
        language_cache: cache_stats(CacheKind::Language, sizes.language),
        package_cache: cache_stats(CacheKind::Package, sizes.package),
        environment_cache: cache_stats(CacheKind::Environment, sizes.environment),
        watcher: WatchStats {
            subscriptions: telemetry.watcher_subscriptions.load(Relaxed),
            events: telemetry.watcher_events.load(Relaxed),
        },
    }
}

/// RAII guard tracking concurrent client connections.
pub struct ConnectionGuard;

impl ConnectionGuard {
    /// Register a new live connection.
    pub fn new() -> Self {
        telemetry().active_connections.fetch_add(1, Relaxed);
        Self
    }
}

impl Default for ConnectionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        telemetry().active_connections.fetch_sub(1, Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_snapshots_counters() {
        let before = snapshot();
        record_render_request();
        record_lookup(CacheKind::Git, true, Duration::from_micros(2));
        record_lookup(CacheKind::Git, false, Duration::from_micros(4));
        record_eviction(CacheKind::Package);
        record_worker_busy(Duration::from_millis(5));
        record_watcher_subscription();

        let after = snapshot();
        assert_eq!(after.render_requests, before.render_requests + 1);
        assert_eq!(after.git_cache.hits, before.git_cache.hits + 1);
        assert_eq!(after.git_cache.misses, before.git_cache.misses + 1);
        assert_eq!(
            after.package_cache.evictions,
            before.package_cache.evictions + 1
        );
        assert!(
            after.git_cache.avg_lookup_us > 0.0,
            "average lookup latency should be measured"
        );
        assert_eq!(after.worker_busy_ms, before.worker_busy_ms + 5);
        assert_eq!(
            after.watcher.subscriptions,
            before.watcher.subscriptions + 1
        );
    }

    #[test]
    fn connection_guard_tracks_live_connections() {
        let before = snapshot().active_connections;
        {
            let _guard = ConnectionGuard::new();
            assert_eq!(snapshot().active_connections, before + 1);
        }
        assert_eq!(snapshot().active_connections, before);
    }
}
