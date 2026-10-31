use crate::core::layout::visible_width;
use crate::core::prompt::PromptContext;
use std::path::{Path, PathBuf};

/// Render the directory context as pure text.
///
/// The left pipeline never prefixes the path: no project glyph, no folder
/// glyph, and no OS logo. Context is the path itself; platform and environment
/// identification belongs exclusively to right-side telemetry.
pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let raw_path = compact_path(&context.cwd, &context.config.directory);
    let trimmed_path = if visible_width(raw_path.as_str()) > max_visible_width {
        smart_truncate_path(raw_path.as_str(), max_visible_width)
    } else {
        raw_path
    };

    let plain = if visible_width(trimmed_path.as_str()) > max_visible_width {
        crate::core::layout::truncate_plain_to_width(trimmed_path.as_str(), max_visible_width)
    } else {
        trimmed_path
    };

    format!("\x1b[{}m{}\x1b[0m", context.config.directory_color(), plain)
}

fn smart_truncate_path(input: &str, budget: usize) -> String {
    let parts: Vec<&str> = input.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= 2 {
        return crate::core::layout::truncate_plain_to_width(input, budget);
    }

    let mut prefix = "";
    if input.starts_with('~') {
        prefix = "~";
    } else if input.starts_with('/') {
        prefix = "/";
    }

    let last = parts[parts.len() - 1];
    let candidate = if prefix.is_empty() {
        format!(".../{last}")
    } else {
        format!("{prefix}/.../{last}")
    };

    if visible_width(candidate.as_str()) <= budget {
        candidate
    } else {
        crate::core::layout::truncate_plain_to_width(input, budget)
    }
}

fn compact_path(path: &Path, config: &crate::core::config::DirectoryConfig) -> String {
    if config.truncate_to_repo {
        if let Some(repo_root) = find_repo_root(path) {
            return compact_path_from_repo(path, repo_root.as_path(), config.truncation_length);
        }
    }

    compact_general_path(path, config.truncation_length)
}

fn compact_path_from_repo(path: &Path, repo_root: &Path, truncation_length: usize) -> String {
    let Some(repo_name) = repo_root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
    else {
        return compact_general_path(path, truncation_length);
    };

    let rel = path.strip_prefix(repo_root).ok();
    let mut suffix = vec![repo_name];
    if let Some(rel) = rel {
        suffix.extend(path_segments(rel));
    }

    let mut hidden_prefix = false;
    let prefix = leading_prefix(repo_root, &mut hidden_prefix);
    let suffix = trim_segments(suffix, truncation_length.max(1), true, &mut hidden_prefix);

    join_path_display(prefix.as_str(), hidden_prefix, suffix.as_slice())
}

fn compact_general_path(path: &Path, truncation_length: usize) -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .ok();

    let mut hidden_prefix = false;
    let (prefix, base) = if let Some(home_path) = home {
        if let Ok(rel) = path.strip_prefix(home_path) {
            let has_hidden = path_segments(rel).len() > truncation_length.max(1);
            hidden_prefix = hidden_prefix || has_hidden;
            ("~".to_string(), rel.to_path_buf())
        } else {
            (
                "/".to_string(),
                PathBuf::from(path.to_string_lossy().replace('\\', "/")),
            )
        }
    } else {
        (
            "/".to_string(),
            PathBuf::from(path.to_string_lossy().replace('\\', "/")),
        )
    };

    let segments = path_segments(base.as_path());
    let segments = trim_segments(
        segments,
        truncation_length.max(1),
        false,
        &mut hidden_prefix,
    );

    join_path_display(prefix.as_str(), hidden_prefix, segments.as_slice())
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    crate::cache::repo_root_for(start)
}

fn leading_prefix(path: &Path, hidden_prefix: &mut bool) -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .ok();

    if let Some(home_path) = home {
        if let Ok(rel) = path.strip_prefix(&home_path) {
            if !rel.as_os_str().is_empty() {
                *hidden_prefix = true;
            }
            return "~".to_string();
        }
    }

    if path.is_absolute() {
        let root = Path::new("/");
        if let Ok(rel) = path.strip_prefix(root) {
            if rel.components().next().is_some() {
                *hidden_prefix = true;
            }
        }
        "/".to_string()
    } else {
        String::new()
    }
}

fn path_segments(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect()
}

fn trim_segments(
    segments: Vec<String>,
    truncation_length: usize,
    preserve_first: bool,
    hidden_prefix: &mut bool,
) -> Vec<String> {
    if segments.len() <= truncation_length {
        return segments;
    }

    *hidden_prefix = true;

    if preserve_first && truncation_length > 1 {
        let mut out = Vec::with_capacity(truncation_length);
        out.push(segments[0].clone());
        out.extend(
            segments
                .iter()
                .skip(segments.len().saturating_sub(truncation_length - 1))
                .cloned(),
        );
        out
    } else {
        segments
            .iter()
            .skip(segments.len().saturating_sub(truncation_length))
            .cloned()
            .collect()
    }
}

fn join_path_display(prefix: &str, hidden_prefix: bool, segments: &[String]) -> String {
    if prefix.is_empty() {
        if segments.is_empty() {
            return ".".to_string();
        }
        if hidden_prefix {
            return format!(".../{}", segments.join("/"));
        }
        return segments.join("/");
    }

    if segments.is_empty() {
        return prefix.to_string();
    }

    // The filesystem root already ends with `/`; adding another separator
    // would render `//tmp` and make the left prompt look broken.
    let separator = if prefix.ends_with('/') { "" } else { "/" };

    if hidden_prefix {
        format!("{prefix}{separator}.../{}", segments.join("/"))
    } else {
        format!("{prefix}{separator}{}", segments.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::LanguageSnapshot;
    use crate::core::config::Config;
    use crate::core::layout::strip_ansi;
    use crate::core::prompt::{MetadataState, PromptContext};
    use std::sync::Arc;

    fn context(language: MetadataState<LanguageSnapshot>) -> PromptContext {
        PromptContext {
            cwd: std::env::temp_dir(),
            width: 120,
            exit_code: 0,
            last_command_duration_ms: None,
            config: Arc::new(Config::default()),
            git_state: None,
            language_state: Some(language),
            package_state: None,
            environment_state: None,
        }
    }

    fn ready(language: &str) -> MetadataState<LanguageSnapshot> {
        MetadataState::Ready {
            value: LanguageSnapshot {
                language: language.to_string(),
                version: None,
            },
            stale: false,
        }
    }

    #[test]
    fn project_directory_renders_a_pure_path_without_a_language_prefix() {
        let mut config = Config::default();
        // A noisy explicit language glyph (the shape users report as `@ path`)
        // must never be consulted by the directory segment.
        config.metadata.languages.insert(
            "rust".to_string(),
            crate::core::config::LanguageStyleConfig {
                icon: "@".to_string(),
                color: String::new(),
            },
        );
        let mut ctx = context(ready("rust"));
        ctx.config = Arc::new(config);

        let rendered = render_with_max_width(&ctx, 80);
        let plain = strip_ansi(&rendered);
        let expected = compact_path(&ctx.cwd, &ctx.config.directory);
        let language_icon = ctx.config.language_style("rust").icon;

        assert_eq!(
            plain, expected,
            "the left path must contain no prefix glyph: {plain:?}"
        );
        assert!(
            !plain.contains(&language_icon),
            "the project language glyph leaked into the path: {plain:?}"
        );
        assert!(
            !plain.contains(ctx.config.os_icon()),
            "the OS glyph leaked into the path: {plain:?}"
        );
    }

    #[test]
    fn non_project_directory_has_no_prefix_icon() {
        let ctx = context(MetadataState::Absent);
        let rendered = render_with_max_width(&ctx, 80);
        let plain = strip_ansi(&rendered);
        let expected = compact_path(&ctx.cwd, &ctx.config.directory);

        assert_eq!(
            plain, expected,
            "non-project path must be pure text: {plain:?}"
        );
        assert!(
            !plain.contains(ctx.config.os_icon()),
            "the OS glyph is forbidden on the left prompt: {plain:?}"
        );
    }

    #[test]
    fn home_directory_is_plain_even_with_a_stray_manifest() {
        let Some(home) = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
        else {
            return;
        };

        let mut ctx = context(ready("node"));
        ctx.cwd = home;

        let rendered = render_with_max_width(&ctx, 80);
        let plain = strip_ansi(&rendered);
        assert_eq!(plain, "~", "home must render as the bare path: {plain:?}");
        assert!(
            !plain.contains(ctx.config.os_icon()),
            "home must not leak the OS glyph into the left prompt: {plain:?}"
        );
        assert!(
            !plain.contains(&ctx.config.language_style("node").icon),
            "home must not borrow a stray project language glyph: {plain:?}"
        );
    }

    #[test]
    fn path_joining_never_doubles_the_root_slash() {
        assert_eq!(join_path_display("/", false, &["tmp".to_string()]), "/tmp");
        assert_eq!(
            join_path_display("/", true, &["repo".to_string(), "src".to_string()]),
            "/.../repo/src"
        );
        assert_eq!(join_path_display("~", false, &["dev".to_string()]), "~/dev");
    }
}
