---
layout: default
title: sasurahime
---

# sasurahime

**macOS developer cache cleaner** — written in Rust.

Scan, choose, and wipe stale caches from uv, Homebrew, mise, Playwright, Docker, Cargo, Go, Rustup, Gradle, and 30+ other tools — all from a single terminal command.

```bash
cargo install sasurahime
```

Or download the [latest release](https://github.com/armaniacs/sasurahime/releases/latest).

## Quick start

```bash
# Interactive mode — scan all caches and choose what to clean
sasurahime

# Scan only (no deletion)
sasurahime scan

# Clean specific targets
sasurahime clean uv
sasurahime clean brew
sasurahime clean logs

# Preview without deleting
sasurahime clean uv --dry-run

# Non-interactive full clean
sasurahime --yes
```

## Features

- **40+ clean targets** — uv, brew, mise, browsers, cargo, go, pip, npm, yarn, pnpm, docker, colima, rustup, gradle, ollama, huggingface, xcode, device-support, simulator, and more.
- **Interactive TUI** — pick and choose with spacebar, see estimated freed space before confirming.
- **Safe by default** — deleted files go to macOS Trash (Finder-restorable). Use `--permanent` to bypass.
- **Dry-run support** — every clean command previews what would be removed.
- **Configuration file** — `~/.config/sasurahime/config.toml` for defaults.
- **mise pin protection** — versions listed in `~/.config/mise/config.toml` or `.mise.toml` are never deleted.
- **macOS immutable flag handling** — `chflags` removal is automatic.

## Supported targets

| Category | Targets |
|---|---|
| Language / Package managers | uv, brew, bun, go, pip, npm, yarn, pnpm, pipx, poetry, conda, cargo, rustup, deno, gradle, maven, spm, cocoa-pods, flutter |
| Runtimes / Tools | mise, browsers, node-gyp |
| Docker / VM | docker, colima, orbstack |
| IDE / Simulator | xcode, device-support, simulator, jetbrains, vscode-extensions |
| AI/ML / CI | huggingface, ollama, act, pre-commit |
| Logs / Other | logs, library-logs, downloads, trash |

Run `sasurahime targets` for the complete up-to-date list.

## Documentation

- [Usage guide](HOWTO-USE) — detailed command reference
- [Supported targets](SUPPORTED) — full list with descriptions
- [Adding a new target](HOWTO-ADD-target) — contributor guide
- [Changelog](CHANGELOG)

## Name

*sasurahime* is named after **速佐須良比売 (Hayasasurahi-me)**, a goddess from the *Oharae-no-Kotoba* (Great Purification Words) of Shinto tradition. She receives impurities carried downstream — from river to sea to the underworld — and makes them vanish without a trace.

In other words, the ultimate destructor of stale caches.

## License

MIT
