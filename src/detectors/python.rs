//! Python detector: `.python-version`, `pyproject.toml`
//! (`project.requires-python` / Poetry), `Pipfile` and `requirements.txt`.

use super::manifest::{clean_version, first_content_line, parse_toml};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct PythonDetector;

#[async_trait]
impl Detector for PythonDetector {
    fn name(&self) -> &'static str {
        "python"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &[
            "pyproject.toml",
            ".python-version",
            "Pipfile",
            "requirements.txt",
        ]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;

        let version = first_content_line(&dir.join(".python-version"))
            .and_then(|line| clean_version(&line))
            .or_else(|| pyproject_version(&dir.join("pyproject.toml")));

        Some(RuntimeInfo {
            language: "python".to_string(),
            version,
            source: marker,
        })
    }
}

fn pyproject_version(path: &Path) -> Option<String> {
    let value = parse_toml(path)?;
    let requires = value
        .get("project")
        .and_then(|project| project.get("requires-python"))
        .and_then(|value| value.as_str());
    let poetry = value
        .get("tool")
        .and_then(|tool| tool.get("poetry"))
        .and_then(|poetry| poetry.get("dependencies"))
        .and_then(|deps| deps.get("python"))
        .and_then(|value| value.as_str());
    requires.or(poetry).and_then(clean_version)
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn scratch(name: &str) -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("paneship-py-{name}-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_pyproject_requires_python() {
        let dir = scratch("pyproject");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nrequires-python = \">=3.11\"\n",
        )
        .unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.language, "python");
        assert_eq!(info.version.as_deref(), Some("3.11"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn python_version_file_wins() {
        let dir = scratch("version-file");
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nrequires-python=\"3.10\"\n",
        )
        .unwrap();
        fs::write(dir.join(".python-version"), "3.12.2\n").unwrap();
        let info = super::super::detect_runtime(&dir).unwrap();
        assert_eq!(info.version.as_deref(), Some("3.12.2"));
        let _ = fs::remove_dir_all(&dir);
    }
}
