//! Metadata segment: language/runtime version, command duration and clock.
//!
//! Language detection is **never** performed on the hot render path. The daemon
//! resolves it from the cache, dispatching a background worker on a miss or TTL
//! expiry and returning an explicit placeholder (e.g. `[lang...]`) so the
//! segment never blocks on manifest parsing.

use crate::cache::LanguageSnapshot;
use crate::core::layout::{truncate_plain_to_width, visible_width};
use crate::core::prompt::{MetadataState, PromptContext};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let mut rendered = Vec::new();
    let separator = "   ";
    let separator_width = UnicodeWidthStr::width(separator);
    let mut current_width = 0;

    for candidate in metadata_parts(context) {
        if candidate.is_empty() {
            continue;
        }

        let candidate_width = visible_width(candidate.as_str());
        if candidate_width == 0 {
            continue;
        }

        let proposed = if rendered.is_empty() {
            candidate_width
        } else {
            current_width + separator_width + candidate_width
        };

        if proposed <= max_visible_width {
            rendered.push(candidate);
            current_width = proposed;
        }
    }

    let output = rendered.join(separator);
    if output.is_empty() {
        return String::new();
    }

    if visible_width(output.as_str()) <= max_visible_width {
        output
    } else {
        let plain = crate::core::layout::strip_ansi(output.as_str());
        truncate_plain_to_width(plain.as_str(), max_visible_width)
    }
}

fn metadata_parts(context: &PromptContext) -> Vec<String> {
    let mut parts = Vec::new();

    if let Some(language) = language_part(context) {
        parts.push(language);
    }

    if let Some(duration) = render_command_duration(context) {
        parts.push(duration);
    }

    if let Some(time) = current_time_hhmm() {
        let plain = format!("{} {time}", context.config.time_icon());
        parts.push(styled(context.config.time_color(), &plain));
    }

    parts
}

/// Render the language segment from the pre-resolved state.
fn language_part(context: &PromptContext) -> Option<String> {
    match &context.language_state {
        Some(MetadataState::Ready { value, .. }) => Some(render_language(context, value)),
        Some(MetadataState::Pending) => Some(styled(
            context.config.time_color(),
            context.config.language_placeholder(),
        )),
        Some(MetadataState::Absent) => None,
        // No daemon-provided state: cold-start fallback. Cheap manifest parsing
        // only, never an external subprocess.
        None => local_language_part(context),
    }
}

fn render_language(context: &PromptContext, snapshot: &LanguageSnapshot) -> String {
    let style = context.config.language_style(&snapshot.language);
    let plain = match snapshot.version.as_deref() {
        Some(version) if !version.is_empty() => format!("{} {version}", style.icon),
        _ => style.icon.clone(),
    };
    styled(&style.color, &plain)
}

/// Cold-start fallback used only when no daemon state was supplied (e.g. the
/// daemon is unreachable). Extraction is in-process and subprocess-free.
fn local_language_part(context: &PromptContext) -> Option<String> {
    if std::env::var("PANESHIP_DAEMON").is_ok() {
        return None;
    }
    let info = crate::detectors::detect_runtime(context.cwd.as_path())?;
    let snapshot = LanguageSnapshot {
        language: info.language,
        version: info.version,
    };
    Some(render_language(context, &snapshot))
}

fn render_command_duration(context: &PromptContext) -> Option<String> {
    let ms = context.last_command_duration_ms?;
    let icon = context.config.duration_icon();
    let plain = if ms < 1_000 {
        format!("{icon} {ms}ms")
    } else {
        let secs = ms / 1_000;
        if secs < 60 {
            format!("{icon} {secs}s")
        } else {
            let mins = secs / 60;
            let rem_secs = secs % 60;
            format!("{icon} {mins}m{rem_secs:02}s")
        }
    };

    Some(styled(context.config.time_color(), plain.as_str()))
}

fn current_time_hhmm() -> Option<String> {
    unsafe {
        let mut now: libc::time_t = 0;
        libc::time(&mut now as *mut libc::time_t);

        let mut local: libc::tm = std::mem::zeroed();

        #[cfg(not(windows))]
        if libc::localtime_r(&now as *const libc::time_t, &mut local as *mut libc::tm).is_null() {
            return None;
        }

        #[cfg(windows)]
        if libc::localtime_s(&mut local as *mut libc::tm, &now as *const libc::time_t) != 0 {
            return None;
        }

        Some(format!("{:02}:{:02}", local.tm_hour, local.tm_min))
    }
}

/// Compute and cache language + package metadata for the daemon worker.
pub fn compute_metadata_for_daemon(path: &Path) {
    if let Some(matched) = crate::detectors::find_marker(path) {
        let snapshot = crate::detectors::detect_runtime(path).map(|info| LanguageSnapshot {
            language: info.language,
            version: info.version,
        });
        crate::cache::store_language(&matched.marker, snapshot);
    }
    crate::modules::package::compute_package_metadata_for_daemon(path);
}

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use std::sync::Arc;

    fn context(state: MetadataState<LanguageSnapshot>) -> PromptContext {
        PromptContext {
            cwd: std::env::temp_dir(),
            width: 120,
            exit_code: 0,
            last_command_duration_ms: None,
            config: Arc::new(Config::default()),
            git_state: None,
            language_state: Some(state),
            package_state: Some(MetadataState::Absent),
        }
    }

    #[test]
    fn pending_renders_language_placeholder() {
        let rendered = render_with_max_width(&context(MetadataState::Pending), 120);
        assert!(
            rendered.contains("lang..."),
            "placeholder missing: {rendered:?}"
        );
    }

    #[test]
    fn ready_renders_language_and_version() {
        let state = MetadataState::Ready {
            value: LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            },
            stale: false,
        };
        let rendered = render_with_max_width(&context(state), 120);
        assert!(rendered.contains("1.74"), "version missing: {rendered:?}");
    }

    #[test]
    fn absent_renders_nothing() {
        let rendered = render_with_max_width(&context(MetadataState::Absent), 120);
        assert!(!rendered.contains("lang..."));
    }
}
