//! Nix shell and Direnv environment detector.
//!
//! Uses process environment markers (`IN_NIX_SHELL`, `DIRENV_DIR`,
//! `DIRENV_FILE`) and, as a fallback, the presence of a `.envrc` in the
//! current directory chain. No `nix`/`direnv` subprocess is spawned.

use super::{home_dir, EnvironmentInfo, EnvironmentKind};
use std::path::Path;

/// Detect active Nix and/or Direnv environments.
pub fn detect(cwd: &Path) -> Vec<EnvironmentInfo> {
    let mut entries = Vec::new();

    if let Some(label) = nix_label(std::env::var("IN_NIX_SHELL").ok().as_deref()) {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Nix, label));
    }

    let has_envrc = has_envrc(cwd);
    if let Some(label) = direnv_label(
        std::env::var("DIRENV_FILE").ok().as_deref(),
        std::env::var("DIRENV_DIR").ok().as_deref(),
        has_envrc,
    ) {
        entries.push(EnvironmentInfo::new(EnvironmentKind::Direnv, label));
    }

    entries
}

fn nix_label(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value == "0" {
        return None;
    }
    Some(match value {
        "1" => "nix".to_string(),
        other => other.to_string(),
    })
}

fn direnv_label(file: Option<&str>, dir: Option<&str>, has_envrc: bool) -> Option<String> {
    if let Some(name) = file.and_then(base_name) {
        return Some(name);
    }
    if let Some(name) = dir.and_then(base_name) {
        return Some(name);
    }
    has_envrc.then(|| "envrc".to_string())
}

fn base_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches('-');
    if trimmed.is_empty() {
        return None;
    }
    Path::new(trimmed)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.is_empty())
}

/// Walk up from `cwd` looking for a `.envrc`, stopping at the home directory
/// or the filesystem root so a checkout cannot trigger an unbounded walk.
fn has_envrc(cwd: &Path) -> bool {
    let home = home_dir();
    for dir in cwd.ancestors() {
        if dir.join(".envrc").is_file() {
            return true;
        }
        if home.as_deref() == Some(dir) {
            break;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nix_label_only_for_active_shells() {
        assert_eq!(nix_label(Some("pure")).as_deref(), Some("pure"));
        assert_eq!(nix_label(Some("impure")).as_deref(), Some("impure"));
        assert_eq!(nix_label(Some("1")).as_deref(), Some("nix"));
        assert_eq!(nix_label(Some("0")), None);
        assert_eq!(nix_label(None), None);
    }

    #[test]
    fn direnv_label_prefers_active_file() {
        assert_eq!(
            direnv_label(Some("/repo/.envrc"), None, false).as_deref(),
            Some(".envrc")
        );
        // `DIRENV_DIR` is stored with a leading dash.
        assert_eq!(
            direnv_label(None, Some("-/repo/sub"), false).as_deref(),
            Some("sub")
        );
        assert_eq!(direnv_label(None, None, true).as_deref(), Some("envrc"));
        assert_eq!(direnv_label(None, None, false), None);
    }
}
