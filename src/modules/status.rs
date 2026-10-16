use crate::core::prompt::PromptContext;

pub fn render_cursor(context: &PromptContext) -> String {
    if context.exit_code == 0 {
        format!(
            "\x1b[{}m{}\x1b[0m ",
            context.config.success_color(),
            context.config.success_icon()
        )
    } else {
        format!(
            "\x1b[{}m{} [{}]\x1b[0m ",
            context.config.failure_color(),
            context.config.failure_icon(),
            context.exit_code
        )
    }
}
