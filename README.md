# Paneship
[![Crates.io](https://img.shields.io/crates/v/paneship?style=flat-square)](https://crates.io/crates/paneship)
[![Docs.rs](https://img.shields.io/docsrs/paneship?style=flat-square)](https://docs.rs/paneship)
[![License](https://img.shields.io/badge/license-MIT-yellow?style=flat-square)](https://opensource.org/licenses/MIT)
[![GitHub](https://img.shields.io/badge/github-andev0x/paneship-blue?style=flat-square)](https://github.com/andev0x/paneship)
[![CI](https://img.shields.io/github/actions/workflow/status/andev0x/paneship/ci.yml?branch=main&style=flat-square)](https://github.com/andev0x/paneship/actions)
[![Stars](https://img.shields.io/github/stars/andev0x/paneship?style=flat-square&color=F9E2AF)](https://github.com/andev0x/paneship/stargazers)
[![Downloads](https://img.shields.io/github/downloads/andev0x/paneship/total?style=flat-square&color=89B4FA)](https://github.com/andev0x/paneship/releases)

A high-performance shell prompt written in Rust, optimized for tmux environments, large Git repositories, and developers who value speed and clarity.

## Demo
<div align="center">
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/paneship/paneship.png" width="80%" alt="Paneship" />
</div>

## Overview

**Paneship** is a fast, modern shell prompt that displays essential workspace information at a glance. With a persistent background daemon, asynchronous metadata updates, and an optimized socket-based rendering pipeline, it renders in **~1.5ms** on average—enabling snappy shell interactions without sacrificing functionality.

### Key Metrics

- **Average render time**: ~1.5ms per prompt
- **Performance**: Up to ~10x faster than comparable alternatives
- **Daemon-side rendering**: Client-only overhead is minimal (~0.5ms socket call)
- **Async Background Refreshes**: Git and metadata updates never block your prompt
- **Smart Invalidation**: Triggered by Git HEAD changes and shell exit codes

## Features

- **⚡ Ultra-fast rendering** — ~1.5ms average by offloading all logic to a persistent background daemon
- **🔄 Async Background Worker** — Heavy operations (Git status, language versions) run in a separate thread and never block the renderer
- **🐚 Broad Shell Support** — Native support for 10+ shells including Bash, Zsh, Fish, PowerShell, Nushell, and more
- **🎯 Tmux-aware** — Automatic pane width detection, responsive truncation, and zero-lag cross-pane cache sharing
- **🔧 Rich Git integration** — Branch display and file counts using `gix`, with smart refreshes on commit/branch switch
- **🔢 Language & Metadata** — Automatic detection for Rust, Node.js, Python, Go, and more; version info is fetched asynchronously
- **⏱️ Command timing** — Shows last command duration in human-readable format (ms/s/m)
- **🖌️ Fully customizable** — Simple TOML config for colors, icons, and layout
- **🔒 Safe & Efficient** — Written in Rust with a minimal resource footprint and system allocator for fast startup

## Quick Start

### Installation

Install the latest release from [crates.io](https://crates.io/crates/paneship):

```bash
cargo install paneship
```

Or build from source:

```bash
git clone https://github.com/andev0x/paneship.git
cd paneship
cargo install --path .
```

### Setup

Paneship supports **10+ shells** including `bash`, `zsh`, `fish`, `powershell`, `nushell`, `elvish`, `xonsh`, `tcsh`, `ion`, and `cmd`.

#### Automatic Setup (Recommended)
Run the following command and restart your shell:
```bash
paneship init <shell> --onboarding
```
*Example:* `paneship init zsh --onboarding` or `paneship init bash --onboarding`

#### Manual Setup
Add the initialization script to your shell's configuration file:

**Zsh / Bash**
Add to `~/.zshrc` or `~/.bashrc`:
```bash
eval "$(paneship init <zsh|bash>)"
```

**Fish**
Add to `~/.config/fish/config.fish`:
```fish
paneship init fish | source
```

**PowerShell**
Add to your `$PROFILE`:
```powershell
Invoke-Expression (&paneship init powershell)
```

**Nushell**
Add to your config:
```nushell
paneship init nushell | save -f ~/.cache/paneship/init.nu
```
> If error

```nushell
mkdir ~/.cache/paneship
paneship init nu | save --force ~/.cache/paneship/init.nu
```

```nushell
# Then add `source ~/.cache/paneship/init.nu` to your config.nu
```

**Other Shells**
Paneship also supports `elvish`, `xonsh`, `tcsh`, `ion`, and `cmd`. Use `paneship init <shell>` to generate the respective initialization scripts.

## Prompt Layout

Paneship displays a two-line prompt:

```
 ~/.../paneship   main ~2  ·  📦 v1.0.0                  🦀 1.95.0   󰞌 11ms   󰥔 20:56
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

### Daemon-Driven Model

Paneship uses a client-daemon architecture to achieve sub-millisecond responsiveness:

1.  **Thin Client**: The `paneship render` command is a lightweight binary that merely sends your current context (CWD, exit code, width) to the daemon via a Unix socket.
2.  **Persistent Daemon**: A background process (`paneship daemon`) maintains an in-memory cache and performs all rendering logic.
3.  **Background Worker**: A dedicated thread in the daemon handles "heavy" tasks (like running `node -v` or `gix status`).
4.  **Instant Response**: The daemon always returns the *best available* cached data immediately. If the data is stale (e.g., you just changed branches), it triggers a background refresh for the *next* prompt.

### Performance Optimizations

1.  **Socket-Based Rendering** — The client does zero repository discovery or filesystem walking.
2.  **Smart Invalidation** — The daemon monitors Git `HEAD` and shell exit codes to know when to refresh data.
3.  **System Allocator** — Optimized for fast process startup times.
4.  **Zero-Recursion** — Advanced process detection prevents redundant socket calls during background updates.

## Commands

### Rendering

```bash
# Render prompt with current context
paneship render

# Render with specific shell formatting
paneship render --shell zsh

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

# Append initialization to ~/.zshrc (or equivalent)
paneship init zsh --onboarding

# Supports: bash, zsh, fish, powershell, nushell, elvish, xonsh, tcsh, ion, cmd
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

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started, report bugs, and submit pull requests.

## Roadmap

Potential features and improvements:

- [ ] Nix shell integration
- [ ] User-defined prompt modules via plugins
- [ ] Integration with system package managers for language detection
- [ ] VSCode integrated terminal support
- [ ] Performance profiling mode for debugging slow prompts
- [ ] Right-side prompt support for Zsh and Fish

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
| **Paneship (Daemon)** | **~1.5ms** |
| Paneship (Cold/No Daemon) | ~35ms |
| Starship | ~15ms |
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

- Open a [GitHub Discussion](https://github.com/andev0x/paneship/discussions)
- File an [issue](https://github.com/andev0x/paneship/issues)
- Check existing documentation and examples

---

**Paneship**: Fast, focused, productive. Happy coding!
