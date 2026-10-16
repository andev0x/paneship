use crate::core::icons::{IconProfile, ThemeProfile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub directory: DirectoryConfig,
    pub git: GitConfig,
    pub status: StatusConfig,
    pub metadata: MetadataConfig,
    pub icons: IconsConfig,
    pub theme: ThemeConfig,
    pub prompt: PromptConfig,
    pub environment: EnvironmentConfig,
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

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StatusConfig {
    pub success_icon: String,
    pub failure_icon: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataConfig {
    pub time_color: String,
    pub paneship_color: String,
    pub languages: HashMap<String, LanguageStyleConfig>,
}

/// Per-language override. Empty fields fall back to the active icon/theme
/// profile (see [`crate::core::icons`]).
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct LanguageStyleConfig {
    pub icon: String,
    pub color: String,
}

/// Icon-set profile selection (Phase 2, Module D).
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct IconsConfig {
    pub profile: String,
}

/// Colour theme profile selection (Phase 2, Module D).
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: String,
}

/// Right-side prompt (`RPROMPT`) layout (Phase 3, Module A).
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptConfig {
    /// Render the right-aligned metadata region at all.
    pub rprompt: bool,
    /// Minimum terminal width (columns) required to render the RPROMPT. Below
    /// this the right region is dropped entirely rather than risking a wrap.
    pub rprompt_min_width: usize,
}

/// Infrastructure / environment detector selection (Phase 3, Module B).
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EnvironmentConfig {
    /// Master switch for the environment segment.
    pub enabled: bool,
    /// Kubernetes context / namespace detection (`~/.kube/config`).
    pub kubernetes: bool,
    /// Container detection (`.dockerenv`, cgroup, `container` env).
    pub docker: bool,
    /// Active AWS/GCP/Azure profile detection.
    pub cloud: bool,
    /// Nix shell (`IN_NIX_SHELL`) and Direnv (`.envrc`) detection.
    pub nix_direnv: bool,
    /// Opt-in system health checks (load average / battery), off by default.
    pub health: bool,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            // Empty icons resolve dynamically from the active icon profile.
            icon: String::new(),
            truncation_length: 3,
            truncate_to_repo: true,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            branch_icon: String::new(),
            staged_icon: "+".to_string(),
            unstaged_icon: "!".to_string(),
            untracked_icon: "?".to_string(),
        }
    }
}

impl Default for IconsConfig {
    fn default() -> Self {
        Self {
            profile: IconProfile::default().as_str().to_string(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: ThemeProfile::default().as_str().to_string(),
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            rprompt: true,
            rprompt_min_width: 20,
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            kubernetes: true,
            docker: true,
            cloud: true,
            nix_direnv: true,
            health: false,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if let Ok(content) = fs::read_to_string(config_path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Shared, cached configuration. Cheap to clone (`Arc`).
    pub fn load_cached() -> Arc<Self> {
        let cache = config_cache();
        read_lock(cache).clone()
    }

    /// Re-read the config file and atomically replace the cached value.
    ///
    /// Used by the daemon's `RELOAD_CONFIG` handler so edits made in the TUI
    /// take effect across all active tmux panes without a restart.
    pub fn reload() -> Arc<Self> {
        let fresh = Arc::new(Self::load());
        let cache = config_cache();
        *write_lock(cache) = Arc::clone(&fresh);
        fresh
    }

    /// Absolute path of `~/.config/paneship/config.toml`.
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("paneship")
            .join("config.toml")
    }

    // -----------------------------------------------------------------------
    // Module D — dynamic icon / theme resolution
    // -----------------------------------------------------------------------

    /// Active icon profile (falls back to Nerd Font on an unknown key).
    pub fn icon_profile(&self) -> IconProfile {
        IconProfile::parse(&self.icons.profile).unwrap_or_default()
    }

    /// Active colour theme (falls back to Default on an unknown key).
    pub fn theme_profile(&self) -> ThemeProfile {
        ThemeProfile::parse(&self.theme.name).unwrap_or_default()
    }

    /// Resolved directory icon: explicit value wins, else the active profile.
    pub fn directory_icon(&self) -> &str {
        non_empty(&self.directory.icon).unwrap_or_else(|| self.icon_profile().tokens().directory)
    }

    /// Resolved Git branch icon.
    pub fn git_branch_icon(&self) -> &str {
        non_empty(&self.git.branch_icon).unwrap_or_else(|| self.icon_profile().tokens().git_branch)
    }

    /// Resolved staged-change token.
    pub fn git_staged_icon(&self) -> &str {
        non_empty(&self.git.staged_icon).unwrap_or_else(|| self.icon_profile().tokens().git_staged)
    }

    /// Resolved unstaged-change token.
    pub fn git_unstaged_icon(&self) -> &str {
        non_empty(&self.git.unstaged_icon)
            .unwrap_or_else(|| self.icon_profile().tokens().git_unstaged)
    }

    /// Resolved untracked-file token.
    pub fn git_untracked_icon(&self) -> &str {
        non_empty(&self.git.untracked_icon)
            .unwrap_or_else(|| self.icon_profile().tokens().git_untracked)
    }

    /// Resolved success cursor.
    pub fn success_icon(&self) -> &str {
        non_empty(&self.status.success_icon).unwrap_or_else(|| self.icon_profile().tokens().success)
    }

    /// Resolved failure cursor.
    pub fn failure_icon(&self) -> &str {
        non_empty(&self.status.failure_icon).unwrap_or_else(|| self.icon_profile().tokens().failure)
    }

    /// Resolved package icon.
    pub fn package_icon(&self) -> &str {
        self.icon_profile().tokens().package
    }

    /// Resolved infrastructure/environment icon for a detector key.
    pub fn environment_icon(&self, key: &str) -> Option<&str> {
        self.icon_profile().tokens().environment(key)
    }

    /// Resolved infrastructure/environment colour.
    pub fn environment_color(&self) -> &str {
        self.theme_profile().colors().environment
    }

    /// Resolved time icon.
    pub fn time_icon(&self) -> &str {
        self.icon_profile().tokens().time
    }

    /// Resolved command-duration icon.
    pub fn duration_icon(&self) -> &str {
        self.icon_profile().tokens().duration
    }

    /// Placeholder shown while Git state is being computed.
    pub fn git_placeholder(&self) -> &str {
        self.icon_profile().tokens().placeholder_git
    }

    /// Placeholder shown while language metadata is being computed.
    pub fn language_placeholder(&self) -> &str {
        self.icon_profile().tokens().placeholder_lang
    }

    /// Resolved directory colour.
    pub fn directory_color(&self) -> &str {
        self.theme_profile().colors().directory
    }

    /// Resolved Git branch colour.
    pub fn git_branch_color(&self) -> &str {
        self.theme_profile().colors().git_branch
    }

    /// Resolved staged-change colour.
    pub fn git_staged_color(&self) -> &str {
        self.theme_profile().colors().staged
    }

    /// Resolved unstaged-change colour.
    pub fn git_unstaged_color(&self) -> &str {
        self.theme_profile().colors().unstaged
    }

    /// Resolved untracked-file colour.
    pub fn git_untracked_color(&self) -> &str {
        self.theme_profile().colors().untracked
    }

    /// Resolved success colour.
    pub fn success_color(&self) -> &str {
        self.theme_profile().colors().success
    }

    /// Resolved failure colour.
    pub fn failure_color(&self) -> &str {
        self.theme_profile().colors().failure
    }

    /// Resolved time colour (explicit config wins).
    pub fn time_color(&self) -> &str {
        non_empty(&self.metadata.time_color).unwrap_or_else(|| self.theme_profile().colors().time)
    }

    /// Resolved paneship/package colour (explicit config wins).
    pub fn paneship_color(&self) -> &str {
        non_empty(&self.metadata.paneship_color)
            .unwrap_or_else(|| self.theme_profile().colors().paneship)
    }

    /// Fully resolved style for a language: explicit override wins, else the
    /// active profile supplies the icon and the active theme the colour.
    pub fn language_style(&self, name: &str) -> LanguageStyleConfig {
        let explicit = self.metadata.languages.get(name);
        let icon = explicit
            .and_then(|style| non_empty(&style.icon))
            .map(str::to_string)
            .or_else(|| {
                self.icon_profile()
                    .tokens()
                    .language(name)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "?".to_string());
        let color = explicit
            .and_then(|style| non_empty(&style.color))
            .map(str::to_string)
            .or_else(|| {
                self.theme_profile()
                    .colors()
                    .language(name)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "1;37".to_string());
        LanguageStyleConfig { icon, color }
    }
}

fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn config_cache() -> &'static RwLock<Arc<Config>> {
    static CONFIG_CACHE: OnceLock<RwLock<Arc<Config>>> = OnceLock::new();
    CONFIG_CACHE.get_or_init(|| RwLock::new(Arc::new(Config::load())))
}

fn read_lock<'a>(lock: &'a RwLock<Arc<Config>>) -> std::sync::RwLockReadGuard<'a, Arc<Config>> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_lock<'a>(lock: &'a RwLock<Arc<Config>>) -> std::sync::RwLockWriteGuard<'a, Arc<Config>> {
    lock.write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_profile_overrides_defaults() {
        let mut config = Config::default();
        assert_eq!(
            config.directory_icon(),
            IconProfile::NerdFont.tokens().directory
        );

        config.icons.profile = "emoji".to_string();
        assert_eq!(
            config.directory_icon(),
            IconProfile::Emoji.tokens().directory
        );
        assert_eq!(
            config.git_branch_icon(),
            IconProfile::Emoji.tokens().git_branch
        );

        config.icons.profile = "minimal".to_string();
        assert_eq!(config.directory_icon(), "dir:");
        assert!(config.directory_icon().is_ascii());
    }

    #[test]
    fn explicit_icon_wins_over_profile() {
        let mut config = Config::default();
        config.icons.profile = "minimal".to_string();
        config.directory.icon = "CUSTOM".to_string();
        assert_eq!(config.directory_icon(), "CUSTOM");
    }

    #[test]
    fn theme_supplies_colors_and_explicit_wins() {
        let mut config = Config::default();
        config.theme.name = "dracula".to_string();
        assert_eq!(
            config.directory_color(),
            ThemeProfile::Dracula.colors().directory
        );

        config.metadata.time_color = "0;99".to_string();
        assert_eq!(config.time_color(), "0;99");
    }

    #[test]
    fn language_style_merges_profile_and_theme() {
        let mut config = Config::default();
        config.icons.profile = "emoji".to_string();
        config.theme.name = "nord".to_string();
        let style = config.language_style("rust");
        assert_eq!(style.icon, "\u{1f980}");
        assert_eq!(
            style.color,
            ThemeProfile::Nord.colors().language("rust").unwrap()
        );

        // Explicit override still wins.
        config.metadata.languages.insert(
            "rust".to_string(),
            LanguageStyleConfig {
                icon: "@".to_string(),
                color: "9;9".to_string(),
            },
        );
        let style = config.language_style("rust");
        assert_eq!(style.icon, "@");
        assert_eq!(style.color, "9;9");
    }

    #[test]
    fn unknown_profile_falls_back_to_default() {
        let mut config = Config::default();
        config.icons.profile = "not-a-profile".to_string();
        assert_eq!(config.icon_profile(), IconProfile::NerdFont);
    }
}
