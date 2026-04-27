use crate::core::prompt::PromptContext;
use std::fs;

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    let Some(version) = find_cargo_manifest_version(context.cwd.as_path()) else {
        return String::new();
    };

    let plain = format!("📦 v{version}");
    if crate::core::layout::visible_width(plain.as_str()) > max_visible_width {
        return String::new();
    }

    styled(&context.config.metadata.paneship_color, plain.as_str())
}

fn find_cargo_manifest_version(start: &std::path::Path) -> Option<String> {
    for dir in start.ancestors() {
        let path = dir.join("Cargo.toml");
        if !path.is_file() {
            continue;
        }

        let content = fs::read_to_string(path).ok()?;
        let parsed: toml::Value = toml::from_str(content.as_str()).ok()?;
        let version = parsed
            .get("package")
            .and_then(|package| package.get("version"))
            .and_then(|version| version.as_str())?
            .trim();

        if !version.is_empty() {
            return Some(version.to_string());
        }
    }

    None
}

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}
