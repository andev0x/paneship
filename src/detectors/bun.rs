//! Bun detector: recognises `bun.lockb` / `bunfig.toml` and reads the pinned
//! runtime from `package.json`'s `packageManager` field (`bun@1.2.3`).

use super::manifest::{clean_version, parse_json};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct BunDetector;

#[async_trait]
impl Detector for BunDetector {
    fn name(&self) -> &'static str {
        "bun"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["bun.lockb", "bunfig.toml"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;
        let version = package_manager_bun(&dir.join("package.json"));
        Some(RuntimeInfo {
            language: "bun".to_string(),
            version,
            source: marker,
        })
    }
}

fn package_manager_bun(manifest: &Path) -> Option<String> {
    let value = parse_json(manifest)?;
    let manager = value
        .get("packageManager")
        .and_then(|manager| manager.as_str())?;
    let (tool, version) = manager.split_once('@')?;
    if tool == "bun" {
        clean_version(version)
    } else {
        None
    }
}
