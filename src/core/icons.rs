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
//! Any explicit per-section value (`status.success_icon`, `git.branch_icon`, …)
//! still wins over the profile, so existing configurations keep working.
//!
//! # Glyph policy
//!
//! The default `nerdfont` profile is strictly **emoji-free**: every glyph is a
//! Nerd Font code point or a standard geometric Unicode symbol. The `emoji`
//! profile is an explicit, manually selected opt-in and is never chosen by
//! [`IconProfile::default`].
//!
//! The left prompt path is pure text: there is no directory, folder or project
//! prefix glyph. Host OS identification lives in the right-side telemetry
//! region, and runtime/toolchain glyphs travel with the runtime version there,
//! so `cd`/`z` navigation can never leak a logo into the left prompt.

/// Built-in icon profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconProfile {
    /// Full Nerd Font glyph suite (requires a patched font). This is the
    /// default and contains no raw emoji.
    #[default]
    NerdFont,
    /// Universal Unicode emoji. Explicit opt-in only: selecting this profile is
    /// the supported way to override the emoji-free default policy.
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
            Self::Emoji => "Emoji (opt-in)",
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
    pub git_branch: &'static str,
    pub git_staged: &'static str,
    pub git_unstaged: &'static str,
    pub git_untracked: &'static str,
    pub success: &'static str,
    pub failure: &'static str,
    pub time: &'static str,
    pub duration: &'static str,
    pub placeholder_git: &'static str,
    pub placeholder_lang: &'static str,
    pub languages: &'static [(&'static str, &'static str)],
    /// Infrastructure/environment glyphs, keyed by `EnvironmentKind::key()`.
    pub environments: &'static [(&'static str, &'static str)],
    /// Operating-system glyphs, keyed by [`current_os_key`].
    pub operating_systems: &'static [(&'static str, &'static str)],
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

    /// Icon for a normalized operating-system key (e.g. `"macos"`).
    pub fn operating_system(&self, key: &str) -> Option<&'static str> {
        self.operating_systems
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, icon)| *icon)
    }
}

/// Normalize `std::env::consts::OS` into a key used by
/// [`ProfileIcons::operating_system`].
pub fn current_os_key() -> &'static str {
    match std::env::consts::OS {
        "macos" | "ios" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        "android" => "android",
        "freebsd" | "openbsd" | "netbsd" | "dragonfly" => "bsd",
        other => other,
    }
}

static NERD_FONT: ProfileIcons = ProfileIcons {
    git_branch: "\u{e0a0}",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    // The canonical prompt cursor: a clean geometric chevron, not an emoji.
    // Failure keeps the same symbol and signals through colour plus `[code]`.
    success: "\u{276f}",
    failure: "\u{276f}",
    time: "\u{f2f2}",
    duration: "\u{f19b}",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "\u{e7a8}"),
        ("node", "\u{e718}"),
        ("bun", "\u{e76f}"),
        ("deno", "\u{e7c0}"),
        ("python", "\u{e73c}"),
        ("go", "\u{e627}"),
        ("ruby", "\u{e791}"),
        ("php", "\u{e608}"),
        ("java", "\u{e738}"),
        ("typescript", "\u{e628}"),
        ("javascript", "\u{e60c}"),
        ("vue", "\u{e6a0}"),
        ("svelte", "\u{e697}"),
        ("astro", "\u{e6b3}"),
        ("c", "\u{e61e}"),
        ("cpp", "\u{e61d}"),
        ("csharp", "\u{f031b}"),
        ("fsharp", "\u{e7a7}"),
        ("swift", "\u{e755}"),
        ("kotlin", "\u{e634}"),
        ("scala", "\u{e737}"),
        ("dart", "\u{e798}"),
        ("zig", "\u{e6a9}"),
        ("lua", "\u{e620}"),
        ("elixir", "\u{e62d}"),
        ("erlang", "\u{e7b1}"),
        ("haskell", "\u{e61f}"),
        ("ocaml", "\u{e67a}"),
        ("clojure", "\u{e768}"),
        ("r", "\u{f07d4}"),
        ("julia", "\u{e624}"),
        ("perl", "\u{e769}"),
        ("crystal", "\u{e62f}"),
        ("nim", "\u{e677}"),
        ("solidity", "\u{e656}"),
        ("terraform", "\u{e69a}"),
        ("nix", "\u{f313}"),
        ("elm", "\u{e62c}"),
        ("gleam", "\u{f005}"),
        ("odin", "\u{f07e2}"),
        ("prisma", "\u{e684}"),
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
    operating_systems: &[
        ("macos", "\u{f179}"),
        ("windows", "\u{f17a}"),
        ("linux", "\u{f17c}"),
        ("android", "\u{f17b}"),
        ("bsd", "\u{f30c}"),
    ],
};

static EMOJI: ProfileIcons = ProfileIcons {
    git_branch: "\u{1f33f}",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    // Keep the prompt cursor geometric even in the opt-in emoji profile.
    success: "\u{276f}",
    failure: "\u{274c}",
    time: "\u{1f552}",
    duration: "\u{23f1}",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "\u{1f980}"),
        ("node", "\u{2b22}"),
        ("bun", "\u{1f95f}"),
        ("deno", "\u{1f995}"),
        ("python", "\u{1f40d}"),
        ("go", "\u{1f439}"),
        ("ruby", "\u{1f48e}"),
        ("php", "\u{1f418}"),
        ("java", "\u{2615}"),
        ("typescript", "\u{1f7e6}"),
        ("javascript", "\u{1f7e8}"),
        ("vue", "\u{1f596}"),
        ("svelte", "\u{1f9e1}"),
        ("astro", "\u{1f680}"),
        ("c", "C"),
        ("cpp", "C++"),
        ("csharp", "C#"),
        ("fsharp", "F#"),
        ("swift", "\u{1f426}"),
        ("kotlin", "\u{1f7ea}"),
        ("scala", "\u{1f53b}"),
        ("dart", "\u{1f3af}"),
        ("zig", "\u{26a1}"),
        ("lua", "\u{1f319}"),
        ("elixir", "\u{1f4a7}"),
        ("erlang", "\u{1f4de}"),
        ("haskell", "\u{03bb}"),
        ("ocaml", "\u{1f42b}"),
        ("clojure", "\u{1f343}"),
        ("r", "\u{1f4ca}"),
        ("julia", "\u{1f9ee}"),
        ("perl", "\u{1f42a}"),
        ("crystal", "\u{1f4a0}"),
        ("nim", "\u{1f451}"),
        ("solidity", "\u{26d3}"),
        ("terraform", "\u{1f3d7}"),
        ("nix", "\u{2744}"),
        ("elm", "\u{1f333}"),
        ("gleam", "\u{2b50}"),
        ("odin", "\u{1fa84}"),
        ("prisma", "\u{1f537}"),
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
    operating_systems: &[
        ("macos", "\u{1f34e}"),
        ("windows", "\u{1fa9f}"),
        ("linux", "\u{1f427}"),
        ("android", "\u{1f916}"),
        ("bsd", "\u{1f608}"),
    ],
};

static MINIMAL: ProfileIcons = ProfileIcons {
    git_branch: "git:",
    git_staged: "+",
    git_unstaged: "!",
    git_untracked: "?",
    success: ">",
    failure: "x",
    time: "time:",
    duration: "took:",
    placeholder_git: "[git...]",
    placeholder_lang: "[lang...]",
    languages: &[
        ("rust", "rs:"),
        ("node", "node:"),
        ("bun", "bun:"),
        ("deno", "deno:"),
        ("python", "py:"),
        ("go", "go:"),
        ("ruby", "rb:"),
        ("php", "php:"),
        ("java", "java:"),
        ("typescript", "ts:"),
        ("javascript", "js:"),
        ("vue", "vue:"),
        ("svelte", "svelte:"),
        ("astro", "astro:"),
        ("c", "c:"),
        ("cpp", "c++:"),
        ("csharp", "cs:"),
        ("fsharp", "fs:"),
        ("swift", "swift:"),
        ("kotlin", "kt:"),
        ("scala", "scala:"),
        ("dart", "dart:"),
        ("zig", "zig:"),
        ("lua", "lua:"),
        ("elixir", "ex:"),
        ("erlang", "erl:"),
        ("haskell", "hs:"),
        ("ocaml", "ml:"),
        ("clojure", "clj:"),
        ("r", "r:"),
        ("julia", "jl:"),
        ("perl", "pl:"),
        ("crystal", "cr:"),
        ("nim", "nim:"),
        ("solidity", "sol:"),
        ("terraform", "tf:"),
        ("nix", "nix:"),
        ("elm", "elm:"),
        ("gleam", "gleam:"),
        ("odin", "odin:"),
        ("prisma", "prisma:"),
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
    operating_systems: &[
        ("macos", "mac:"),
        ("windows", "win:"),
        ("linux", "linux:"),
        ("android", "android:"),
        ("bsd", "bsd:"),
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
    /// Colour for the OS identification glyph in the right telemetry region.
    pub os: &'static str,
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
    os: "2;37",
    environment: "1;35",
    languages: &[
        ("rust", "1;33"),
        ("node", "1;32"),
        ("bun", "1;38;5;208"),
        ("deno", "1;32"),
        ("python", "1;34"),
        ("go", "1;36"),
        ("ruby", "1;31"),
        ("php", "1;35"),
        ("java", "1;31"),
        ("typescript", "1;34"),
        ("javascript", "1;33"),
        ("vue", "1;32"),
        ("svelte", "1;31"),
        ("astro", "1;35"),
        ("c", "1;34"),
        ("cpp", "1;34"),
        ("csharp", "1;35"),
        ("fsharp", "1;36"),
        ("swift", "1;33"),
        ("kotlin", "1;35"),
        ("scala", "1;31"),
        ("dart", "1;36"),
        ("zig", "1;33"),
        ("lua", "1;34"),
        ("elixir", "1;35"),
        ("erlang", "1;31"),
        ("haskell", "1;35"),
        ("ocaml", "1;33"),
        ("clojure", "1;32"),
        ("r", "1;34"),
        ("julia", "1;35"),
        ("perl", "1;36"),
        ("crystal", "1;37"),
        ("nim", "1;33"),
        ("solidity", "1;37"),
        ("terraform", "1;35"),
        ("nix", "1;36"),
        ("elm", "1;34"),
        ("gleam", "1;35"),
        ("odin", "1;34"),
        ("prisma", "1;37"),
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
    os: "38;5;103",
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
        ("typescript", "38;5;117"),
        ("javascript", "38;5;228"),
        ("vue", "38;5;84"),
        ("svelte", "38;5;203"),
        ("astro", "38;5;212"),
        ("c", "38;5;117"),
        ("cpp", "38;5;117"),
        ("csharp", "38;5;141"),
        ("fsharp", "38;5;123"),
        ("swift", "38;5;215"),
        ("kotlin", "38;5;141"),
        ("scala", "38;5;203"),
        ("dart", "38;5;117"),
        ("zig", "38;5;215"),
        ("lua", "38;5;117"),
        ("elixir", "38;5;141"),
        ("erlang", "38;5;203"),
        ("haskell", "38;5;141"),
        ("ocaml", "38;5;215"),
        ("clojure", "38;5;84"),
        ("r", "38;5;117"),
        ("julia", "38;5;141"),
        ("perl", "38;5;123"),
        ("crystal", "38;5;255"),
        ("nim", "38;5;215"),
        ("solidity", "38;5;255"),
        ("terraform", "38;5;141"),
        ("nix", "38;5;123"),
        ("elm", "38;5;117"),
        ("gleam", "38;5;212"),
        ("odin", "38;5;117"),
        ("prisma", "38;5;255"),
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
    os: "38;5;245",
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
        ("typescript", "38;5;110"),
        ("javascript", "38;5;222"),
        ("vue", "38;5;150"),
        ("svelte", "38;5;174"),
        ("astro", "38;5;216"),
        ("c", "38;5;110"),
        ("cpp", "38;5;110"),
        ("csharp", "38;5;216"),
        ("fsharp", "38;5;109"),
        ("swift", "38;5;216"),
        ("kotlin", "38;5;216"),
        ("scala", "38;5;174"),
        ("dart", "38;5;110"),
        ("zig", "38;5;222"),
        ("lua", "38;5;110"),
        ("elixir", "38;5;216"),
        ("erlang", "38;5;174"),
        ("haskell", "38;5;216"),
        ("ocaml", "38;5;222"),
        ("clojure", "38;5;150"),
        ("r", "38;5;110"),
        ("julia", "38;5;216"),
        ("perl", "38;5;109"),
        ("crystal", "38;5;255"),
        ("nim", "38;5;222"),
        ("solidity", "38;5;255"),
        ("terraform", "38;5;216"),
        ("nix", "38;5;109"),
        ("elm", "38;5;110"),
        ("gleam", "38;5;216"),
        ("odin", "38;5;110"),
        ("prisma", "38;5;255"),
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
    os: "38;5;60",
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
        ("typescript", "38;5;111"),
        ("javascript", "38;5;222"),
        ("vue", "38;5;114"),
        ("svelte", "38;5;210"),
        ("astro", "38;5;176"),
        ("c", "38;5;111"),
        ("cpp", "38;5;111"),
        ("csharp", "38;5;216"),
        ("fsharp", "38;5;116"),
        ("swift", "38;5;216"),
        ("kotlin", "38;5;216"),
        ("scala", "38;5;210"),
        ("dart", "38;5;111"),
        ("zig", "38;5;222"),
        ("lua", "38;5;111"),
        ("elixir", "38;5;216"),
        ("erlang", "38;5;210"),
        ("haskell", "38;5;216"),
        ("ocaml", "38;5;222"),
        ("clojure", "38;5;114"),
        ("r", "38;5;111"),
        ("julia", "38;5;216"),
        ("perl", "38;5;116"),
        ("crystal", "38;5;255"),
        ("nim", "38;5;222"),
        ("solidity", "38;5;255"),
        ("terraform", "38;5;176"),
        ("nix", "38;5;116"),
        ("elm", "38;5;111"),
        ("gleam", "38;5;216"),
        ("odin", "38;5;111"),
        ("prisma", "38;5;255"),
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
    os: "38;5;245",
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
        ("typescript", "38;5;109"),
        ("javascript", "38;5;214"),
        ("vue", "38;5;142"),
        ("svelte", "38;5;167"),
        ("astro", "38;5;175"),
        ("c", "38;5;109"),
        ("cpp", "38;5;109"),
        ("csharp", "38;5;175"),
        ("fsharp", "38;5;108"),
        ("swift", "38;5;214"),
        ("kotlin", "38;5;175"),
        ("scala", "38;5;167"),
        ("dart", "38;5;109"),
        ("zig", "38;5;214"),
        ("lua", "38;5;109"),
        ("elixir", "38;5;175"),
        ("erlang", "38;5;167"),
        ("haskell", "38;5;214"),
        ("ocaml", "38;5;214"),
        ("clojure", "38;5;142"),
        ("r", "38;5;109"),
        ("julia", "38;5;175"),
        ("perl", "38;5;108"),
        ("crystal", "38;5;255"),
        ("nim", "38;5;214"),
        ("solidity", "38;5;255"),
        ("terraform", "38;5;175"),
        ("nix", "38;5;108"),
        ("elm", "38;5;109"),
        ("gleam", "38;5;214"),
        ("odin", "38;5;109"),
        ("prisma", "38;5;255"),
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
        for token in [icons.git_branch, icons.time, icons.duration] {
            assert!(token.is_ascii(), "token not ASCII: {token:?}");
        }
        for (name, icon) in icons.languages {
            assert!(icon.is_ascii(), "{name} icon not ASCII: {icon:?}");
        }
        for (name, icon) in icons.operating_systems {
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
        // Language glyphs must be distinct so the directory prefix identifies
        // the project instead of repeating the generic folder.
        assert_ne!(
            IconProfile::NerdFont.tokens().language("node"),
            Some("\u{f17a}"),
            "node must not reuse the Windows glyph"
        );
    }

    #[test]
    fn profiles_expose_operating_system_icons() {
        for profile in IconProfile::ALL {
            let icons = profile.tokens();
            for key in ["macos", "windows", "linux"] {
                assert!(
                    icons.operating_system(key).is_some(),
                    "{} is missing a {key} glyph",
                    profile.as_str()
                );
            }
        }
        assert_eq!(
            IconProfile::NerdFont.tokens().operating_system("macos"),
            Some("\u{f179}")
        );
        assert_eq!(
            IconProfile::NerdFont.tokens().operating_system("windows"),
            Some("\u{f17a}")
        );
    }

    #[test]
    fn default_profile_is_nerdfont_and_emoji_free() {
        assert_eq!(IconProfile::default(), IconProfile::NerdFont);
        for profile in [IconProfile::NerdFont, IconProfile::Minimal] {
            let icons = profile.tokens();
            for token in [
                icons.git_branch,
                icons.git_staged,
                icons.git_unstaged,
                icons.git_untracked,
                icons.success,
                icons.failure,
                icons.time,
                icons.duration,
            ] {
                assert!(
                    !contains_emoji(token),
                    "{} prompt token contains a raw emoji: {token:?}",
                    profile.as_str()
                );
            }
            for (name, icon) in icons.languages {
                assert!(
                    !contains_emoji(icon),
                    "{} language {name} contains a raw emoji: {icon:?}",
                    profile.as_str()
                );
            }
            for (name, icon) in icons.environments {
                assert!(
                    !contains_emoji(icon),
                    "{} environment {name} contains a raw emoji: {icon:?}",
                    profile.as_str()
                );
            }
            for (name, icon) in icons.operating_systems {
                assert!(
                    !contains_emoji(icon),
                    "{} OS {name} contains a raw emoji: {icon:?}",
                    profile.as_str()
                );
            }
        }
    }

    #[test]
    fn prompt_cursor_is_a_geometric_chevron() {
        assert_eq!(IconProfile::NerdFont.tokens().success, "\u{276f}");
        assert_eq!(IconProfile::Emoji.tokens().success, "\u{276f}");
        assert_eq!(IconProfile::Minimal.tokens().success, ">");
    }

    /// Conservative emoji detector for the default profiles. Covers the
    /// pictographic blocks Nerd Font users would notice as "emoji" pollution.
    fn contains_emoji(value: &str) -> bool {
        value.chars().any(|ch| {
            matches!(
                ch as u32,
                0x1f000..=0x1faff // emoji, pictographs, symbols and supplemental
                    | 0x2600..=0x27bf // misc symbols + dingbats (emoji-presented subset)
            ) && !matches!(ch, '\u{276f}' | '\u{2717}' | '\u{279c}')
        })
    }
}
