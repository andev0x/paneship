# Paneship — developer Makefile
#
# Wraps the common build/test/release workflows so contributors, CI and the
# maintainer all run the same commands. Run `make` (or `make help`) for the
# full target list.
#
# Overridable variables:
#   CARGO=...        cargo binary                 (default: cargo)
#   PREFIX=...       install prefix               (default: ~/.local)
#   INSTALL_DIR=...  binary destination           (default: $(PREFIX)/bin)
#   BENCH_ARGS=...   args for `make bench`        (default: --iterations 300 --panes 4)
#   DRY_RUN=1        only verify `make publish`   (default: unset)

CARGO       ?= cargo
BIN         ?= paneship
TARGET_DIR  ?= target
RELEASE_DIR := $(TARGET_DIR)/release
PREFIX      ?= $(HOME)/.local
INSTALL_DIR ?= $(PREFIX)/bin
BENCH_ARGS  ?= --iterations 300 --panes 4

export CARGO_TERM_COLOR = always

# Package version, read from Cargo.toml (single source of truth).
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

# Platform-tagged archive name, matching .github/workflows/release.yml.
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)
ifeq ($(UNAME_S),Linux)
  OS_PART := unknown-linux-gnu
else ifeq ($(UNAME_S),Darwin)
  OS_PART := apple-darwin
else
  OS_PART := unknown
endif
ifeq ($(UNAME_M),x86_64)
  ARCH_PART := x86_64
else ifeq ($(UNAME_M),arm64)
  ARCH_PART := aarch64
else
  ARCH_PART := $(UNAME_M)
endif
TARGET_TRIPLE ?= $(ARCH_PART)-$(OS_PART)
DIST_NAME     := $(BIN)-$(TARGET_TRIPLE)
DIST_DIR      := $(TARGET_DIR)/dist

.DEFAULT_GOAL := help

.PHONY: help all build release run check ci fmt fmt-check clippy test \
        bench bench-starship top config daemon ping invalidate install \
        install-release dist doc audit clean version publish publish-crate

# ---------------------------------------------------------------------------
# Meta
# ---------------------------------------------------------------------------

help: ## Show this help
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

version: ## Print the crate version
	@echo "$(BIN) $(VERSION)"

# ---------------------------------------------------------------------------
# Build & run
# ---------------------------------------------------------------------------

all: build ## Alias for build

build: ## Compile a debug binary
	$(CARGO) build

release: ## Compile an optimized release binary (locked)
	$(CARGO) build --release --locked

run: ## Run the prompt renderer (ARGS="--shell zsh --layout split")
	$(CARGO) run -- $(ARGS)

daemon: release ## Start the background daemon
	$(RELEASE_DIR)/$(BIN) daemon

ping: release ## Ping a running daemon
	$(RELEASE_DIR)/$(BIN) daemon ping

invalidate: release ## Signal the daemon that Git state changed (CWD=path)
	$(RELEASE_DIR)/$(BIN) invalidate --cwd "$(or $(CWD),$(CURDIR))"

config: release ## Open the interactive TUI configurator
	$(RELEASE_DIR)/$(BIN) config

# ---------------------------------------------------------------------------
# Quality gate
# ---------------------------------------------------------------------------

check: fmt-check clippy test ## Full gate: formatting, lints, tests
	@echo "✓ all quality gates passed"

ci: check ## Alias for check

fmt: ## Format the source tree in place
	$(CARGO) fmt

fmt-check: ## Verify formatting without modifying files
	$(CARGO) fmt -- --check

clippy: ## Lint all targets, warnings denied
	$(CARGO) clippy --all-targets -- -D warnings

test: ## Run the full test suite (all targets)
	$(CARGO) test --all-targets

audit: ## Check dependencies for known vulnerabilities
	$(CARGO) audit

# ---------------------------------------------------------------------------
# Performance
# ---------------------------------------------------------------------------

bench: release ## Run the render benchmark (BENCH_ARGS=...)
	$(RELEASE_DIR)/$(BIN) benchmark $(BENCH_ARGS)

bench-starship: release ## Benchmark against Starship
	$(RELEASE_DIR)/$(BIN) benchmark --compare-starship $(BENCH_ARGS)

top: release ## Live daemon telemetry (ARGS="--once" or "--json")
	$(RELEASE_DIR)/$(BIN) top $(ARGS)

# ---------------------------------------------------------------------------
# Install & package
# ---------------------------------------------------------------------------

install: ## Install from the local source tree into Cargo's bin dir
	$(CARGO) install --locked --path . --force

install-release: ## Install a published release via install.sh (INSTALL_DIR=...)
	./install.sh --dir "$(INSTALL_DIR)"

dist: release ## Package the release binary as $(DIST_NAME).tar.gz
	@mkdir -p "$(DIST_DIR)/$(DIST_NAME)"
	@cp "$(RELEASE_DIR)/$(BIN)" "$(DIST_DIR)/$(DIST_NAME)/$(BIN)"
	@tar -czf "$(DIST_DIR)/$(DIST_NAME).tar.gz" -C "$(DIST_DIR)" "$(DIST_NAME)"
	@echo "packaged $(DIST_DIR)/$(DIST_NAME).tar.gz"

doc: ## Build and open the API documentation
	$(CARGO) doc --no-deps --open

# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------

publish: check ## Publish the crate to crates.io (DRY_RUN=1 to only verify)
	@if [ -n "$$(git status --porcelain)" ]; then \
		echo "error: working tree is dirty; commit or stash changes first" >&2; \
		exit 1; \
	fi
	$(CARGO) publish --locked $(if $(DRY_RUN),--dry-run)

publish-crate: publish ## Alias for publish

clean: ## Remove build artifacts
	$(CARGO) clean
