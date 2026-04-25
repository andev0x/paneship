use crate::core::config::Config;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub cwd: PathBuf,
    pub width: usize,
    pub exit_code: i32,
    pub is_tmux: bool,
    pub config: std::sync::Arc<Config>,
}

impl PromptContext {
    pub fn from_inputs(cwd: Option<PathBuf>, width: Option<usize>, exit_code: i32) -> Self {
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

        let is_tmux = std::env::var("TMUX").is_ok();
        let config = std::sync::Arc::new(Config::load());

        Self {
            cwd,
            width,
            exit_code,
            is_tmux,
            config,
        }
    }
}
// Since I added terminal_size, I should check if it is in Cargo.toml
