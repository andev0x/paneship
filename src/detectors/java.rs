//! Java detector: Maven `pom.xml` (`java.version`, `maven.compiler.source`)
//! and Gradle `sourceCompatibility` / `JavaLanguageVersion.of(…)`.

use super::manifest::{between, read_bounded};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

pub struct JavaDetector;

#[async_trait]
impl Detector for JavaDetector {
    fn name(&self) -> &'static str {
        "java"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["pom.xml", "build.gradle", "build.gradle.kts"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;
        let version = if marker.ends_with("pom.xml") {
            pom_version(&marker)
        } else {
            gradle_version(&marker)
        };
        let _ = dir;
        Some(RuntimeInfo {
            language: "java".to_string(),
            version,
            source: marker,
        })
    }
}

fn pom_version(path: &Path) -> Option<String> {
    let content = read_bounded(path)?;
    for tag in [
        "java.version",
        "maven.compiler.source",
        "maven.compiler.release",
    ] {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        if let Some(value) = between(&content, &open, &close) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn gradle_version(path: &Path) -> Option<String> {
    let content = read_bounded(path)?;
    for line in content.lines() {
        for anchor in [
            "sourceCompatibility",
            "targetCompatibility",
            "JavaLanguageVersion.of(",
        ] {
            if let Some(idx) = line.find(anchor) {
                if let Some(version) = first_number_run(&line[idx..]) {
                    return Some(version);
                }
            }
        }
    }
    let _ = PathBuf::new();
    None
}

/// Extract the first dotted numeric run (e.g. `17` from `VERSION_17`).
fn first_number_run(input: &str) -> Option<String> {
    let mut out = String::new();
    let mut started = false;
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            started = true;
            out.push(ch);
        } else if ch == '.' && started {
            out.push(ch);
        } else if started {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.trim_end_matches('.').to_string())
    }
}
