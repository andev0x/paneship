# Paneship Phase 4 — Production Finalization, Robustness Audit & Release Hardening

**Report date:** 2026-10-16
**Component:** `paneship` (Rust shell prompt) — v1.1.6 → **v1.2.0**
**Base commit:** `ed143a0` (refactor(core): IPC, detectors & TUI)
**Branch:** `phase-4/finalize`
**Scope:** (1) production packaging & release hardening, (2) robustness/bug audit of the critical paths, (3) Phase 4 roadmap features, (4) verification and reporting.
**Status:** ✅ Complete and green on macOS (aarch64). Windows remains **compile-verified in CI only** (see §7).

---

## 1. Executive summary

Phase 4 turns the Phase 3 feature set into a shippable release and fixes a
handful of genuine defects found by auditing the hot path, the cache and the
release pipeline.

1. **A real performance bug was found and fixed.** The `benchmark` harness
   rendered with `width: None`, which made `PromptContext::from_inputs` shell
   out to `tmux display-message` on *every* iteration. The benchmark therefore
   measured a subprocess spawn (7.8 ms) instead of the render path. With the
   harness pinned to a fixed width — and with width resolution reordered to use
   the cheap `terminal_size` `ioctl` before the `tmux` fallback — the same
   benchmark now reports **~0.34 ms**.
2. **A stale-cache correctness bug was fixed.** Repository-root resolutions were
   cached forever, including *negatives*, so `git init` in a tracked directory
   was never noticed. The post-`git` `INVALIDATE_CWD` signal now evicts the
   repository-root cache for the path and its ancestors.
3. **The release pipeline was broken for multi-platform assets.** All three OS
   jobs uploaded a file literally named `paneship`, so the last job clobbered
   the others; the Windows storefront job also pointed at a path that never
   exists (`paneship.exe`). Releases now publish uniquely named
   `paneship-<target>.tar.gz` / `.zip` archives, which the new `install.sh`
   consumes.
4. **New capabilities:** a one-line installer, a `--version` command, a native
   Nushell RPROMPT binding, per-category cache TTLs, a rolling-rate view in
   `paneship top`, and macOS battery health via `pmset`.
5. **Hardening:** release profile tuning (fat LTO, single codegen unit, symbol
   stripping) and a cross-platform CI quality gate (`fmt`, `clippy
   --all-targets`, `test --all-targets`, release build).

Verification: **100 unit tests + 1 integration test pass**, `cargo clippy
--all-targets -- -D warnings`, `cargo fmt -- --check`, `cargo build --release
--locked` and `install.sh --from-source` are all clean.

---

## 2. Deliverables checklist

| # | Deliverable | Status | Location |
|---|-------------|--------|----------|
| P.1 | One-line cross-platform installer | ✅ | `install.sh` |
| P.2 | Release profile tuning (LTO/codegen-units/strip) | ✅ | `Cargo.toml` |
| P.3 | CI quality gate across OS matrix | ✅ | `.github/workflows/ci.yml` |
| P.4 | Fixed multi-platform release assets | ✅ | `.github/workflows/release.yml` |
| P.5 | Fixed Windows artifact path in build workflow | ✅ | `.github/workflows/build.yml` |
| P.6 | `--version` / `paneship version` CLI | ✅ | `src/main.rs` |
| R.1 | Benchmark no longer times tmux subprocess | ✅ | `src/benchmark.rs` |
| R.2 | Width resolution prefers cheap ioctl before tmux | ✅ | `src/core/prompt.rs` |
| R.3 | Repository-root cache invalidation on `INVALIDATE_CWD` | ✅ | `src/cache/mod.rs`, `src/daemon/mod.rs` |
| R.4 | Benchmark worker panic handled without aborting | ✅ | `src/benchmark.rs` |
| F.1 | Native Nushell RPROMPT (`PROMPT_COMMAND_RIGHT`) | ✅ | `src/main.rs` |
| F.2 | Per-category cache TTLs (`PANESHIP_*_TTL_MS`) | ✅ | `src/cache/mod.rs` |
| F.3 | `paneship top` rolling rate (8-sample window) | ✅ | `src/top.rs` |
| F.4 | macOS battery health via `pmset`, looser budget | ✅ | `src/detectors/infra/{health,mod}.rs` |
| T.1 | Unit tests for every new unit | ✅ | see §6 |
| D.1 | README / example config updated | ✅ | `README.md`, `example_config.toml` |

---

## 3. Module P — Production packaging & release hardening

### 3.1 Installer (`install.sh`)

A dependency-free `bash` installer that:

1. Detects the target triple (`uname -s`/`uname -m` → e.g. `aarch64-apple-darwin`).
2. Downloads `paneship-<target>.tar.gz` from the matching GitHub Release
   (`/releases/latest/download/...` or `/releases/download/<tag>/...`) using
   `curl` or `wget`.
3. Extracts, installs to the destination and smoke-checks the binary with
   `paneship --version`.
4. **Falls back to `cargo install --locked`** when no matching artifact exists,
   the download fails, or `--from-source` is passed; the produced binary is
   mirrored into the requested `--dir` when it differs from Cargo's bin dir.

Flags: `--version <tag>`, `--dir <path>`, `--from-source`, `--no-modify-path`,
`--help`. Env: `PANESHIP_INSTALL_DIR`, `PANESHIP_VERSION`, `NO_COLOR`.

### 3.2 Release profile (`Cargo.toml`)

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = "symbols"
panic = "unwind"
```

Full LTO and a single codegen unit maximise cross-crate inlining through `gix`.
`panic` is **deliberately left as `unwind`**: the daemon worker and the detector
harness recover from panics with `catch_unwind`, so a single bad repository or
detector must not abort the whole daemon. This is documented inline.

### 3.3 CI/CD

| Workflow | Change |
|----------|--------|
| `ci.yml` | Now an OS matrix (ubuntu/macos/windows) running `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo build --release --locked`. The previous clippy step ran without `--all-targets`, so test/bench targets were never linted. |
| `build.yml` | Fixed the artifact path (`target/release/paneship*`) — on Windows the previous `target/release/paneship` never matched `paneship.exe` and the upload would fail. Added a `--version` smoke test and `if-no-files-found: error`. |
| `release.yml` | **Fixed asset collisions.** Releases now publish `paneship-<target>.tar.gz`/`.zip` for `x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`; the installer consumes exactly these names. |

---

## 4. Module R — Robustness & bug audit

### 4.1 The benchmark measured `tmux`, not Paneship (critical)

`benchmark::run` built each `PromptContext` with `width: opt.width`, which
defaults to `None`. `PromptContext::from_inputs` then called
`crate::tmux::get_pane_width()`, which spawns `tmux display-message -p
'#{pane_width}'` — a process spawn — once per iteration. Inside tmux the
headline number was:

```
Benchmark Report
Paneship avg:  7.821395ms          # width = None  → tmux subprocess per render
Paneship avg:  334.682µs           # --width 120   → actual render path
```

Fix: the harness resolves a fixed width **once** and reuses it, so it measures
rendering only. Additionally, `from_inputs` now prefers `terminal_size()`
(a single `ioctl`, which inside a tmux pane already returns the pane width)
and only falls back to the `tmux` subprocess when the terminal size is
genuinely unavailable. Default benchmark after the fix: **344.859µs**.

### 4.2 Stale repository-root cache (correctness)

`repo_root_for` cached *negative* results (`Some(None)`) permanently for
performance. Combined with `INVALIDATE_CWD` only clearing the Git cache, a
directory that became a repository after the daemon started kept rendering as
"not a repository" until LRU eviction. Fix: `cache::invalidate_repo_root(path)`
evicts `path` and every ancestor, called by the daemon's `InvalidateCwd`
handler *before* re-resolving — so `git init` (a `git `-prefixed command) is
reflected on the very next prompt. Covered by
`repo_root_invalidation_rediscovers_new_repository`.

### 4.3 Panic-safety in the benchmark harness

`handles.into_iter().map(|h| h.join().unwrap())` would panic the benchmark on a
worker panic; `current_dir().unwrap()` could panic on a removed CWD. Both are
now graceful (`Err`/`unwrap_or_default`).

### 4.4 Audited and found sound

The IPC framing (`read_frame` caps at `MAX_FRAME` = 4 MiB), the cache poison
handling (`unwrap_or_else(into_inner)`), the watcher debounce/coalescing, the
bounded detector harness, and the render-path width arithmetic (`saturating_sub`
throughout) were reviewed and are correct.

---

## 5. Module F — Phase 4 features

### 5.1 Native Nushell RPROMPT

`nushell_init_script` now requests `--layout split`, splits the `\u{1f}`-framed
response, stashes the right region in `$env.PANESHIP_RPROMPT`, and binds
`$env.PROMPT_COMMAND_RIGHT`. This brings Nushell to parity with Zsh and Fish.

### 5.2 Per-category cache TTLs

Metadata TTLs are resolved through a pure, tested `parse_ttl` helper and can be
overridden per category:

| Variable | Default | Applies to |
|----------|---------|------------|
| `PANESHIP_GIT_TTL_MS` | 5000 | Git state |
| `PANESHIP_LANG_TTL_MS` | 60000 | language metadata |
| `PANESHIP_PACKAGE_TTL_MS` | 120000 | package version |
| `PANESHIP_ENV_TTL_MS` | 30000 | infrastructure/environment |

Slow-changing signals can now be cached far longer than fast-moving Git state.

### 5.3 `paneship top` rolling rate

The live view keeps an 8-sample window of counter deltas (`compute_rates`) and
displays a rolling average:

```
  rate (8s avg)     12.5 renders/s · 1.0 git/s · 3.0 events/s
```

Deltas use `saturating_sub`, so a daemon restart or clock adjustment clamps a
rate to zero instead of producing a negative value.

### 5.4 macOS battery health

The opt-in health detector now reads battery state on macOS via `pmset -g batt`,
parsed by a pure, platform-independent `parse_pmset_battery`. Because `pmset`
is a subprocess, health runs under a looser `HEALTH_TIMEOUT` (250 ms) than the
10 ms file-probe budget — health is off by default and always resolved in the
background, so it never touches the render path. Linux `/sys` reading is
unchanged.

---

## 6. Tests added (86 → 100 unit tests)

```
cache::tests::{ttl_override_parses_and_falls_back_to_default,
  repo_root_invalidation_rediscovers_new_repository}
detectors::infra::tests::bounded_with_timeout_allows_slower_work
detectors::infra::health::tests::{pmset_output_parses_capacity_and_status,
  pmset_without_battery_yields_nothing,
  pmset_capacity_feeds_status_only_when_discharging}
top::tests::{compute_rates_uses_counter_deltas_over_elapsed,
  compute_rates_clamps_on_counter_reset_and_zero_elapsed,
  rate_averages_window_rolls_and_means,
  report_includes_rolling_rate_when_present}
bin::tests::{version_flag_is_recognised_in_every_spelling,
  help_and_empty_args_are_handled, nushell_script_binds_native_right_prompt,
  zsh_and_fish_scripts_use_split_layout}
```

---

## 7. Verification log

| Command | Result |
|---------|--------|
| `cargo fmt -- --check` | ✅ clean |
| `cargo clippy --all-targets -- -D warnings` | ✅ clean |
| `cargo test --all-targets` | ✅ 100 unit + 1 integration, 0 failed |
| `cargo build --release --locked` | ✅ clean (45.6 s with fat LTO) |
| `./install.sh --from-source --dir <tmp>` | ✅ installed `paneship 1.2.0` |
| `./install.sh --help` / bad flag | ✅ help printed / exit 1 |
| `paneship benchmark` (default, 4 panes) | ✅ 344.859 µs |
| `paneship benchmark --width 120` | ✅ 334.682 µs |
| `paneship top --benchmark` | ✅ git 53.6 µs · metadata 51.5 µs · package 45.3 µs · directory 0.96 µs · status 0.10 µs |
| Live daemon smoke (ping, split render, invalidate, `top --once`) | ✅ |
| `cargo check --target x86_64-pc-windows-msvc` | ⚠️ not runnable here (see below) |

Live daemon-mode measurement (isolated `XDG_RUNTIME_DIR`):

```
--version        : 7.785 ms/run   (bare process spawn; no IPC, no render)
init zsh         : 8.304 ms/run   (no IPC)
render (daemon)  : 8.716 ms/run   → IPC + render delta ≈ 0.93 ms
```

The daemon render round trip is well inside the **2.5 ms** budget; the ~8 ms is
the fixed OS cost of `exec`+dyld for any process, paid by every prompt engine.

### Windows cross-verification

Not reproducible in this environment: the active `rustc` is Homebrew's
(1.98.1), which shadows rustup and cannot resolve the `x86_64-pc-windows-msvc`
`core` (`rustup`'s target std is installed but unused). CI (`ci.yml`,
`release.yml`) still cross-builds Windows on a native runner. This is the same
limitation recorded in Phase 3 §9.

---

## 8. Decisions, deviations & limitations

1. **`panic = "unwind"` retained deliberately** — `panic = "abort"` would make
   the daemon's `catch_unwind` recovery a process abort, violating the "one bad
   repository must not kill the daemon" guarantee.
2. **"Per-detector TTLs" implemented per metadata category**, not per infra
   detector: an `EnvironmentSnapshot` bundles all infra signals into one cache
   entry, so genuine per-detector TTLs would require splitting that cache. The
   category-level TTLs deliver the intent (independent freshness) at low risk.
3. **macOS battery uses `pmset`** rather than IOKit FFI: it avoids new unsafe
   FFI/dependencies, and the looser health budget keeps it off the hot path.
   A future IOKit-backed detector could drop the looser budget.
4. **Nushell script is string-tested, not runtime-executed** — no Nushell host
   was available; the generator asserts the split-layout request and
   `PROMPT_COMMAND_RIGHT` binding.
5. **Version bumped to 1.2.0** for the feature release.

---

## 9. Follow-ups (Phase 5 candidates)

- Runtime-verify Windows pipe ACL/backoff and the Windows release archive in CI.
- IOKit-native macOS battery (drop the 250 ms budget).
- Split the environment cache per detector for true per-detector TTLs.
- Native RPROMPT bindings for Powerline-compatible shells.
- Optional `serde_yaml` kubeconfig parser for multi-document configs.
