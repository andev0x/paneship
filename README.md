# Paneship

**Paneship** is a high-performance shell prompt designed for speed, responsiveness, and heavy workflows. It is optimized for tmux environments and large repositories.

## Features

* **Blazing Fast**: Render times under 10ms (3x faster than Starship in benchmarks).
* **Tmux Optimized**: Detects tmux panes and respects pane width for responsive truncation.
* **Shared Cache**: Background daemon provides a shared Git cache across all tmux panes.
* **Gitoxide Powered**: Uses the high-performance `gix` library for Git status.
* **Zero Config**: Works out of the box with sensible defaults.

## Performance

Comparison against Starship in a repository with 8 concurrent panes:

| Prompt | Avg Render Time |
|--------|-----------------|
| Paneship | ~6.6ms |
| Starship | ~20.4ms |

## Installation

```bash
cargo install --path .
```

## Setup

### Zsh

Add the following to your `.zshrc`:

```zsh
# Start the paneship daemon if not running
if ! pgrep -x "paneship" > /dev/null; then
    paneship daemon > /dev/null 2>&1 &
    disown
fi

PROMPT='$(paneship render --exit-code $? --width $COLUMNS)'
```

## Usage

* `paneship render`: Renders the prompt.
* `paneship daemon`: Starts the background cache daemon.
* `paneship benchmark`: Runs a performance comparison.

## License

MIT
