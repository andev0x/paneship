use crate::core::layout::visible_width;
use crate::core::prompt::PromptContext;
use crate::modules::{directory, environment, git, metadata, package, status};
use serde::{Deserialize, Serialize};

/// Structured prompt output split into its layout regions.
///
/// Shells with a native right-side prompt (zsh `RPROMPT`/`RPS1`, fish
/// `fish_right_prompt`, nushell `PROMPT_COMMAND_RIGHT`) consume these regions
/// directly; shells without one let [`combine`] pad the right region onto the
/// first line.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptParts {
    /// Left-aligned prompt content (directory, git, environment, package).
    pub left: String,
    /// Right-aligned metadata (language, duration, clock). Empty when disabled
    /// or when the pane is too narrow to host it.
    pub right: String,
    /// Cursor / exit-status segment rendered on its own line.
    pub cursor: String,
}

/// Render the classic single-string prompt (left and right share one line).
pub fn render(context: &PromptContext) -> String {
    combine(&render_parts(context), context.width.max(1))
}

/// Render the structured prompt regions without combining them.
///
/// Width-aware allocation happens here: the daemon knows the terminal width
/// budget and reserves space for the right region before laying out the left,
/// so a long directory can never push the RPROMPT into a second terminal row.
pub fn render_parts(context: &PromptContext) -> PromptParts {
    let width = context.width.max(1);
    let right = if rprompt_enabled(context, width) {
        render_right(context, width)
    } else {
        String::new()
    };
    let right_width = visible_width(right.as_str());

    let left_budget = if right_width == 0 {
        width
    } else {
        width.saturating_sub(right_width + 1).max(1)
    };

    PromptParts {
        left: render_left(context, left_budget),
        right,
        cursor: status::render_cursor(context),
    }
}

/// Pad [`PromptParts`] into a two-line prompt, right-aligning the RPROMPT.
pub fn combine(parts: &PromptParts, width: usize) -> String {
    let width = width.max(1);
    let right_width = visible_width(parts.right.as_str());
    let left_width = visible_width(parts.left.as_str());

    let first_line = if right_width == 0 || left_width + 1 + right_width > width {
        parts.left.clone()
    } else {
        let spacing = width.saturating_sub(left_width + right_width);
        format!("{}{}{}", parts.left, " ".repeat(spacing), parts.right)
    };

    format!("{first_line}\n{}", parts.cursor)
}

fn rprompt_enabled(context: &PromptContext, width: usize) -> bool {
    context.config.prompt.rprompt && width >= context.config.prompt.rprompt_min_width
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
    left = append_segment(
        left,
        "  ",
        environment::render_with_max_width,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use crate::modules::git::GitResolution;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn context(width: usize) -> PromptContext {
        PromptContext {
            cwd: PathBuf::from("/tmp"),
            width,
            exit_code: 0,
            last_command_duration_ms: Some(1500),
            config: Arc::new(Config::default()),
            git_state: Some(GitResolution::NotRepository),
            language_state: None,
            package_state: None,
            environment_state: None,
        }
    }

    #[test]
    fn parts_expose_left_right_and_cursor() {
        let parts = render_parts(&context(80));
        assert!(!parts.left.is_empty(), "left region should not be empty");
        assert!(
            parts.right.contains("1s"),
            "right region should carry the duration: {:?}",
            parts.right
        );
        assert!(
            !parts.cursor.is_empty(),
            "cursor region should not be empty"
        );
    }

    #[test]
    fn narrow_width_drops_the_right_region() {
        let parts = render_parts(&context(10));
        assert!(
            parts.right.is_empty(),
            "right region must collapse below the minimum width: {:?}",
            parts.right
        );
    }

    #[test]
    fn combine_right_aligns_within_the_width_budget() {
        let parts = render_parts(&context(120));
        let combined = combine(&parts, 120);
        let first_line = combined.lines().next().unwrap();
        assert!(visible_width(first_line) <= 120);
    }
}
