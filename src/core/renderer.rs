use crate::core::layout::visible_width;
use crate::core::prompt::PromptContext;
use crate::modules::{directory, git, metadata, package, status};

pub fn render(context: &PromptContext) -> String {
    let width = context.width.max(1);
    let right = render_right(context, width);
    let right_width = visible_width(right.as_str());

    let left_budget = if right_width == 0 {
        width
    } else {
        width.saturating_sub(right_width + 1).max(1)
    };

    let left = render_left(context, left_budget);
    let left_width = visible_width(left.as_str());

    let first_line = if right_width == 0 || left_width + 1 + right_width > width {
        left
    } else {
        let spacing = width.saturating_sub(left_width + right_width);
        format!("{left}{}{right}", " ".repeat(spacing))
    };

    let second_line = status::render_cursor(context);
    format!("{first_line}\n{second_line}")
}

fn render_right(context: &PromptContext, width: usize) -> String {
    let min_left_budget = 16;
    let right_budget = width.saturating_sub(min_left_budget + 1);
    metadata::render_with_max_width(context, right_budget)
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

    let mut left = directory::render_with_max_width(context, dir_budget);
    left = append_segment(
        left,
        "  ",
        git::render_with_max_width,
        context,
        max_visible_width,
    );
    append_segment(
        left,
        "  ·  ",
        package::render_with_max_width,
        context,
        max_visible_width,
    )
}

fn append_segment(
    current: String,
    separator: &str,
    renderer: fn(&PromptContext, usize) -> String,
    context: &PromptContext,
    max_visible_width: usize,
) -> String {
    let current_width = visible_width(current.as_str());
    if current_width >= max_visible_width {
        return current;
    }

    let separator = if current.is_empty() { "" } else { separator };
    let separator_width = visible_width(separator);
    let remaining = max_visible_width.saturating_sub(current_width + separator_width);
    if remaining == 0 {
        return current;
    }

    let segment = renderer(context, remaining);
    if segment.is_empty() {
        return current;
    }

    let candidate = if current.is_empty() {
        segment
    } else {
        format!("{current}{separator}{segment}")
    };

    if visible_width(candidate.as_str()) <= max_visible_width {
        candidate
    } else {
        current
    }
}
