use crate::cache::get_or_compute_package_version;
use crate::core::prompt::PromptContext;
use std::fs;
use std::path::{Path, PathBuf};

pub fn render_with_max_width(context: &PromptContext, max_visible_width: usize) -> String {
    if max_visible_width == 0 {
        return String::new();
    }

    #[cfg(unix)]
    {
        if std::env::var("PANESHIP_DAEMON").is_err() {
            let (_language, package) = crate::daemon::query_metadata(context.cwd.as_path());

            if let Some(version) = package {
                let plain = format!("📦 v{version}");

                if crate::core::layout::visible_width(plain.as_str()) <= max_visible_width {
                    return styled(&context.config.metadata.paneship_color, plain.as_str());
                }
            }
        }
    }

    let manifest_path = match find_cargo_manifest_path(context.cwd.as_path()) {
        Some(path) => path,
        None => return String::new(),
    };

    let Some(version) = get_or_compute_package_version(manifest_path.as_path(), || {
        resolve_package_version(manifest_path.as_path())
    }) else {
        return String::new();
    };

    let plain = format!("📦 v{version}");

    if crate::core::layout::visible_width(plain.as_str()) > max_visible_width {
        return String::new();
    }

    styled(&context.config.metadata.paneship_color, plain.as_str())
}

pub fn compute_package_metadata_for_daemon(path: &Path) {
    if let Some(manifest_path) = find_cargo_manifest_path(path) {
        let _ = get_or_compute_package_version(manifest_path.as_path(), || {
            resolve_package_version(manifest_path.as_path())
        });
    }
}

fn find_cargo_manifest_path(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let manifest = dir.join("Cargo.toml");

        if manifest.is_file() {
            return Some(manifest);
        }
    }

    None
}

fn resolve_package_version(manifest_path: &Path) -> Option<String> {
    let parsed = parse_manifest(manifest_path)?;

    // ------------------------------------------------------------
    // Case 1:
    // [package]
    // version = "0.1.0"
    // ------------------------------------------------------------
    if let Some(version) = parsed
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(version.to_string());
    }

    // ------------------------------------------------------------
    // Case 2:
    // [package]
    // version.workspace = true
    // ------------------------------------------------------------
    let uses_workspace_version = parsed
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.get("workspace"))
        .and_then(|workspace| workspace.as_bool())
        .unwrap_or(false);

    if uses_workspace_version {
        let workspace_manifest = find_workspace_manifest(manifest_path)?;

        let workspace_parsed = parse_manifest(workspace_manifest.as_path())?;

        return workspace_parsed
            .get("workspace")
            .and_then(|workspace| workspace.get("package"))
            .and_then(|package| package.get("version"))
            .and_then(|version| version.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
    }

    // ------------------------------------------------------------
    // Case 3:
    // Workspace root only
    //
    // [workspace.package]
    // version = "0.1.0"
    // ------------------------------------------------------------
    parsed
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("version"))
        .and_then(|version| version.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn find_workspace_manifest(start_manifest: &Path) -> Option<PathBuf> {
    let start_dir = start_manifest.parent()?;

    for dir in start_dir.ancestors() {
        let manifest = dir.join("Cargo.toml");

        if !manifest.is_file() {
            continue;
        }

        let parsed = parse_manifest(manifest.as_path())?;

        let is_workspace_root = parsed.get("workspace").is_some();

        if is_workspace_root {
            return Some(manifest);
        }
    }

    None
}

fn parse_manifest(path: &Path) -> Option<toml::Value> {
    let content = fs::read_to_string(path).ok()?;

    toml::from_str(content.as_str()).ok()
}

fn styled(color_code: &str, value: &str) -> String {
    format!("\x1b[{color_code}m{value}\x1b[0m")
}
