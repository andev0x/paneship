use crate::cache::get_or_compute_language;
use crate::core::layout::{truncate_plain_to_width, visible_width};
use crate::core::prompt::PromptContext;
use std::fs;
use std::path::Path;
use std::process::Command;
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
    let metadata_config = &context.config.metadata;
    let mut parts = Vec::new();

    if let Some((language_name, version)) = detect_language_version(context.cwd.as_path()) {
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

pub fn compute_metadata_for_daemon(path: &Path) {
    if let Some((name, marker)) = detect_language_marker(path) {
        let _ = crate::cache::get_or_compute_language(marker.as_path(), || {
            fetch_language_version(path, name.as_str(), marker.as_path())
        });
    }
    crate::modules::package::compute_package_metadata_for_daemon(path);
}

fn detect_language_version(cwd: &Path) -> Option<(String, String)> {
    #[cfg(unix)]
    {
        if std::env::var("PANESHIP_DAEMON").is_err() {
            let (language, _package) = crate::daemon::query_metadata(cwd);
            if language.is_some() {
                return language;
            }
        }
    }

    if let Some((name, marker)) = detect_language_marker(cwd) {
        return get_or_compute_language(marker.as_path(), || {
            fetch_language_version(cwd, name.as_str(), marker.as_path())
        });
    }
    None
}

fn detect_language_marker(cwd: &Path) -> Option<(String, std::path::PathBuf)> {
    for dir in cwd.ancestors() {
        if dir.join("Cargo.toml").exists() {
            return Some(("rust".to_string(), dir.join("Cargo.toml")));
        }
        if dir.join("package.json").exists() {
            return Some(("node".to_string(), dir.join("package.json")));
        }
        if dir.join("bun.lockb").exists() || dir.join("bunfig.toml").exists() {
            return Some(("bun".to_string(), dir.to_path_buf()));
        }
        if dir.join("deno.json").exists() || dir.join("deno.jsonc").exists() {
            return Some(("deno".to_string(), dir.to_path_buf()));
        }
        if dir.join("go.mod").exists() {
            return Some(("go".to_string(), dir.join("go.mod")));
        }
        if dir.join("pyproject.toml").exists() || dir.join("requirements.txt").exists() {
            return Some(("python".to_string(), dir.to_path_buf()));
        }
        if dir.join("Gemfile").exists() {
            return Some(("ruby".to_string(), dir.join("Gemfile")));
        }
        if dir.join("composer.json").exists() {
            return Some(("php".to_string(), dir.join("composer.json")));
        }
        if dir.join("pom.xml").exists() || dir.join("build.gradle").exists() {
            return Some(("java".to_string(), dir.to_path_buf()));
        }
        // Limit search depth for performance
        if dir.join(".git").exists()
            || dir.to_string_lossy() == "/"
            || dir.to_string_lossy().ends_with("/$USER")
        {
            break;
        }
    }
    None
}

fn fetch_language_version(
    cwd: &Path,
    language: &str,
    marker_path: &Path,
) -> Option<(String, String)> {
    match language {
        "rust" => {
            fetch_rust_version(cwd, marker_path).map(|version| (language.to_string(), version))
        }
        "node" => fetch_command_version(cwd, "node", &["-v"]).map(|version| {
            (
                language.to_string(),
                version.trim_start_matches('v').to_string(),
            )
        }),
        "bun" => fetch_command_version(cwd, "bun", &["--version"])
            .map(|version| (language.to_string(), version)),
        "python" => fetch_python_version(cwd).map(|version| (language.to_string(), version)),
        "go" => fetch_go_version(cwd).map(|version| (language.to_string(), version)),
        "deno" => fetch_command_version(cwd, "deno", &["--version"]).and_then(|text| {
            let first_line = text.lines().next()?.trim();
            let version = first_line.split_whitespace().nth(1)?.to_string();
            Some((language.to_string(), version))
        }),
        "ruby" => fetch_command_version(cwd, "ruby", &["--version"]).and_then(|text| {
            let version = text.split_whitespace().nth(1)?.to_string();
            Some((language.to_string(), version))
        }),
        "php" => fetch_command_version(cwd, "php", &["-v"]).and_then(|text| {
            let first_line = text.lines().next()?.trim();
            let mut parts = first_line.split_whitespace();
            let _ = parts.next();
            let version = parts.next()?.to_string();
            Some((language.to_string(), version))
        }),
        "java" => fetch_java_version(cwd).map(|version| (language.to_string(), version)),
        _ => None,
    }
}

fn fetch_rust_version(cwd: &Path, marker_path: &Path) -> Option<String> {
    if let Some(version) = rust_toolchain_version_from_path(marker_path) {
        return Some(version);
    }

    fetch_command_version(cwd, "rustc", &["--version"]).and_then(|text| {
        let mut parts = text.split_whitespace();
        let _ = parts.next();
        parts.next().map(|version| version.to_string())
    })
}

fn fetch_command_version(cwd: &Path, cmd: &str, args: &[&str]) -> Option<String> {
    let cmd = cmd.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let cwd = cwd.to_path_buf();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let output = Command::new(cmd).args(args).current_dir(cwd).output().ok();
        let _ = tx.send(output);
    });

    let output = rx
        .recv_timeout(std::time::Duration::from_millis(150))
        .ok()
        .flatten()?;

    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn fetch_python_version(cwd: &Path) -> Option<String> {
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
    parts.next().map(|version| version.to_string())
}

fn fetch_go_version(cwd: &Path) -> Option<String> {
    let output = Command::new("go")
        .arg("version")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.split_whitespace()
        .find(|token| token.starts_with("go1."))
        .map(|token| token.trim_start_matches("go").to_string())
}

fn fetch_java_version(cwd: &Path) -> Option<String> {
    let output = Command::new("java")
        .arg("-version")
        .current_dir(cwd)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stderr).to_string();
    let first_line = text.lines().next()?.trim();
    first_line
        .split('"')
        .nth(1)
        .map(|quoted| quoted.to_string())
}

fn rust_toolchain_version_from_path(path: &Path) -> Option<String> {
    if path.file_name().and_then(|name| name.to_str()) == Some("rust-toolchain.toml") {
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

    if path.file_name().and_then(|name| name.to_str()) == Some("rust-toolchain") {
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

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}
