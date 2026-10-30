# AGENTS.md

This file provides guidance to AI coding agents (Claude Code, Codex, Cursor, Gemini CLI, etc.) when working with code in this repository. `CLAUDE.md` is a symlink to this file so Claude Code keeps picking it up. Area-specific guidance lives in nested `AGENTS.md` files (see the map at the bottom).

## Project overview

sasurahime is a macOS developer cache cleaner written in Rust. See `README.md` for the user-facing description and `docs/SUPPORTED.md` for the supported tools.

## Commands

```bash
cargo build                  # build
cargo test                   # run all tests
cargo test <test_name>       # run a single test by name
cargo clippy --tests -- -D warnings  # lint (must pass with zero warnings)
cargo fmt --check            # check formatting
```

Target platform: macOS (arm64 + x86_64). Do not add Linux/Windows support unless explicitly requested.

## Safety rules

- mise runtime deletion **must** cross-check global `~/.config/mise/config.toml` AND any `.mise.toml` found within HOME (max depth 5) before removing a version.
- When deleting on macOS, handle `uchg` immutable flags: run `chflags -R nouchg <path>` before `rm -rf`.
- External tools (uv, brew, bun, go, pip) are invoked via `std::process::Command`. If the tool is not in PATH, return a `NotFound` status rather than erroring.

## Implementation workflow (subagent-driven)

Implementation must always be subagent-driven (サブエージェント駆動). The orchestrating session never writes PBI implementation code directly: it decomposes the work into independent tasks, dispatches one subagent per task, and reviews and integrates the results. When the executing environment offers a plan-execution mode or skill, always select the subagent-driven variant (e.g., `subagent-driven-development`).

## Where to look

| Topic | File |
|---|---|
| Cleaner trait, cleaners map, entry points | `src/AGENTS.md` |
| Testing strategy (Outside-In TDD, `CommandRunner` mock) | `tests/AGENTS.md` |
| Backlog, PBI process, branch/commit rules | `pbi/AGENTS.md`, `pbi/README.md` |
| User-facing docs | `README.md`, `docs/SUPPORTED.md`, `docs/HOWTO-ADD-target.md` |
