//! Prompt region composition.
//!
//! The prompt is split into two independent pipelines that never share segment
//! builders:
//!
//! * [`left_segments`] — context only: directory, Git and project version.
//! * [`right_segments`] — telemetry only: OS/environment identification,
//!   runtime/toolchain version, command duration and clock.
//!
//! Keeping the pipelines separate is an architectural invariant, not just a
//! formatting choice: host OS logos and infrastructure badges are resolved by
//! the right region alone, so directory navigation (`cd`, `z`) cannot leak an
//! icon into the left prompt.

use crate::core::layout::visible_width;
use crate::core::prompt::PromptContext;
use crate::modules::{directory, git, metadata, package, status};
use serde::{Deserialize, Serialize};

/// Separator between left-context segments.
const LEFT_SEPARATOR: &str = "  ";
/// Wider separator before the project version, for visual grouping.
const PACKAGE_SEPARATOR: &str = "  ·  ";
/// Keep at least this much room for the left context when sizing the RPROMPT.
const MIN_LEFT_BUDGET: usize = 16;

/// Structured prompt output split into its layout regions.
///
/// Shells with a native right-side prompt (zsh `RPROMPT`/`RPS1`, fish
/// `fish_right_prompt`, nushell `PROMPT_COMMAND_RIGHT`) consume these regions
/// directly; shells without one let [`combine`] pad the right region onto the
/// first line.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptParts {
    /// Left-aligned context (directory, Git, project version). Never contains
    /// OS, distribution or environment identification.
    pub left: String,
    /// Right-aligned telemetry (OS/environment, runtime, duration, clock).
    /// Empty when disabled or when the pane is too narrow to host it.
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
/// Width-aware allocation happens here: the right region is measured first and
/// its column budget is reserved before the left context is laid out, so a long
/// directory can never push the RPROMPT onto a second terminal row.
pub fn render_parts(context: &PromptContext) -> PromptParts {
    let width = context.width.max(1);

    // Build the right pipeline first, then reserve its width. The two
    // pipelines are otherwise independent and read disjoint segment builders.
    let right = if rprompt_enabled(context, width) {
        right_segments(context, width)
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
        left: left_segments(context, left_budget),
        right,
        cursor: status::render_cursor(context),
    }
}

/// Pad [`PromptParts`] into a two-line prompt, right-aligning the RPROMPT.
pub fn combine(parts: &PromptParts, width: usize) -> String {
    let width = width.max(1);
    let right_width = visible_width(parts.right.as_str());
    let left_width = visible_width(parts.left.as_str());

    let mut output = String::with_capacity(
        parts.left.len() + parts.right.len() + parts.cursor.len() + width + 2,
    );

    output.push_str(parts.left.as_str());

    if right_width > 0 && left_width + 1 + right_width <= width {
        for _ in 0..width.saturating_sub(left_width + right_width) {
            output.push(' ');
        }
        output.push_str(parts.right.as_str());
    }

    output.push('\n');
    output.push_str(parts.cursor.as_str());
    output
}

fn rprompt_enabled(context: &PromptContext, width: usize) -> bool {
    context.config.prompt.rprompt && width >= context.config.prompt.rprompt_min_width
}

/// Right telemetry pipeline.
///
/// Contractual order: OS/environment identification, runtime/toolchain,
/// command execution time, system timestamp. The metadata module owns priority
/// collapse within the reserved budget.
fn right_segments(context: &PromptContext, width: usize) -> String {
    let right_budget = width.saturating_sub(MIN_LEFT_BUDGET + 1);
    metadata::render_with_max_width(context, right_budget)
}

/// Left context pipeline.
///
/// Only directory, Git and project/crate version are permitted here. OS and
/// environment detection are intentionally absent.
fn left_segments(context: &PromptContext, max_visible_width: usize) -> String {
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
        LEFT_SEPARATOR,
        git::render_with_max_width,
        context,
        max_visible_width,
    );
    append_segment(
        left,
        PACKAGE_SEPARATOR,
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

    if current.is_empty() {
        return segment;
    }

    let mut candidate = String::with_capacity(current.len() + separator.len() + segment.len());
    candidate.push_str(current.as_str());
    candidate.push_str(separator);
    candidate.push_str(segment.as_str());

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
    use crate::core::prompt::MetadataState;
    use crate::detectors::infra::{EnvironmentInfo, EnvironmentKind, EnvironmentSnapshot};
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

    fn context_with_environment(width: usize) -> PromptContext {
        let mut ctx = context(width);
        ctx.environment_state = Some(MetadataState::Ready {
            value: EnvironmentSnapshot {
                entries: vec![EnvironmentInfo {
                    kind: EnvironmentKind::Kubernetes,
                    label: "prod/team-a".to_string(),
                }],
            },
            stale: false,
        });
        ctx
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

    #[test]
    fn os_icon_is_telemetry_only_never_left_context() {
        let ctx = context_with_environment(120);
        let parts = render_parts(&ctx);
        assert!(
            !parts.left.contains(ctx.config.os_icon()),
            "OS icon leaked into the left prompt: {:?}",
            parts.left
        );
        assert!(
            parts.right.contains(ctx.config.os_icon()),
            "right telemetry must identify the OS: {:?}",
            parts.right
        );
    }

    #[test]
    fn infrastructure_tokens_are_right_aligned_not_path_prefixes() {
        let ctx = context_with_environment(120);
        let parts = render_parts(&ctx);
        assert!(
            !parts.left.contains("prod/team-a"),
            "environment badge leaked into the left prompt: {:?}",
            parts.left
        );
        assert!(
            parts.right.contains("prod/team-a"),
            "environment badge belongs to right telemetry: {:?}",
            parts.right
        );
    }

    #[test]
    fn left_pipeline_orders_context_before_version() {
        let mut ctx = context(120);
        ctx.package_state = Some(MetadataState::Ready {
            value: "1.2.3".to_string(),
            stale: false,
        });
        let left = left_segments(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&left);
        assert!(
            plain.contains("1.2.3"),
            "package version missing: {plain:?}"
        );
        assert!(
            plain.find("1.2.3").unwrap() > 0,
            "version must follow the directory context: {plain:?}"
        );
    }

    #[test]
    fn left_pipeline_starts_with_the_bare_path() {
        let mut ctx = context(120);
        ctx.cwd = PathBuf::from("/tmp/paneship");
        ctx.language_state = Some(MetadataState::Ready {
            value: crate::cache::LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            },
            stale: false,
        });

        let left = left_segments(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&left);
        assert!(
            plain.starts_with('/'),
            "the left prompt must begin with a pure path: {plain:?}"
        );
        let runtime_icon = ctx.config.language_style("rust").icon;
        assert!(
            !plain.contains(&runtime_icon),
            "the project language glyph leaked into the path: {plain:?}"
        );
        assert!(
            !plain.contains(ctx.config.os_icon()),
            "the OS glyph leaked into the path: {plain:?}"
        );
        assert!(
            !plain.contains(ctx.config.time_icon()),
            "the clock glyph leaked into the left prompt: {plain:?}"
        );
    }

    #[test]
    fn package_version_carries_no_prefix_glyph() {
        let mut ctx = context(120);
        ctx.package_state = Some(MetadataState::Ready {
            value: "1.2.3".to_string(),
            stale: false,
        });
        let left = left_segments(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&left);
        assert!(
            plain.contains("v1.2.3"),
            "version must be plain text: {plain:?}"
        );
    }

    #[test]
    fn os_glyph_appears_exactly_once_in_right_telemetry() {
        let ctx = context_with_environment(120);
        let parts = render_parts(&ctx);
        let os = ctx.config.os_icon();
        assert_eq!(
            parts.right.matches(os).count(),
            1,
            "OS identification must appear exactly once: {:?}",
            parts.right
        );
        assert_eq!(
            parts.left.matches(os).count(),
            0,
            "OS identification is right-telemetry only: {:?}",
            parts.left
        );
    }
}
