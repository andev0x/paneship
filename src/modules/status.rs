use crate::core::prompt::PromptContext;
use crate::modules::Module;

pub struct Status;

impl Module for Status {
    fn render(&self, context: &PromptContext) -> String {
        let tmux_indicator = if context.is_tmux {
            "\x1b[33m[T]\x1b[0m "
        } else {
            ""
        };

        if context.exit_code == 0 {
            format!("{}\x1b[32m➜\x1b[0m", tmux_indicator)
        } else {
            format!("{}\x1b[31m➜ [{}]\x1b[0m", tmux_indicator, context.exit_code)
        }
    }
}
