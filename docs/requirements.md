

---

# Paneship

**Paneship** is a high-performance shell prompt designed for speed, responsiveness, and heavy workflows. It is optimized for tmux environments and large repositories, while remaining useful for all power users.

---

## Vision

* Build a fast, low-latency shell prompt optimized for tmux and large repositories
* Not limited to tmux: a general-purpose, power-user prompt (tmux-first)
* Compete on performance and smooth UX

---

## MVP (2–4 weeks)

### Goals

* Render under 5ms
* Zero noticeable lag in tmux

### Features

* Detect tmux via environment variables
* Correct pane width detection
* Basic modules:

  * current directory
  * git branch and status
  * exit code
* Git powered by Gitoxide
* Responsive truncation based on available width

### Architecture

* Simple Rust CLI binary
* Each module is a function returning a string
* Lightweight in-memory cache for git data (per path)

### Out of Scope (for MVP)

* TUI configuration
* Plugin system
* Scripting
* Complex async runtime

---

## Version 0.2 (tmux-focused)

* Background daemon process
* Communication via Unix sockets
* Shared git cache across panes
* Async rendering:

  * render fast parts immediately
  * update heavy parts later
* Debounced redraw to avoid flicker
* Incremental rendering (update only changed parts)

---

## Version 0.3 (adoption)

* Simple config file (TOML or YAML)
* Basic theme presets
* Shell support:

  * bash
  * zsh
  * fish
* Distribution:

  * cargo
  * Homebrew
  * Linux packages

---

## Version 0.4+

* Plugin system (Wasm)
* Priority-based modules
* Advanced truncation logic
* Theme system
* Optional:

  * TUI configuration
  * scripting support

---

## Suggested Project Structure

```
src/
  core/       (prompt, renderer)
  modules/    (git, dir, status)
  tmux/       (detect, layout)
  daemon/     (server, client)
  cache/      (git cache)
  main.rs
```

---

## Benchmark Requirements

* Render time (milliseconds)
* CPU usage with multiple tmux panes
* Performance on large repositories

Direct comparison against Starship is required.

---

## Positioning

> High-performance shell prompt for large repos and tmux workflows

Focus on:

* zero lag
* multi-pane efficiency

---

## Release Checklist

* One-command installation
* Works out of the box (no config required)
* No lag in tmux
* Clear demo (GIF or video)
* Published benchmark results

---

## Core Principles

* Performance over features
* Avoid premature optimization complexity
* Do not build plugin systems too early
* Always benchmark before optimizing

---

