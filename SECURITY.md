# Security Policy

## Supported Versions

Paneship is distributed as a native binary and as a shell-prompt runtime. Only
the latest minor release line receives security fixes.

| Version | Supported |
| ------- | --------- |
| 1.2.x   | :white_check_mark: |
| 1.1.x and earlier | :x: |

Security fixes are released as a new patch version and announced in
[CHANGELOG.md](./CHANGELOG.md) and the GitHub release notes.

## Reporting a Vulnerability

**Do not open a public issue for a security problem.**

Please use GitHub's private vulnerability reporting:

<https://github.com/andev0x/paneship/security/advisories/new>

If private reporting is unavailable, contact the maintainer,
[@andev0x](https://github.com/andev0x), through a private channel on GitHub and
ask for a secure way to send the report.

A useful report includes:

- The affected version (`paneship --version`) and commit, if known
- Operating system, shell, terminal, and install method
- A description of the impact and the attacker model
- Reproduction steps or a proof of concept
- Relevant logs from `~/.cache/paneship/daemon.log` with secrets redacted

Please redact tokens, cloud credentials, Kubernetes context names, and other
environment details before sharing logs or configuration.

## Response Process

The maintainer aims to:

1. Acknowledge a report within **72 hours**.
2. Triage and confirm the impact within **7 days**.
3. Prepare and release a fix, or publish a mitigation, within **30 days** for
   confirmed vulnerabilities.

Timelines may vary with severity and complexity. Reporters are credited in the
release notes unless they ask to remain anonymous.

## Scope

In scope:

- The `paneship` binary and its shell integration scripts
- The background daemon, local IPC transport, and filesystem watcher
- The one-line installer (`install.sh`)
- Release artifacts and the release workflow in this repository

Out of scope:

- Vulnerabilities in third-party dependencies that have no exploitable impact
  on Paneship (report them upstream; dependency advisories are still welcome)
- Issues that require an already-compromised local account, shell, or
  `config.toml`
- Prompt rendering differences that do not cross a security boundary
- Social engineering, phishing, or physical access attacks

## Security Model

Paneship is a local developer tool. It does not listen on a network interface.

- **Unix-like systems:** the daemon listens on a Unix domain socket at
  `$XDG_RUNTIME_DIR/paneship.sock`, falling back to
  `~/.cache/paneship/paneship.sock` and then a per-user temporary path. Access
  is governed by the socket file permissions and the parent directory.
- **Windows:** the daemon uses a user-scoped named pipe,
  `\\.\pipe\paneship-<user>`, with a DACL limited to the current user and
  `PIPE_REJECT_REMOTE_CLIENTS`.
- **Protocol:** length-prefixed `bincode` frames with a 4 MiB maximum frame
  size. The client applies short read/write timeouts.
- **Filesystem access:** detection reads project manifests, Git metadata,
  `~/.kube/config`, and related local files. Files above the documented size
  limits are ignored.
- **No telemetry:** Paneship does not send usage data or prompt contents
  anywhere.

The daemon runs with the privileges of the user who started it. Running
`paneship daemon` as root is not supported and is not required.

## Dependency and Supply-Chain Hygiene

- `Cargo.lock` is committed for reproducible builds.
- Release artifacts are built from tagged commits and accompanied by
  `SHA256SUMS.txt`.
- Dependabot monitors Cargo and GitHub Actions dependencies.
- `make audit` runs `cargo audit` when the tool is installed.

## Disclosure Policy

Please give the maintainer a reasonable opportunity to release a fix before
public disclosure. Coordinated disclosure is appreciated, and the maintainer
will work with you on a publication date.
