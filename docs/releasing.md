# Releasing Paneship

This guide is for maintainers. Releases are automated by
[`.github/workflows/release.yml`](../.github/workflows/release.yml), with a
small manual checklist before tagging.

## Versioning

Paneship follows [Semantic Versioning](https://semver.org/):

- **Major** for incompatible CLI, configuration, or wire-protocol changes.
- **Minor** for new user-facing features that remain backward compatible.
- **Patch** for fixes, documentation, and internal changes.

`Cargo.toml` is the single source of truth for the version. The release workflow
refuses to publish a tag that does not match it.

## Release checklist

1. **Start from a clean, tested `main`.**
   ```bash
   git switch main
   git pull --ff-only
   make check
   ```
2. **Bump the version** in `Cargo.toml`, then refresh `Cargo.lock`.
   ```bash
   # edit Cargo.toml: version = "<version>"
   cargo check
   ```
   Run `cargo check --locked` afterward to confirm the lock file is up to date.
3. **Update `CHANGELOG.md`.** Move entries from `Unreleased` into a dated
   release section and add the comparison links.
4. **Review user-facing documentation.** Check the README, configuration
   reference, and CLI docs for changes introduced since the last release.
5. **Run the full quality gate and a package dry run.** The working tree is
   still dirty at this point, so allow it for the local package check.
   ```bash
   make check
   cargo package --allow-dirty --locked
   cargo publish --dry-run --allow-dirty --locked
   ```
6. **Commit and tag.**
   ```bash
   git add -A
   git commit -m "chore(release): bump to <version>"
   git tag -a "v<version>" -m "paneship v<version>"
   git push origin main
   git push origin "v<version>"
   ```
7. **Watch the release workflow.** It verifies formatting, Clippy, tests, then
   builds the platform matrix, packages artifacts, generates checksums, publishes
   to crates.io when needed, and creates the GitHub release.

## What the workflow does

### Verify

The `verify` job runs on Ubuntu with stable Rust:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
```

### Build matrix

| Target | Method | Artifact |
| ------ | ------ | -------- |
| `x86_64-unknown-linux-gnu` | native | `tar.gz` |
| `x86_64-unknown-linux-musl` | cross | `tar.gz` |
| `aarch64-unknown-linux-gnu` | cross | `tar.gz` |
| `x86_64-apple-darwin` | `cargo-zigbuild` | `tar.gz` |
| `aarch64-apple-darwin` | `cargo-zigbuild` | `tar.gz` |
| `x86_64-pc-windows-gnu` | cross | `zip` |

macOS binaries are cross-compiled on Ubuntu with Zig and the kqueue notify
backend, so the workflow never allocates metered macOS or Windows runners. The
committed `.github/macos-stubs/libiconv.tbd` supplies the Darwin install name
that Zig's sysroot does not bundle.

### Publish

- The tag version must equal the `Cargo.toml` version.
- If the version already exists on crates.io, the crate publish step is skipped
  so a partial rerun can finish the GitHub release.
- Artifacts are uploaded with `SHA256SUMS.txt`.
- A GitHub release is created with generated release notes.
- Pushing a tag publishes automatically. A manual `workflow_dispatch` publishes
  only when a `tag` input is supplied; otherwise it stops after building
  artifacts.

The workflow uses `CARGO_REGISTRY_TOKEN` from repository secrets. The token must
be allowed to publish the `paneship` crate.

## Artifact contents

Archives contain:

- `paneship` (or `paneship.exe`)
- `README.md`
- `LICENSE`
- `CHANGELOG.md`

The one-line installer downloads the matching archive and verifies the expected
layout. Keep the archive naming stable (`paneship-<target>.tar.gz` / `.zip`).

## If a release fails

- **Tag does not match `Cargo.toml`:** delete the tag, fix the version, and
  re-tag. Never force-push a published version.
- **crates.io already has the version:** the workflow skips publishing and
  continues with the GitHub release. Do not bump the version just to retry.
- **A single target fails:** rerun the failed jobs after fixing the target.
  Successful targets are reused by the workflow run.
- **Wrong contents in a GitHub release:** edit the release assets. Published
  crates.io versions are immutable; yank the release if it is unsafe and publish
  a patch.

## Post-release

- Confirm the README badges point at the new version.
- Verify `cargo install paneship` resolves the new release after crates.io
  updates.
- Announce the release with the highlights from `CHANGELOG.md`.
- Keep `Unreleased` at the top of the changelog for the next cycle.

## Minimum supported Rust version

The MSRV is declared as `rust-version` in `Cargo.toml` and kept in sync with the
dependency graph. Raising it is a minor-version change and must be called out in
the changelog.

## Related documentation

- [Contributing](../CONTRIBUTING.md)
- [Architecture](architecture.md)
- [Security policy](../SECURITY.md)
