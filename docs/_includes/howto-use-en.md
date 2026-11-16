<details{% unless page.lang == 'ja' %} open{% endunless %} markdown="1" data-lang="en" lang="en">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

<div class="doc-body" markdown="1">

**sasurahime** is a macOS developer cache cleaner. It scans known cache
locations, reports disk usage, and lets you selectively remove stale data.

---

## Installation

### Via Homebrew (recommended)

```bash
brew tap armaniacs/sasurahime
brew install sasurahime
```

### Via Cargo

```bash
cargo install sasurahime
```

Requires Rust 1.70+ and macOS (arm64 or x86_64). No Linux/Windows support.

---

## Quick Start

1. **Scan — see what's taking space (no deletion):**
   ```bash
   sasurahime scan
   ```

2. **Preview before deleting:**
   ```bash
   sasurahime clean brew --dry-run
   ```

3. **Clean a specific target:**
   ```bash
   sasurahime clean brew
   ```

That's it. Files go to macOS Trash by default — recoverable from Finder.

---

## Commands

### `sasurahime scan`

Scan every cache target, print a summary table, and exit without deleting anything.

After the table, sasurahime also checks `~/Library/Caches/`, `~/Library/Application Support/`, and `~/Library/Logs/` for large caches that belong to running apps (Edge, VSCode, Slack, Claude, Obsidian, etc.). For any entry above the threshold it prints a **Tip** block with the recommended manual command, and — for apps that can be safely auto-quit — offers to quit the app, delete the cache, and relaunch it for you.

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

────────────────────────────────────────────────────────────
 Tip: The following caches can be freed manually:
────────────────────────────────────────────────────────────
  Claude (desktop)           428 MB  [running — quit first]
    $ rm -rf ~/Library/Application\ Support/Claude/Cache
  VSCode caches              693 MB  [running — quit first]
    $ rm -rf ~/Library/Application\ Support/Code/Cache
    $ rm -rf ~/Library/Application\ Support/Code/CachedExtensionVSIXs
    $ rm -rf ~/Library/Application\ Support/Code/CachedData
  Microsoft Edge             1.8 GB  [running — quit first]
    $ rm -rf ~/Library/Caches/Microsoft\ Edge
────────────────────────────────────────────────────────────

Quit Claude (desktop) and clear cache? (428 MB will be freed, delete ~/Library/Application Support/Claude/Cache) [y/N]
```

Answering `y` quits the app via `osascript`, deletes the cache, and relaunches the app if appropriate. Answering `n` (or pressing Enter) skips it. Apps that should not be auto-relaunched (Slack, Zoom, Claude) are noted — you relaunch those yourself.

The Tip block shows at most **5 entries**, sorted by size descending. Entries below 64 MB (1 MB for logs) are hidden. System-managed caches (GeoServices, etc.) are always excluded.

### `sasurahime clean <target>`

Clean a single cache target. Replace `<target>` with one of the names below.

| Target          | What it removes                                                       |
|-----------------|-----------------------------------------------------------------------|
| `act`           | `~/.cache/act/` (GitHub Actions runner)                               |
| `apfs-snapshot` | APFS local Time Machine snapshots (tmutil deletelocalsnapshot)         |
| `app-support`   | Cache dirs of known apps in `~/Library/Application Support/` (interactive only) |
| `brew`          | `brew cleanup -s --prune=all`                                         |
| `browsers`      | Old Puppeteer Chrome / Playwright (`ms-playwright*`) builds            |
| `bun`           | `bun pm cache rm`                                                     |
| `cargo`         | Cargo registry cache + `target/` directories                          |
| `cocoa-pods`    | `pod cache clean --all`                                               |
| `colima`        | Colima VM disk cache (`colima prune --all --force`)                   |
| `conda`         | `conda clean --all -y`                                                |
| `caches`        | All generic caches (bun, go, pip, node-gyp, npm, yarn, pnpm)         |
| `deno`          | `deno cache -r`                                                       |
| `device-support`| Old Xcode DeviceSupport directories (keeps recent N versions)         |
| `docker`        | `docker system prune -f`                                              |
| `downloads`     | `~/Downloads` old files                                               |
| `flutter`       | `dart pub cache clean`                                                |
| `go`            | `go clean -cache`                                                     |
| `gradle`        | Gradle old version caches                                             |
| `hints`         | Known app hints: quit + delete + relaunch for the top 5 above-threshold caches (per-app via `--sub`) |
| `huggingface`   | Hugging Face model cache (`hub/`) via CLI or fallback                  |
| `ios-backup`    | iOS device backups (interactive only — never in --yes mode)           |
| `jetbrains`     | JetBrains IDE caches (old versions)                                   |
| `library-caches` | Per-app `~/Library/Caches/` listing; known app caches removable (interactive only) |
| `library-logs`  | `~/Library/Logs/` — interactive heuristic scan (suggested cleanables) |
| `logs`          | Log files older than N days (see `--keep-days`)                       |
| `maven`         | Maven local repo (`mvn dependency:purge-local-repository`)            |
| `mise`          | Unused runtime versions under `~/.local/share/mise/installs/`          |
| `node-gyp`      | Deletes `~/.cache/node-gyp/`                                          |
| `npm`           | `npm cache clean --force`                                             |
| `ollama`        | Ollama model cache — interactive selection or directory deletion       |
| `orphan-containers` | Leftover `~/Library/Containers` / `~/Library/Group Containers` of uninstalled apps (interactive only) |
| `orbstack`      | `orb prune`                                                           |
| `pip`           | `pip cache purge`                                                     |
| `pipx`          | `pipx cache purge`                                                    |
| `pnpm`          | `pnpm store prune`                                                    |
| `poetry`        | `poetry cache clear --all`                                            |
| `pre-commit`    | Pre-commit hook environment cache (via CLI or fallback)                |
| `rustup`        | Unused Rust toolchains                                                |
| `bundle`        | Bundler cache (`bundle clean`)                                        |
| `dotnet`        | .NET NuGet cache (`dotnet nuget locals all --clear`)                  |
| `gem`           | RubyGems old gem versions (`gem cleanup`)                             |
| `sbt`           | Scala/sbt build cache + Ivy cache                                     |
| `terraform`     | Terraform plugin cache (`~/.terraform.d/plugin-cache/`)               |
| `tree-sitter`   | tree-sitter parser compilation cache                                  |
| `uv`            | Stale `simple-vN` index dirs + `uv cache prune --force`               |
| `volta`         | Volta Node.js manager cache (`~/.volta/cache/`)                       |
| `vscode-extensions` | VS Code extensions cache (`~/.vscode/extensions/`)                |
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

### `sasurahime explore`

OmniDiskSweeper-style disk explorer. Scans well-known cache and data directories,
groups usage by first-level subdirectory (= app name), and lets you act on what you find.

Unlike `scan`, `explore` covers **every** app's folder — not just the ones sasurahime
has a registered cleaner for. It is the right tool when you want to answer
"what is eating my disk?" without prior knowledge of the culprit.

```bash
$ sasurahime explore --top 5

━━━ Managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┌──────────────────────────────────────┬──────────┬──────────────────────────────┐
│ Path                                 │ Size     │ Clean with                   │
├──────────────────────────────────────┼──────────┼──────────────────────────────┤
│ ~/Library/Caches/Homebrew            │  9.1 GB  │ sasurahime clean brew        │
│ ~/.local/share/mise                  │  5.4 GB  │ sasurahime clean mise        │
│ ~/.cache/uv                          │  1.8 GB  │ sasurahime clean uv          │
└──────────────────────────────────────┴──────────┴──────────────────────────────┘

Select managed entries to clean (space to toggle, enter to confirm):
> [ ] ~/Library/Caches/Homebrew   9.1 GB
  [ ] ~/.local/share/mise         5.4 GB
  [ ] ~/.cache/uv                 1.8 GB

━━━ Not managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━

┌──────────────────────────────────────┬──────────┐
│ Path                                 │ Size     │
├──────────────────────────────────────┼──────────┤
│ ~/Library/Application Support/Adobe  │ 22.3 GB  │
│ ~/Library/Application Support/Spotify│  3.2 GB  │
└──────────────────────────────────────┴──────────┘

Select unmanaged entries to inspect:
> [ ] ~/Library/Application Support/Adobe    22.3 GB
  [ ] ~/Library/Application Support/Spotify   3.2 GB
```

The output has two sections:

- **Managed** — paths that sasurahime knows how to clean. Select entries to run
  `sasurahime clean <target>` immediately. After cleaning, the table is refreshed
  with updated sizes.
- **Not managed** — everything else. Select entries to see the full path (for
  copy-paste) and optionally open the folder in Finder.

Default scan roots: `~/Library/Application Support/`, `~/Library/Caches/`,
`~/.cache/`, `~/.local/share/`. Empty directories (size 0) are never shown.

Requires a TTY (terminal).

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--top N` | 20 | Show top N largest entries per section |
| `--all` | — | Show all entries (overrides `--top`) |
| `--dir PATH` | — | Scan this root instead of defaults (repeatable; replaces defaults entirely) |

**Examples:**

```bash
# Default: top 20 per section across 4 default roots
sasurahime explore

# Show only the 5 biggest entries per section
sasurahime explore --top 5

# Scan a single directory, show all entries
sasurahime explore --dir ~/Library/Application\ Support --all

# Preview managed cleans without deleting
sasurahime explore --dry-run
```

### `sasurahime` (no arguments)

Opens an interactive TUI with a checkbox list. Select which cache targets to
clean, then confirm to proceed.

After cleaning, the same Tip block and auto-quit prompts shown by `scan` are displayed.

Requires a TTY (terminal). In CI or scripting use `--yes` instead.

### `sasurahime --yes`

Non-interactive mode — cleans every pruneable target without prompting.
Only the unattended target set is cleaned; docker, npm, cargo, downloads and the other targets are shown by scan but never run by --yes.

```bash
# Clean everything, no questions asked
sasurahime --yes
```

Files are moved to Trash by default (see "Trash mode" below). If you
combine `--yes` with `--permanent`, a confirmation prompt is shown before
permanent deletion proceeds; declining it (or reaching EOF on a redirected
stdin) aborts with a non-zero exit code.

After cleaning, the Tip block and auto-quit prompts are shown (the prompts still require interactive input, so in a fully non-interactive pipeline redirect stdin from `/dev/null` to suppress them).

Ideal for cron jobs, CI pipelines, or maintenance scripts.

---

### `sasurahime stats`

Show aggregated deletion history and statistics.

```bash
$ sasurahime stats
╔══════════════════════════════════════╗
║  sasurahime Statistics              ║
╠══════════════════════════════════════╣
║  Total freed:  12.5 GB              ║
║  Runs:         15                   ║
╚══════════════════════════════════════╝

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-26 10:30   uv             500.0 MB
  2  2026-05-25 22:15   brew           1.2 GB
  3  2026-05-25 18:00   xcode          3.5 GB
```

History is automatically recorded every time `sasurahime clean` frees disk
space. Records are stored in `~/.local/share/sasurahime/history.json`.

```bash
# Show only the last 5 entries
sasurahime stats --last 5
```

If the history file is missing or corrupted, `stats` shows a friendly message
("No history yet") or a warning, and exits with code 0.

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

### `--permanent`

Bypass Trash and permanently delete files instead. By default every
cleaner sends removed files to the macOS Trash for safety.

```bash
# Permanently delete uv caches, bypassing Trash
sasurahime clean uv --permanent
```

When combined with `--yes`, a confirmation prompt is shown before
permanently deleting anything.

```bash
# Show confirmation before permanent bulk deletion
sasurahime --yes --permanent
```

### `--config <path>`

Override the config file path. By default, sasurahime reads
`~/.config/sasurahime/config.toml`. Use this flag to use a different file.

```bash
# Use a custom config file
sasurahime scan --config /tmp/my-config.toml
```

If the specified file does not exist, a warning is shown and defaults
are used.

---

## Configuration File

sasurahime reads `~/.config/sasurahime/config.toml` if it exists.
The file is optional — all defaults are sensible for day-to-day use.

### Example: change log retention to 30 days

```toml
[logs]
keep_days = 30
```

### Example: disable Trash permanently

```toml
trash_mode = false
```

### Example: add extra log directories

```toml
[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

### Example: exclude cleaners from scan/TUI

```toml
exclude = ["huggingface", "ollama"]
```

Excluded cleaners are hidden from `scan` output and the interactive TUI.
You can still run `sasurahime clean <target>` directly on an excluded target.

### Example: add custom cache directories

```toml
[[custom]]
name = "my-project"
path = "~/work/.cache"
```

Custom targets appear as regular cleaners in `scan` and the TUI. They can
be cleaned individually via `sasurahime clean my-project`. Only the contents
of the directory are removed (the root directory is preserved).

### Example: per-cleaner filters

```toml
[cleaner.act]
older_than_days = 30

[cleaner.downloads]
larger_than_mb = 500

[cleaner.logs]
older_than_days = 14
```

Filters apply to directory-scanning cleaners (`DeleteDirs` method).
`older_than_days` skips entries modified within that many days.
`larger_than_mb` skips entries smaller than the threshold.
`logs` accepts `older_than_days` only.

A filter that a cleaner will not use is reported once at startup on stderr:
command-based cleaners (uv, brew, bun, ...), the interactive cleaners
(orphan-containers, library-caches, app-support), `larger_than_mb` on `logs`,
and unknown cleaner names. For example:

    [config] some [cleaner.*] settings have no effect:
      - [docker] older_than_days: this cleaner has no per-cleaner filters

### Complete config example

```toml
# ~/.config/sasurahime/config.toml

# Global settings
trash_mode = true

# Exclude certain cleaners from scan/TUI
exclude = ["huggingface"]

# Custom cache targets
[[custom]]
name = "my-project"
path = "~/work/.cache"

# Per-cleaner filters
[cleaner.act]
older_than_days = 30

[cleaner.downloads]
larger_than_mb = 500

# Log settings
[logs]
keep_days = 30

[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

### Configuration fields

| Field                      | Type            | Default  | Description                              |
|----------------------------|-----------------|----------|------------------------------------------|
| `trash_mode`               | boolean         | `true`   | Send deleted files to Trash by default   |
| `exclude`                  | string[]        | `[]`     | Cleaners to hide from scan/TUI           |
| `[[custom]]`               | array of tables | `[]`     | User-defined cache directories           |
| `custom[].name`            | string          | required | Display name for custom target           |
| `custom[].path`            | string          | required | Path (supports `~` expansion)            |
| `[cleaner.<name>]`         | table           | —        | Per-cleaner filter settings              |
| `cleaner.<name>.older_than_days` | integer  | unset    | Only delete entries older than N days    |
| `cleaner.<name>.larger_than_mb`  | integer  | unset    | Only delete entries larger than N MB     |
| `[logs]`                   | table           | —        | Log retention settings                   |
| `logs.keep_days`           | integer         | `7`      | Global log retention period              |
| `[[logs.targets]]`         | array of tables | `[]`     | Extra log directories to scan            |
| `logs.targets[].name`      | string          | required | Display name                             |
| `logs.targets[].path`      | string          | required | Path (supports `~` expansion)            |
| `logs.targets[].exclude`   | string[]        | `[]`     | Filenames to never delete                |

---

## Safety

### Trash mode (default)

Every deleted file is sent to the macOS Trash by default. Nothing is
permanently erased unless you either:

- pass the `--permanent` flag, or
- set `trash_mode = false` in `~/.config/sasurahime/config.toml`

This gives you a safety net — accidentally removed caches can be restored
from Trash via Finder.

### `--dry-run` first

Every `clean` subcommand supports `--dry-run`. Preview before deleting —
zero side effects are verified on CI.

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

---

📖 See also: [Supported Targets]({{ '/SUPPORTED' | relative_url }}) · [How to Add a Target]({{ '/HOWTO-ADD-target' | relative_url }})

</div>

</details>
