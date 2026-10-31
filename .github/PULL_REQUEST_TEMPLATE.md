## Summary

<!-- What does this pull request change, and why? -->

## Related issues

<!-- e.g. Closes #123, Relates to #456 -->

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Refactor or internal cleanup
- [ ] Documentation
- [ ] Build, CI, or release tooling
- [ ] Breaking change (describe below)

## How has this been tested?

<!--
Describe the commands you ran and the environments you covered.
Include benchmark numbers for performance-sensitive changes.
-->

```text
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo doc --no-deps --locked
```

## Screenshots or recordings

<!-- For prompt, TUI, or terminal behavior changes. Before/after captures help. -->

## Checklist

- [ ] I have read [CONTRIBUTING.md](../CONTRIBUTING.md).
- [ ] The change is focused and does not include unrelated refactors.
- [ ] I added or updated tests for behavior changes.
- [ ] I updated the documentation and `CHANGELOG.md` when user-visible behavior changed.
- [ ] Formatting, Clippy, tests, and rustdoc pass locally.
- [ ] I did not add a new runtime dependency without prior discussion.
- [ ] I have described any breaking change and its migration path.

## Breaking changes

<!-- Remove this section when not applicable. -->
