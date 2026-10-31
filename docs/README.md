# Paneship Documentation

Start with the [README](../README.md) for installation and a quick start, then
use the focused guides below.

## User guide

| Guide | What it covers |
| ----- | -------------- |
| [README: Installation](../README.md#installation) | One-line installer, Cargo, and source builds |
| [README: Shell setup](../README.md#shell-setup) | Automatic onboarding and manual integration |
| [Configuration](configuration.md) | Full `config.toml` reference, profiles, themes, and environment variables |
| [CLI reference](cli.md) | Every command, flag, output shape, and exit code |
| [Troubleshooting](troubleshooting.md) | Icons, daemon failures, stale state, narrow panes, and logs |
| [Changelog](../CHANGELOG.md) | Release history and notable changes |

## Architecture and development

| Guide | What it covers |
| ----- | -------------- |
| [Architecture](architecture.md) | Process model, IPC, caching, invalidation, detectors, and rendering |
| [Contributing](../CONTRIBUTING.md) | Development setup, coding standards, tests, and pull-request process |
| [Releasing](releasing.md) | Versioning, release checklist, artifacts, and crates.io publishing |
| [Support](../SUPPORT.md) | Diagnostics, questions, and where to report issues |
| [Security policy](../SECURITY.md) | Supported versions, reporting, and the local security model |
| [Code of Conduct](../CODE_OF_CONDUCT.md) | Community expectations and enforcement |

## Historical reports

The [`docs/report/`](report/) directory contains the dated engineering reports
for the Phase 1-4 work that shaped Paneship 1.2. They are preserved as design
history, not current user documentation. If a report conflicts with the code or
the guides above, the guides win.

## Documentation conventions

- User-facing behavior is documented in the README or `docs/`.
- `src/` uses rustdoc comments for module-level design notes and public items.
- Every user-visible change should update [CHANGELOG.md](../CHANGELOG.md).
- Commands are tested on the platforms listed in the README; tell us when a
  guide is wrong by opening an issue.
