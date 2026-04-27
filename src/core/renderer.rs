use crate::core::layout::visible_width;
use crate::core::prompt::PromptContext;
use crate::modules::{directory, git, metadata, status};

pub fn render(context: &PromptContext) -> String {
    let width = context.width.max(1);
    let left = render_left(context, width);
    let left_width = visible_width(left.as_str());

    let right_budget = width.saturating_sub(left_width + 1);
    let right = metadata::render_with_max_width(context, right_budget);
    let right_width = visible_width(right.as_str());

    let first_line = if right_width == 0 || left_width + 1 + right_width > width {
        left
    } else {
        let spacing = width.saturating_sub(left_width + right_width);
        format!("{left}{}{right}", " ".repeat(spacing))
    };

    let second_line = status::render_cursor(context);
    format!("{first_line}\n{second_line}")
}

fn render_left(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let dir_budget = if max_visible_width >= 36 {
        max_visible_width.saturating_mul(3) / 5
    } else {
        max_visible_width
    }
    .max(1)
    .min(max_visible_width);

    let dir = directory::render_with_max_width(context, dir_budget);
    let dir_width = visible_width(dir.as_str());

    let remaining = max_visible_width.saturating_sub(dir_width);
    if remaining <= 2 {
        return dir;
    }

    let git = git::render_with_max_width(context, remaining - 1);
    if git.is_empty() {
        return dir;
    }

    let git_width = visible_width(git.as_str());
    if dir_width + 1 + git_width <= max_visible_width {
        format!("{dir} {git}")
    } else {
        dir
    }
}
