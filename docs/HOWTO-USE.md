---
layout: doc
title: "HOW TO USE / 使い方"
permalink: /HOWTO-USE
---

<details open markdown="1">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

**sasurahime** is a macOS developer cache cleaner. It scans known cache
locations, reports disk usage, and lets you selectively remove stale data.

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

Quit Claude (desktop) and clear cache? (428 MB will be freed) [y/N]
```

Answering `y` quits the app via `osascript`, deletes the cache, and relaunches the app if appropriate. Answering `n` (or pressing Enter) skips it. Apps that should not be auto-relaunched (Slack, Zoom, Claude) are noted — you relaunch those yourself.

The Tip block shows at most **5 entries**, sorted by size descending. Entries below 64 MB (1 MB for logs) are hidden. System-managed caches (GeoServices, etc.) are always excluded.

### `sasurahime clean <target>`

Clean a single cache target. Replace `<target>` with one of the names below.

| Target          | What it removes                                                       |
|-----------------|-----------------------------------------------------------------------|
| `act`           | `~/.cache/act/` (GitHub Actions runner)                               |
| `apfs-snapshot` | APFS local Time Machine snapshots (tmutil deletelocalsnapshot)         |
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
| `huggingface`   | Hugging Face model cache (`hub/`) via CLI or fallback                  |
| `ios-backup`    | iOS device backups (interactive only — never in --yes mode)           |
| `jetbrains`     | JetBrains IDE caches (old versions)                                   |
| `library-logs`  | `~/Library/Logs/` — interactive heuristic scan (suggested cleanables) |
| `logs`          | Log files older than N days (see `--keep-days`)                       |
| `maven`         | Maven local repo (`mvn dependency:purge-local-repository`)            |
| `mise`          | Unused runtime versions under `~/.local/share/mise/installs/`          |
| `node-gyp`      | Deletes `~/.cache/node-gyp/`                                          |
| `npm`           | `npm cache clean --force`                                             |
| `ollama`        | Ollama model cache — interactive selection or directory deletion       |
| `orbstack`      | `orb prune`                                                           |
| `pip`           | `pip cache purge`                                                     |
| `pipx`          | `pipx cache purge`                                                    |
| `pnpm`          | `pnpm store prune`                                                    |
| `poetry`        | `poetry cache clear --all`                                            |
| `pre-commit`    | Pre-commit hook environment cache (via CLI or fallback)                |
| `rustup`        | Unused Rust toolchains                                                |
| `sbt`           | Scala/sbt build cache + Ivy cache                                     |
| `simulator`     | iOS Simulator cache (`xcrun simctl delete unavailable`)                |
| `spm`           | SwiftPM cache directory                                               |
| `terraform`     | Terraform provider plugin cache                                       |
| `trash`         | `~/.Trash` size (scan only — use Finder to empty)                     |
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

```bash
# Clean everything, no questions asked
sasurahime --yes
```

Files are moved to Trash by default (see "Trash mode" below). If you
combine `--yes` with `--permanent`, a confirmation prompt is shown before
permanent deletion proceeds.

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

[cleaner.colima]
larger_than_mb = 500

[cleaner.logs]
older_than_days = 14
```

Filters apply to directory-scanning cleaners (`DeleteDirs` method).
`older_than_days` skips entries modified within that many days.
`larger_than_mb` skips entries smaller than the threshold.
Command-based cleaners (uv, brew, bun, etc.) show a warning that the
filter does not apply — they delegate deletion to external tools.

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

[cleaner.colima]
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
</details>

<details markdown="1">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

**sasurahime** は macOS 開発者向けキャッシュクリーナーです。既知のキャッシュ
ロケーションをスキャンし、ディスク使用量を報告し、古くなったデータを選択して削除します。

---

## インストール

```bash
cargo install sasurahime
```

Rust 1.70+ と macOS（arm64 または x86_64）が必要です。Linux/Windows はサポートしていません。

---

## コマンド

### `sasurahime scan`

すべてのキャッシュターゲットをスキャンし、サマリテーブルを表示して、何も削除せずに終了します。

テーブル表示後、`~/Library/Caches/`・`~/Library/Application Support/`・`~/Library/Logs/` を調べ、起動中アプリ（Edge、VSCode、Slack、Claude、Obsidian 等）に属する大きなキャッシュを検出します。しきい値を超えるエントリが見つかると **Tip** ブロックに手動実行コマンドを表示し、安全に自動終了できるアプリについては「終了→キャッシュ削除→再起動」を代わりに行うか確認します。

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

Quit Claude (desktop) and clear cache? (428 MB will be freed) [y/N]
```

`y` と答えると `osascript` でアプリを終了し、キャッシュを削除し、適切であれば再起動します。`n`（または Enter）でスキップします。自動再起動しないアプリ（Slack、Zoom、Claude）は手動で再起動してください。

Tip ブロックはサイズ降順で最大 **5 件** 表示します。64 MB 未満（ログは 1 MB 未満）のエントリは非表示。GeoServices など OS 管理のキャッシュは常に除外されます。

### `sasurahime clean <target>`

単一のキャッシュターゲットをクリーンします。`<target>` を以下のいずれかの名前に置き換えてください。

| ターゲット      | 削除対象                                                             |
|-----------------|----------------------------------------------------------------------|
| `act`           | `~/.cache/act/`（GitHub Actions ランナー）                             |
| `apfs-snapshot` | APFS ローカル Time Machine スナップショット（tmutil deletelocalsnapshot）|
| `brew`          | `brew cleanup -s --prune=all`                                        |
| `browsers`      | 古い Puppeteer Chrome / Playwright（`ms-playwright*`）ビルド           |
| `bun`           | `bun pm cache rm`                                                    |
| `cargo`         | Cargo レジストリキャッシュ + `target/` ディレクトリ                      |
| `cocoa-pods`    | `pod cache clean --all`                                              |
| `colima`        | Colima VM ディスクキャッシュ（`colima prune --all --force`）            |
| `conda`         | `conda clean --all -y`                                               |
| `caches`        | すべての汎用キャッシュ（bun, go, pip, node-gyp, npm, yarn, pnpm）     |
| `deno`          | `deno cache -r`                                                      |
| `device-support`| 古い Xcode DeviceSupport ディレクトリ（最新Nバージョン保持）             |
| `docker`        | `docker system prune -f`                                             |
| `downloads`     | `~/Downloads` のファイル                                              |
| `flutter`       | `dart pub cache clean`                                                |
| `go`            | `go clean -cache`                                                    |
| `gradle`        | Gradle の古いバージョンキャッシュ                                      |
| `huggingface`   | Hugging Face モデルキャッシュ（`hub/`）CLI またはフォールバック          |
| `ios-backup`    | iOS デバイスバックアップ（インタラクティブのみ — --yes では非実行）     |
| `jetbrains`     | JetBrains IDE キャッシュ（古いバージョン）                              |
| `library-logs`  | `~/Library/Logs/` — インタラクティブヒューリスティックスキャン         |
| `logs`          | N 日より古いログファイル（`--keep-days` を参照）                        |
| `maven`         | Maven ローカルリポジトリ（`mvn dependency:purge-local-repository`）     |
| `mise`          | `~/.local/share/mise/installs/` 下の未使用ランタイムバージョン          |
| `node-gyp`      | `~/.cache/node-gyp/` を削除                                          |
| `npm`           | `npm cache clean --force`                                            |
| `ollama`        | Ollama モデルキャッシュ — 対話的選択またはディレクトリ削除                |
| `orbstack`      | `orb prune`                                                          |
| `pip`           | `pip cache purge`                                                    |
| `pipx`          | `pipx cache purge`                                                   |
| `pnpm`          | `pnpm store prune`                                                   |
| `poetry`        | `poetry cache clear --all`                                           |
| `pre-commit`    | pre-commit フック環境キャッシュ（CLI またはフォールバック）               |
| `rustup`        | 未使用の Rust ツールチェーン                                           |
| `sbt`           | Scala/sbt ビルドキャッシュ + Ivy キャッシュ                             |
| `simulator`     | iOS シミュレータキャッシュ（`xcrun simctl delete unavailable`）          |
| `spm`           | SwiftPM キャッシュディレクトリ                                         |
| `terraform`     | Terraform プロバイダプラグインキャッシュ                                 |
| `trash`         | `~/.Trash` のサイズ（スキャンのみ — Finder から空にしてください）       |
| `tree-sitter`   | tree-sitter パーサーコンパイルキャッシュ                                |
| `uv`            | 古い `simple-vN` インデックスディレクトリ + `uv cache prune --force`    |
| `volta`         | Volta Node.js マネージャキャッシュ（`~/.volta/cache/`）                 |
| `vscode-extensions` | VS Code 拡張機能キャッシュ（`~/.vscode/extensions/`）              |
| `xcode`         | Xcode DerivedData プロジェクトディレクトリ                              |
| `yarn`          | `yarn cache clean`                                                   |

**実行例:**

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean logs
```

### `sasurahime explore`

OmniDiskSweeper 風のディスク探索コマンドです。よく知られたキャッシュ・データディレクトリをスキャンし、第1レベルのサブディレクトリ（＝アプリ名）ごとに使用量をまとめて、発見したものに対してその場でアクションを取れます。

`scan` と異なり、`explore` は sasurahime がクリーナーを登録していないアプリのフォルダも含め **すべて** カバーします。「ディスクを食っているのは誰か？」を犯人不明の状態から調べるときに最適です。

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

出力は2つのセクションに分かれています：

- **Managed（管理済み）** — sasurahime がクリーン方法を知っているパス。エントリを選択すると即座に `sasurahime clean <target>` を実行します。クリーン後はテーブルが更新されたサイズで再表示されます。
- **Not managed（未管理）** — それ以外のすべて。エントリを選択するとフルパスを表示（コピー用）し、Finder でフォルダを開くか確認します。

デフォルトのスキャン対象：`~/Library/Application Support/`・`~/Library/Caches/`・`~/.cache/`・`~/.local/share/`。サイズ 0（空）のディレクトリは表示されません。

TTY（ターミナル）が必要です。

**オプション：**

| フラグ | デフォルト | 説明 |
|--------|------------|------|
| `--top N` | 20 | セクションごとに上位 N 件を表示 |
| `--all` | — | すべて表示（`--top` を上書き） |
| `--dir PATH` | — | デフォルトの代わりにこのルートをスキャン（繰り返し可、デフォルトを完全置換） |

**実行例：**

```bash
# デフォルト：4つのデフォルトルートからセクションごと上位 20 件
sasurahime explore

# セクションごとに上位 5 件のみ表示
sasurahime explore --top 5

# 単一ディレクトリをスキャンしてすべて表示
sasurahime explore --dir ~/Library/Application\ Support --all

# 実際に削除せずにクリーン対象をプレビュー
sasurahime explore --dry-run
```

### `sasurahime`（引数なし）

チェックボックスリスト付きのインタラクティブ TUI を起動します。クリーンするキャッシュターゲットを選択して確認するだけです。

クリーン完了後、`scan` と同じ Tip ブロックと自動終了プロンプトが表示されます。

TTY（ターミナル）が必要です。CI やスクリプトでは代わりに `--yes` を使用してください。

### `sasurahime --yes`

非インタラクティブモード — 確認なしですべての削除可能ターゲットをクリーンします。

```bash
# すべてクリーン、確認なし
sasurahime --yes
```

ファイルはデフォルトでゴミ箱に移動されます。`--yes` と `--permanent` を組み合わせると、完全削除の前に確認プロンプトが表示されます。

クリーン後、Tip ブロックと自動終了プロンプトが表示されます（プロンプトにはインタラクティブ入力が必要なので、完全に非インタラクティブなパイプラインでは `< /dev/null` で stdin を抑制してください）。

cron ジョブ、CI パイプライン、メンテナンススクリプトに最適です。

### `sasurahime stats`

削除履歴と統計情報を表示します。

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
```

履歴は `sasurahime clean` がディスク容量を解放するたびに自動記録され、
`~/.local/share/sasurahime/history.json` に保存されます。

```bash
# 直近 5 件のみ表示
sasurahime stats --last 5
```

履歴ファイルがない場合や破損している場合は、親切なメッセージとともに
終了コード 0 で正常終了します。

---

## 共通フラグ

### `--dry-run`

実際に削除せずに、削除予定の内容をプレビューします。

```bash
sasurahime clean uv --dry-run
sasurahime clean brew --dry-run
sasurahime clean logs --dry-run
```

すべての `clean` サブコマンドでサポートされています。副作用ゼロを保証します。

### `--all`（library-logs のみ）

インタラクティブプロンプトをスキップし、提案されたすべてのエントリを削除します。

```bash
sasurahime clean library-logs --all
```

`--all` なしの場合、`library-logs` は各クリーン可能エントリとその理由（`[large]`、`[stale N days]`）を表示するインタラクティブ選択を開きます。

### `--keep-days`（logs のみ）

ログファイルのデフォルト保持期間を上書きします。

```bash
# 14 日以内のログを保持、それより古いものを削除
sasurahime clean logs --keep-days 14
```

デフォルトは 7 日です（または設定ファイルの値）。

### `--permanent`

ゴミ箱を経由せず、ファイルを完全に削除します。デフォルトではすべてのクリーナーは安全のため削除ファイルを macOS のゴミ箱に送ります。

```bash
# uv キャッシュを完全に削除（ゴミ箱を経由しない）
sasurahime clean uv --permanent
```

`--yes` と組み合わせると、完全削除の前に確認プロンプトが表示されます。

```bash
# 完全一括削除の前に確認を表示
sasurahime --yes --permanent
```

### `--config <path>`

設定ファイルのパスを上書きします。デフォルトでは `~/.config/sasurahime/config.toml` を
読み込みます。このフラグで別のファイルを指定できます。

```bash
# カスタム設定ファイルを使用
sasurahime scan --config /tmp/my-config.toml
```

指定したファイルが存在しない場合は警告が表示され、デフォルト値が使用されます。

---

## 設定ファイル

sasurahime は `~/.config/sasurahime/config.toml` が存在すれば読み込みます。このファイルはオプションです — デフォルト値は日常的な使用に適しています。

### 例：ログ保持期間を 30 日に変更

```toml
[logs]
keep_days = 30
```

### 例：ゴミ箱を完全に無効化

```toml
trash_mode = false
```

### 例：追加ログディレクトリを設定

```toml
[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

### 例：クリーナーをスキャン/TUI から除外

```toml
exclude = ["huggingface", "ollama"]
```

除外されたクリーナーは `scan` 出力とインタラクティブ TUI に表示されません。
`sasurahime clean <target>` で直接指定すれば実行できます。

### 例：カスタムキャッシュディレクトリを追加

```toml
[[custom]]
name = "my-project"
path = "~/work/.cache"
```

カスタムターゲットは `scan` や TUI で通常のクリーナーと同様に表示されます。
`sasurahime clean my-project` で個別にクリーンできます。ディレクトリの
中身のみ削除され（ルートディレクトリは維持）、`uchg` フラグは自動処理されます。

### 例：クリーナーごとのフィルタ

```toml
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

[cleaner.logs]
older_than_days = 14
```

フィルタはディレクトリスキャン方式（`DeleteDirs`）のクリーナーに適用されます。
`older_than_days` は指定日数以内に変更されたエントリをスキップし、
`larger_than_mb` は閾値未満のエントリをスキップします。
コマンドベースのクリーナー（uv, brew, bun 等）にフィルタを設定すると、
実行時に「フィルタは適用できません」というワーニングが表示されます。

### 完全な設定例

```toml
# ~/.config/sasurahime/config.toml

# グローバル設定
trash_mode = true

# スキャン/TUI から除外するクリーナー
exclude = ["huggingface"]

# カスタムキャッシュターゲット
[[custom]]
name = "my-project"
path = "~/work/.cache"

# クリーナーごとのフィルタ
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

# ログ設定
[logs]
keep_days = 30

[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

| フィールド                   | 型               | デフォルト | 説明                             |
|------------------------------|------------------|------------|----------------------------------|
| `trash_mode`                 | boolean          | `true`     | 削除ファイルをデフォルトでゴミ箱へ |
| `exclude`                    | string[]         | `[]`       | スキャン/TUI から除外するクリーナー |
| `[[custom]]`                 | array of tables  | `[]`       | ユーザー定義キャッシュディレクトリ  |
| `custom[].name`              | string           | 必須       | カスタムターゲットの表示名         |
| `custom[].path`              | string           | 必須       | パス（`~` 展開対応）              |
| `[cleaner.<name>]`           | table            | —          | クリーナーごとのフィルタ設定       |
| `cleaner.<name>.older_than_days` | integer      | 未設定     | N 日より古いエントリのみ削除       |
| `cleaner.<name>.larger_than_mb`  | integer      | 未設定     | N MB より大きいエントリのみ削除    |
| `[logs]`                     | table            | —          | ログ保持設定                      |
| `logs.keep_days`             | integer          | `7`        | グローバルログ保持期間            |
| `[[logs.targets]]`           | array of tables  | `[]`       | スキャンする追加ログディレクトリ   |
| `logs.targets[].name`        | string           | 必須       | 表示名                            |
| `logs.targets[].path`        | string           | 必須       | パス（`~` 展開対応）              |
| `logs.targets[].exclude`     | string[]         | `[]`       | 削除しないファイル名              |

---

## 安全性

### ゴミ箱モード（デフォルト）

デフォルトでは、削除されたすべてのファイルは macOS のゴミ箱に送られます。以下を行わない限り、完全に消去されることはありません：

- `--permanent` フラグを渡す
- `~/.config/sasurahime/config.toml` で `trash_mode = false` を設定する

これにより安全網が確保されます — 誤って削除したキャッシュは Finder から復元できます。

### まず `--dry-run`

すべての `clean` サブコマンドは `--dry-run` をサポートしています。削除前に必ずプレビューしましょう。`--dry-run` 使用時の副作用ゼロは CI で検証済みです。

### `.mise.toml` のピン留め

mise ランタイムの削除は、バージョンを削除する前にグローバルの `~/.config/mise/config.toml` と HOME 下のすべての `.mise.toml`（最大深さ 5）をクロスチェックします。これらのファイルのいずれかに固定 (pinned) されているバージョンは決して削除されません。

### macOS 不変フラグ（`uchg`）

ディレクトリに macOS 不変フラグ（`uchg`）が設定されている場合 — パッケージマネージャやシステムキャッシュで一般的 — sasurahime は削除前に自動的に `chflags -R nouchg` を実行します。これはディレクトリを削除するすべてのクリーナーに適用されます。

### Xcode 実行中検出

`sasurahime clean xcode` 実行時に Xcode が実行中の場合は、DerivedData をクリーンする前に確認を求められます。`--yes` モードではプロンプトはバイパスされ、チェックはスキップされます。

### `~/Library/Logs/` の安全性

`library-logs` クリーナーはスキャン結果から常に `CrashReporter` と `DiagnosticReports` を除外します。ドットファイル（`.DS_Store` など）はスキップされます。ヒューリスティックルール（サイズ > 100 MB または最終変更 > 90 日前）に該当しないエントリは非表示になります。

---

## 終了コード

| コード | 意味                                        |
|--------|---------------------------------------------|
| 0      | 成功（または削除するものがない）              |
| 1      | 設定のパースエラー / ターミナルではない       |

---

## 実行例

```bash
# 回収可能な容量の概要を表示
sasurahime scan

# brew キャッシュをクリーン（まずプレビュー）
sasurahime clean brew --dry-run
sasurahime clean brew

# すべての汎用キャッシュを一度にクリーン
sasurahime clean caches

# 古いブラウザビルドを削除
sasurahime clean browsers

# 30 日より古いログをクリーン
sasurahime clean logs --keep-days 30

# ~/Library/Logs/ のインタラクティブヒューリスティックスキャン＋選択
sasurahime clean library-logs

# ~/Library/Logs/ の全候補を一括削除（プロンプトスキップ）
sasurahime clean library-logs --all

# 完全自動化（cron 用）
sasurahime --yes

# インタラクティブ選択
sasurahime
```

---

📖 関連ドキュメント: [対応ターゲット一覧]({{ '/SUPPORTED' | relative_url }}) · [ターゲット追加方法]({{ '/HOWTO-ADD-target' | relative_url }})
</details>
