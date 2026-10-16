//! Icon-set and theme profiles (Phase 2, Module D).
//!
//! Terminal font support varies wildly between local Nerd Font users,
//! emoji-friendly terminals and constrained SSH sessions. Instead of forcing
//! every user to hand-edit each glyph, the config layer resolves prompt tokens
//! dynamically from a small set of **profiles** selected in `config.toml`:
//!
//! ```toml
//! [icons]
//! profile = "nerdfont" # nerdfont | emoji | minimal
//!
//! [theme]
//! name = "default"     # default | dracula | nord | tokyo-night | gruvbox
//! ```
//!
//! Any explicit per-section value (`directory.icon`, `status.success_icon`, …)
//! still wins over the profile, so existing configurations keep working.

/// Built-in icon profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconProfile {
    /// Full Nerd Font glyph suite (requires a patched font).
    #[default]
    NerdFont,
    /// Universal Unicode emoji, renders in nearly every modern terminal.
    Emoji,
    /// Pure ASCII fallback for constrained environments or SSH sessions.
    Minimal,
}

impl IconProfile {
    /// Every selectable profile, in display order.
    pub const ALL: [IconProfile; 3] = [Self::NerdFont, Self::Emoji, Self::Minimal];

    /// Parse a profile key from configuration (case-insensitive, with aliases).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "nerdfont" | "nerd" | "nerd-font" | "nerd_font" => Some(Self::NerdFont),
            "emoji" | "unicode" => Some(Self::Emoji),
            "minimal" | "ascii" | "plain" => Some(Self::Minimal),
            _ => None,
        }
    }

    /// Canonical config key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NerdFont => "nerdfont",
            Self::Emoji => "emoji",
            Self::Minimal => "minimal",
        }
    }

    /// Human-readable label for the TUI.
    pub fn label(self) -> &'static str {
        match self {
            Self::NerdFont => "Nerd Font",
            Self::Emoji => "Emoji",
            Self::Minimal => "Minimal (ASCII)",
        }
    }

    /// Resolve the static token table for this profile.
    pub fn tokens(self) -> &'static ProfileIcons {
        match self {
            Self::NerdFont => &NERD_FONT,
            Self::Emoji => &EMOJI,
            Self::Minimal => &MINIMAL,
        }
    }
}

/// Static glyph tokens for one [`IconProfile`].
pub struct ProfileIcons {
    pub directory: &'static str,
    pub git_branch: &'static str,
    pub git_staged: &'static str,
    pub git_unstaged: &'static str,
    pub git_untracked: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub package: &'static str,
    pub time: &'static str,
    pub duration: &'static str,
    pub placeholder_git: &'static str,
    pub placeholder_lang: &'static str,
    pub languages: &'static [(&'static str, &'static str)],
    /// Infrastructure/environment glyphs, keyed by `EnvironmentKind::key()`.
    pub environments: &'static [(&'static str, &'static str)],
}

impl ProfileIcons {
    /// Icon for a language key (e.g. `"rust"`), if the profile defines one.
    pub fn language(&self, name: &str) -> Option<&'static str> {
        self.languages
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, icon)| *icon)
    }

    /// Icon for an environment key (e.g. `"kubernetes"`), if defined.
    pub fn environment(&self, key: &str) -> Option<&'static str> {
        self.environments
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, icon)| *icon)
    }
}

static NERD_FONT: ProfileIcons = ProfileIcons {
    directory: "\u{f07b}",
    git_branch: "\u{e0a0}",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    success: "\u{279c}",
    failure: "\u{2717}",
    package: "\u{f1b2}",
    time: "\u{f2f2}",
    duration: "\u{f19b}",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "\u{e7a8}"),
        ("node", "\u{f17a}"),
        ("bun", "\u{e76f}"),
        ("python", "\u{e73c}"),
        ("go", "\u{e627}"),
        ("deno", "\u{e7c0}"),
        ("ruby", "\u{f0d7}"),
        ("php", "\u{e608}"),
        ("java", "\u{e256}"),
    ],
    environments: &[
        ("kubernetes", "\u{f10fe}"),
        ("docker", "\u{f308}"),
        ("aws", "\u{e7ad}"),
        ("gcp", "\u{e7b2}"),
        ("azure", "\u{e754}"),
        ("nix", "\u{f1105}"),
        ("direnv", "\u{f0c5}"),
        ("load", "\u{f0e4}"),
        ("battery", "\u{f240}"),
    ],
};

static EMOJI: ProfileIcons = ProfileIcons {
    directory: "\u{1f4c1}",
    git_branch: "\u{1f33f}",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    success: "\u{27a1}",
    failure: "\u{274c}",
    package: "\u{1f4e6}",
    time: "\u{1f552}",
    duration: "\u{23f1}",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "\u{1f980}"),
        ("node", "\u{2b22}"),
        ("bun", "\u{1f95f}"),
        ("python", "\u{1f40d}"),
        ("go", "\u{1f439}"),
        ("deno", "\u{1f995}"),
        ("ruby", "\u{1f48e}"),
        ("php", "\u{1f418}"),
        ("java", "\u{2615}"),
    ],
    environments: &[
        ("kubernetes", "\u{2638}"),
        ("docker", "\u{1f433}"),
        ("aws", "\u{2601}"),
        ("gcp", "\u{2601}"),
        ("azure", "\u{2601}"),
        ("nix", "\u{2744}"),
        ("direnv", "\u{1f4c1}"),
        ("load", "\u{1f525}"),
        ("battery", "\u{1f50b}"),
    ],
};

static MINIMAL: ProfileIcons = ProfileIcons {
    directory: "dir:",
    git_branch: "git:",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    success: ">",
    failure: "x",
    package: "pkg:",
    time: "time:",
    duration: "took:",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "rs:"),
        ("node", "js:"),
        ("bun", "bun:"),
        ("python", "py:"),
        ("go", "go:"),
        ("deno", "deno:"),
        ("ruby", "rb:"),
        ("php", "php:"),
        ("java", "java:"),
    ],
    environments: &[
        ("kubernetes", "k8s:"),
        ("docker", "docker:"),
        ("aws", "aws:"),
        ("gcp", "gcp:"),
        ("azure", "az:"),
        ("nix", "nix:"),
        ("direnv", "direnv:"),
        ("load", "load:"),
        ("battery", "batt:"),
    ],
};

/// Built-in colour themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeProfile {
    #[default]
    Default,
    Dracula,
    Nord,
    TokyoNight,
    Gruvbox,
}

impl ThemeProfile {
    /// Every selectable theme, in display order.
    pub const ALL: [ThemeProfile; 5] = [
        Self::Default,
        Self::Dracula,
        Self::Nord,
        Self::TokyoNight,
        Self::Gruvbox,
    ];

    /// Parse a theme key from configuration (case-insensitive, with aliases).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "default" | "base" => Some(Self::Default),
            "dracula" => Some(Self::Dracula),
            "nord" => Some(Self::Nord),
            "tokyo-night" | "tokyo_night" | "tokyonight" => Some(Self::TokyoNight),
            "gruvbox" => Some(Self::Gruvbox),
            _ => None,
        }
    }

    /// Canonical config key.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Dracula => "dracula",
            Self::Nord => "nord",
            Self::TokyoNight => "tokyo-night",
            Self::Gruvbox => "gruvbox",
        }
    }

    /// Human-readable label for the TUI.
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Dracula => "Dracula",
            Self::Nord => "Nord",
            Self::TokyoNight => "Tokyo Night",
            Self::Gruvbox => "Gruvbox",
        }
    }

    /// Resolve the static colour table for this theme.
    pub fn colors(self) -> &'static ThemeColors {
        match self {
            Self::Default => &THEME_DEFAULT,
            Self::Dracula => &THEME_DRACULA,
            Self::Nord => &THEME_NORD,
            Self::TokyoNight => &THEME_TOKYO_NIGHT,
            Self::Gruvbox => &THEME_GRUVBOX,
        }
    }
}

/// Static ANSI SGR colour tokens for one [`ThemeProfile`].
pub struct ThemeColors {
    pub directory: &'static str,
    pub git_branch: &'static str,
    pub staged: &'static str,
    pub unstaged: &'static str,
    pub untracked: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub time: &'static str,
    pub paneship: &'static str,
    /// Colour for infrastructure/environment tokens.
    pub environment: &'static str,
    pub languages: &'static [(&'static str, &'static str)],
}

impl ThemeColors {
    /// Colour for a language key (e.g. `"rust"`), if the theme defines one.
    pub fn language(&self, name: &str) -> Option<&'static str> {
        self.languages
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, color)| *color)
    }
}

static THEME_DEFAULT: ThemeColors = ThemeColors {
    directory: "1;34",
    git_branch: "1;36",
    staged: "32",
    unstaged: "33",
    untracked: "31",
    success: "32",
    failure: "31",
    time: "2;37",
    paneship: "1;32",
    environment: "1;35",
    languages: &[
        ("rust", "1;33"),
        ("node", "1;32"),
        ("bun", "1;38;5;208"),
        ("python", "1;34"),
        ("go", "1;36"),
        ("deno", "1;32"),
        ("ruby", "1;31"),
        ("php", "1;35"),
        ("java", "1;31"),
    ],
};

static THEME_DRACULA: ThemeColors = ThemeColors {
    directory: "1;38;5;141",
    git_branch: "1;38;5;117",
    staged: "38;5;84",
    unstaged: "38;5;228",
    untracked: "38;5;203",
    success: "38;5;84",
    failure: "38;5;203",
    time: "38;5;103",
    paneship: "1;38;5;212",
    environment: "38;5;141",
    languages: &[
        ("rust", "38;5;209"),
        ("node", "38;5;84"),
        ("bun", "38;5;215"),
        ("python", "38;5;117"),
        ("go", "38;5;123"),
        ("deno", "38;5;84"),
        ("ruby", "38;5;203"),
        ("php", "38;5;183"),
        ("java", "38;5;203"),
    ],
};

static THEME_NORD: ThemeColors = ThemeColors {
    directory: "1;38;5;110",
    git_branch: "1;38;5;109",
    staged: "38;5;150",
    unstaged: "38;5;222",
    untracked: "38;5;174",
    success: "38;5;150",
    failure: "38;5;174",
    time: "38;5;245",
    paneship: "1;38;5;108",
    environment: "38;5;110",
    languages: &[
        ("rust", "38;5;216"),
        ("node", "38;5;150"),
        ("bun", "38;5;216"),
        ("python", "38;5;110"),
        ("go", "38;5;109"),
        ("deno", "38;5;150"),
        ("ruby", "38;5;174"),
        ("php", "38;5;183"),
        ("java", "38;5;174"),
    ],
};

static THEME_TOKYO_NIGHT: ThemeColors = ThemeColors {
    directory: "1;38;5;111",
    git_branch: "1;38;5;116",
    staged: "38;5;114",
    unstaged: "38;5;222",
    untracked: "38;5;210",
    success: "38;5;114",
    failure: "38;5;210",
    time: "38;5;60",
    paneship: "1;38;5;176",
    environment: "38;5;111",
    languages: &[
        ("rust", "38;5;216"),
        ("node", "38;5;114"),
        ("bun", "38;5;216"),
        ("python", "38;5;111"),
        ("go", "38;5;116"),
        ("deno", "38;5;114"),
        ("ruby", "38;5;210"),
        ("php", "38;5;183"),
        ("java", "38;5;210"),
    ],
};

static THEME_GRUVBOX: ThemeColors = ThemeColors {
    directory: "1;38;5;109",
    git_branch: "1;38;5;108",
    staged: "38;5;142",
    unstaged: "38;5;214",
    untracked: "38;5;167",
    success: "38;5;142",
    failure: "38;5;167",
    time: "38;5;245",
    paneship: "1;38;5;142",
    environment: "38;5;109",
    languages: &[
        ("rust", "38;5;214"),
        ("node", "38;5;142"),
        ("bun", "38;5;214"),
        ("python", "38;5;109"),
        ("go", "38;5;108"),
        ("deno", "38;5;142"),
        ("ruby", "38;5;167"),
        ("php", "38;5;175"),
        ("java", "38;5;167"),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_keys_round_trip() {
        for profile in IconProfile::ALL {
            assert_eq!(IconProfile::parse(profile.as_str()), Some(profile));
        }
        assert_eq!(IconProfile::parse("Nerd-Font"), Some(IconProfile::NerdFont));
        assert_eq!(IconProfile::parse("  EMOJI "), Some(IconProfile::Emoji));
        assert_eq!(IconProfile::parse("bogus"), None);
    }

    #[test]
    fn theme_keys_round_trip() {
        for theme in ThemeProfile::ALL {
            assert_eq!(ThemeProfile::parse(theme.as_str()), Some(theme));
        }
        assert_eq!(
            ThemeProfile::parse("Tokyonight"),
            Some(ThemeProfile::TokyoNight)
        );
        assert_eq!(ThemeProfile::parse("nope"), None);
    }

    #[test]
    fn minimal_profile_is_pure_ascii() {
        let icons = IconProfile::Minimal.tokens();
        for token in [
            icons.directory,
            icons.git_branch,
            icons.package,
            icons.time,
            icons.duration,
        ] {
            assert!(token.is_ascii(), "token not ASCII: {token:?}");
        }
        for (name, icon) in icons.languages {
            assert!(icon.is_ascii(), "{name} icon not ASCII: {icon:?}");
        }
    }

    #[test]
    fn profiles_expose_language_icons() {
        assert_eq!(
            IconProfile::Emoji.tokens().language("rust"),
            Some("\u{1f980}")
        );
        assert_eq!(IconProfile::Minimal.tokens().language("go"), Some("go:"));
        assert_eq!(IconProfile::NerdFont.tokens().language("unknown"), None);
    }
}
