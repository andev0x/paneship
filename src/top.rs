//! `paneship top` — live daemon telemetry (Phase 3, Module D).
//!
//! Queries the daemon's [`crate::daemon::Request::Stats`] endpoint and renders
//! worker utilization, cache hit ratios, LRU evictions and watcher throughput.
//! A live `ratatui` view is shown on a TTY; `--once`/`--json` (and any
//! non-interactive stdout) print a single snapshot. When no daemon is running
//! it falls back to the Phase 1 in-process component benchmark.

use crate::daemon::telemetry::{CacheStats, TelemetrySnapshot};
use std::io::IsTerminal;
use std::time::Duration;

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

    let result = (|| {
        loop {
            // A dropped daemon degrades to an empty snapshot rather than exiting.
            let snapshot = crate::daemon::query_telemetry().unwrap_or_default();
            terminal
                .draw(|frame| {
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title("Paneship Daemon Telemetry");
                    let paragraph = Paragraph::new(Text::from(format_report(&snapshot, true)))
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
}
