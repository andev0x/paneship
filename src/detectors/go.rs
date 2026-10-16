//! Go detector: reads the `go` directive (and optional `toolchain` line) from
//! `go.mod` with a light byte scan.

use super::manifest::scan_after_key;
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct GoDetector;

#[async_trait]
impl Detector for GoDetector {
    fn name(&self) -> &'static str {
        "go"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["go.mod"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (_dir, marker) = find_marker_for(self, cwd)?;
        // Prefer an explicit toolchain pin, then the language version directive.
        let version = scan_after_key(&marker, "toolchain")
            .map(|raw| raw.trim_start_matches("go").to_string())
            .or_else(|| scan_after_key(&marker, "go "));
        Some(RuntimeInfo {
            language: "go".to_string(),
            version,
            source: marker,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn reads_go_directive() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paneship-go-{id}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("go.mod"), "module example\n\ngo 1.21\n").unwrap();

        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.language, "go");
        assert_eq!(info.version.as_deref(), Some("1.21"));
        let _ = fs::remove_dir_all(&dir);
    }
}
