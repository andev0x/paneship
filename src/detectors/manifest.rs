//! Shared manifest-reading helpers for detectors.
//!
//! All helpers are bounded and allocation-light: files larger than
//! [`MAX_MANIFEST_BYTES`] are ignored rather than slurped into memory.

use std::path::Path;

/// Upper bound on a manifest we are willing to parse (512 KiB).
pub const MAX_MANIFEST_BYTES: u64 = 512 * 1024;

/// Read a manifest file, refusing anything implausibly large.
pub fn read_bounded(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Parse a TOML manifest.
pub fn parse_toml(path: &Path) -> Option<toml::Value> {
    let content = read_bounded(path)?;
    toml::from_str(content.as_str()).ok()
}

/// Parse a JSON manifest.
pub fn parse_json(path: &Path) -> Option<serde_json::Value> {
    let content = read_bounded(path)?;
    serde_json::from_str(content.as_str()).ok()
}

/// Normalise a version constraint into a display-friendly token.
///
/// `">=18.0.0"` → `"18.0.0"`, `"v1.2.3"` → `"1.2.3"`, `"^20"` → `"20"`.
pub fn clean_version(raw: &str) -> Option<String> {
    let token = raw
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .split(|c: char| c.is_whitespace() || c == ',' || c == '|')
        .find(|token| !token.is_empty())?
        .trim();
    let token = token.trim_start_matches(['v', 'V', '^', '~', '>', '=', '<']);
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// First non-empty, non-comment line of a small text file (`.nvmrc`, etc.).
pub fn first_content_line(path: &Path) -> Option<String> {
    let content = read_bounded(path)?;
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
}

/// Byte-scan a file for the first value after `key`, e.g. `go 1.21`.
pub fn scan_after_key(path: &Path, key: &str) -> Option<String> {
    let content = read_bounded(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest = rest.trim();
            if rest.is_empty() {
                continue;
            }
            let token = rest
                .split(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .next()
                .unwrap_or("")
                .trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Extract the first substring enclosed in the given delimiters.
pub fn between(content: &str, open: &str, close: &str) -> Option<String> {
    let start = content.find(open)? + open.len();
    let rest = &content[start..];
    let end = rest.find(close)?;
    Some(rest[..end].trim().to_string())
}
