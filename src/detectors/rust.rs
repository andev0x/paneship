//! Rust detector: reads `rust-toolchain.toml` / `rust-toolchain` then the
//! `rust-version` pin from `Cargo.toml` (workspace-aware).

use super::manifest::{first_content_line, parse_toml};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct RustDetector;

#[async_trait]
impl Detector for RustDetector {
    fn name(&self) -> &'static str {
        "rust"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["Cargo.toml"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;
        let version = toolchain_channel(&dir).or_else(|| cargo_version(&marker));
        Some(RuntimeInfo {
            language: "rust".to_string(),
            version,
            source: marker,
        })
    }
}

/// `[toolchain] channel = "…"` from `rust-toolchain.toml`, or the first line of
/// the legacy `rust-toolchain` file.
fn toolchain_channel(dir: &Path) -> Option<String> {
    let toml_path = dir.join("rust-toolchain.toml");
    if let Some(value) = parse_toml(&toml_path) {
        if let Some(channel) = value
            .get("toolchain")
            .and_then(|toolchain| toolchain.get("channel"))
            .and_then(|channel| channel.as_str())
        {
            return Some(channel.to_string());
        }
    }

    let legacy = dir.join("rust-toolchain");
    first_content_line(&legacy).filter(|line| !line.starts_with('['))
}

/// `[package] rust-version` or `[workspace.package] rust-version`.
fn cargo_version(manifest: &Path) -> Option<String> {
    let value = parse_toml(manifest)?;
    let from_package = value
        .get("package")
        .and_then(|package| package.get("rust-version"))
        .and_then(|version| version.as_str());
    let from_workspace = value
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("rust-version"))
        .and_then(|version| version.as_str());
    from_package.or(from_workspace).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn scratch(name: &str) -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paneship-rust-{name}-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_toolchain_channel() {
        let dir = scratch("toolchain");
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::write(
            dir.join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"nightly-2024-01-01\"\n",
        )
        .unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.language, "rust");
        assert_eq!(info.version.as_deref(), Some("nightly-2024-01-01"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_cargo_rust_version_pin() {
        let dir = scratch("cargo");
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname=\"x\"\nrust-version = \"1.74\"\n",
        )
        .unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.version.as_deref(), Some("1.74"));
        let _ = fs::remove_dir_all(&dir);
    }
}
