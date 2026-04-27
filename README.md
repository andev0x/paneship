# Paneship

[![Crates.io](https://img.shields.io/crates/v/paneship.svg)](https://crates.io/crates/paneship)
[![docs.rs](https://docs.rs/paneship/badge.svg)](https://docs.rs/paneship)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


> A high-performance shell prompt for large repositories and tmux workflows.

**Paneship** is a blazingly fast, zero-lag shell prompt optimized for tmux environments, large Git repositories, and power users. Render times under 10ms with intelligent caching, language-aware metadata, and responsive truncation.

## ⚡ Features

- **Blazing Fast**: Render times ~6.6ms (3x faster than Starship in benchmarks)
- **Tmux Optimized**: Automatic pane width detection and responsive truncation
- **Shared Cache**: Background daemon provides zero-lag Git caching across panes
- **Language Aware**: Detect and display project language with custom icons and colors
- **Gitoxide Powered**: High-performance Git introspection via `gix` library
- **Zero Config**: Works out of the box with sensible defaults
- **Fully Customizable**: TOML-based configuration with color and icon customization
- **Efficient**: Low CPU/memory footprint, even with 10+ concurrent tmux panes

## 📊 Performance

Benchmark comparison against Starship in a repository with 8 concurrent panes:

| Prompt | Avg Render Time | Speedup |
|--------|-----------------|---------|
| **Paneship** | ~6.6ms | **3.1x** |
| Starship | ~20.4ms | baseline |

Run your own benchmark:
```bash
paneship benchmark --compare-starship --panes 8 --iterations 200
```

## 🚀 Quick Start

### Installation

Install from [crates.io](https://crates.io/crates/paneship):

```bash
cargo install paneship
```

Or build from source:

```bash
git clone https://github.com/anomalyco/paneship.git
cd paneship
cargo install --path .
```

### Setup

#### Zsh

Add to your `~/.zshrc`:

```bash
eval "$(paneship init zsh to onboarding)"
```

Or manually:

```bash
if ! pgrep -x "paneship" > /dev/null; then
    paneship daemon > /dev/null 2>&1 &
    disown
fi

PROMPT='$(paneship render --exit-code $? --width $COLUMNS)'
```

Restart your shell:
```bash
exec zsh
```

#### Bash

Add to your `~/.bashrc`:

```bash
eval "$(paneship render --exit-code $? --width $COLUMNS)"
```

(Note: Bash support is limited as it lacks reliable right-side prompt rendering.)

#### Fish

Add to your `~/.config/fish/config.fish`:

```fish
function fish_prompt
    set -l exit_status $status
    paneship render --exit-code $exit_status --width $COLUMNS
end

if ! pgrep -x "paneship" > /dev/null
    paneship daemon > /dev/null 2>&1 &
end
```

## 🎨 Layout

Paneship renders a two-line, manually-positioned prompt:

```
~/.../paneship   main  +1 ?2             v0.3.0  🦀 1.87  13:09
❯
```

**Left side** (context):
- Directory (with smart truncation)
- Git branch and status

**Right side** (metadata):
- Paneship version
- Project language with icon and version
- Current time

## ⚙️ Configuration

Paneship reads configuration from `~/.config/paneship/config.toml`.

### Default Configuration

```toml
[directory]
icon = ""  # Nerd Font folder icon
truncation_length = 3
truncate_to_repo = true

[git]
branch_icon = ""  # Nerd Font git icon
staged_icon = "+"
unstaged_icon = "!"
untracked_icon = "?"

[status]
success_icon = "➜"
failure_icon = "➜"

[metadata]
time_color = "2;37"      # Dim white
paneship_color = "1;32"  # Bright green

[metadata.languages.rust]
icon = "🦀"
color = "1;33"  # Bright yellow

[metadata.languages.node]
icon = "⬢"
color = "1;32"  # Bright green

[metadata.languages.python]
icon = "🐍"
color = "1;34"  # Bright blue

[metadata.languages.go]
icon = "🐹"
color = "1;36"  # Bright cyan

[metadata.languages.ruby]
icon = "💎"
color = "1;31"  # Bright red

[metadata.languages.php]
icon = "🐘"
color = "1;35"  # Bright magenta

[metadata.languages.java]
icon = "☕"
color = "1;31"  # Bright red

[metadata.languages.deno]
icon = "🦕"
color = "1;32"  # Bright green

[metadata.languages.bun]
icon = "🥟"
color = "1;38;5;208"  # Orange
```

### Custom Configuration

Create `~/.config/paneship/config.toml` to override defaults:

```toml
[directory]
icon = ""
truncation_length = 4

[git]
branch_icon = ""
staged_icon = "✓"
unstaged_icon = "✗"

[metadata.languages.rust]
icon = "♦"
color = "1;31"  # Bright red instead of yellow
```

Color codes are ANSI SGR parameters:
- `1;31` = bright red
- `1;32` = bright green
- `1;33` = bright yellow
- `1;34` = bright blue
- `1;35` = bright magenta
- `1;36` = bright cyan
- `1;37` = bright white
- `2;37` = dim white

See [ANSI color codes](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit) for more options.

## 📝 Usage

### Commands

```bash
# Render the prompt (called by shell)
paneship render [OPTIONS]

# Initialize shell configuration
paneship init zsh [--onboarding | to onboarding]

# Start the background cache daemon
paneship daemon

# Run performance benchmarks
paneship benchmark [OPTIONS]

# Show help
paneship help
```

### Render Options

```
--exit-code <code>    Last command exit code (default: 0)
--width <cols>        Terminal width in columns (auto-detected if omitted)
--cwd <path>          Working directory (default: current directory)
```

### Benchmark Options

```
--iterations <n>      Renders per pane (default: 200)
--panes <n>           Number of concurrent panes (default: 4)
--compare-starship    Include Starship comparison
--width <cols>        Terminal width
--exit-code <code>    Exit code for rendering
```

## 🔧 Architecture

Paneship consists of:

- **CLI Binary**: Fast, lightweight command-line interface
- **Renderer**: Unicode-aware width calculation and layout engine
- **Git Module**: Gitoxide-powered status detection
- **Daemon**: Background service for shared Git caching across tmux panes
- **Config Loader**: TOML-based configuration with defaults

Key design decisions:

1. **Synchronous Rendering**: Full prompt renders in one pass (no async updates)
2. **Manual Layout**: Full-line rendering without shell `RPROMPT` support
3. **Width-Safe**: All text truncation respects visible character width
4. **Tmux-First**: Pane-aware width detection and stable caching
5. **Zero Dependencies** (in shell): Only requires Rust binary and `date` command

## 🛠️ Building from Source

### Requirements

- Rust 1.56+ (2021 edition)
- Cargo

### Build

```bash
git clone https://github.com/anomalyco/paneship.git
cd paneship
cargo build --release
```

Binary location: `target/release/paneship`

### Tests

```bash
cargo test
```

### Benchmarks

```bash
cargo run --release -- benchmark --compare-starship --panes 8
```

## 📦 Installing from Release

Download prebuilt binaries from [GitHub Releases](https://github.com/andev0x/paneship/releases).

Supported platforms:
- Linux x86_64 (musl, glibc)
- macOS x86_64 and ARM64 (Apple Silicon)
- Windows x86_64 (experimental)

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

Paneship is licensed under the MIT License. See [LICENSE](LICENSE) for details.

## 💡 Tips & Tricks

### Disable the daemon temporarily

```bash
pkill -f "paneship daemon"
```

### View daemon cache

The daemon stores cache in `/tmp/paneship-{uid}.sock`.

### Performance optimization

For the best performance in tmux:

1. Ensure daemon is running: `ps aux | grep "paneship daemon"`
2. Use dedicated terminal emulator (iTerm2, Alacritty, or WezTerm)
3. Disable other prompt plugins (Oh My Zsh, etc.)

### Debugging

Enable verbose output:

```bash
RUST_LOG=debug paneship render --exit-code 0 --width 120
```

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/andev0x/paneship/issues)
- **Discussions**: [GitHub Discussions](https://github.com/andev0x/paneship/discussions)
- **Crates.io**: [paneship on crates.io](https://crates.io/crates/paneship)

## 🎯 Roadmap

- [ ] Bash/Fish full support
- [ ] Async metadata loading (time, language detection)
- [ ] Plugin system for custom modules
- [ ] Web-based configuration UI
- [ ] macOS/Homebrew distribution
- [ ] Language-specific module performance profiling
- [ ] Alternative layout presets

## 📊 Metrics

- **Startup time**: ~2ms (binary + config load)
- **Render time**: ~6.6ms (average in tmux)
- **Memory overhead**: ~5MB daemon + ~1MB per render
- **Git status**: <1ms (cached), ~50ms (first detection)

---

**Enjoy blazing fast prompts!** 🚀

