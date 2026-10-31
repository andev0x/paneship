//! Metadata segment: language/runtime version, command duration and clock.
//!
//! Language detection is **never** performed on the hot render path. The daemon
//! resolves it from the cache, dispatching a background worker on a miss or TTL
//! expiry and returning an explicit placeholder (e.g. `[lang...]`) so the
//! segment never blocks on manifest parsing.

use crate::cache::LanguageSnapshot;
use crate::core::layout::visible_width;
use crate::core::prompt::{MetadataState, PromptContext};
use std::path::Path;

/// Relative priority of each RPROMPT element. Higher values survive longer as
/// the pane narrows. The OS identification glyph is the anchor and is retained
/// the longest; language/version follows, then the clock. Infrastructure tokens
/// and command timing collapse first.
const PRIORITY_OS: u8 = 110;
const PRIORITY_LANGUAGE: u8 = 100;
const PRIORITY_TIME: u8 = 80;
const PRIORITY_ENVIRONMENT: u8 = 70;
const PRIORITY_DURATION: u8 = 60;

/// One candidate RPROMPT segment, with its display order and collapse priority.
struct Candidate {
    order: usize,
    priority: u8,
    styled: String,
    width: usize,
}

impl Candidate {
    fn new(order: usize, priority: u8, styled: String) -> Self {
        let width = visible_width(styled.as_str());
        Self {
            order,
            priority,
            styled,
            width,
        }
    }
}

/// Render the RPROMPT metadata region within `max_visible_width`, dropping the
/// lowest-priority elements first when space is constrained.
///
/// Selection is priority-driven, but the surviving elements are emitted in
/// their natural reading order (language, timing, clock).
pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let separator = "   ";
    let separator_width = visible_width(separator);

    let mut candidates: Vec<Candidate> = metadata_parts(context, max_visible_width)
        .into_iter()
        .filter(|candidate| candidate.width > 0)
        .collect();

    // Greedy allocation by descending priority; ties keep display order.
    candidates.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.order.cmp(&b.order))
    });

    let mut selected: Vec<Candidate> = Vec::new();
    let mut used = 0usize;
    for candidate in candidates {
        let extra = if selected.is_empty() {
            candidate.width
        } else {
            separator_width + candidate.width
        };
        if used + extra <= max_visible_width {
            used += extra;
            selected.push(candidate);
        }
    }

    if selected.is_empty() {
        return String::new();
    }

    selected.sort_by_key(|candidate| candidate.order);
    selected
        .into_iter()
        .map(|candidate| candidate.styled)
        .collect::<Vec<_>>()
        .join(separator)
}

/// Build the right-telemetry candidates in their contractual display order:
///
/// 1. OS / environment identification
/// 2. runtime / toolchain version
/// 3. command execution time
/// 4. system timestamp
///
/// The `order` field preserves this sequence after priority-based collapse.
fn metadata_parts(context: &PromptContext, max_visible_width: usize) -> Vec<Candidate> {
    let mut parts = Vec::with_capacity(5);
    let mut order = 0;

    if let Some(os) = os_part(context) {
        parts.push(Candidate::new(order, PRIORITY_OS, os));
        order += 1;
    }

    if let Some(environment) = environment_part(context, max_visible_width) {
        parts.push(Candidate::new(order, PRIORITY_ENVIRONMENT, environment));
        order += 1;
    }

    if let Some(language) = language_part(context) {
        parts.push(Candidate::new(order, PRIORITY_LANGUAGE, language));
        order += 1;
    }

    if let Some(duration) = render_command_duration(context) {
        parts.push(Candidate::new(order, PRIORITY_DURATION, duration));
        order += 1;
    }

    if let Some(time) = current_time_hhmm() {
        let plain = format!("{} {time}", context.config.time_icon());
        parts.push(Candidate::new(
            order,
            PRIORITY_TIME,
            styled(context.config.time_color(), &plain),
        ));
    }

    parts
}

/// Host OS identification glyph. Right telemetry only; never a path prefix.
fn os_part(context: &PromptContext) -> Option<String> {
    let icon = context.config.os_icon();
    if icon.trim().is_empty() {
        return None;
    }
    Some(styled(context.config.os_color(), icon))
}

/// Infrastructure tokens (Kubernetes, container, cloud, Nix/Direnv) are
/// environment identification, so they belong beside the OS glyph on the
/// right. They are emitted as one candidate so the outer priority allocator
/// can drop the whole group when the pane narrows.
fn environment_part(context: &PromptContext, max_visible_width: usize) -> Option<String> {
    let rendered = crate::modules::environment::render_with_max_width(context, max_visible_width);
    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
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
    use crate::detectors::infra::{EnvironmentInfo, EnvironmentKind, EnvironmentSnapshot};
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
            environment_state: None,
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
    fn absent_renders_no_language_placeholder() {
        let rendered = render_with_max_width(&context(MetadataState::Absent), 120);
        assert!(!rendered.contains("lang..."));
    }

    #[test]
    fn right_telemetry_starts_with_os_identification() {
        let ctx = context(MetadataState::Absent);
        let rendered = render_with_max_width(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&rendered);
        assert!(
            plain.starts_with(ctx.config.os_icon()),
            "OS identification must lead the right region: {plain:?}"
        );
    }

    #[test]
    fn right_telemetry_order_is_os_version_duration_clock() {
        let ready = MetadataState::Ready {
            value: LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            },
            stale: false,
        };
        let mut ctx = context(ready);
        ctx.last_command_duration_ms = Some(123);

        let rendered = render_with_max_width(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&rendered);

        let os = plain.find(ctx.config.os_icon()).expect("OS glyph");
        let version = plain.find("1.74").expect("runtime version");
        let duration = plain.find("123ms").expect("duration");
        let clock = plain
            .find(current_time_hhmm().unwrap().as_str())
            .expect("clock");

        assert!(os < version, "OS must precede runtime: {plain:?}");
        assert!(
            version < duration,
            "runtime must precede duration: {plain:?}"
        );
        assert!(duration < clock, "duration must precede clock: {plain:?}");
    }

    #[test]
    fn environment_identification_sits_between_os_and_runtime() {
        let ready = MetadataState::Ready {
            value: LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            },
            stale: false,
        };
        let mut ctx = context(ready);
        ctx.environment_state = Some(MetadataState::Ready {
            value: EnvironmentSnapshot {
                entries: vec![EnvironmentInfo {
                    kind: EnvironmentKind::Kubernetes,
                    label: "dev/team-a".to_string(),
                }],
            },
            stale: false,
        });

        let rendered = render_with_max_width(&ctx, 120);
        let plain = crate::core::layout::strip_ansi(&rendered);

        let os = plain.find(ctx.config.os_icon()).expect("OS glyph");
        let environment = plain.find("dev/team-a").expect("environment");
        let version = plain.find("1.74").expect("runtime version");

        assert!(os < environment, "OS must precede environment: {plain:?}");
        assert!(
            environment < version,
            "environment must precede runtime: {plain:?}"
        );
    }

    #[test]
    fn narrow_width_drops_low_priority_rprompt_elements() {
        let ready = MetadataState::Ready {
            value: LanguageSnapshot {
                language: "rust".to_string(),
                version: Some("1.74".to_string()),
            },
            stale: false,
        };
        let mut ctx = context(ready);
        ctx.last_command_duration_ms = Some(123);

        let wide = render_with_max_width(&ctx, 120);
        assert!(wide.contains("1.74"), "language missing: {wide:?}");
        assert!(wide.contains("123ms"), "duration missing: {wide:?}");

        // Enough room for the OS anchor and the runtime version, but not the
        // lower-priority timing/clock elements.
        let narrow = render_with_max_width(&ctx, 10);
        assert!(
            narrow.contains("1.74"),
            "language should survive: {narrow:?}"
        );
        assert!(
            !narrow.contains("123ms"),
            "duration must collapse first: {narrow:?}"
        );
    }
}
