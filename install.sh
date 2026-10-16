#!/usr/bin/env bash
#
# Paneship installer.
#
# Prefers a prebuilt release binary from GitHub Releases and transparently
# falls back to building from source with Cargo when no matching artifact is
# published (or when `--from-source` is passed).
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/andev0x/paneship/main/install.sh | bash
#   ./install.sh --from-source
#   ./install.sh --version v1.2.0 --dir "$HOME/.local/bin"
#
# Environment overrides:
#   PANESHIP_INSTALL_DIR   Destination directory for the binary.
#   PANESHIP_VERSION       Release tag to install (same as --version).
#
set -euo pipefail

REPO="andev0x/paneship"
BIN="paneship"
VERSION="${PANESHIP_VERSION:-latest}"
INSTALL_DIR="${PANESHIP_INSTALL_DIR:-}"
FROM_SOURCE=0

# ---------------------------------------------------------------------------
# Logging helpers
# ---------------------------------------------------------------------------
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_BLUE='\033[1;34m'; C_YELLOW='\033[1;33m'; C_RED='\033[1;31m'; C_RESET='\033[0m'
else
    C_BLUE=''; C_YELLOW=''; C_RED=''; C_RESET=''
fi

info()  { printf "${C_BLUE}==>${C_RESET} %s\n" "$*"; }
warn()  { printf "${C_YELLOW}warning:${C_RESET} %s\n" "$*" >&2; }
err()   { printf "${C_RED}error:${C_RESET} %s\n" "$*" >&2; exit 1; }

usage() {
    cat <<'EOF'
Paneship installer

USAGE:
    install.sh [OPTIONS]

OPTIONS:
    --version <tag>     Install a specific release tag (e.g. v1.2.0).
    --dir <path>        Install the binary into <path>.
    --from-source       Skip the prebuilt binary and build with Cargo.
    --no-modify-path    Do not print PATH setup instructions.
    -h, --help          Show this help.

ENVIRONMENT:
    PANESHIP_INSTALL_DIR   Same as --dir.
    PANESHIP_VERSION       Same as --version.
    NO_COLOR               Disable coloured output.
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
NO_MODIFY_PATH="${PANESHIP_NO_MODIFY_PATH:-0}"
PRINT_HELP=0

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            [ "$#" -ge 2 ] || err "missing value for --version"
            VERSION="$2"; shift 2 ;;
        --version=*) VERSION="${1#*=}"; shift ;;
        --dir)
            [ "$#" -ge 2 ] || err "missing value for --dir"
            INSTALL_DIR="$2"; shift 2 ;;
        --dir=*) INSTALL_DIR="${1#*=}"; shift ;;
        --from-source) FROM_SOURCE=1; shift ;;
        --no-modify-path) NO_MODIFY_PATH=1; shift ;;
        -h|--help) PRINT_HELP=1; shift ;;
        *) err "unknown argument: $1 (try --help)" ;;
    esac
done

if [ "$PRINT_HELP" -eq 1 ]; then
    usage
    exit 0
fi

# ---------------------------------------------------------------------------
# Tooling detection
# ---------------------------------------------------------------------------
DOWNLOADER=""
if command -v curl >/dev/null 2>&1; then
    DOWNLOADER="curl"
elif command -v wget >/dev/null 2>&1; then
    DOWNLOADER="wget"
fi

fetch() {
    # fetch <url> <destination>
    case "$DOWNLOADER" in
        curl) curl -fSL --retry 3 --connect-timeout 15 -o "$2" "$1" ;;
        wget) wget -q -T 15 -t 3 -O "$2" "$1" ;;
        *) return 1 ;;
    esac
}

detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)  os_part="unknown-linux-gnu" ;;
        Darwin) os_part="apple-darwin" ;;
        *) err "unsupported operating system: $os (re-run with --from-source)" ;;
    esac
    case "$arch" in
        x86_64|amd64) arch_part="x86_64" ;;
        arm64|aarch64) arch_part="aarch64" ;;
        *) err "unsupported architecture: $arch (re-run with --from-source)" ;;
    esac
    printf '%s-%s' "$arch_part" "$os_part"
}

# ---------------------------------------------------------------------------
# Destination selection
# ---------------------------------------------------------------------------
choose_install_dir() {
    if [ -n "$INSTALL_DIR" ]; then
        return
    fi
    if [ -n "${CARGO_HOME:-}" ] && [ -d "${CARGO_HOME}/bin" ]; then
        INSTALL_DIR="${CARGO_HOME}/bin"
    elif [ -d "${HOME}/.cargo/bin" ]; then
        INSTALL_DIR="${HOME}/.cargo/bin"
    else
        INSTALL_DIR="${HOME}/.local/bin"
    fi
}

path_hint() {
    [ "$NO_MODIFY_PATH" -eq 1 ] && return 0
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) return 0 ;;
    esac
    cat <<EOF

Paneship was installed to ${INSTALL_DIR}, which is not on your PATH.
Add it, for example:

    export PATH="${INSTALL_DIR}:\$PATH"

EOF
}

verify_binary() {
    local candidate="$1"
    [ -x "$candidate" ] || return 1
    "$candidate" --version >/dev/null 2>&1
}

install_from_release() {
    local url tmp archive target
    target="$(detect_target)"
    if [ "$VERSION" = "latest" ]; then
        url="https://github.com/${REPO}/releases/latest/download/${BIN}-${target}.tar.gz"
    else
        url="https://github.com/${REPO}/releases/download/${VERSION}/${BIN}-${target}.tar.gz"
    fi

    [ -n "$DOWNLOADER" ] || return 1
    command -v tar >/dev/null 2>&1 || return 1

    tmp="$(mktemp -d "${TMPDIR:-/tmp}/paneship.XXXXXX")"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmp'" EXIT
    archive="$tmp/${BIN}.tar.gz"

    info "Downloading ${url}"
    if ! fetch "$url" "$archive"; then
        rm -rf "$tmp"
        return 1
    fi

    if ! tar -xzf "$archive" -C "$tmp"; then
        rm -rf "$tmp"
        return 1
    fi

    local extracted
    extracted="$(find "$tmp" -type f -name "$BIN" -perm -u+x 2>/dev/null | head -n 1)"
    [ -n "$extracted" ] || extracted="$(find "$tmp" -type f -name "$BIN" 2>/dev/null | head -n 1)"
    if [ -z "$extracted" ]; then
        rm -rf "$tmp"
        return 1
    fi

    mkdir -p "$INSTALL_DIR"
    install -m 0755 "$extracted" "${INSTALL_DIR}/${BIN}" 2>/dev/null \
        || { cp "$extracted" "${INSTALL_DIR}/${BIN}" && chmod 0755 "${INSTALL_DIR}/${BIN}"; }
    rm -rf "$tmp"
    trap - EXIT
    return 0
}

install_from_source() {
    command -v cargo >/dev/null 2>&1 \
        || err "cargo is required for --from-source; install Rust from https://rustup.rs"

    info "Building and installing from source"
    if [ -f "Cargo.toml" ] && grep -q '^name = "paneship"' Cargo.toml 2>/dev/null; then
        cargo install --locked --path . --force
    elif [ "$VERSION" != "latest" ]; then
        cargo install --locked --force --version "${VERSION#v}" "$BIN"
    else
        cargo install --locked --force "$BIN"
    fi

    # `cargo install` always targets its own bin directory; mirror the binary
    # into the requested destination when they differ.
    local cargo_home cargo_bin
    cargo_home="${CARGO_HOME:-$HOME/.cargo}"
    cargo_bin="${cargo_home}/bin/${BIN}"
    if [ -x "$cargo_bin" ] && [ "$INSTALL_DIR" != "${cargo_home}/bin" ]; then
        mkdir -p "$INSTALL_DIR"
        cp "$cargo_bin" "${INSTALL_DIR}/${BIN}"
        chmod 0755 "${INSTALL_DIR}/${BIN}"
    fi
}

main() {
    if [ "${PANESHIP_FORCE_SOURCE:-0}" = "1" ]; then
        FROM_SOURCE=1
    fi

    choose_install_dir

    if [ "$FROM_SOURCE" -eq 0 ]; then
        if install_from_release; then
            if verify_binary "${INSTALL_DIR}/${BIN}"; then
                info "Installed $("${INSTALL_DIR}/${BIN}" --version) to ${INSTALL_DIR}"
                path_hint
                return 0
            fi
            warn "downloaded binary failed its smoke check; falling back to source"
        else
            warn "no prebuilt binary available for this platform/tag; falling back to source"
        fi
    fi

    install_from_source
    if verify_binary "${INSTALL_DIR}/${BIN}"; then
        info "Installed $("${INSTALL_DIR}/${BIN}" --version) to ${INSTALL_DIR}"
        path_hint
        return 0
    fi

    local resolved
    resolved="$(command -v "$BIN" 2>/dev/null || true)"
    if [ -n "$resolved" ] && verify_binary "$resolved"; then
        info "Installed $("$resolved" --version) to $resolved"
    else
        warn "installation finished but paneship was not found on PATH"
        path_hint
    fi
}

main "$@"
