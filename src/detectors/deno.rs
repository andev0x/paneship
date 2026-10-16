//! Deno detector: recognises `deno.json` / `deno.jsonc`.
//!
//! Deno projects do not pin the runtime in their manifest, so the version is
//! reported as unknown and only the language segment is rendered.

use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct DenoDetector;

#[async_trait]
impl Detector for DenoDetector {
    fn name(&self) -> &'static str {
        "deno"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["deno.json", "deno.jsonc"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (_dir, marker) = find_marker_for(self, cwd)?;
        Some(RuntimeInfo {
            language: "deno".to_string(),
            version: None,
            source: marker,
        })
    }
}
