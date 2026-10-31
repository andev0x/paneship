# Contributing to Paneship

Thank you for considering a contribution. Paneship is a small, performance-
sensitive Rust project, and every report, documentation fix, test, detector, or
shell integration helps.

By participating, you agree to follow our
[Code of Conduct](CODE_OF_CONDUCT.md). For security vulnerabilities, use the
[Security Policy](SECURITY.md) instead of a public issue.

## Table of contents

- [Ways to contribute](#ways-to-contribute)
- [Reporting bugs](#reporting-bugs)
- [Suggesting features](#suggesting-features)
- [Development setup](#development-setup)
- [Project structure](#project-structure)
- [Adding a language detector](#adding-a-language-detector)
- [Coding standards](#coding-standards)
- [Testing](#testing)
- [Documentation and changelog](#documentation-and-changelog)
- [Commit messages](#commit-messages)
- [Pull request process](#pull-request-process)
- [Release process](#release-process)

## Ways to contribute

- Report a reproducible bug with the
  [bug report template](https://github.com/andev0x/paneship/issues/new?template=bug_report.yml).
- Request a feature or describe a workflow that is slow or awkward.
- Improve documentation, examples, or error messages.
- Add or fix a shell integration.
- Add a language or infrastructure detector.
- Improve tests, benchmarks, or CI.
- Review pull requests and reproduce reported issues.

## Reporting bugs

Search [existing issues](https://github.com/andev0x/paneship/issues) first. A
useful report includes:

- A clear title and a minimal reproduction.
- Expected and actual behavior.
- Operating system, architecture, shell, terminal, and tmux version.
- Paneship version (`paneship --version`) and Rust version for source builds.
- Relevant configuration with secrets redacted.
- Daemon diagnostics:
  ```bash
  paneship daemon ping
  paneship top --once
  tail -n 50 ~/.cache/paneship/daemon.log
  ```

Do not include tokens, cloud credentials, Kubernetes context names, or other
sensitive environment details.

## Suggesting features

Open a feature request describing the problem before the implementation. Explain
who is affected, the desired behavior, alternatives considered, and whether you
can contribute. Large changes, new runtime dependencies, and wire-format changes
should be discussed in an issue before a pull request.

## Development setup

### Prerequisites

- **Rust 1.88 or newer.** Install it with [rustup](https://rustup.rs/).
- A Unix-like system for the full test suite and benchmarks. Windows builds are
  supported and compiled in release CI.
- Optional: `cargo-audit` for dependency checks and Starship for comparison
  benchmarks.

### Build and run

```bash
git clone https://github.com/andev0x/paneship.git
cd paneship
cargo build
./target/debug/paneship render
```

Run a shell integration without changing your configuration:

```bash
./target/debug/paneship init zsh
./target/debug/paneship render --shell zsh --layout split
```

Start the daemon in a second terminal when working on caching or IPC:

```bash
cargo run -- daemon
cargo run -- daemon ping
cargo run -- top
```

### Quality gate

```bash
make check
```

This runs formatting checks, Clippy with warnings denied, the test suite, and the
rustdoc build. The underlying commands are:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

Use `cargo fmt` to apply formatting.

### Benchmarks

```bash
make bench
# or
cargo run --release -- benchmark --iterations 300 --panes 4
```

For performance changes, include before/after numbers and the hardware used.
Keep the render path free of synchronous filesystem or process work; see
[Architecture](docs/architecture.md) for the design constraints.

## Project structure

```text
src/
  main.rs          CLI parsing, shell init scripts, onboarding
  core/            config, icons/themes, layout, prompt context, renderer
  modules/         directory, Git, status, metadata, package, environment
  detectors/       language/runtime registry and infrastructure detectors
  cache/           LRU caches, TTLs, repository-root resolution
  daemon/          async server, worker, Git watcher, telemetry, logging
  ipc/             Unix socket and Windows named-pipe transports
  tui/             `paneship config` editor and atomic persistence
  benchmark.rs     render benchmark and component micro-benchmark
  top.rs           live telemetry view and JSON snapshots
tests/             integration tests (daemon lifecycle)
docs/              user, architecture, release, and troubleshooting guides
```

Each module begins with a rustdoc comment explaining its invariant and the
performance constraint it must preserve. Please keep those comments accurate.

## Adding a language detector

1. Create `src/detectors/<language>.rs`.
2. Implement the `Detector` trait:
   - `name()` returns the canonical config/icon key.
   - `marker_files()` lists marker files in priority order.
   - `detect()` returns `RuntimeInfo` with an optional version and the manifest
     that produced it.
3. Use `src/detectors/manifest.rs` for bounded TOML/JSON reads and version
   cleanup. Do not spawn `node -v`, `python --version`, or similar commands.
4. Register the detector in `build_registry()` in `src/detectors/mod.rs`.
5. Add its glyph to `nerdfont`, `emoji`, and `minimal` profiles in
   `src/core/icons.rs` when appropriate.
6. Add unit tests for marker discovery and version parsing.
7. Update the feature list and, if user-visible, the changelog.

Infrastructure detectors live in `src/detectors/infra/` and must stay
best-effort: a timeout, I/O error, or panic must remove only that signal.

## Coding standards

- Follow standard Rust style; `rustfmt` is authoritative.
- Clippy must pass with `-D warnings` on all targets.
- Prefer clear, small functions and explicit error handling. Avoid panics on
  user input or filesystem state.
- Keep the hot render path synchronous and allocation-conscious. Do not create
  an async runtime in the foreground client.
- Avoid new runtime dependencies unless the benefit is clear and discussed in an
  issue. The dependency surface is part of the performance and security budget.
- `unsafe` is allowed only when necessary for platform APIs or measured
  performance. Include a `// SAFETY:` comment explaining the invariants.
- Cross-platform behavior matters: changes to IPC, cache paths, or watcher logic
  must keep Unix and Windows in mind.
- User-visible errors should be actionable and should not leak secrets.

## Testing

- Add tests for bug fixes and behavior changes.
- Unit tests live beside the code; integration tests live in `tests/`.
- The daemon lifecycle test may start a process and use temporary paths. Avoid
  relying on your personal configuration or a running daemon.
- Tests must not modify the developer's real `~/.config/paneship` or
  `~/.cache/paneship`.
- For rendering changes, include a before/after capture or a focused test.
- For performance changes, include benchmark numbers and explain the trade-off.

Run the suite with:

```bash
cargo test --all-targets --locked
```

## Documentation and changelog

- Update the README or the relevant guide in `docs/` when behavior or
  configuration changes.
- Keep the CLI reference, configuration reference, and rustdoc comments in sync
  with the code.
- Add an entry under `Unreleased` in [CHANGELOG.md](CHANGELOG.md) for
  user-visible changes.
- Documentation-only pull requests are welcome and reviewed like code.

## Commit messages

Use the present tense and imperative mood. [Conventional Commits](https://www.conventionalcommits.org/)
prefixes are encouraged and match the existing history:

```text
feat(detectors): add elixir version detection
fix(cache): invalidate repository roots after git init
docs(config): document per-language overrides
perf(render): avoid an allocation in the width allocator
ci(release): include the changelog in archives
```

Reference issues and pull requests in the body when useful. Keep commits focused
and avoid mixing unrelated refactors.

## Pull request process

1. Fork the repository and create a branch from `main`.
2. Make a focused change with tests and documentation.
3. Run `make check`.
4. Fill in the pull-request template, including the test evidence.
5. Keep the pull request description up to date if review changes the scope.
6. Address review comments with new commits; do not force-push over review
   history unless asked.

A maintainer will review for correctness, performance, cross-platform impact,
and consistency with the existing architecture. CI must pass before merge.

## Release process

Releases are automated from version tags. The maintainer checklist, artifact
matrix, and crates.io publishing steps are documented in
[docs/releasing.md](docs/releasing.md).

## Questions

If you are unsure where a change belongs, open a draft pull request or an issue
and ask. Small, focused questions are easier to answer than a complete
implementation that needs to be redesigned.

Thank you for contributing.
