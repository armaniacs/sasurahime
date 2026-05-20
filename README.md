# sasurahime

A macOS developer cache cleaner written in Rust.

sasurahime scans well-known cache locations — uv, Homebrew, mise runtimes, Playwright/Puppeteer browsers, bun, Go, pip, and more — shows you what's taking up space, and lets you choose what to remove. Safely.

```
$ sasurahime scan

Category             Size       Status
────────────────────────────────────────
uv (archive)         18.2 GB    pruneable
Homebrew downloads   16.6 GB    stale
bun cache             5.5 GB    clearable
mise / node (old)     3.4 GB    unused
Playwright (old)      0.5 GB    stale
────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **Note on sizes**: The numbers above are from one developer's machine.
> Your environment will differ — these are examples, not guarantees.

## Usage

```bash
# Scan and report (no deletion)
sasurahime scan

# List all supported clean targets
sasurahime targets

# Show version
sasurahime --version

# Clean everything interactively
sasurahime

# Clean specific targets
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs

# Preview without deleting
sasurahime clean uv --dry-run

# Non-interactive (CI / scripting)
sasurahime --yes

# Permanently delete (bypass Trash)
sasurahime clean uv --permanent
```

## Safety first

- Every cleaner supports `--dry-run` — nothing is deleted until you confirm.
- **Trash mode is on by default** — removed files go to macOS Trash, so you can
  restore them from Finder. Use `--permanent` to permanently delete.
- mise runtime removal checks both global and per-project `.mise.toml` before deleting.
- macOS immutable flags (`uchg`) are handled automatically.

## Name

*sasurahime* is named after **速佐須良比売（Hayasasurahi-me）**, a goddess who appears in the *Oharae-no-Kotoba* (Great Purification Words) of Shinto tradition.

In Japanese mythology, impurities swept from the world are carried downstream — from river to sea, and finally to the depths of the underworld (*Ne-no-Kuni*). It is Hayasasurahi-me who receives them at the very end and makes them vanish without a trace.

She is, in other words, the ultimate destructor.

The name was chosen from the world of the *Kojiki* and *Nihon Shoki* for its role — purging accumulated impurities (stale caches, orphaned files) completely and finally — and for how cleanly it reads as a command name in a terminal.

## License

MIT

## Contributing

Issues and PRs are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

For feature proposals, please open an issue first so we can discuss the scope before diving into code.
