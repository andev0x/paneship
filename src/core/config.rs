use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub git: GitConfig,
    pub status: StatusConfig,
    pub metadata: MetadataConfig,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DirectoryConfig {
    pub icon: String,
    pub truncation_length: usize,
    pub truncate_to_repo: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    pub branch_icon: String,
    pub staged_icon: String,
    pub unstaged_icon: String,
    pub untracked_icon: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusConfig {
    pub success_icon: String,
    pub failure_icon: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataConfig {
    pub time_color: String,
    pub paneship_color: String,
    pub languages: HashMap<String, LanguageStyleConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct LanguageStyleConfig {
    pub icon: String,
    pub color: String,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            icon: "\u{f07b}".to_string(),
            truncation_length: 3,
            truncate_to_repo: true,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            branch_icon: "\u{e0a0}".to_string(),
            staged_icon: "+".to_string(),
            unstaged_icon: "!".to_string(),
            untracked_icon: "?".to_string(),
        }
    }
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            success_icon: "\u{279c}".to_string(),
            failure_icon: "\u{279c}".to_string(),
        }
    }
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            time_color: "2;37".to_string(),
            paneship_color: "1;32".to_string(),
            languages: default_language_styles(),
        }
    }
}

impl Default for LanguageStyleConfig {
    fn default() -> Self {
        Self {
            icon: "?".to_string(),
            color: "1;37".to_string(),
        }
    }
}

impl MetadataConfig {
    pub fn language_style(&self, name: &str) -> LanguageStyleConfig {
        self.languages.get(name).cloned().unwrap_or_else(|| {
            let mut defaults = default_language_styles();
            defaults
                .remove(name)
                .unwrap_or_default()
        })
    }
}

fn default_language_styles() -> HashMap<String, LanguageStyleConfig> {
    let mut styles = HashMap::new();

    styles.insert(
        "rust".to_string(),
        LanguageStyleConfig {
            icon: "🦀".to_string(),
            color: "1;33".to_string(),
        },
    );
    styles.insert(
        "node".to_string(),
        LanguageStyleConfig {
            icon: "⬢".to_string(),
            color: "1;32".to_string(),
        },
    );
    styles.insert(
        "bun".to_string(),
        LanguageStyleConfig {
            icon: "🥟".to_string(),
            color: "1;38;5;208".to_string(),
        },
    );
    styles.insert(
        "python".to_string(),
        LanguageStyleConfig {
            icon: "🐍".to_string(),
            color: "1;34".to_string(),
        },
    );
    styles.insert(
        "go".to_string(),
        LanguageStyleConfig {
            icon: "🐹".to_string(),
            color: "1;36".to_string(),
        },
    );
    styles.insert(
        "deno".to_string(),
        LanguageStyleConfig {
            icon: "🦕".to_string(),
            color: "1;32".to_string(),
        },
    );
    styles.insert(
        "ruby".to_string(),
        LanguageStyleConfig {
            icon: "💎".to_string(),
            color: "1;31".to_string(),
        },
    );
    styles.insert(
        "php".to_string(),
        LanguageStyleConfig {
            icon: "🐘".to_string(),
            color: "1;35".to_string(),
        },
    );
    styles.insert(
        "java".to_string(),
        LanguageStyleConfig {
            icon: "☕".to_string(),
            color: "1;31".to_string(),
        },
    );

    styles
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::get_config_path();
        if let Ok(content) = fs::read_to_string(config_path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn get_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config/paneship/config.toml")
    }
}
