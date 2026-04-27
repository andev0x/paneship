use crate::core::prompt::PromptContext;
use unicode_width::UnicodeWidthStr;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let config = &context.config.directory;
    let icon_with_space = format!("{} ", config.icon);
    let icon_width = UnicodeWidthStr::width(icon_with_space.as_str());

    let path_budget = max_visible_width.saturating_sub(icon_width).max(1);
    let raw_path = compact_path(&context.cwd);
    let trimmed_path = if UnicodeWidthStr::width(raw_path.as_str()) > path_budget {
        smart_truncate_path(raw_path.as_str(), path_budget)
    } else {
        raw_path
    };

    let mut plain = format!("{} {}", config.icon, trimmed_path);
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

fn compact_path(path: &std::path::Path) -> String {
    let home = std::env::var("HOME").map(std::path::PathBuf::from).ok();

    let mut display_path = if let Some(home_path) = home {
        if let Ok(rel) = path.strip_prefix(&home_path) {
            if rel.as_os_str().is_empty() {
                "~".to_string()
            } else {
                format!("~/{}", rel.display())
            }
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    };

    if display_path.len() > 1 && display_path.ends_with('/') {
        display_path.pop();
    }

    display_path
}
