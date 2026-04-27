use crate::core::prompt::PromptContext;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let config = &context.config.directory;
    let icon_width = if config.icon.trim().is_empty() {
        0
    } else {
        UnicodeWidthStr::width(config.icon.as_str()) + 1
    };

    let path_budget = max_visible_width.saturating_sub(icon_width).max(1);
    let raw_path = compact_path(&context.cwd, config);
    let trimmed_path = if UnicodeWidthStr::width(raw_path.as_str()) > path_budget {
        smart_truncate_path(raw_path.as_str(), path_budget)
    } else {
        raw_path
    };

    let mut plain = if config.icon.trim().is_empty() {
        trimmed_path
    } else {
        format!("{} {}", config.icon, trimmed_path)
    };
    if UnicodeWidthStr::width(plain.as_str()) > max_visible_width {
        plain = crate::core::layout::truncate_plain_to_width(plain.as_str(), max_visible_width);
    }

    format!("\x1b[1;34m{}\x1b[0m", plain)
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

    if UnicodeWidthStr::width(candidate.as_str()) <= budget {
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
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
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

    if hidden_prefix {
        format!("{prefix}/.../{}", segments.join("/"))
    } else {
        format!("{prefix}/{}", segments.join("/"))
    }
}
