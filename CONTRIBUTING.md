# Contributing to Paneship

First off, thank you for considering contributing to Paneship! It's people like you that make Paneship such a great tool for the community.

This document provides guidelines and instructions for contributing to this project.

## Table of Contents

- [How Can I Contribute?](#how-can-i-contribute)
  - [Reporting Bugs](#reporting-bugs)
  - [Suggesting Enhancements](#suggesting-enhancements)
  - [Pull Requests](#pull-requests)
- [Local Development Setup](#local-development-setup)
- [Project Structure](#project-structure)
- [Coding Standards](#coding-standards)
- [Testing](#testing)

## How Can I Contribute?

### Reporting Bugs

If you find a bug, please search the [GitHub Issues](https://github.com/andev0x/paneship/issues) to see if it has already been reported. If it hasn't, please open a new issue and include:

- A clear, descriptive title.
- Steps to reproduce the issue.
- Expected behavior vs. actual behavior.
- Your environment details: OS, shell (zsh/bash/etc.), Rust version (`rustc --version`), and terminal emulator.
- Any relevant configuration from your `~/.config/paneship/config.toml`.

### Suggesting Enhancements

We're always looking for ways to make Paneship better! If you have an idea for a new feature or an improvement:

- Check existing issues to see if the feature has been requested.
- Open a new issue with the "enhancement" tag.
- Describe the feature, why it would be useful, and how it might be implemented.

### Pull Requests

1. **Fork** the repository and create your branch from `main`.
2. If you've added code that should be tested, **add tests**.
3. If you've changed APIs, **update the documentation**.
4. Ensure the test suite passes (`cargo test`).
5. Run `cargo clippy -- -D warnings` and `cargo fmt`.
6. Use descriptive commit messages (see [Commit Messages](#commit-messages)).

## Local Development Setup

To build and test Paneship locally, you'll need the Rust toolchain installed.

1. **Install Rust**: Follow the instructions at [rustup.rs](https://rustup.rs/).
2. **Clone the repository**:
   ```bash
   git clone https://github.com/andev0x/paneship.git
   cd paneship
   ```
3. **Build the project**:
   ```bash
   cargo build
   ```
4. **Run the local binary**:
   ```bash
   ./target/debug/paneship render
   ```

## Project Structure

A quick overview of the `src/` directory:

- `core/`: Core logic, including configuration parsing, layout management, and the renderer.
- `modules/`: Individual prompt modules (Git, directory, package version, etc.). This is where most new prompt features will be added.
- `daemon/`: Background daemon for cross-pane caching.
- `tmux/`: Tmux-specific integration and detection.
- `cache/`: Caching mechanisms used by the daemon.
- `main.rs`: Entry point and CLI command handling.

## Coding Standards

### Rust Style

We follow the standard Rust style guidelines. Before submitting a PR, please run:

```bash
cargo fmt
```

### Commit Messages

- Use the present tense ("Add feature" not "Added feature").
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...").
- Reference issues and pull requests liberally after the first line.
- Consider using [Conventional Commits](https://www.conventionalcommits.org/).

### Unsafe Code

Paneship aims to be safe. Avoid `unsafe` code unless it's strictly necessary for system calls or performance-critical sections where safety can be manually verified. Always include a `// SAFETY:` comment explaining why the code is safe.

## Testing

Run the full test suite with:

```bash
cargo test
```

For performance-related changes, please run the benchmark:

```bash
cargo run --release -- benchmark --iterations 200
```

---

Thank you for contributing!
