use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub git: GitConfig,
    pub status: StatusConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DirectoryConfig {
    pub icon: String,
    pub truncation_length: usize,
    pub truncate_to_repo: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitConfig {
    pub branch_icon: String,
    pub staged_icon: String,
    pub unstaged_icon: String,
    pub untracked_icon: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusConfig {
    pub success_icon: String,
    pub failure_icon: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            directory: DirectoryConfig {
                icon: "\u{f07b}".to_string(),
                truncation_length: 3,
                truncate_to_repo: true,
            },
            git: GitConfig {
                branch_icon: "\u{e0a0}".to_string(),
                staged_icon: "+".to_string(),
                unstaged_icon: "!".to_string(),
                untracked_icon: "?".to_string(),
            },
            status: StatusConfig {
                success_icon: "\u{279c}".to_string(),
                failure_icon: "\u{279c}".to_string(),
            },
        }
    }
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
