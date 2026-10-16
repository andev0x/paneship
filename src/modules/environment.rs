//! Infrastructure / environment segment (Phase 3, Module B).
//!
//! Renders the active Kubernetes context, container, cloud profile and
//! Nix/Direnv signals. Like every other segment, the state is resolved by the
//! daemon's worker pool and injected into [`PromptContext`]; the render path
//! never runs a detector.

use crate::core::prompt::{MetadataState, PromptContext};
use crate::detectors::infra::EnvironmentInfo;
use unicode_width::UnicodeWidthStr;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let snapshot = match &context.environment_state {
        Some(MetadataState::Ready { value, .. }) => value,
        // The daemon resolves this off-path; a cold client renders nothing
        // rather than running detectors inside the prompt budget.
        Some(MetadataState::Pending) | Some(MetadataState::Absent) | None => return String::new(),
    };

    if snapshot.is_empty() {
        return String::new();
    }

    let separator = "  ";
    let separator_width = UnicodeWidthStr::width(separator);
    let mut selected: Vec<String> = Vec::new();
    let mut used = 0usize;

    for entry in &snapshot.entries {
        let token = render_entry(context, entry);
        if token.is_empty() {
            continue;
        }
        let token_width = crate::core::layout::visible_width(token.as_str());
        if token_width == 0 {
            continue;
        }
        let extra = if selected.is_empty() {
            token_width
        } else {
            separator_width + token_width
        };
        if used + extra <= max_visible_width {
            used += extra;
            selected.push(token);
        }
    }

    selected.join(separator)
}

fn render_entry(context: &PromptContext, entry: &EnvironmentInfo) -> String {
    if entry.label.trim().is_empty() {
        return String::new();
    }

    let icon = context
        .config
        .environment_icon(entry.kind.key())
        .unwrap_or_default();
    let plain = if icon.is_empty() {
        entry.label.clone()
    } else {
        format!("{icon} {}", entry.label)
    };

    format!("\x1b[{}m{plain}\x1b[0m", context.config.environment_color())
}

/// Compute and cache the environment snapshot for the daemon worker.
pub fn compute_environment_for_daemon(path: &std::path::Path) {
    let config = crate::core::config::Config::load_cached();
    let snapshot = crate::detectors::infra::detect_environment(path, &config.environment);
    crate::cache::store_environment(path, Some(snapshot));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use crate::detectors::infra::{EnvironmentInfo, EnvironmentKind, EnvironmentSnapshot};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn context(entries: Vec<EnvironmentInfo>) -> PromptContext {
        PromptContext {
            cwd: PathBuf::from("/tmp"),
            width: 120,
            exit_code: 0,
            last_command_duration_ms: None,
            config: Arc::new(Config::default()),
            git_state: None,
            language_state: None,
            package_state: None,
            environment_state: Some(MetadataState::Ready {
                value: EnvironmentSnapshot { entries },
                stale: false,
            }),
        }
    }

    #[test]
    fn renders_active_environment_tokens() {
        let ctx = context(vec![
            EnvironmentInfo {
                kind: EnvironmentKind::Kubernetes,
                label: "dev/team-a".to_string(),
            },
            EnvironmentInfo {
                kind: EnvironmentKind::Docker,
                label: "docker".to_string(),
            },
        ]);
        let rendered = render_with_max_width(&ctx, 120);
        assert!(
            rendered.contains("dev/team-a"),
            "k8s label missing: {rendered:?}"
        );
        assert!(
            rendered.contains("docker"),
            "docker label missing: {rendered:?}"
        );
    }

    #[test]
    fn empty_and_pending_states_render_nothing() {
        assert!(render_with_max_width(&context(vec![]), 120).is_empty());

        let mut ctx = context(vec![EnvironmentInfo {
            kind: EnvironmentKind::Nix,
            label: "pure".to_string(),
        }]);
        ctx.environment_state = Some(MetadataState::Pending);
        assert!(render_with_max_width(&ctx, 120).is_empty());
    }

    #[test]
    fn narrow_width_drops_trailing_tokens() {
        let ctx = context(vec![
            EnvironmentInfo {
                kind: EnvironmentKind::Kubernetes,
                label: "dev".to_string(),
            },
            EnvironmentInfo {
                kind: EnvironmentKind::Docker,
                label: "docker".to_string(),
            },
        ]);
        let rendered = render_with_max_width(&ctx, 4);
        assert!(
            !rendered.contains("docker"),
            "docker should not fit: {rendered:?}"
        );
    }
}
