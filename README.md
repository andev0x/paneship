
<div align="center">
<img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/paneship/logo-paneship.png" width="30%" alt="Paneship" />


[![Crates.io](https://img.shields.io/crates/v/paneship?style=flat-square)](https://crates.io/crates/paneship)
[![Docs.rs](https://img.shields.io/docsrs/paneship?style=flat-square)](https://docs.rs/paneship)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](./LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/andev0x/paneship/ci.yml?branch=main&style=flat-square)](https://github.com/andev0x/paneship/actions)
[![GitHub Stars](https://img.shields.io/github/stars/andev0x/paneship?style=flat-square)](https://github.com/andev0x/paneship/stargazers)

A high-performance shell prompt written in Rust, optimized for speed and clarity.

[Demo](#demo) • [Features](#features) • [Installation](#installation) • [Documentation](#documentation) • [Contributing](#contributing)

</div>

---

## About

**Paneship** is a blazingly-fast, modern shell prompt that displays essential workspace information with minimal overhead. Powered by a persistent background daemon and asynchronous metadata updates, it renders in ~2.5ms on average—delivering snappy shell interactions without sacrificing functionality.

Built for developers who work with large Git repositories, tmux environments, and value both speed and clarity.

## ✨ Features

- **Ultra-fast rendering** (~2.5ms average) via daemon-driven architecture
- **Async background worker** for Git status and language version detection
- **Broad shell support** – Bash, Zsh, Fish, PowerShell, Nushell, Elvish, Xonsh, and more
- **Tmux-aware** – Automatic pane width detection and responsive truncation
- **Rich Git integration** – Branch name, file counts, and smart cache invalidation
- **Language detection** – Automatic version info for Rust, Node.js, Python, Go, and more
- **Command timing** – Last command duration in human-readable format
- **Fully customizable** – Simple TOML configuration for colors, icons, and layout
- **Lightweight & efficient** – Written in Rust with minimal resource footprint

## Demo

<div align="center">
  <img src="https://raw.githubusercontent.com/andev0x/description-image-archive/refs/heads/main/paneship/paneship.gif" width="70%" alt="Paneship Demo" />
</div>

## Quick Start

### Installation

Install from [crates.io](https://crates.io/crates/paneship):

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

Initialize Paneship for your shell:

```bash
# Automatic setup (recommended)
paneship init <shell> --onboarding

# Example for Zsh
paneship init zsh --onboarding
```

For manual setup, add one of the following to your shell config:

**Zsh** (~/.zshrc)
```bash
eval "$(paneship init zsh)"
```

**Bash** (~/.bashrc)
```bash
eval "$(paneship init bash)"
```

**Fish** (~/.config/fish/config.fish)
```fish
paneship init fish | source
```

**PowerShell** ($PROFILE)
```powershell
Invoke-Expression (&paneship init powershell)
```

**Nushell** (config.nu)
```nushell
mkdir ~/.cache/paneship
paneship init nushell | save --force ~/.cache/paneship/init.nu
source ~/.cache/paneship/init.nu
```

For other shells (Elvish, Xonsh, Tcsh, Ion, Cmd), use `paneship init <shell>`.

## Configuration

Paneship reads configuration from `~/.config/paneship/config.toml`. If no file exists, sensible defaults apply.

### Example Config

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

| Section | Option | Default | Description |
|---------|--------|---------|-------------|
| `directory` | `icon` | `""` | Symbol before directory path |
| | `truncation_length` | `3` | Path components to display |
| | `truncate_to_repo` | `true` | Show only repo name inside repos |
| `git` | `branch_icon` | `""` | Symbol before branch name |
| | `staged_icon` | `"+"` | Symbol for staged changes |
| | `unstaged_icon` | `"~"` | Symbol for unstaged changes |
| | `untracked_icon` | `"?"` | Symbol for untracked files |
| `status` | `success_icon` | `"❯"` | Cursor for successful exit |
| | `failure_icon` | `"❯"` | Cursor for failed exit |
| `metadata` | `time_color` | `"2;37"` | ANSI color code for time |
| | `paneship_color` | `"1;32"` | ANSI color code for metadata |

## Prompt Layout

```
 ~/.../paneship   main ~2  ·  📦 v1.0.0                  🦀 1.95.0   󰞌 11ms   󰥔 20:56
❯
```

**Line 1 - Left (Context):**
- Directory (project-aware path truncation)
- Git branch and status icons
- Package version

**Line 1 - Right (Metadata):**
- Detected language and version
- Last command duration
- Current time

**Line 2:**
- Cursor character (customizable)

## Architecture

### Daemon-Driven Design

Paneship uses a client-daemon architecture for optimal performance:

1. **Thin Client** – Sends context (CWD, exit code, width) via Unix socket
2. **Persistent Daemon** – Maintains cache and renders prompts
3. **Background Worker** – Handles heavy operations asynchronously
4. **Smart Invalidation** – Updates cache based on Git HEAD and exit codes

This design ensures the shell never blocks, and cached data is always available.

### Performance

| Prompt | Avg Render Time |
|--------|-----------------|
| **Paneship (Daemon)** | **~2.5ms** |
| Paneship (Cold Start) | ~35ms |
| Starship | ~20ms |
| Oh My Zsh | ~150ms+ |

*Benchmarks on 2021 MacBook Pro; results vary by system and repository size.*

## Commands

### Rendering

```bash
paneship render                              # Render current prompt
paneship render --shell zsh                  # Render for specific shell
paneship render --exit-code 1                # Render with exit code
paneship render --cwd /path/to/project       # Render for custom directory
paneship render --width 120                  # Render with custom width
paneship render --duration-ms 2500           # Render with command duration
```

### Initialization

```bash
paneship init zsh                            # Print Zsh init script
paneship init zsh --onboarding               # Append to ~/.zshrc
```

### Daemon Management

```bash
paneship daemon                              # Start background daemon
paneship daemon ping                         # Check daemon status
```

### Benchmarking

```bash
paneship benchmark --iterations 150 --panes 4
paneship benchmark --iterations 150 --compare-starship
```

## Development

### Prerequisites

- Rust 1.70+
- Cargo

### Building

```bash
# Build debug version
cargo build

# Build release version
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Run clippy linter
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Benchmarking

```bash
cargo run --release -- benchmark --iterations 200 --panes 4
```

## Documentation

- [CONTRIBUTING.md](./CONTRIBUTING.md) – Contribution guidelines
- [Example Config](./example_config.toml) – Configuration reference
- [Crates.io](https://crates.io/crates/paneship) – Package info
- [Docs.rs](https://docs.rs/paneship) – API documentation

## Troubleshooting

### Prompt not updating

Check daemon status:
```bash
paneship daemon ping
```

Start daemon manually:
```bash
paneship daemon > /dev/null 2>&1 &
```

### High CPU usage

- Verify Git repository health: `git fsck`
- Check language detection speed: `paneship render --cwd <path>`
- Increase cache TTL in config for large monorepos

### Incorrect directory display

Adjust `truncate_to_repo` in config:
- `true` – Shows only repository name and relative path
- `false` – Shows full home-relative path

## Roadmap

- [ ] Nix shell integration
- [ ] User-defined prompt modules via plugins
- [ ] System package manager integration
- [ ] VSCode integrated terminal support
- [ ] Performance profiling mode
- [ ] Right-side prompt support (Zsh, Fish)

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for:

- How to report bugs
- How to propose features
- Guidelines for pull requests
- Code style and standards

## License

This project is licensed under the MIT License – see [LICENSE](./LICENSE) file for details.

## Acknowledgments

Built with:
- [Gitoxide (gix)](https://github.com/Byron/gitoxide) – Fast Git operations
- [unicode-width](https://crates.io/crates/unicode-width) – Unicode width handling
- [toml](https://crates.io/crates/toml) – Configuration parsing
- [bincode](https://crates.io/crates/bincode) – Serialization

## Support

- 📝 [GitHub Discussions](https://github.com/andev0x/paneship/discussions)
- 🐛 [GitHub Issues](https://github.com/andev0x/paneship/issues)
- 📖 Check documentation and examples

---

<div align="center">

Made with ❤️ by the Paneship community

[⬆ back to top](#paneship)

</div>
