use crate::core::config::Config;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub cwd: PathBuf,
    pub width: usize,
    pub exit_code: i32,
    pub last_command_duration_ms: Option<u64>,
    pub config: std::sync::Arc<Config>,
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

        let config = std::sync::Arc::new(Config::load());

        Self {
            cwd,
            width,
            exit_code,
            last_command_duration_ms,
            config,
        }
    }
}

fn read_duration_ms_from_env() -> Option<u64> {
    let raw = std::env::var("PANESHIP_LAST_CMD_DURATION_MS").ok()?;
    raw.trim().parse::<u64>().ok()
}
