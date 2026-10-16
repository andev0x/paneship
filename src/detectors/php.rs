//! PHP detector: the `require.php` / `require-dev.php` constraint from
//! `composer.json`.

use super::manifest::{clean_version, parse_json};
use super::{find_marker_for, Detector, RuntimeInfo};
use async_trait::async_trait;
use std::path::Path;

pub struct PhpDetector;

#[async_trait]
impl Detector for PhpDetector {
    fn name(&self) -> &'static str {
        "php"
    }

    fn marker_files(&self) -> &'static [&'static str] {
        &["composer.json"]
    }

    async fn detect(&self, cwd: &Path) -> Option<RuntimeInfo> {
        let (_dir, marker) = find_marker_for(self, cwd)?;
        Some(RuntimeInfo {
            language: "php".to_string(),
            version: composer_php(&marker),
            source: marker,
        })
    }
}

fn composer_php(manifest: &Path) -> Option<String> {
    let value = parse_json(manifest)?;
    let from = |section: &str| {
        value
            .get(section)
            .and_then(|require| require.get("php"))
            .and_then(|php| php.as_str())
            .and_then(clean_version)
    };
    from("require").or_else(|| from("require-dev"))
}
