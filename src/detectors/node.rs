//! Node.js detector: `package.json` (`volta.node`, `engines.node`), with
//! `.nvmrc` / `.node-version` taking precedence.

use super::manifest::{clean_version, first_content_line, parse_json};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct NodeDetector;

#[async_trait]
impl Detector for NodeDetector {
    fn name(&self) -> &'static str {
        "node"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["package.json"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;

        let version = first_content_line(&dir.join(".nvmrc"))
            .or_else(|| first_content_line(&dir.join(".node-version")))
            .and_then(|line| clean_version(&line))
            .or_else(|| package_json_version(&marker));

        Some(RuntimeInfo {
            language: "node".to_string(),
            version,
            source: marker,
        })
    }
}

fn package_json_version(manifest: &Path) -> Option<String> {
    let value = parse_json(manifest)?;
    let volta = value
        .get("volta")
        .and_then(|volta| volta.get("node"))
        .and_then(|node| node.as_str());
    let engines = value
        .get("engines")
        .and_then(|engines| engines.get("node"))
        .and_then(|node| node.as_str());
    volta.or(engines).and_then(clean_version)
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn scratch(name: &str) -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paneship-node-{name}-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_engines_constraint() {
        let dir = scratch("engines");
        fs::write(
            dir.join("package.json"),
            r#"{"engines":{"node":">=18.17.0"}}"#,
        )
        .unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.language, "node");
        assert_eq!(info.version.as_deref(), Some("18.17.0"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nvmrc_beats_package_json() {
        let dir = scratch("nvmrc");
        fs::write(dir.join("package.json"), r#"{"engines":{"node":">=18"}}"#).unwrap();
        fs::write(dir.join(".nvmrc"), "v20.11.1\n").unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.version.as_deref(), Some("20.11.1"));
        let _ = fs::remove_dir_all(&dir);
    }
}
