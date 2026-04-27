use crate::cache::get_or_compute_language;
use crate::core::layout::{truncate_plain_to_width, visible_width};
use crate::core::prompt::PromptContext;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let mut rendered = Vec::new();
    let separator = "   ";

    for candidate in metadata_parts(context) {
        if candidate.is_empty() {
            continue;
        }

        let assembled = if rendered.is_empty() {
            candidate.clone()
        } else {
            format!("{}{}{}", rendered.join(separator), separator, candidate)
        };

        if visible_width(assembled.as_str()) <= max_visible_width {
            rendered.push(candidate);
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
    let metadata_config = &context.config.metadata;
    let mut parts = Vec::new();

    if let Some((language_name, version)) = get_or_compute_language(context.cwd.as_path(), || {
        detect_language_version(context.cwd.as_path())
    }) {
        let language_style = metadata_config.language_style(language_name.as_str());
        let language_value = format!("{} {version}", language_style.icon);
        parts.push(styled(&language_style.color, &language_value));
    }

    if let Some(duration) = render_command_duration(context) {
        parts.push(duration);
    }

    if let Some(time) = current_time_hhmm() {
        parts.push(styled(&metadata_config.time_color, &format!("󰥔 {time}")));
    }

    parts
}

fn render_command_duration(context: &PromptContext) -> Option<String> {
    let ms = context.last_command_duration_ms?;
    let plain = if ms < 1_000 {
        format!("󰞌 {ms}ms")
    } else {
        let secs = ms / 1_000;
        if secs < 60 {
            format!("󰞌 {secs}s")
        } else {
            let mins = secs / 60;
            let rem_secs = secs % 60;
            format!("󰞌 {mins}m{rem_secs:02}s")
        }
    };

    Some(styled("2;37", plain.as_str()))
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

fn detect_language_version(cwd: &Path) -> Option<(String, String)> {
    detect_rust(cwd)
        .or_else(|| detect_node(cwd))
        .or_else(|| detect_bun(cwd))
        .or_else(|| detect_python(cwd))
        .or_else(|| detect_go(cwd))
        .or_else(|| detect_deno(cwd))
        .or_else(|| detect_ruby(cwd))
        .or_else(|| detect_php(cwd))
        .or_else(|| detect_java(cwd))
}

fn detect_rust(cwd: &Path) -> Option<(String, String)> {
    if let Some(version) = rust_toolchain_version(cwd) {
        return Some(("rust".to_string(), version));
    }

    find_upwards(cwd, "Cargo.toml")?;

    let output = Command::new("rustc")
        .arg("--version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let _ = parts.next();
    let version = parts.next()?.to_string();
    Some(("rust".to_string(), version))
}

fn detect_node(cwd: &Path) -> Option<(String, String)> {
    find_upwards(cwd, "package.json")?;

    let output = Command::new("node")
        .arg("-v")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return None;
    }

    Some((
        "node".to_string(),
        version.trim_start_matches('v').to_string(),
    ))
}

fn detect_bun(cwd: &Path) -> Option<(String, String)> {
    if find_upwards(cwd, "bun.lockb").is_none() && find_upwards(cwd, "bun.lock").is_none() {
        return None;
    }

    let output = Command::new("bun")
        .arg("--version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        return None;
    }

    Some(("bun".to_string(), version))
}

fn detect_python(cwd: &Path) -> Option<(String, String)> {
    if find_upwards(cwd, "pyproject.toml").is_none()
        && find_upwards(cwd, "requirements.txt").is_none()
        && find_upwards(cwd, "setup.py").is_none()
    {
        return None;
    }

    let output = Command::new("python3")
        .arg("--version")
        .current_dir(cwd)
        .output()
        .ok()?;

    let text = if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        String::from_utf8_lossy(&output.stderr).trim().to_string()
    };

    if text.is_empty() {
        return None;
    }

    let mut parts = text.split_whitespace();
    let _ = parts.next();
    let version = parts.next()?.to_string();
    Some(("python".to_string(), version))
}

fn detect_go(cwd: &Path) -> Option<(String, String)> {
    find_upwards(cwd, "go.mod")?;

    let output = Command::new("go")
        .arg("version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let version = text
        .split_whitespace()
        .find(|token| token.starts_with("go1."))?
        .to_string();
    Some((
        "go".to_string(),
        version.trim_start_matches("go").to_string(),
    ))
}

fn detect_deno(cwd: &Path) -> Option<(String, String)> {
    if find_upwards(cwd, "deno.json").is_none() && find_upwards(cwd, "deno.jsonc").is_none() {
        return None;
    }

    let output = Command::new("deno")
        .arg("--version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next()?.trim();
    let version = first_line.split_whitespace().nth(1)?.to_string();
    Some(("deno".to_string(), version))
}

fn detect_ruby(cwd: &Path) -> Option<(String, String)> {
    if find_upwards(cwd, "Gemfile").is_none() && find_upwards(cwd, ".ruby-version").is_none() {
        return None;
    }

    let output = Command::new("ruby")
        .arg("--version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.split_whitespace().nth(1)?.to_string();
    Some(("ruby".to_string(), version))
}

fn detect_php(cwd: &Path) -> Option<(String, String)> {
    find_upwards(cwd, "composer.json")?;

    let output = Command::new("php")
        .arg("-v")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next()?.trim();
    let mut parts = first_line.split_whitespace();
    let _ = parts.next();
    let version = parts.next()?.to_string();
    Some(("php".to_string(), version))
}

fn detect_java(cwd: &Path) -> Option<(String, String)> {
    if find_upwards(cwd, "pom.xml").is_none()
        && find_upwards(cwd, "build.gradle").is_none()
        && find_upwards(cwd, "build.gradle.kts").is_none()
    {
        return None;
    }

    let output = Command::new("java")
        .arg("-version")
        .current_dir(cwd)
        .output()
        .ok()?;

    let text = if output.status.success() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        return None;
    };

    let first_line = text.lines().next()?.trim();
    let quoted = first_line.split('"').nth(1)?.to_string();
    Some(("java".to_string(), quoted))
}

fn rust_toolchain_version(cwd: &Path) -> Option<String> {
    if let Some(path) = find_upwards(cwd, "rust-toolchain.toml") {
        let content = fs::read_to_string(path).ok()?;
        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(value) = trimmed.strip_prefix("channel") {
                let value = value.trim();
                if let Some(raw) = value.strip_prefix('=') {
                    let raw = raw.trim();
                    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
                        return Some(raw[1..raw.len() - 1].to_string());
                    }
                }
            }
        }
    }

    if let Some(path) = find_upwards(cwd, "rust-toolchain") {
        let content = fs::read_to_string(path).ok()?;
        for line in content.lines() {
            let value = line.trim();
            if value.is_empty() || value.starts_with('#') {
                continue;
            }
            if value.starts_with('[') {
                break;
            }
            return Some(value.to_string());
        }
    }

    None
}

fn find_upwards(start: &Path, file_name: &str) -> Option<std::path::PathBuf> {
    for dir in start.ancestors() {
        let candidate = dir.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}
