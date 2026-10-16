//! `paneship top` — live daemon telemetry (Phase 3, Module D).
//!
//! Queries the daemon's [`crate::daemon::Request::Stats`] endpoint and renders
//! worker utilization, cache hit ratios, LRU evictions and watcher throughput.
//! A live `ratatui` view is shown on a TTY; `--once`/`--json` (and any
//! non-interactive stdout) print a single snapshot. When no daemon is running
//! it falls back to the Phase 1 in-process component benchmark.

use crate::daemon::telemetry::{CacheStats, TelemetrySnapshot};
use std::collections::VecDeque;
use std::io::IsTerminal;
use std::time::{Duration, Instant};

/// `paneship top` options.
#[derive(Debug, Clone, Default)]
pub struct TopOptions {
    /// Print a single snapshot and exit (no live view).
    pub once: bool,
    /// Emit the raw snapshot as JSON.
    pub json: bool,
    /// Run the local component micro-benchmark instead of querying the daemon.
    pub benchmark: bool,
}

/// Rolling-average window (in one-second samples) shown in the live view.
const RATE_WINDOW: usize = 8;

/// Per-interval rates derived from two successive telemetry snapshots.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rates {
    pub renders_per_s: f64,
    pub git_refreshes_per_s: f64,
    pub watcher_events_per_s: f64,
}

/// Compute counter rates between two snapshots over `elapsed`.
///
/// Telemetry counters are monotonic, so deltas use saturating subtraction: a
/// daemon restart (or a clock adjustment) can only clamp a rate to zero, never
/// produce a nonsensical negative value.
pub fn compute_rates(
    prev: &TelemetrySnapshot,
    curr: &TelemetrySnapshot,
    elapsed: Duration,
) -> Rates {
    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 {
        return Rates::default();
    }
    let per_second = |delta: u64| delta as f64 / seconds;
    Rates {
        renders_per_s: per_second(curr.render_requests.saturating_sub(prev.render_requests)),
        git_refreshes_per_s: per_second(curr.git_refreshes.saturating_sub(prev.git_refreshes)),
        watcher_events_per_s: per_second(curr.watcher.events.saturating_sub(prev.watcher.events)),
    }
}

/// Fixed-size window of recent [`Rates`] samples backing the rolling average.
#[derive(Default)]
struct RateAverages {
    window: VecDeque<Rates>,
}

impl RateAverages {
    fn push(&mut self, rates: Rates) {
        if self.window.len() == RATE_WINDOW {
            self.window.pop_front();
        }
        self.window.push_back(rates);
    }

    fn is_empty(&self) -> bool {
        self.window.is_empty()
    }

    fn average(&self) -> Rates {
        if self.window.is_empty() {
            return Rates::default();
        }
        let count = self.window.len() as f64;
        let mean = |select: fn(&Rates) -> f64| self.window.iter().map(select).sum::<f64>() / count;
        Rates {
            renders_per_s: mean(|rates| rates.renders_per_s),
            git_refreshes_per_s: mean(|rates| rates.git_refreshes_per_s),
            watcher_events_per_s: mean(|rates| rates.watcher_events_per_s),
        }
    }
}

/// Run the diagnostics view.
pub fn run(options: &TopOptions) -> Result<(), String> {
    if options.benchmark {
        return crate::benchmark::run_top();
    }

    let Some(snapshot) = crate::daemon::query_telemetry() else {
        eprintln!(
            "paneship: no daemon is running; showing the local component benchmark instead.\n\
             Start the daemon with `paneship daemon` for live telemetry."
        );
        return crate::benchmark::run_top();
    };

    if options.json {
        let json = serde_json::to_string_pretty(&snapshot).map_err(|err| err.to_string())?;
        println!("{json}");
        return Ok(());
    }

    if options.once || !std::io::stdout().is_terminal() {
        print!("{}", format_report(&snapshot, false));
        return Ok(());
    }

    run_live()
}

fn run_live() -> Result<(), String> {
    use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use ratatui::text::Text;
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

    let mut terminal =
        ratatui::try_init().map_err(|err| format!("failed to initialise terminal: {err}"))?;

    let mut previous: Option<TelemetrySnapshot> = None;
    let mut last_sample = Instant::now();
    let mut averages = RateAverages::default();

    let result = (|| {
        loop {
            // A dropped daemon degrades to an empty snapshot rather than exiting.
            let snapshot = crate::daemon::query_telemetry().unwrap_or_default();
            let now = Instant::now();
            if let Some(prev) = &previous {
                averages.push(compute_rates(
                    prev,
                    &snapshot,
                    now.duration_since(last_sample),
                ));
            }
            previous = Some(snapshot.clone());
            last_sample = now;
            let rates = (!averages.is_empty()).then(|| averages.average());

            terminal
                .draw(|frame| {
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title("Paneship Daemon Telemetry");
                    let paragraph = Paragraph::new(Text::from(format_report_with_rates(
                        &snapshot, rates, true,
                    )))
                    .block(block)
                    .wrap(Wrap { trim: false });
                    frame.render_widget(paragraph, frame.area());
                })
                .map_err(|err| err.to_string())?;

            if event::poll(Duration::from_secs(1)).map_err(|err| err.to_string())? {
                if let Event::Key(key) = event::read().map_err(|err| err.to_string())? {
                    if key.kind == KeyEventKind::Press
                        && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    {
                        return Ok(());
                    }
                }
            }
        }
    })();

    ratatui::restore();
    result
}

/// Format a snapshot as a human-readable report.
fn format_report(snapshot: &TelemetrySnapshot, live: bool) -> String {
    format_report_with_rates(snapshot, None, live)
}

/// Format a snapshot plus optional rolling rates.
fn format_report_with_rates(
    snapshot: &TelemetrySnapshot,
    rates: Option<Rates>,
    live: bool,
) -> String {
    let mut out = String::new();
    out.push_str("Paneship daemon telemetry\n");
    out.push_str(&format!(
        "  uptime            {}\n",
        format_duration(snapshot.uptime_ms)
    ));
    out.push_str(&format!(
        "  renders           {}\n",
        snapshot.render_requests
    ));
    if let Some(rates) = rates {
        out.push_str(&format!(
            "  rate ({}s avg)     {:.1} renders/s · {:.1} git/s · {:.1} events/s\n",
            RATE_WINDOW, rates.renders_per_s, rates.git_refreshes_per_s, rates.watcher_events_per_s
        ));
    }
    out.push_str(&format!(
        "  worker            {} ms busy ({:.1}% utilization)\n",
        snapshot.worker_busy_ms,
        snapshot.worker_utilization * 100.0
    ));
    out.push_str(&format!(
        "  connections       {} active\n",
        snapshot.active_connections
    ));
    out.push_str(&format!(
        "  refreshes         git {} · metadata {} · environment {}\n",
        snapshot.git_refreshes, snapshot.metadata_refreshes, snapshot.environment_refreshes
    ));
    out.push_str(&format_cache("  git cache         ", &snapshot.git_cache));
    out.push_str(&format_cache(
        "  language cache    ",
        &snapshot.language_cache,
    ));
    out.push_str(&format_cache(
        "  package cache     ",
        &snapshot.package_cache,
    ));
    out.push_str(&format_cache(
        "  environment cache ",
        &snapshot.environment_cache,
    ));
    out.push_str(&format!(
        "  watcher           {} subscriptions · {} events\n",
        snapshot.watcher.subscriptions, snapshot.watcher.events
    ));
    if live {
        out.push_str("\n  press q to quit\n");
    }
    out
}

fn format_cache(label: &str, stats: &CacheStats) -> String {
    format!(
        "{label}{} entries · hits {} · misses {} · evictions {} · {:.1} us avg · {} hit rate\n",
        stats.entries,
        stats.hits,
        stats.misses,
        stats.evictions,
        stats.avg_lookup_us,
        format_ratio(stats)
    )
}

fn format_ratio(stats: &CacheStats) -> String {
    let lookups = stats.hits + stats.misses;
    if lookups == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1}%", stats.hits as f64 / lookups as f64 * 100.0)
    }
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1_000;
    if secs < 60 {
        format!("{ms} ms")
    } else if secs < 3_600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3_600, (secs % 3_600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::telemetry::WatchStats;

    fn sample() -> TelemetrySnapshot {
        TelemetrySnapshot {
            uptime_ms: 65_000,
            render_requests: 10,
            worker_busy_ms: 250,
            worker_utilization: 0.004,
            git_cache: CacheStats {
                hits: 3,
                misses: 1,
                evictions: 2,
                entries: 4,
                avg_lookup_us: 2.5,
            },
            watcher: WatchStats {
                subscriptions: 2,
                events: 9,
            },
            ..TelemetrySnapshot::default()
        }
    }

    #[test]
    fn report_includes_cache_and_watcher_metrics() {
        let report = format_report(&sample(), false);
        assert!(report.contains("renders           10"));
        assert!(report.contains("hits 3"));
        assert!(report.contains("evictions 2"));
        assert!(report.contains("2.5 us avg"));
        assert!(report.contains("75.0% hit rate"));
        assert!(report.contains("2 subscriptions · 9 events"));
        assert!(report.contains("1m05s"));
    }

    #[test]
    fn hit_rate_is_na_without_lookups() {
        assert_eq!(format_ratio(&CacheStats::default()), "n/a");
    }

    #[test]
    fn compute_rates_uses_counter_deltas_over_elapsed() {
        let prev = sample();
        let mut curr = sample();
        curr.render_requests += 20;
        curr.git_refreshes += 4;
        curr.watcher.events += 6;

        let rates = compute_rates(&prev, &curr, Duration::from_secs(2));
        assert!((rates.renders_per_s - 10.0).abs() < 1e-9);
        assert!((rates.git_refreshes_per_s - 2.0).abs() < 1e-9);
        assert!((rates.watcher_events_per_s - 3.0).abs() < 1e-9);
    }

    #[test]
    fn compute_rates_clamps_on_counter_reset_and_zero_elapsed() {
        let mut prev = sample();
        prev.render_requests = 100;
        let mut curr = sample();
        curr.render_requests = 5; // daemon restart: counter went backwards

        let rates = compute_rates(&prev, &curr, Duration::from_secs(1));
        assert_eq!(rates.renders_per_s, 0.0);
        assert_eq!(
            compute_rates(&prev, &curr, Duration::ZERO),
            Rates::default()
        );
    }

    #[test]
    fn rate_averages_window_rolls_and_means() {
        let mut averages = RateAverages::default();
        assert!(averages.is_empty());
        assert_eq!(averages.average(), Rates::default());

        for _ in 0..RATE_WINDOW + 4 {
            averages.push(Rates {
                renders_per_s: 4.0,
                ..Rates::default()
            });
        }
        assert_eq!(averages.window.len(), RATE_WINDOW);
        assert!((averages.average().renders_per_s - 4.0).abs() < 1e-9);
    }

    #[test]
    fn report_includes_rolling_rate_when_present() {
        let report = format_report_with_rates(
            &sample(),
            Some(Rates {
                renders_per_s: 12.5,
                git_refreshes_per_s: 1.0,
                watcher_events_per_s: 3.0,
            }),
            false,
        );
        assert!(report.contains("12.5 renders/s"), "report: {report}");
        assert!(report.contains("1.0 git/s"));
    }
}
