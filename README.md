# Paneship

[![Crates.io](https://img.shields.io/crates/v/paneship.svg)](https://crates.io/crates/paneship)
[![docs.rs](https://docs.rs/paneship/badge.svg)](https://docs.rs/paneship)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

> A high-performance shell prompt for large repositories and tmux workflows.

**Paneship** is a blazingly fast, open-source, and professional shell prompt optimized for tmux environments, large Git repositories, and developers seeking efficiency. Render times under 10ms with intelligent caching, language-aware metadata, and responsive truncation ensure a seamless user experience.

## ⚡ Features

- 🚀 **Blazing Fast**: Render times ~6.6ms (3x faster than Starship in benchmarks)
- 🎯 **Tmux Optimized**: Automatic pane width detection and responsive truncation
- 📦 **Shared Cache**: Background daemon provides zero-lag Git caching across panes
- 🔢 **Language Aware**: Detect and display project language with custom icons/colors
- 🛠 **Gitoxide Powered**: High-performance Git detection via `gix` library
- 🖌 **Fully Customizable**: Single config file with advanced customizations
- 🔋 **Efficient**: Low resource footprint, even with multiple tmux panes

## 🚀 Quick Start

### Installation

Install the latest release from [crates.io](https://crates.io/crates/paneship):

```bash
cargo install paneship
```

Or install from source for the latest updates:

```bash
git clone https://github.com/anomalyco/paneship.git
cd paneship
cargo install --path .
```

### Setup

#### Zsh

Update `~/.zshrc`:

```bash
eval "$(paneship init zsh)"
```

*Restart your shell to enable Paneship.*

#### Other Shells

For Bash and Fish instructions, [visit the documentation](https://github.com/anomalyco/paneship).

## 🎨 Example Layout

Paneship renders a **two-line shell prompt**:

```
~/project   main  📦 v0.3.0            🦀 Rust 1.67   󰞌 32ms   13:09
❯
```

### Breakdown:
- **Left (Context)**:
  - Directory (truncated smartly within the project scope)
  - Git branch and status
  - Package version (📦 v0.3.0 for Rust projects)
- **Right (Metadata)**:
  - Programming language (with icon and version)
  - Last command duration (e.g., 󰞌 32ms)
  - Current time

This ensures all vital workspace information is instantly visible.

---

## 🤝 Contributing

Contributions are welcome! Follow these steps to get started:

1. Fork the repository on GitHub.
2. Clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/paneship.git
   ```
3. Create a feature branch for your changes:
   ```bash
   git checkout -b my-feature
   ```
4. Submit a pull request with a clear description of your changes.

### Bug Reports & Feature Requests

Have an issue or a great idea? [File an issue](https://github.com/anomalyco/paneship/issues) or join discussions [here](https://github.com/anomalyco/paneship/discussions)!

---

## 📄 License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.