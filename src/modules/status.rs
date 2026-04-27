use crate::core::prompt::PromptContext;

pub fn render_cursor(context: &PromptContext) -> String {
    let config = &context.config.status;

    if context.exit_code == 0 {
        format!("\x1b[32m{}\x1b[0m ", config.success_icon)
    } else {
        format!("\x1b[31m{} [{}]\x1b[0m ", config.failure_icon, context.exit_code)
    }
}
