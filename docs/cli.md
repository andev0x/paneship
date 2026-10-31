# CLI reference

Paneship has no subcommand parser dependency; the CLI is intentionally small and
starts quickly. Running `paneship` with no arguments is equivalent to
`paneship render`.

```text
paneship [render] [options]
paneship init <shell> [--onboarding | to onboarding]
paneship config
paneship daemon [ping | reload]
paneship invalidate [--cwd <path>]
paneship benchmark [options]
paneship top [options]
paneship version | --version | -V
paneship help | --help | -h
```

## Exit codes

| Code | Meaning |
| ---- | ------- |
| `0` | Success |
| `1` | Runtime failure in an administration, benchmark, `top`, or `config` command |
| `2` | Invalid CLI arguments |

A daemon timeout during `render` is not an error: the CLI falls back to a local
render.

## `render`

Render the prompt for a directory and print it to stdout.

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `-s`, `--exit-code <code>` | `0` | Exit status of the previous command. Non-zero switches the cursor style. |
| `-w`, `--width <cols>` | terminal / `tmux` / `80` | Width budget in columns. |
| `--cwd <path>` | current directory | Directory to render for. |
| `--duration-ms <ms>` | `PANESHIP_LAST_CMD_DURATION_MS` | Previous command duration. |
| `--shell <name>` | `plain` | Output escaping: `plain`, `bash`, `zsh`, `fish`, `powershell`/`pwsh`, `nushell`/`nu`, `elvish`, `xonsh`, `tcsh`, `ion`, `cmd`. |
| `--layout <mode>` | `combined` | `combined` prints one line plus the cursor; `split` (alias `rprompt`) emits native right-prompt regions. |

### Output shapes

`combined`:

```text
<left ... right>\n<cursor>
```

The right region is padded to the requested width when it fits. When it does
not fit, it is omitted rather than wrapped.

`split`:

```text
<left><US><right>\n<cursor>
```

`<US>` is `0x1f` (ASCII unit separator). This is the format consumed by the Zsh,
Fish, and Nushell init scripts. Each region is escaped for the selected shell
before it is joined.

### Examples

```bash
# Preview the current prompt
paneship render

# Simulate a failed command in a specific directory
paneship render --cwd ~/src/paneship --exit-code 1

# Inspect the native right-prompt wire format
paneship render --shell zsh --layout split

# Render with a fixed width and command duration
paneship render --width 100 --duration-ms 1500
```

## `init`

Print or install a shell integration script.

```bash
paneship init zsh                  # print the script for eval
paneship init zsh --onboarding     # append/update a block in ~/.zshrc
paneship init zsh to onboarding    # accepted alias
```

Onboarding writes a marker-delimited block:

```text
# >>> paneship initialize >>>
...
# <<< paneship initialize <<<
```

Re-running onboarding replaces that block rather than appending duplicates. If
a start marker exists without an end marker, the command fails with an error so
the file is not corrupted.

Onboarding targets:

| Shell | Configuration file |
| ----- | ------------------ |
| Bash | `~/.bashrc` |
| Zsh | `~/.zshrc` |
| Fish | `~/.config/fish/config.fish` |
| PowerShell | `~/.config/powershell/Microsoft.PowerShell_profile.ps1` |
| Nushell | `~/.config/nushell/config.nu` |
| Elvish | `~/.elvish/rc.elv` |
| Xonsh | `~/.xonshrc` |
| Tcsh | `~/.tcshrc` |
| Ion | `~/.config/ion/initrc` |

Cmd has no onboarding file; use the printed script with your Cmd prompt
enhancement.

## `config`

Open the interactive TUI configurator. Sections are listed on the left, with a
live prompt preview and the selected section's fields on the right.

| Key | Action |
| --- | ------ |
| `Ctrl+S` | Save atomically and hot-reload the daemon |
| `q` / `Esc` | Quit |

The TUI requires an interactive terminal.

## `daemon`

| Command | Description |
| ------- | ----------- |
| `paneship daemon` | Start the background daemon in the foreground. Shell hooks start it automatically when needed. |
| `paneship daemon ping` | Print `ok` and exit 0 when the daemon answers; exit 1 otherwise. |
| `paneship daemon reload` | Re-read `config.toml`, clear metadata caches, and print `ok`. |

Only one daemon should run per user. A second daemon exits when it detects an
existing listener. The daemon also retires itself when its executable changes
on disk, allowing the next prompt to start the upgraded binary.

## `invalidate`

```bash
paneship invalidate [--cwd <path>]
```

Tell the daemon that Git metadata under `<path>` changed. The shell hooks call
this after any command starting with `git `. It clears the cached repository
resolution for the path and its ancestors, invalidates the Git entry, queues a
refresh, and registers the repository with the watcher.

Exit code is `0` on success and `1` when the daemon cannot be reached.

## `benchmark`

Measure the in-process render path. Available on Unix.

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `-n`, `--iterations <n>` | `200` | Renders per pane. Must be greater than zero. |
| `-p`, `--panes <n>` | `4` | Concurrent panes. Must be greater than zero. |
| `--compare-starship` | off | Run a head-to-head comparison when Starship is installed. |
| `-w`, `--width <cols>` | `80` | Fixed width used during the timed loop. |
| `--cwd <path>` | current directory | Directory to render for. |
| `-s`, `--exit-code <code>` | `0` | Exit code used during the loop. |

The benchmark pins a width deliberately. Resolving the terminal width inside
the timed loop can spawn `tmux` and would measure process startup instead of
rendering.

```bash
paneship benchmark --iterations 300 --panes 4
paneship benchmark --compare-starship
```

## `top`

Show daemon telemetry.

| Flag | Description |
| ---- | ----------- |
| `--once`, `-1` | Print one snapshot and exit. |
| `--json` | Emit the snapshot as JSON. Implies `--once`. |
| `--benchmark`, `--local` | Run the in-process component benchmark instead of querying the daemon. |

With no flags on an interactive terminal, `top` shows a live view. On a
non-interactive stdout, or when no daemon is running, it prints one snapshot.

```bash
paneship top --once
paneship top --json
paneship top --benchmark
```

## `version` and `help`

```bash
paneship --version
paneship version
paneship -V
```

All three print `paneship <version>`. `help`, `--help`, and `-h` print the
built-in usage text.

## Related documentation

- [Configuration](configuration.md)
- [Troubleshooting](troubleshooting.md)
- [Architecture](architecture.md)
