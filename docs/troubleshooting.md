# Troubleshooting

Start with these checks:

```bash
paneship --version
paneship daemon ping          # prints "ok" when the daemon answers
paneship render               # preview the prompt outside your shell hook
paneship top --once           # daemon health and cache statistics
```

Daemon warnings and panics are appended to `~/.cache/paneship/daemon.log`. Set
`PANESHIP_DAEMON_DEBUG=1` before starting the daemon to also echo them to
stderr:

```bash
PANESHIP_DAEMON_DEBUG=1 paneship daemon
```

## Icons show as boxes, question marks, or garbled glyphs

The default `nerdfont` profile requires a patched Nerd Font (for example
FiraCode Nerd Font) selected in your terminal. If you cannot install one, switch
profiles:

```toml
[icons]
profile = "minimal"   # pure ASCII
# or
profile = "emoji"     # explicit Unicode emoji opt-in
```

```bash
paneship config       # switch profiles with a live preview
```

Also check that the terminal itself is using the font you installed; many
terminals require a separate font setting per profile.

## The prompt is missing, empty, or falls back to a basic prompt

1. Confirm the shell integration was evaluated:
   ```bash
   paneship init zsh          # compare with your shell config
   ```
2. If you used onboarding, check for the marker block in your shell file:
   ```text
   # >>> paneship initialize >>>
   ```
3. Run `paneship render --shell zsh` and inspect the output.
4. Ensure `paneship` is on `PATH` in the same environment as the shell. The init
   scripts return quietly when the binary is not found.
5. Reload the shell configuration or open a new terminal.

The Zsh and Fish hooks fall back to a minimal prompt if rendering returns an
empty result or a malformed region payload, so a broken daemon does not leave
you without a prompt. Bash, PowerShell, and Nushell print whatever the render
command returns; check the daemon if that output is empty.

## The prompt is not showing Git updates

Check the daemon first:

```bash
paneship daemon ping
```

Then force a refresh for the current repository:

```bash
paneship invalidate --cwd "$PWD"
```

Notes:

- The watcher starts for repositories the daemon has rendered. A brand-new
  repository may need one prompt (or `invalidate`) to register.
- The explicit invalidation hook fires for commands that begin with `git `.
  Shell aliases or wrappers that do not match that prefix rely on the watcher.
- `git push` and `git fetch` update remote-tracking refs; if the prompt still
  lags, run `paneship invalidate` and check the daemon log.
- A `[git...]` placeholder means a refresh is in flight or the repository did
  not finish within the 30 ms first-render budget. It should resolve on the next
  prompt. Persistent placeholders usually indicate a very large repository or a
  worker failure logged in `~/.cache/paneship/daemon.log`.

## A language or environment badge is missing

- Language detection is manifest-first and walks upward from the working
  directory. It stops at the repository root and at your home directory, so a
  stray `~/package.json` does not label unrelated folders.
- `[lang...]` means the first manifest parse is still running. It should resolve
  on the next prompt.
- Check that the relevant detector is not disabled under `[environment]`.
- Infrastructure badges are best-effort and capped at 10 ms per detector. A slow
  network mount or oversized `~/.kube/config` can cause one badge to be omitted
  without affecting the rest of the prompt.

## The right prompt (RPROMPT) is missing

- Confirm `prompt.rprompt = true`.
- The terminal must be at least `prompt.rprompt_min_width` columns wide
  (default 20).
- Native right prompts are bound by the Zsh, Fish, and Nushell init scripts.
  Bash, PowerShell, and other shells use the combined single-line layout.
- In a narrow tmux pane, low-priority elements collapse first by design. Widen
  the pane to see timings and the clock.
- Check that your shell script uses `--layout split`:
  ```bash
  paneship render --shell zsh --layout split | cat -v
  ```
  The output should contain a `^_` (unit separator) between the left and right
  regions.

## The prompt wraps or the RPROMPT overlaps

The width passed by the shell hook comes from `$COLUMNS`, the terminal size, or
a `tmux` fallback. If it is stale after a resize:

```bash
paneship render --width "$(tput cols)"
```

Increase `prompt.rprompt_min_width` if you prefer dropping the right region
earlier, or set `prompt.rprompt = false` to disable it entirely.

## The daemon will not start or ping fails

```bash
paneship daemon ping
tail -n 50 ~/.cache/paneship/daemon.log
```

- Another daemon may already be listening. Only one daemon per user is needed;
  a second one exits cleanly.
- A crashed daemon can leave a socket file behind. The daemon removes a stale
  Unix socket automatically on startup. If you need to inspect it first:
  - `$XDG_RUNTIME_DIR/paneship.sock` when `XDG_RUNTIME_DIR` is set
  - otherwise `~/.cache/paneship/paneship.sock`
  - Windows: `\\.\pipe\paneship-<user>`
- To restart, find the process and stop it with your process manager:
  ```bash
  pgrep -af paneship
  # kill <pid> of the daemon, then let the shell hook start it again
  ```
  On Windows, use `Get-Process paneship` and `Stop-Process`.
- The daemon retires itself within about two seconds when its executable changes
  on disk. If an upgrade seems stuck, restart the daemon once.

## Configuration changes have no effect

- The daemon caches configuration. After a manual edit, run:
  ```bash
  paneship daemon reload
  ```
  `paneship config` does this automatically after `Ctrl+S`.
- If no daemon is running, the next render reads the file directly.
- Invalid TOML silently falls back to defaults. Validate the file with your
  editor or a TOML linter, and check for typos in section or key names.
- Unknown icon profiles and themes fall back to `nerdfont` and `default`.
- `[directory].icon` is accepted but intentionally ignored; the left path is
  pure text.

## The prompt feels slow

- Confirm the daemon is running (`paneship daemon ping`). Without it, each
  prompt pays cold-start Git and manifest costs.
- The first render for a repository can take longer while the cache is cold.
  Subsequent renders should be much faster.
- Run the benchmark:
  ```bash
  paneship benchmark
  paneship top --once
  ```
- Increase cache TTLs only if your workflow tolerates older state:
  ```bash
  PANESHIP_GIT_TTL_MS=10000
  PANESHIP_LANG_TTL_MS=120000
  ```
  Set these in the environment that starts the daemon.
- A very large monorepo may exceed the 30 ms first-render Git budget. Paneship
  shows `[git...]` and resolves in the background rather than blocking.

## WSL2 and containers

- WSL2 uses the Unix socket transport. Ensure `XDG_RUNTIME_DIR` points to a
  writable runtime directory; otherwise Paneship falls back to
  `~/.cache/paneship`.
- Container badges detect common markers (`.dockerenv`, cgroups, `$container`).
  If your runtime uses a different marker, a badge may be absent by design.
- Kubernetes detection reads `$KUBECONFIG` or `~/.kube/config` and refuses
  configuration files larger than 256 KiB.

## Windows

Paneship builds and ships a Windows binary and uses a user-scoped named pipe.
Windows runtime coverage is verified by CI builds and community reports rather
than a dedicated Windows runner. If you hit a Windows-specific issue, include
the exact Windows version, shell, and terminal in the bug report.

## Reset or uninstall

To reset configuration and cached state:

```bash
rm -f ~/.config/paneship/config.toml
rm -rf ~/.cache/paneship
```

To uninstall:

1. Remove the paneship block from your shell configuration file (the
   `# >>> paneship initialize >>>` block), or remove the manual `eval` line.
2. Remove the binary from your install directory (for example
   `~/.local/bin/paneship` or Cargo's bin directory).
3. Optionally remove `~/.config/paneship` and `~/.cache/paneship`.

## Still stuck?

Open an issue with the
[bug report template](https://github.com/andev0x/paneship/issues/new?template=bug_report.yml)
and include the diagnostics listed there. For security problems, use the
[security policy](../SECURITY.md) instead of a public issue.
