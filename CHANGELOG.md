# Changelog

All notable changes to Paneship are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Standard open-source project files: Code of Conduct, Security Policy, issue
  and pull-request templates, Dependabot configuration, and a documentation
  index.
- In-depth documentation for architecture, configuration, the CLI, release
  management, and troubleshooting.

### Changed

- The README is now an overview with links to focused documentation, and the
  documented minimum supported Rust version matches the dependency graph.
- CI verifies rustdoc warnings, and release archives include the changelog.

## [1.2.1] - 2026-10-31

### Changed

- Consolidated the release workflow so crates.io publishing runs in the same
  job as the GitHub release, avoiding an extra runner.
- Standardized CI and release automation on GitHub-hosted Ubuntu runners.

### Documentation

- Documented the `[prompt]` and `[environment]` configuration sections.

## [1.2.0] - 2026-10-16

### Added

- Production-finalization work from Phase 4: packaging, release hardening, and
  robustness fixes across the render path.
- Background infrastructure and environment detectors for Kubernetes,
  containers, AWS/GCP/Azure profiles, Nix, Direnv, and opt-in system health.
- Per-category cache TTLs through `PANESHIP_LANG_TTL_MS`,
  `PANESHIP_PACKAGE_TTL_MS`, and `PANESHIP_ENV_TTL_MS`.
- Native, width-aware right prompts for Zsh, Fish, and Nushell.
- `paneship top` live daemon telemetry and the one-line `install.sh` installer.
- Cross-platform IPC with Unix domain sockets and hardened Windows named pipes.

### Fixed

- The benchmark harness no longer measures a `tmux` subprocess spawn instead of
  the render path.
- Stale repository-root resolutions are invalidated when a directory becomes or
  stops being a Git repository.
- Git and metadata segments render explicit placeholders instead of silently
  disappearing when a worker misses its budget.

## [1.1.6] - 2026-06-05

### Added

- Cargo workspace package-version resolution (`[workspace.package].version`).

## [1.1.5] - 2026-05-22

### Fixed

- Language detection logic.

### Changed

- Dependency and vendoring updates.

## [1.1.4] - 2026-05-12

### Changed

- Documentation and packaging updates.

## [1.1.3] - 2026-05-12

### Added

- Support for ten additional shell integrations.

## [1.1.2] - 2026-05-04

### Added

- `paneship top` command.
- New icon assets and a template configuration.

## [1.1.1] - 2026-04-30

### Changed

- Implemented stale-while-revalidate caching.

## [1.1.0] - 2026-04-29

### Changed

- Implemented asynchronous daemon-side rendering.

## [1.0.4] and earlier

See the [GitHub Releases](https://github.com/andev0x/paneship/releases) page for
the complete history of the 1.0.x and 0.x releases.

[Unreleased]: https://github.com/andev0x/paneship/compare/v1.2.1...HEAD
[1.2.1]: https://github.com/andev0x/paneship/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/andev0x/paneship/compare/v1.1.6...v1.2.0
[1.1.6]: https://github.com/andev0x/paneship/compare/v1.1.5...v1.1.6
[1.1.5]: https://github.com/andev0x/paneship/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/andev0x/paneship/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/andev0x/paneship/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/andev0x/paneship/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/andev0x/paneship/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/andev0x/paneship/compare/v1.0.4...v1.1.0
