use crate::cache::get_or_compute_package_version;
use crate::core::prompt::PromptContext;
use std::fs;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let manifest_path = match find_cargo_manifest_path(context.cwd.as_path()) {
        Some(path) => path,
        None => return String::new(),
    };

    let Some(version) = get_or_compute_package_version(manifest_path.as_path(), || {
        find_cargo_manifest_version_from_path(manifest_path.as_path())
    }) else {
        return String::new();
    };

    let plain = format!("📦 v{version}");
    if crate::core::layout::visible_width(plain.as_str()) > max_visible_width {
        return String::new();
    }

    styled(&context.config.metadata.paneship_color, plain.as_str())
}

fn find_cargo_manifest_path(start: &std::path::Path) -> Option<std::path::PathBuf> {
    for dir in start.ancestors() {
        let path = dir.join("Cargo.toml");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn find_cargo_manifest_version_from_path(path: &std::path::Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let parsed: toml::Value = toml::from_str(content.as_str()).ok()?;
    let version = parsed
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())?
        .trim();

    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}
