# Paneship

[![Crates.io](https://img.shields.io/crates/v/paneship.svg)](https://crates.io/crates/paneship)
[![docs.rs](https://docs.rs/paneship/badge.svg)](https://docs.rs/paneship)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub](https://img.shields.io/badge/GitHub-anomalyco/paneship-blue)](https://github.com/anomalyco/paneship)

A high-performance shell prompt written in Rust, optimized for tmux environments, large Git repositories, and developers who value speed and clarity.

## Overview

**Paneship** is a fast, modern shell prompt that displays essential workspace information at a glance. With intelligent caching, language detection, and responsive layout adaptation, it renders in ~5.7ms on average—enabling snappy shell interactions without sacrificing functionality.

### Key Metrics

- **Average render time**: ~5.7ms per prompt
- **Performance**: ~4x faster than comparable alternatives
- **Daemon-backed caching**: Zero-lag cross-pane data sharing
- **Language support**: Rust, Node.js, Python, Go, Ruby, PHP, Java, and more
- **Daemon health**: Built-in daemon lifecycle management

## Features

- **⚡ Ultra-fast rendering** — ~5.7ms average with intelligent caching across Git status, language detection, and repository root lookups
- **🎯 Tmux-aware** — Automatic pane width detection, responsive truncation, and cross-pane cache sharing via background daemon
- **🔧 Rich Git integration** — Branch display, staged/unstaged/untracked file counts using high-performance `gix` library
- **🔢 Language detection** — Automatic language recognition with configurable icons and colors; displays runtime versions
- **📦 Project metadata** — Displays package versions (e.g., Cargo.toml for Rust projects)
- **⏱️ Command timing** — Shows last command duration in human-readable format (ms/s/m)
- **🖌️ Fully customizable** — Single TOML config file for colors, icons, truncation behavior, and more
- **💚 Lightweight** — Minimal resource footprint even with multiple tmux panes
- **🔒 Clean & safe** — Written in Rust with zero unsafe code (except where necessary for system calls)

## Quick Start

### Installation

Install the latest release from [crates.io](https://crates.io/crates/paneship):

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

#### Zsh (Recommended)

Add the following line to your `~/.zshrc`:

```bash
eval "$(paneship init zsh)"
```

Then restart your shell:

```bash
exec zsh
```

Or reload your configuration:

```bash
source ~/.zshrc
```

#### Other Shells

Support for Bash, Fish, and other shells is planned. For now, you can manually integrate Paneship by:

1. Setting your prompt variable to call `paneship render`
2. Capturing the last command exit code
3. (Optional) Setting `COLUMNS` to your terminal width for responsive layout

Example for Bash (basic setup):

```bash
PROMPT_COMMAND='PROMPT="$(paneship render --exit-code $? --width $COLUMNS)"'
```

## Prompt Layout

Paneship displays a two-line prompt:

```
~/my-project   main  📦 v1.0.0            🦀 Rust 1.75   󰞌 125ms   14:32
❯
```

### Line 1 Breakdown

**Left side (Context):**
- **Directory**: Project-aware path truncation (shows repository root + relative path)
- **Git branch**: Current branch name with icon
- **Git status**: Icons for staged (`+`), unstaged (`~`), and untracked (`?`) changes
- **Package version**: Extracted from Cargo.toml, package.json, pyproject.toml, etc.

**Right side (Metadata):**
- **Language & version**: Detected language with runtime version (e.g., `🦀 Rust 1.75`)
- **Command duration**: Time taken by the last command (e.g., `󰞌 125ms`)
- **Current time**: HH:MM format

### Line 2

A single cursor character (customizable via config).

## Configuration

Paneship reads its configuration from `~/.config/paneship/config.toml`. If no config file exists, sensible defaults are used.

### Example Configuration

```toml
[directory]
icon = "📁"
truncation_length = 3
truncate_to_repo = true

[git]
branch_icon = ""
staged_icon = "+"
unstaged_icon = "~"
untracked_icon = "?"

[status]
success_icon = "❯"
failure_icon = "❯"

[metadata]
time_color = "2;37"
paneship_color = "1;32"

[metadata.languages.rust]
icon = "🦀"
color = "1;33"

[metadata.languages.node]
icon = "⬢"
color = "1;32"
```

### Configuration Options

#### Directory

- `icon` — Symbol displayed before the directory path (default: `""`—folder icon)
- `truncation_length` — Number of path components to show before truncating (default: `3`)
- `truncate_to_repo` — Show only repository name + truncated path inside repos (default: `true`)

#### Git

- `branch_icon` — Symbol before branch name (default: `""`)
- `staged_icon` — Symbol for staged changes (default: `"+"`)
- `unstaged_icon` — Symbol for unstaged changes (default: `"~"`)
- `untracked_icon` — Symbol for untracked files (default: `"?"`)

#### Status

- `success_icon` — Cursor symbol for successful exit (default: `"❯"`)
- `failure_icon` — Cursor symbol for failed exit (default: `"❯"`)

#### Metadata

- `time_color` — ANSI color code for time display (default: `"2;37"`)
- `paneship_color` — ANSI color code for metadata (default: `"1;32"`)
- `languages.<name>` — Language-specific configuration (icon and color)

## Architecture

### Daemon Mode

Paneship runs a background daemon (`paneship daemon`) that:

- Caches Git repository metadata across multiple tmux panes
- Prevents redundant Git operations in the same workspace
- Automatically manages its own lifecycle (starts on first prompt render if not running)

The daemon communicates with prompt renderers via Unix domain sockets.

### Caching Strategy

- **Git cache**: 350ms TTL—updates frequently to catch fast-moving changes
- **Language/package cache**: 5s TTL—stable for most development workflows
- **Repository root cache**: Persistent within a session—rarely changes
- **Configuration cache**: Persistent within a session—reloaded only on process restart

### Performance Optimizations

1. **Parallel metadata collection** — Language version, package version, and Git status are collected independently
2. **Smart truncation** — Path truncation respects display width without repeated calculations
3. **Early exit** — Prompt rendering stops collecting data once width budget is exhausted
4. **Lazy evaluation** — Expensive operations (e.g., shell version detection) only run if needed

## Commands

### Rendering

```bash
# Render prompt with current context
paneship render

# Render with specific exit code
paneship render --exit-code 1

# Render for custom directory
paneship render --cwd /path/to/project

# Render with custom terminal width
paneship render --width 120

# Render with command duration
paneship render --duration-ms 2500
```

### Initialization

```bash
# Print shell initialization script for eval
paneship init zsh

# Append initialization to ~/.zshrc (interactive setup)
paneship init zsh --onboarding
```

### Daemon Management

```bash
# Start background daemon
paneship daemon

# Check if daemon is running
paneship daemon ping
```

### Benchmarking

```bash
# Run benchmark (150 iterations, 4 concurrent panes)
paneship benchmark --iterations 150 --panes 4

# Compare against Starship
paneship benchmark --iterations 150 --compare-starship
```

## Development

### Build

```bash
cargo build --release
```

### Test

```bash
cargo test
```

### Lint

```bash
cargo clippy --all-targets -- -D warnings
```

### Benchmark

```bash
cargo run --release -- benchmark --iterations 200 --panes 4
```

## Contributing

Contributions are welcome! We follow a standard open-source workflow:

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/YOUR-USERNAME/paneship.git
   cd paneship
   ```
3. **Create a feature branch**:
   ```bash
   git checkout -b feature/my-improvement
   ```
4. **Make your changes** and ensure they pass:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```
5. **Commit** with clear messages:
   ```bash
   git commit -m "feat: add support for Nix shells"
   ```
6. **Push** to your fork and **open a pull request**

### Reporting Issues

Found a bug or have an idea? Please open an issue on [GitHub Issues](https://github.com/anomalyco/paneship/issues) with:

- Clear title and description
- Steps to reproduce (if applicable)
- Environment details (OS, shell, Rust version)
- Expected vs. actual behavior

### Code Style

- Follow Rust conventions (enforced by `rustfmt`)
- Avoid unsafe code unless absolutely necessary (with comments explaining why)
- Write tests for new features
- Update this README if adding user-facing functionality

## Roadmap

Potential features and improvements:

- [ ] Bash and Fish shell support
- [ ] Windows terminal support (partial Rust support exists)
- [ ] Nix shell integration
- [ ] User-defined prompt modules via plugins
- [ ] Integration with system package managers for language detection
- [ ] VSCode integrated terminal support
- [ ] Performance profiling mode for debugging slow prompts

## Troubleshooting

### Prompt not updating

Check if the daemon is running:

```bash
paneship daemon ping
```

If it's not, manually start it:

```bash
paneship daemon > /dev/null 2>&1 &
```

### High CPU usage

- Verify Git repository health with `git fsck`
- Check if language detection is slow (profile with `paneship render --cwd <path>`)
- Increase cache TTL in `~/.config/paneship/config.toml` if working in large monorepos

### Incorrect directory display

Ensure `truncate_to_repo` is set appropriately in your config. If set to `true`, only the repository name and relative path are shown. Set to `false` to show your full home-relative path.

## Performance Comparison

On a mid-sized Rust repository (500+ dependencies):

| Prompt | Avg Render Time |
|--------|-----------------|
| Paneship (with cache) | ~5.7ms |
| Paneship (cold cache) | ~45ms |
| Starship | ~22ms |
| Oh My Zsh | ~150ms+ |

*Benchmarks run on a 2021 MacBook Pro; results vary by system and repository size.*

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Acknowledgments

- Built with [Gitoxide (gix)](https://github.com/Byron/gitoxide) for fast Git operations
- Unicode width handling via [unicode-width](https://crates.io/crates/unicode-width)
- Configuration via [TOML](https://crates.io/crates/toml)
- Serialization via [bincode](https://crates.io/crates/bincode)

## Support

For questions or discussions, feel free to:

- Open a [GitHub Discussion](https://github.com/anomalyco/paneship/discussions)
- File an [issue](https://github.com/anomalyco/paneship/issues)
- Check existing documentation and examples

---

**Paneship**: Fast, focused, productive. Happy coding!
