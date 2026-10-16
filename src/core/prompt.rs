use crate::cache::LanguageSnapshot;
use crate::core::config::Config;
use crate::modules::git::GitResolution;
use std::path::PathBuf;

/// Resolution state for a daemon-resolved metadata segment.
///
/// Mirrors [`GitResolution`] so the render path can emit an explicit
/// placeholder while a worker computes rather than blocking on manifest I/O.
#[derive(Debug, Clone)]
pub enum MetadataState<T> {
    /// A usable value. `stale` marks data served from an expired entry while a
    /// background refresh is in flight.
    Ready {
        value: T,
        #[allow(dead_code)]
        stale: bool,
    },
    /// The source was detected but its metadata is not available yet.
    Pending,
    /// No language/package applies to this directory.
    Absent,
}

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub cwd: PathBuf,
    pub width: usize,
    pub exit_code: i32,
    pub last_command_duration_ms: Option<u64>,
    pub config: std::sync::Arc<Config>,
    /// Git state pre-resolved by the daemon render path. When `None` the Git
    /// module resolves it (via the daemon socket or a bounded local compute).
    pub git_state: Option<GitResolution>,
    /// Language metadata pre-resolved by the daemon render path.
    pub language_state: Option<MetadataState<LanguageSnapshot>>,
    /// Package version pre-resolved by the daemon render path.
    pub package_state: Option<MetadataState<String>>,
}

impl PromptContext {
    pub fn from_inputs(
        cwd: Option<PathBuf>,
        width: Option<usize>,
        exit_code: i32,
        last_command_duration_ms: Option<u64>,
    ) -> Self {
        let cwd =
            cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        #[cfg(unix)]
        let width = width
            .or_else(crate::tmux::get_pane_width)
            .unwrap_or_else(|| {
                // Default to 80 if we can't detect terminal width
                terminal_size::terminal_size()
                    .map(|(w, _)| w.0 as usize)
                    .unwrap_or(80)
            });

        #[cfg(not(unix))]
        let width = width.unwrap_or_else(|| {
            terminal_size::terminal_size()
                .map(|(w, _)| w.0 as usize)
                .unwrap_or(80)
        });

        let last_command_duration_ms = last_command_duration_ms.or_else(read_duration_ms_from_env);

        let config = Config::load_cached();

        Self {
            cwd,
            width,
            exit_code,
            last_command_duration_ms,
            config,
            git_state: None,
            language_state: None,
            package_state: None,
        }
    }
}

fn read_duration_ms_from_env() -> Option<u64> {
    let raw = std::env::var("PANESHIP_LAST_CMD_DURATION_MS").ok()?;
    raw.trim().parse::<u64>().ok()
}
