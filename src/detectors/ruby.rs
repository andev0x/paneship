//! Ruby detector: `.ruby-version` then the `ruby "…"` line in `Gemfile`.

use super::manifest::{clean_version, first_content_line, read_bounded};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct RubyDetector;

#[async_trait]
impl Detector for RubyDetector {
    fn name(&self) -> &'static str {
        "ruby"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["Gemfile", ".ruby-version"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (dir, marker) = find_marker_for(self, cwd)?;

        let version = first_content_line(&dir.join(".ruby-version"))
            .and_then(|line| clean_version(&line))
            .or_else(|| gemfile_version(&dir.join("Gemfile")));

        Some(RuntimeInfo {
            language: "ruby".to_string(),
            version,
            source: marker,
        })
    }
}

fn gemfile_version(path: &Path) -> Option<String> {
    let content = read_bounded(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("ruby") else {
            continue;
        };
        let rest = rest.trim();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let inner = rest[1..].split(quote).next().unwrap_or("");
        if let Some(version) = clean_version(inner) {
            return Some(version);
        }
    }
    None
}
