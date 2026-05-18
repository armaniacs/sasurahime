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

| Target      | What it removes                                           |
|-------------|-----------------------------------------------------------|
| `uv`        | Stale `simple-vN` index dirs + `uv cache prune --force`   |
| `brew`      | `brew cleanup -s --prune=all`                             |
| `mise`      | Unused runtime versions under `~/.local/share/mise/installs/` |
| `browsers`  | Old Puppeteer Chrome / Playwright (`ms-playwright*`) builds |
| `bun`       | `bun pm cache rm`                                         |
| `go`        | `go clean -cache`                                         |
| `pip`       | `pip cache purge`                                         |
| `node-gyp`  | Deletes `~/.cache/node-gyp/`                              |
| `npm`       | `npm cache clean --force`                                 |
| `yarn`      | `yarn cache clean`                                        |
| `pnpm`      | `pnpm store prune`                                        |
| `caches`    | All of the above (bun, go, pip, node-gyp, npm, yarn, pnpm) |
| `logs`      | Log files older than N days (see `--keep-days`)           |
| `xcode`     | Xcode DerivedData project directories                      |

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
remove directories (mise, generic node-gyp, and others).

### Xcode running detection

If Xcode is currently running when you run `sasurahime clean xcode`,
you will be prompted to confirm before cleaning DerivedData. In `--yes`
mode, the prompt is bypassed and the check is skipped.

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

# Full automation (cron)
sasurahime --yes

# Interactive pick-and-choose
sasurahime
```
