# Configuration

Paneship reads `~/.config/paneship/config.toml` at startup. On Windows, the home
directory is resolved from `USERPROFILE`. If the file is missing or cannot be
parsed as TOML, Paneship falls back to the built-in defaults; it does not fail
the prompt. Use `paneship config` or a TOML validator to catch syntax mistakes.

The interactive editor is the easiest way to change settings:

```bash
paneship config
```

It previews the prompt live, writes the file atomically, and asks a running
daemon to reload. For manual edits, run `paneship daemon reload` (or restart the
daemon) after saving.

A complete commented example is available in
[`example_config.toml`](../example_config.toml).

## Complete example

```toml
[icons]
profile = "nerdfont"        # nerdfont | emoji | minimal

[theme]
name = "default"            # default | dracula | nord | tokyo-night | gruvbox

[prompt]
rprompt = true              # render the right-aligned telemetry region
rprompt_min_width = 20      # below this width, drop the right region

[environment]
enabled = true              # master switch for environment identification
kubernetes = true           # ~/.kube/config or $KUBECONFIG
docker = true               # container markers and cgroup state
cloud = true                # active AWS / GCP / Azure profile
nix_direnv = true           # IN_NIX_SHELL and .envrc state
health = false              # opt-in load pressure / low battery

[directory]
truncation_length = 3       # maximum displayed folder depth
truncate_to_repo = true     # hide path components outside the repository

[git]
branch_icon = ""            # empty = active icon profile
staged_icon = "+"
unstaged_icon = "!"
untracked_icon = "?"

[status]
success_icon = ""           # empty = active icon profile
failure_icon = ""           # empty = active icon profile

[metadata]
time_color = ""             # empty = active theme
paneship_color = ""         # empty = active theme

# Optional per-language overrides. Empty fields fall back to the active
# icon profile and theme.
[metadata.languages.rust]
icon = ""
color = "1;38;5;208"
```

## `[icons]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `profile` | string | `"nerdfont"` | Glyph profile: `nerdfont` (default, emoji-free), `emoji` (explicit opt-in), or `minimal` (ASCII). |

The default Nerd Font profile contains Nerd Font glyphs and standard geometric
symbols only, including the `❯` prompt cursor. It does not contain raw emoji.
Select `emoji` explicitly if your terminal and font prefer Unicode emoji, or
`minimal` for constrained SSH sessions.

An unknown profile falls back to `nerdfont`.

## `[theme]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `name` | string | `"default"` | Colour theme: `default`, `dracula`, `nord`, `tokyo-night`, or `gruvbox`. |

An unknown theme falls back to `default`.

## `[prompt]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `rprompt` | boolean | `true` | Render the right-side telemetry region at all. |
| `rprompt_min_width` | integer | `20` | Minimum terminal width in columns. Below this the right region is dropped instead of wrapping. |

## `[environment]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `enabled` | boolean | `true` | Master switch for the environment segment. |
| `kubernetes` | boolean | `true` | Kubernetes context and namespace from `~/.kube/config` or `$KUBECONFIG`. |
| `docker` | boolean | `true` | Container detection from `/.dockerenv`, `/proc/1/cgroup`, and `$container`. |
| `cloud` | boolean | `true` | Active AWS, GCP, or Azure profile. |
| `nix_direnv` | boolean | `true` | Nix shell (`IN_NIX_SHELL`) and Direnv (`.envrc`) state. |
| `health` | boolean | `false` | Opt-in CPU load pressure and low-battery indicators. On macOS this may invoke `pmset` under a 250 ms background budget. |

Infrastructure detection runs in background workers with a 10 ms budget per
detector. A detector that times out simply contributes no badge.

## `[directory]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `truncation_length` | integer | `3` | Maximum folder depth to display before truncating. |
| `truncate_to_repo` | boolean | `true` | When inside a Git repository, hide path components outside the repository root. |

The left path is always pure text. `[directory].icon` is no longer rendered;
older configurations containing the key still parse and the key is ignored.

## `[git]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `branch_icon` | string | `""` | Prefix for the Git branch. Empty uses the icon profile. |
| `staged_icon` | string | `"+"` | Symbol for staged changes. |
| `unstaged_icon` | string | `"!"` | Symbol for unstaged changes. |
| `untracked_icon` | string | `"?"` | Symbol for untracked files. |

## `[status]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `success_icon` | string | `""` | Cursor on command success. Empty uses the icon profile. |
| `failure_icon` | string | `""` | Cursor on command failure. Empty uses the icon profile. |

## `[metadata]`

| Key | Type | Default | Description |
| --- | ---- | ------- | ----------- |
| `time_color` | string | `""` | ANSI SGR code for the clock and timing. Empty uses the theme. |
| `paneship_color` | string | `""` | ANSI SGR code for the project version. Empty uses the theme. |
| `languages` | table | `{}` | Per-language `icon` and `color` overrides. |

### Per-language overrides

```toml
[metadata.languages.go]
icon = "󰟓"
color = "1;36"

[metadata.languages.python]
icon = ""
color = "1;33"
```

An empty `icon` or `color` falls back to the active profile/theme. The key is
the detector's canonical language name, for example `rust`, `node`, `go`,
`python`, `ruby`, `php`, `java`, `bun`, or `deno`.

## Colour codes

Paneship uses standard ANSI SGR parameter codes. Values may be a single code or
a semicolon-separated list:

| Code | Effect |
| ---- | ------ |
| `1` | Bold |
| `2` | Dim / faint |
| `4` | Underline |
| `30`-`37` | Standard foreground colours (red 31, green 32, yellow 33, blue 34, magenta 35, cyan 36, white 37) |
| `1;33` | Bold yellow |
| `1;38;5;208` | 256-colour orange foreground |

## Environment variables

| Variable | Default | Description |
| -------- | ------- | ----------- |
| `PANESHIP_CACHE_CAPACITY` | `500` | Entries per LRU cache. Must be greater than zero. |
| `PANESHIP_GIT_TTL_MS` | `5000` | Git state TTL in milliseconds. |
| `PANESHIP_LANG_TTL_MS` | `60000` | Language metadata TTL in milliseconds. |
| `PANESHIP_PACKAGE_TTL_MS` | `120000` | Package version TTL in milliseconds. |
| `PANESHIP_ENV_TTL_MS` | `30000` | Environment metadata TTL in milliseconds. |
| `PANESHIP_LAST_CMD_DURATION_MS` | unset | Fallback command duration when the shell does not pass `--duration-ms`. |
| `PANESHIP_DAEMON_DEBUG` | unset | When set, daemon log lines are also written to stderr. |
| `PANESHIP_DAEMON` | internal | Set by the daemon process; do not set it yourself. |
| `PANESHIP_INSTALL_DIR` | `~/.local/bin` | Installer destination directory. |
| `PANESHIP_VERSION` | `latest` | Installer release tag. |
| `NO_COLOR` | unset | Disables installer colour output when set. |

TTLs and capacity are read by the daemon, so set them in the environment that
launches the daemon (typically your shell profile) and restart the daemon for
changes to apply. Invalid values fall back to the compiled defaults. A TTL of
`0` is valid and disables caching for that category; capacity must be greater
than zero.

## Precedence

When values overlap, the most specific source wins:

1. CLI flags for the current render (for example `--duration-ms`).
2. Explicit per-section values such as `git.branch_icon` or
   `metadata.languages.<key>.icon`.
3. The active icon profile and theme.
4. Built-in defaults.

`[directory].icon` is accepted for backward compatibility but has no effect.

## Reloading

- `paneship config` saves atomically and sends `ReloadConfig` to the daemon.
- `paneship daemon reload` does the same for manual edits.
- A reload re-reads `config.toml` and clears language, package, and environment
  metadata caches. Git state keeps its normal TTL and watcher behavior.
- If no daemon is running, the next `paneship render` process reads the new
  configuration directly.

## Related documentation

- [Troubleshooting](troubleshooting.md)
- [CLI reference](cli.md)
- [Architecture: caching](architecture.md#caching)
