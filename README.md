# Paneship

A high-performance shell prompt written in Rust.

## Features

- Fast prompt rendering for terminal shells
- Git status integration
- Tmux awareness
- Benchmarking tools for performance comparison
- Configurable via CLI arguments

## Installation

```bash
cargo install paneship
```

## Usage

```bash
# Render prompt (default)
paneship

# With options
paneship --exit-code 1 --width 80 --cwd /path/to/dir

# Benchmark mode
paneship benchmark --iterations 200 --panes 4

# Compare with Starship
paneship benchmark --iterations 200 --panes 4 --compare-starship

# Help
paneship help
```

### Options

| Flag | Description |
|------|-------------|
| `-s, --exit-code <code>` | Last command exit code |
| `-w, --width <cols>` | Prompt width budget |
| `--cwd <path>` | Directory to render the prompt for |

### Benchmark Options

| Flag | Description |
|------|-------------|
| `-n, --iterations <n>` | Renders per pane (default: 200) |
| `-p, --panes <n>` | Number of concurrent panes (default: 4) |
| `--compare-starship` | Include direct Starship comparison |

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
cargo clippy
```

## License

MIT