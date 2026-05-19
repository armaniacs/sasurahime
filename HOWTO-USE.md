# sasurahime — HOW TO USE

**sasurahime** is a macOS developer cache cleaner. It scans known cache
locations, reports disk usage, and selectively removes stale data.

---

## Installation

```bash
cargo install sasurahime
```

Requires Rust 1.70+ and macOS (arm64 or x86_64). No Linux/Windows support.

---

## Commands

### `sasurahime scan`

Scan every cache target, print a summary table, and exit without deleting anything.

```bash
$ sasurahime scan

Category               Size       Status
────────────────────────────────────────
uv (archive)          18.2 GB    pruneable
Homebrew downloads    16.6 GB    stale
bun cache              5.5 GB    clearable
mise / node (old)      3.4 GB    unused
────────────────────────────────────────
Total reclaimable     43.7 GB
```

### `sasurahime clean <target>`

Clean a single cache target. Replace `<target>` with one of the names below.

| Target          | What it removes                                                       |
|-----------------|-----------------------------------------------------------------------|
| `act`           | `~/.cache/act/` (GitHub Actions runner)                               |
| `brew`          | `brew cleanup -s --prune=all`                                         |
| `browsers`      | Old Puppeteer Chrome / Playwright (`ms-playwright*`) builds            |
| `bun`           | `bun pm cache rm`                                                     |
| `cargo`         | Cargo registry cache + `target/` directories                          |
| `cocoa-pods`    | `pod cache clean --all`                                               |
| `conda`         | `conda clean --all -y`                                                |
| `caches`        | All generic caches (bun, go, pip, node-gyp, npm, yarn, pnpm)         |
| `deno`          | `deno cache -r`                                                       |
| `docker`        | `docker system prune -f`                                              |
| `downloads`     | `~/Downloads` old files                                               |
| `go`            | `go clean -cache`                                                     |
| `gradle`        | Gradle old version caches                                             |
| `huggingface`   | Hugging Face model cache (`hub/`) via CLI or fallback                  |
| `jetbrains`     | JetBrains IDE caches (old versions)                                   |
| `library-logs`  | `~/Library/Logs/` — interactive heuristic scan (suggested cleanables) |
| `logs`          | Log files older than N days (see `--keep-days`)                       |
| `mise`          | Unused runtime versions under `~/.local/share/mise/installs/`          |
| `node-gyp`      | Deletes `~/.cache/node-gyp/`                                          |
| `npm`           | `npm cache clean --force`                                             |
| `orbstack`      | `orb prune`                                                           |
| `pip`           | `pip cache purge`                                                     |
| `pipx`          | `pipx cache purge`                                                    |
| `pnpm`          | `pnpm store prune`                                                    |
| `poetry`        | `poetry cache clear --all`                                            |
| `pre-commit`    | Pre-commit hook environment cache (via CLI or fallback)                |
| `rustup`        | Unused Rust toolchains                                                |
| `spm`           | SwiftPM cache directory                                               |
| `trash`         | `~/.Trash` size (scan only — use Finder to empty)                     |
| `uv`            | Stale `simple-vN` index dirs + `uv cache prune --force`               |
| `xcode`         | Xcode DerivedData project directories                                  |
| `yarn`          | `yarn cache clean`                                                    |

**Examples:**

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean logs
```

### `sasurahime` (no arguments)

Opens an interactive TUI with `dialoguer::MultiSelect`. You select which cache
targets to clean from a checkbox list.

Requires a TTY (terminal). In CI or scripting use `--yes` instead.

### `sasurahime --yes`

Non-interactive mode — cleans every pruneable target without prompting.

```bash
# Clean everything, no questions asked
sasurahime --yes
```

Ideal for cron jobs, CI pipelines, or maintenance scripts.

---

## Common Flags

### `--dry-run`

Preview what would be removed without actually deleting anything.

```bash
sasurahime clean uv --dry-run
sasurahime clean brew --dry-run
sasurahime clean logs --dry-run
```

Supported by every `clean` subcommand. Zero side effects guaranteed.

### `--all` (library-logs only)

Skip interactive prompt and delete all suggested entries.

```bash
sasurahime clean library-logs --all
```

Without `--all`, `library-logs` opens an interactive selection showing each
cleanable entry with reasons (`[large]`, `[stale N days]`).

### `--keep-days` (logs only)

Override the default retention period for log files.

```bash
# Keep logs newer than 14 days; delete everything older
sasurahime clean logs --keep-days 14
```

The default is 7 days (or the value from the config file).

---

## Configuration File

sasurahime reads `~/.config/sasurahime/config.toml` if it exists.
The file is optional — all defaults are sensible for day-to-day use.

### Example: change log retention to 30 days

```toml
[logs]
keep_days = 30
```

### Example: add extra log directories

```toml
[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

| Field          | Type            | Default  | Description                              |
|----------------|-----------------|----------|------------------------------------------|
| `keep_days`    | integer         | `7`      | Global log retention period              |
| `targets`      | array of tables | `[]`     | Extra log directories to scan            |
| `targets[].name` | string        | required | Display name                             |
| `targets[].path` | string        | required | Path (supports `~` expansion)            |
| `targets[].exclude` | string[]    | `[]`     | Filenames to never delete                |

---

## Safety

### `--dry-run` first

Every `clean` subcommand supports `--dry-run`. Make it a habit to preview
before deleting. The tool is tested on CI to guarantee zero side effects
when `--dry-run` is active.

### `.mise.toml` pinning

mise runtime deletion cross-checks both the global
`~/.config/mise/config.toml` and any `.mise.toml` files found under HOME
(max depth 5) before removing a version. If a version is pinned in any of
these files, it will never be removed.

### macOS immutable flags (`uchg`)

When a directory has the macOS immutable flag (`uchg`) set — common for
package managers and system caches — sasurahime automatically runs
`chflags -R nouchg` before deletion. This applies to all cleaners that
remove directories.

### Xcode running detection

If Xcode is currently running when you run `sasurahime clean xcode`,
you will be prompted to confirm before cleaning DerivedData. In `--yes`
mode, the prompt is bypassed and the check is skipped.

### `~/Library/Logs/` safety

The `library-logs` cleaner always excludes `CrashReporter` and
`DiagnosticReports` from scan results. Dot-files (`.DS_Store`, etc.) are
skipped. Entries that don't trigger any heuristic rule (size > 100 MB or
last modified > 90 days ago) are hidden.

---

## Exit Codes

| Code | Meaning                                  |
|------|------------------------------------------|
| 0    | Success (or nothing to clean)            |
| 1    | Config parse error / not a terminal      |

---

## Examples

```bash
# Quick overview of reclaimable space
sasurahime scan

# Clean brew cache (with preview first)
sasurahime clean brew --dry-run
sasurahime clean brew

# Clean all generic caches in one go
sasurahime clean caches

# Remove old browser builds
sasurahime clean browsers

# Clean logs older than 30 days
sasurahime clean logs --keep-days 30

# Interactive heuristic scan + selection for ~/Library/Logs/
sasurahime clean library-logs

# Bulk delete all suggested ~/Library/Logs/ entries (skip prompt)
sasurahime clean library-logs --all

# Full automation (cron)
sasurahime --yes

# Interactive pick-and-choose
sasurahime
```
