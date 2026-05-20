---
layout: doc
title: "Supported Targets / 対応ターゲット一覧"
permalink: /SUPPORTED
---

<details open markdown="1">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

sasurahime provides **32 clean targets** organized by sprint.
Every target supports both `detect` (read-only, no side effects) and
`clean` (removal). All `clean` subcommands accept `--dry-run`.

---

## 1. `sasurahime clean uv`

**Category:** Sprint 1

**What it removes:** Stale `simple-vN` index directories inside
`~/.cache/uv/` and runs `uv cache prune --force`.

**How detect works:**
1. Reads entries in `~/.cache/uv/`.
2. Filters for names matching `simple-v<N>` (e.g. `simple-v16`, `simple-v21`)
   using `UvCleaner::parse_simple_version`. Symlinks are skipped.
3. Collects all version numbers, finds the highest.
4. Reports the sum of disk sizes of all versions *except* the highest as
   reclaimable.

**How clean works:**
1. Calls `uv cache prune --force` (removes orphaned/unpacked archives).
2. Deletes every `simple-vN` directory whose version is *not* the highest,
   using `fs::remove_dir_all`.
3. If `--dry-run` is set: lists what would be removed, does not delete.

**Safety:** Only the lowest version numbers are removed; the highest
`simple-v<N>` (most recent) is always kept.

---

## 2. `sasurahime clean brew`

**Category:** Sprint 1

**What it removes:** Homebrew download cache. Delegates to
`brew cleanup -s --prune=all`.

**How detect works:**
1. Checks if `~/Library/Caches/Homebrew` exists.
2. If yes, reports the total `dir_size` of the cache directory as pruneable.

**How clean works:**
1. If `brew` is not in `PATH`, prints a message and exits (0).
2. Runs `brew cleanup -s --prune=all`.
3. Parses the freed size from brew's output
   (`"freed approximately <N>GB of disk space"`) using
   `BrewCleaner::parse_brew_freed_bytes`, which in turn uses
   `BrewCleaner::parse_size_str` (supports `GB`, `MB`, `KB` — case-insensitive
   and space-separated variants).
4. Reports the parsed freed bytes.

**Safety:** The Homebrew CLI itself handles safety (scoped to its own cache).

---

## 3. `sasurahime clean mise`

**Category:** Sprint 2

**What it removes:** Unused runtime versions installed by
[mise](https://mise.jdx.dev/) under
`~/.local/share/mise/installs/<tool>/<version>`.

**How detect works:**
1. Runs `mise ls --current` to get the set of currently active
   `(tool, version)` pairs. Parses each tab/space-separated line via
   `MiseCleaner::parse_active_versions`.
2. Scans `~/.config/mise/config.toml` and every `.mise.toml` under HOME
   (max depth 5) via `MiseCleaner::scan_pinned_versions` to collect
   pinned `(tool, version)` pairs.
3. Reads the directory tree under `~/.local/share/mise/installs/`.
4. Any installed version that is **neither** in the active set **nor** in the
   pinned set is considered unused. Sums their disk sizes.

**How clean works:**
1. Same active + pinned detection as above.
2. For each unused `(tool, version, path)` triple:
   - **dry-run:** prints `[dry-run] would remove: <tool> <version>`.
   - **real run:** calls `MiseCleaner::remove_with_uchg` which:
     1. Runs `chflags -R nouchg <path>` to clear macOS immutable flags.
     2. Then runs `fs::remove_dir_all`.
     3. If `chflags` fails, the error is propagated (not silently ignored).
3. Reports total freed bytes.

**Safety (per CLAUDE.md §Safety rules):**
- Cross-checks global `~/.config/mise/config.toml` **and** any `.mise.toml`
  found within HOME (max depth 5) before removing any version.
- Pinned versions are **never** removed even if they are not currently active.
- macOS immutable flags are handled via `chflags -R nouchg`.

---

## 4. `sasurahime clean browsers`

**Category:** Sprint 2

**What it removes:** Old browser engine builds used by
[Puppeteer](https://pptr.dev/) and [Playwright](https://playwright.dev/).
Keeps only the highest version per browser family.

**Scanned locations:**

| Label                      | Path                                 |
|----------------------------|--------------------------------------|
| puppeteer/chrome           | `~/.cache/puppeteer/chrome`          |
| puppeteer/chrome-headless-shell | `~/.cache/puppeteer/chrome-headless-shell` |
| ms-playwright              | `~/Library/Caches/ms-playwright`     |
| ms-playwright-go           | `~/Library/Caches/ms-playwright-go` |

**Version comparison:**
- `BrowserCleaner::version_key` converts directory names to `Vec<u32>` by
  extracting all runs of ASCII digits.
- Example: `mac_arm-131.0.6778.204` → `[131, 0, 6778, 204]`,
  `chromium-1208` → `[1208]`.
- Comparison is lexicographic on the vector (standard Rust `Vec<u32>::cmp`),
  which correctly handles both semver-style and flat build-number formats.

**How detect works:**
1. For each group, calls `BrowserCleaner::find_old_versions(parent)`.
2. `find_old_versions` reads the directory, skips symlinks and unparseable
   directory names (those yielding empty version keys), finds the highest
   version key, and returns paths of **all entries except the highest**.
3. Returns 0 or 1 entries → nothing to remove. More than 1 → sums sizes of
   the old versions.

**How clean works:**
1. For each group, calls `find_old_versions`.
2. If `--dry-run`: prints what would be removed.
3. Otherwise: calls `fs::remove_dir_all` on each old version path.
4. Reports total freed bytes.

**Safety:**
- The highest version (most recent browser binary) is **always** kept.
- Symlinks are skipped (GAP-005) to avoid deleting through a stale link.
- Directories with unparseable names (e.g. `nightly`) are skipped.

---

## 5. `sasurahime clean bun`

**Category:** Sprint 3 — Generic caches

**What it removes:** [Bun](https://bun.sh/) package cache.

**Method:** `bun pm cache rm`

**How detect works:**
Checks if `bun` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `bun` not found, prints message and exits (0).
2. Runs `bun pm cache rm`.
3. Reports 0 freed bytes (the tool does not report freed space).

**Safety:** Delegates to the official `bun` CLI.

---

## 6. `sasurahime clean go`

**Category:** Sprint 3 — Generic caches

**What it removes:** [Go](https://go.dev/) build cache.

**Method:** `go clean -cache`

**How detect works:**
Checks if `go` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `go` not found, prints message and exits (0).
2. Runs `go clean -cache`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official `go` CLI.

---

## 7. `sasurahime clean pip`

**Category:** Sprint 3 — Generic caches

**What it removes:** [pip](https://pip.pypa.io/) package cache.

**Method:** `pip cache purge`

**How detect works:**
Checks if `pip` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `pip` not found, prints message and exits (0).
2. Runs `pip cache purge`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official `pip` CLI.

---

## 8. `sasurahime clean node-gyp`

**Category:** Sprint 3 — Generic caches

**What it removes:** [node-gyp](https://github.com/nodejs/node-gyp) build
cache directories.

**Scanned locations:**
- `~/.cache/node-gyp/`
- `~/Library/Caches/node-gyp/`

**How detect works:**
1. Checks which of the two directories exist.
2. Sums their `dir_size`.

**How clean works:**
1. Before deleting, runs `chflags -R nouchg <path>` (clears macOS immutable
   flags, error is silently ignored to avoid breaking on non-APFS filesystems).
2. Calls `fs::remove_dir_all` on each existing directory.
3. Reports total freed bytes.

**Safety:** macOS `uchg` flag is handled automatically (GAP-010).

---

## 9. `sasurahime clean npm`

**Category:** Sprint 3 — Generic caches

**What it removes:** [npm](https://www.npmjs.com/) package cache.

**Method:** `npm cache clean --force`

**How detect works:**
Checks if `npm` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `npm` not found, prints message and exits (0).
2. Runs `npm cache clean --force`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official `npm` CLI.

---

## 10. `sasurahime clean yarn`

**Category:** Sprint 3 — Generic caches

**What it removes:** [Yarn](https://yarnpkg.com/) package cache.

**Method:** `yarn cache clean`

**How detect works:**
Checks if `yarn` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `yarn` not found, prints message and exits (0).
2. Runs `yarn cache clean`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official `yarn` CLI.

---

## 11. `sasurahime clean pnpm`

**Category:** Sprint 3 — Generic caches

**What it removes:** [pnpm](https://pnpm.io/) store.

**Method:** `pnpm store prune`

**How detect works:**
Checks if `pnpm` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `pnpm` not found, prints message and exits (0).
2. Runs `pnpm store prune`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official `pnpm` CLI.

---

## 12. `sasurahime clean caches`

**Category:** Sprint 3 — Generic caches (aggregate)

**What it removes:** **All** generic caches in a single command.
Equivalent to running each of `bun`, `go`, `pip`, `node-gyp`, `npm`, `yarn`,
and `pnpm` sequentially.

Each sub-cleaner is invoked independently. If a tool is not installed, it is
skipped with a message (exit code is still 0).

---

## 13. `sasurahime clean logs`

**Category:** Sprint 3

**What it removes:** Log files older than `N` days from known and
user-configured log directories.

**Built-in log targets (scanned automatically):**

| Name         | Path                              | Files excluded from deletion |
|--------------|-----------------------------------|------------------------------|
| kilo         | `~/.local/share/kilo/log`         | `dev.log`                    |
| opencode     | `~/.local/share/opencode/logs`    | _(none)_                     |
| claude-code  | `~/.local/share/claude/logs`      | _(none)_                     |

**Extra log targets** can be added via the config file (see `HOWTO-USE.md`
for `config.toml` syntax). Each extra target has its own path and optional
exclusion list.

**How detect works:**
1. Iterates all targets (built-in + config extras).
2. For each target, calls `LogCleaner::find_old_logs(dir, keep_days, exclude)`.
3. `find_old_logs` reads the directory, filters to plain files only (no
   subdirectories), excludes files whose names match the exclusion list, then
   checks `LogCleaner::is_older_than` against each file's metadata mtime.
4. Sums the byte sizes of all files that are older than `keep_days`.

**How clean works:**
1. Same `find_old_logs` logic as detect.
2. If `--dry-run`: prints `[dry-run] [<name>] would remove: <path>` for each
   old file.
3. Otherwise: calls `fs::remove_file` on each old file.
4. Prints summary: `Removed <N> log files`.
5. Reports total freed bytes.

**Retention policy:**
- Default: `keep_days = 7` (files older than 7 days are deleted).
- Override via `--keep-days <N>` flag.
- Override via config file: `[logs]\nkeep_days = <N>`.
- CLI flag takes precedence over config.

**Safety:**
- Files in the exclusion list are **never** deleted.
- The built-in `kilo` target excludes `dev.log` by default.
- `is_older_than` uses strict `>` comparison (a file exactly `N` days old is
  **not** deleted).
- If file metadata cannot be read, the file is skipped (not deleted).

---

## 14. `sasurahime clean xcode`

**Category:** Sprint 3

**What it removes:** Project build artifact directories inside
Xcode's DerivedData folder (`~/Library/Developer/Xcode/DerivedData/`).

**How detect works:**
1. Checks if `DerivedData` directory exists.
2. If yes, reports the total `dir_size` as pruneable.

**How clean works:**
1. If `DerivedData` does not exist, prints message and exits (0).
2. If Xcode is currently running (checked via `pgrep -x Xcode`):
   - Prints a warning.
   - Prompts for confirmation via stdin.
   - **In `--yes` mode:** `is_xcode_running` always returns `false` (test
     environments have no Xcode process), so the prompt is never shown.
3. Lists subdirectories of `DerivedData` (each is a project build).
4. If `--dry-run`: prints what would be removed per-project.
5. Otherwise: calls `fs::remove_dir_all` on each project directory.
   The `DerivedData` root itself is **never** deleted.
6. Reports total freed bytes.

**Safety:**
- The `DerivedData` root directory is **never** removed.
- If Xcode is running, you are prompted to confirm (or operation aborts).
- Project directories are identified by reading `read_dir` — only
  subdirectories are considered (files at the root are skipped).

---

## 15. `sasurahime clean act`

**Category:** Sprint 5

**What it removes:** [act](https://github.com/nektos/act) GitHub Actions
local runner cache (`~/.cache/act/`, or `$ACT_CACHE_DIR` if set).

**Method:** Directory deletion.

**How detect works:**
1. Checks if the cache directory exists (uses `$ACT_CACHE_DIR` env var or
   `~/.cache/act/` as fallback).
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Runs `is_safe_delete_target` to verify the path is not a system directory.
2. If `$ACT_CACHE_DIR` points to an unsafe path, falls back to the default.
3. Deletes the directory via `fs::remove_dir_all`.

**Safety:** `$ACT_CACHE_DIR` is validated against a blocklist of system paths
(`/`, `/etc`, `/var`, `/usr`, etc.). Unsafe env var values are rejected.

---

## 16. `sasurahime clean cargo`

**Category:** Sprint 5

**What it removes:** [Cargo](https://doc.rust-lang.org/cargo/) registry cache
(`~/.cargo/registry/cache/`) and `target/` build artifact directories.

**Scanned by `detect`:**
1. `~/.cargo/registry/cache/` — downloaded crate archives.
2. User projects under `~/src/`, `~/work/`, `~/dev/` with `target/` dirs.

**How clean works:**
1. Deletes `~/.cargo/registry/cache/` contents.
2. Scans for `target/` directories under common project roots and removes them.
3. `chflags -R nouchg` is run before deletion to handle macOS immutable flags.

---

## 17. `sasurahime clean cocoa-pods`

**Category:** Sprint 5

**What it removes:** [CocoaPods](https://cocoapods.org/) cache.

**Method:** `pod cache clean --all`

**How detect works:**
Checks if `pod` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `pod` not found, prints message and exits (0).
2. Runs `pod cache clean --all`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official CocoaPods CLI.

---

## 18. `sasurahime clean conda`

**Category:** Sprint 5

**What it removes:** [Conda](https://docs.conda.io/) package cache.

**Method:** `conda clean --all -y`

**How detect works:**
Checks if `conda` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `conda` not found, prints message and exits (0).
2. Runs `conda clean --all -y`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official Conda CLI.

---

## 19. `sasurahime clean deno`

**Category:** Sprint 5

**What it removes:** [Deno](https://deno.com/) cache.

**Method:** `deno cache -r` (reload cache)

**How detect works:**
Checks if `deno` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `deno` not found, prints message and exits (0).
2. Runs `deno cache -r`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official Deno CLI.

---

## 20. `sasurahime clean docker`

**Category:** Sprint 5

**What it removes:** [Docker](https://www.docker.com/) dangling images,
containers, build cache, and networks.

**Method:** `docker system prune -f`

**How detect works:**
Checks if `docker` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `docker` not found, prints message and exits (0).
2. Runs `docker system prune -f` (dangling images only — tagged images are
   preserved).
3. Reports 0 freed bytes.

**Safety:** Only dangling (untagged) images are removed. Explicitly avoids `-a`
flag to preserve tagged unused images.

---

## 21. `sasurahime clean downloads`

**Category:** Sprint 5

**What it removes:** Old files inside `~/Downloads/`.

**How detect works:**
1. Checks if `~/Downloads` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Lists immediate children of `~/Downloads`.
2. Deletes files and directories with `chflags -R nouchg` + `remove_dir_all`.
3. **Safety:** Only removes items directly under `~/Downloads/` (no recursion).

---

## 22. `sasurahime clean gradle`

**Category:** Sprint 5

**What it removes:** Old version caches from
[Gradle](https://gradle.org/) (`~/.gradle/caches/`). Keeps only the most
recent version of each cached artifact.

**How detect works:**
1. Scans `~/.gradle/caches/` for per-version directories.
2. For each cached artifact group, keeps the highest version and reports
   the sum of all older versions as reclaimable.

**How clean works:**
1. Identifies old versions using version comparison.
2. Removes old version directories with `chflags -R nouchg` + `remove_dir_all`.
3. Handles macOS immutable flags.

**Safety:** The most recent version of each cached artifact is always kept.

---

## 23. `sasurahime clean huggingface`

**Category:** Sprint 5

**What it removes:** [Hugging Face](https://huggingface.co/) model cache
(`~/.cache/huggingface/hub/` or `$HF_HOME/hub`).

**How detect works:**
1. Checks if `hub/` directory exists (`$HF_HOME/hub` or `~/.cache/huggingface/hub`).
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Attempts CLI first: runs `huggingface-cli delete-cache --yes`.
2. If `huggingface-cli` is not in PATH, falls back to deleting `hub/` contents
   directly (recreates the `hub/` directory after deletion).
3. `$HF_HOME` is validated against `is_safe_delete_target` — unsafe paths cause
   a fallback to the default.

**Safety:** `$HF_HOME` is validated against a blocklist of system paths.
CLI takes precedence over direct deletion.

---

## 24. `sasurahime clean jetbrains`

**Category:** Sprint 5

**What it removes:** Old version caches from
[JetBrains IDEs](https://www.jetbrains.com/) (IntelliJ IDEA, WebStorm, etc.)
under `~/Library/Caches/JetBrains/`.

**Method:** Similar to Gradle cache version pruning — keeps the most recent
version of each IDE cache.

**How detect works:**
1. Scans `~/Library/Caches/JetBrains/` for per-IDE per-version directories.
2. Reports old versions as reclaimable.

**How clean works:**
1. Identifies old versions per IDE family.
2. Removes old version directories with `chflags -R nouchg` + `remove_dir_all`.

**Safety:** The most recent version of each IDE cache is always kept.

---

## 25. `sasurahime clean library-logs`

**Category:** Sprint 5

**What it removes:** User log files under `~/Library/Logs/`. Uses heuristic
rules to suggest which entries to delete. **Interactive** — opens a selection
prompt unless `--all` is used.

**How detect works:**
1. Reads immediate children of `~/Library/Logs/`.
2. For each entry: measures `dir_size` and reads `last_modified` time.
3. Applies two heuristic rules:
   - **Oversized:** size > 100 MB → tagged `[large]`
   - **Stale:** last modified > 90 days ago → tagged `[stale N days]`
4. Entries that trigger at least one rule are included in results.
5. Excludes `CrashReporter`, `DiagnosticReports`, and dot-entries.

**How clean works:**
1. Runs the same scan as detect.
2. **If `--dry-run`:** prints each entry with its reason tags.
3. **If `--all`:** deletes all suggested entries without prompting.
4. **Otherwise:** opens `dialoguer::MultiSelect` with all entries pre-selected.
   User confirms selection, then selected entries are deleted with
   `chflags -R nouchg` + `remove_dir_all`.

**Safety:**
- `CrashReporter` and `DiagnosticReports` are **always** excluded.
- Dot-files and dot-directories (`.DS_Store`, `.localized`, etc.) are skipped.
- Future timestamps due to clock skew are clamped to `SystemTime::now()`.
- `--dry-run` guarantees zero side effects.

---

## 26. `sasurahime clean orbstack`

**Category:** Sprint 5

**What it removes:** [Orbstack](https://orbstack.dev/) Docker runtime cache.

**Method:** `orb prune`

**How detect works:**
Checks if `orb` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `orb` not found, prints message and exits (0).
2. Runs `orb prune`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official Orbstack CLI.

---

## 27. `sasurahime clean pipx`

**Category:** Sprint 5

**What it removes:** [pipx](https://pypa.github.io/pipx/) cache and unused
packages.

**Method:** `pipx cache purge`

**How detect works:**
Checks if `pipx` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `pipx` not found, prints message and exits (0).
2. Runs `pipx cache purge`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official pipx CLI.

---

## 28. `sasurahime clean poetry`

**Category:** Sprint 5

**What it removes:** [Poetry](https://python-poetry.org/) package cache.

**Method:** `poetry cache clear --all`

**How detect works:**
Checks if `poetry` is in `PATH`. Reports as pruneable if found (unknown size).

**How clean works:**
1. If `poetry` not found, prints message and exits (0).
2. Runs `poetry cache clear --all`.
3. Reports 0 freed bytes.

**Safety:** Delegates to the official Poetry CLI.

---

## 29. `sasurahime clean pre-commit`

**Category:** Sprint 5

**What it removes:** [pre-commit](https://pre-commit.com/) hook environment
cache (`~/.cache/pre-commit/`, `$PRE_COMMIT_HOME`, or `$XDG_CACHE_HOME/pre-commit`).

**How detect works:**
1. Resolves the cache directory from env vars:
   `$PRE_COMMIT_HOME` → `$XDG_CACHE_HOME/pre-commit` → `~/.cache/pre-commit`.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Tries CLI first: runs `pre-commit clean`.
2. If `pre-commit` not in PATH, deletes the cache directory directly.
3. Env var paths are validated via `is_safe_delete_target` — unsafe paths
   cause a fallback to `~/.cache/pre-commit`.

**Safety:** Env var paths are validated. CLI takes precedence over direct
deletion.

---

## 30. `sasurahime clean rustup`

**Category:** Sprint 5

**What it removes:** Unused [Rust](https://www.rust-lang.org/) toolchain
versions (the toolchains not selected by `rustup default` or `rustup override`).

**How detect works:**
1. Runs `rustup toolchain list` to enumerate installed toolchains.
2. Identifies the default toolchain (marked with `(default)`) and any
   override toolchains (marked with `(override)`).
3. All toolchains that are **neither** default **nor** override are reported
   as unused.

**How clean works:**
1. Same detection as above.
2. For each unused toolchain: runs `rustup toolchain remove <name>`.
3. Reports total freed bytes (parsed from rustup output).

**Safety:** The default and override toolchains are **never** removed. Only
toolchains not selected by any profile are candidates for removal.

---

## 31. `sasurahime clean spm`

**Category:** Sprint 5

**What it removes:** [Swift Package Manager](https://www.swift.org/package-manager/)
build artifacts and cached packages.

**Method:** Deletes `~/Library/Caches/org.swift.swiftpm/` and
`~/Library/Developer/Xcode/DerivedData/SourcePackages/`.

**How detect works:**
1. Checks if the SPM cache directories exist.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Runs `chflags -R nouchg` on cache directories.
2. Calls `fs::remove_dir_all` on each cache directory.
3. Cached package checkouts and repository clones are removed (packages will
   be re-fetched on next build).

---

## 32. `sasurahime clean trash`

**Category:** Sprint 5

**What it removes:** `~/.Trash` — **scan only**. sasurahime reports the size
of the Trash directory but will not delete it (users should use Finder to
empty Trash).

**How detect works:**
1. Checks if `~/.Trash` exists.
2. Reports its total `dir_size` as pruneable.

**How clean works:**
1. **`--dry-run`:** runs `detect`-style scan and prints the size that would
   be freed.
2. **Otherwise:** prints a warning instructing the user to empty Trash via
   Finder. No files are deleted.

**Safety:** sasurahime refuses to delete `~/.Trash` contents — this is an
intentional safety measure.

---

## Scan (`sasurahime scan`)

Runs `detect()` on every cleaner and prints a formatted table via
`comfy_table`. No side effects — never creates, modifies, or deletes files.

---

## Interactive / Auto

| Mode | Behavior |
|------|----------|
| `sasurahime` (no args, TTY) | Opens an interactive checkbox list. Select targets, confirm, and selected targets are cleaned. |
| `sasurahime --yes` (no args, any) | Cleans every pruneable target without prompting. Exits with 0 if nothing is found. |
| `sasurahime scan` (non-TTY) | Prints the scan table only. |
| `sasurahime clean <target>` | Cleans a specific target directly. |

</details>

<details markdown="1">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

sasurahime は **32 のクリーンターゲット** をスプリント単位で提供しています。
すべてのターゲットは `detect`（読み取り専用、副作用なし）と `clean`（削除）の両方に対応しています。また、すべての `clean` サブコマンドは `--dry-run` をサポートしています。

---

## 1. `sasurahime clean uv`

**カテゴリ:** Sprint 1

**削除対象:** `~/.cache/uv/` 内の古い `simple-vN` インデックスディレクトリ、
および `uv cache prune --force` の実行。

**detect の動作:**
1. `~/.cache/uv/` 内のエントリを読み取ります。
2. `simple-v<N>` にマッチする名前を `UvCleaner::parse_simple_version` でフィルタリングします。シンボリックリンクはスキップされます。
3. すべてのバージョン番号を収集し、最大値を見つけます。
4. 最大値 **以外** の全バージョンのディスクサイズ合計を削除可能量として報告します。

**clean の動作:**
1. `uv cache prune --force` を呼び出します（孤立した/展開済みアーカイブを削除）。
2. 最大バージョン **以外** のすべての `simple-vN` ディレクトリを `fs::remove_dir_all` で削除します。
3. `--dry-run` が設定されている場合：削除予定のリストを表示し、削除は行いません。

**安全性:** 最も新しい `simple-v<N>`（最新）は常に保持され、古いバージョンのみ削除されます。

---

## 2. `sasurahime clean brew`

**カテゴリ:** Sprint 1

**削除対象:** Homebrew のダウンロードキャッシュ。`brew cleanup -s --prune=all` に委譲します。

**detect の動作:**
1. `~/Library/Caches/Homebrew` が存在するか確認します。
2. 存在する場合、キャッシュディレクトリの合計サイズを報告します。

**clean の動作:**
1. `brew` が `PATH` にない場合、メッセージを表示して終了します (0)。
2. `brew cleanup -s --prune=all` を実行します。
3. brew の出力から解放サイズをパースします（`BrewCleaner::parse_brew_freed_bytes` を使用）。
4. パースした解放バイト数を報告します。

**安全性:** Homebrew CLI 自身が安全性を確保します。

---

## 3. `sasurahime clean mise`

**カテゴリ:** Sprint 2

**削除対象:** [mise](https://mise.jdx.dev/) がインストールした未使用のランタイムバージョン（`~/.local/share/mise/installs/<tool>/<version>` 内）。

**detect の動作:**
1. `mise ls --current` を実行し、現在アクティブな `(tool, version)` ペアを取得します。
2. `~/.config/mise/config.toml` と HOME 下のすべての `.mise.toml`（最大深さ 5）をスキャンし、固定 (pinned) されたバージョンを収集します。
3. `~/.local/share/mise/installs/` 以下のディレクトリツリーを読み取ります。
4. アクティブセット **にも** 固定セット **にも** 含まれないインストール済みバージョンを未使用とみなし、そのディスクサイズを合計します。

**clean の動作:**
1. detect と同じアクティブ＋固定検出を行います。
2. 未使用の `(tool, version, path)` トリプルごとに：
   - **dry-run:** `[dry-run] would remove: <tool> <version>` を表示します。
   - **実実行:** `MiseCleaner::remove_with_uchg` を呼び出し、`chflags -R nouchg` で macOS 不変フラグを解除後、`fs::remove_dir_all` を実行します。
3. 解放バイト数の合計を報告します。

**安全性:**
- グローバル設定と HOME 内の `.mise.toml`（最大深さ 5）の **両方** をクロスチェックします。
- 固定 (pinned) されたバージョンは現在アクティブでなくても **決して** 削除されません。

---

## 4. `sasurahime clean browsers`

**カテゴリ:** Sprint 2

**削除対象:** [Puppeteer](https://pptr.dev/) と [Playwright](https://playwright.dev/) が使用する古いブラウザエンジンビルド。ブラウザファミリーごとに最新バージョンのみ保持します。

**スキャン対象パス:**

| ラベル                     | パス                                   |
|----------------------------|----------------------------------------|
| puppeteer/chrome           | `~/.cache/puppeteer/chrome`            |
| puppeteer/chrome-headless-shell | `~/.cache/puppeteer/chrome-headless-shell` |
| ms-playwright              | `~/Library/Caches/ms-playwright`       |
| ms-playwright-go           | `~/Library/Caches/ms-playwright-go`    |

**バージョン比較:**
- `BrowserCleaner::version_key` はディレクトリ名を `Vec<u32>` に変換します（ASCII 数字の連続を抽出）。
- 例: `mac_arm-131.0.6778.204` → `[131, 0, 6778, 204]`、`chromium-1208` → `[1208]`。
- 比較はベクタの辞書順（標準の Rust `Vec<u32>::cmp`）で行われ、semver 形式とフラットなビルド番号形式の両方を正しく処理します。

**detect の動作:**
1. グループごとに `BrowserCleaner::find_old_versions(parent)` を呼び出します。
2. ディレクトリを読み取り、シンボリックリンクと解析不能な名前をスキップし、最新バージョン **以外** のすべてのエントリを返します。
3. 0 または 1 エントリ → 削除対象なし。2 以上 → 古いバージョンのサイズ合計。

**clean の動作:**
1. グループごとに `find_old_versions` を呼び出します。
2. `--dry-run` の場合：削除予定を表示します。
3. それ以外：各古いバージョンパスに対して `fs::remove_dir_all` を実行します。
4. 解放バイト数を報告します。

**安全性:**
- 最新バージョン（最新のブラウザバイナリ）は **常に** 保持されます。
- シンボリックリンクはスキップされ、古いリンク経由の削除を防ぎます。
- `nightly` など解析不能な名前のディレクトリはスキップされます。

---

## 5. `sasurahime clean bun`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [Bun](https://bun.sh/) のパッケージキャッシュ。

**方法:** `bun pm cache rm`

**detect の動作:** `bun` が `PATH` にあるか確認します。見つかった場合は削除可能として報告します（サイズ不明）。

**clean の動作:**
1. `bun` が見つからない場合、メッセージを表示して終了します (0)。
2. `bun pm cache rm` を実行します。
3. 解放バイト数として 0 を報告します（ツールが解放容量を報告しないため）。

**安全性:** 公式の `bun` CLI に委譲します。

---

## 6. `sasurahime clean go`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [Go](https://go.dev/) のビルドキャッシュ。

**方法:** `go clean -cache`

**detect の動作:** `go` が `PATH` にあるか確認します。

**clean の動作:**
1. `go` が見つからない場合、メッセージを表示して終了します (0)。
2. `go clean -cache` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `go` CLI に委譲します。

---

## 7. `sasurahime clean pip`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [pip](https://pip.pypa.io/) のパッケージキャッシュ。

**方法:** `pip cache purge`

**detect の動作:** `pip` が `PATH` にあるか確認します。

**clean の動作:**
1. `pip` が見つからない場合、メッセージを表示して終了します (0)。
2. `pip cache purge` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `pip` CLI に委譲します。

---

## 8. `sasurahime clean node-gyp`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [node-gyp](https://github.com/nodejs/node-gyp) のビルドキャッシュディレクトリ。

**スキャン対象パス:**
- `~/.cache/node-gyp/`
- `~/Library/Caches/node-gyp/`

**detect の動作:**
1. 2 つのディレクトリのうち存在するものを確認します。
2. それぞれの `dir_size` を合計します。

**clean の動作:**
1. 削除前に `chflags -R nouchg <path>` を実行します（macOS 不変フラグを解除）。
2. 既存の各ディレクトリに対して `fs::remove_dir_all` を実行します。
3. 解放バイト数を報告します。

**安全性:** macOS の `uchg` フラグは自動的に処理されます。

---

## 9. `sasurahime clean npm`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [npm](https://www.npmjs.com/) のパッケージキャッシュ。

**方法:** `npm cache clean --force`

**detect の動作:** `npm` が `PATH` にあるか確認します。

**clean の動作:**
1. `npm` が見つからない場合、メッセージを表示して終了します (0)。
2. `npm cache clean --force` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `npm` CLI に委譲します。

---

## 10. `sasurahime clean yarn`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [Yarn](https://yarnpkg.com/) のパッケージキャッシュ。

**方法:** `yarn cache clean`

**detect の動作:** `yarn` が `PATH` にあるか確認します。

**clean の動作:**
1. `yarn` が見つからない場合、メッセージを表示して終了します (0)。
2. `yarn cache clean` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `yarn` CLI に委譲します。

---

## 11. `sasurahime clean pnpm`

**カテゴリ:** Sprint 3 — Generic caches

**削除対象:** [pnpm](https://pnpm.io/) ストア。

**方法:** `pnpm store prune`

**detect の動作:** `pnpm` が `PATH` にあるか確認します。

**clean の動作:**
1. `pnpm` が見つからない場合、メッセージを表示して終了します (0)。
2. `pnpm store prune` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `pnpm` CLI に委譲します。

---

## 12. `sasurahime clean caches`

**カテゴリ:** Sprint 3 — Generic caches（集約）

**削除対象:** **すべての** ジェネリックキャッシュを 1 コマンドで削除します。
`bun`、`go`、`pip`、`node-gyp`、`npm`、`yarn`、`pnpm` を順次実行するのと同等です。

各サブクリーナーは独立して呼び出されます。ツールがインストールされていない場合はスキップされます（終了コードは 0）。

---

## 13. `sasurahime clean logs`

**カテゴリ:** Sprint 3

**削除対象:** 既知およびユーザー設定のログディレクトリ内の `N` 日より古いログファイル。

**組み込みログターゲット（自動スキャン）:**

| 名前         | パス                                 | 削除除外ファイル        |
|--------------|--------------------------------------|-------------------------|
| kilo         | `~/.local/share/kilo/log`            | `dev.log`               |
| opencode     | `~/.local/share/opencode/logs`       | _(なし)_                |
| claude-code  | `~/.local/share/claude/logs`         | _(なし)_                |

**追加ログターゲット** は設定ファイルで追加できます。各ターゲットは独自のパスと除外リストを持ちます。

**detect の動作:**
1. すべてのターゲット（組み込み＋設定追加）を反復処理します。
2. ターゲットごとに `LogCleaner::find_old_logs(dir, keep_days, exclude)` を呼び出します。
3. ディレクトリを読み取り、通常ファイルのみにフィルタリングし、除外リストに一致するファイルを除外して、各ファイルの mtime を確認します。
4. `keep_days` より古いすべてのファイルのバイトサイズを合計します。

**clean の動作:**
1. detect と同じ `find_old_logs` ロジックを使用します。
2. `--dry-run` の場合：各古いファイルに対して削除予定を表示します。
3. それ以外：各古いファイルに対して `fs::remove_file` を呼び出します。
4. 要約を表示：`Removed <N> log files`。
5. 解放バイト数を報告します。

**保持ポリシー:**
- デフォルト: `keep_days = 7`（7 日より古いファイルを削除）。
- `--keep-days <N>` フラグで上書き可能。
- 設定ファイルでも上書き可能：`[logs]\nkeep_days = <N>`。
- CLI フラグが設定より優先されます。

**安全性:**
- 除外リストのファイルは **決して** 削除されません。
- 組み込みの `kilo` ターゲットはデフォルトで `dev.log` を除外します。
- `is_older_than` は厳密な `>` 比較を使用します（正確に `N` 日前のファイルは削除 **されません**）。
- ファイルメタデータが読み取れない場合はスキップされます。

---

## 14. `sasurahime clean xcode`

**カテゴリ:** Sprint 3

**削除対象:** Xcode DerivedData フォルダ内のプロジェクトビルドアーティファクト（`~/Library/Developer/Xcode/DerivedData/`）。

**detect の動作:**
1. `DerivedData` ディレクトリが存在するか確認します。
2. 存在する場合、合計サイズを報告します。

**clean の動作:**
1. `DerivedData` が存在しない場合、メッセージを表示して終了します (0)。
2. Xcode が実行中の場合は警告を表示し、確認を求めます。
3. `DerivedData` のサブディレクトリを一覧表示します。
4. `--dry-run` の場合：プロジェクトごとに削除予定を表示します。
5. それ以外：各プロジェクトディレクトリに対して `fs::remove_dir_all` を呼び出します。`DerivedData` ルート自体は **決して** 削除されません。
6. 解放バイト数を報告します。

**安全性:**
- `DerivedData` ルートディレクトリは **決して** 削除されません。
- Xcode 実行中は確認を求められます。
- ルートのファイルはスキップされ、サブディレクトリのみが削除対象となります。

---

## 15. `sasurahime clean act`

**カテゴリ:** Sprint 5

**削除対象:** [act](https://github.com/nektos/act) GitHub Actions ローカルランナーキャッシュ（`~/.cache/act/`、または `$ACT_CACHE_DIR` が設定されている場合はそのパス）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. キャッシュディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `is_safe_delete_target` を実行してパスがシステムディレクトリでないことを確認します。
2. 安全でないパスの場合はデフォルトにフォールバックします。
3. `fs::remove_dir_all` でディレクトリを削除します。

**安全性:** `$ACT_CACHE_DIR` はシステムパスのブロックリストに対して検証されます。安全でない環境変数値は拒否されます。

---

## 16. `sasurahime clean cargo`

**カテゴリ:** Sprint 5

**削除対象:** [Cargo](https://doc.rust-lang.org/cargo/) レジストリキャッシュ（`~/.cargo/registry/cache/`）と `target/` ビルドアーティファクトディレクトリ。

**detect のスキャン対象:**
1. `~/.cargo/registry/cache/` — ダウンロード済みクレートアーカイブ。
2. `~/src/`、`~/work/`、`~/dev/` 以下のユーザープロジェクトの `target/` ディレクトリ。

**clean の動作:**
1. `~/.cargo/registry/cache/` の内容を削除します。
2. 一般的なプロジェクトルート下の `target/` ディレクトリをスキャンして削除します。
3. 削除前に `chflags -R nouchg` を実行します。

---

## 17. `sasurahime clean cocoa-pods`

**カテゴリ:** Sprint 5

**削除対象:** [CocoaPods](https://cocoapods.org/) キャッシュ。

**方法:** `pod cache clean --all`

**detect の動作:** `pod` が `PATH` にあるか確認します。

**clean の動作:**
1. `pod` が見つからない場合、メッセージを表示して終了します (0)。
2. `pod cache clean --all` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の CocoaPods CLI に委譲します。

---

## 18. `sasurahime clean conda`

**カテゴリ:** Sprint 5

**削除対象:** [Conda](https://docs.conda.io/) パッケージキャッシュ。

**方法:** `conda clean --all -y`

**detect の動作:** `conda` が `PATH` にあるか確認します。

**clean の動作:**
1. `conda` が見つからない場合、メッセージを表示して終了します (0)。
2. `conda clean --all -y` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Conda CLI に委譲します。

---

## 19. `sasurahime clean deno`

**カテゴリ:** Sprint 5

**削除対象:** [Deno](https://deno.com/) キャッシュ。

**方法:** `deno cache -r`（キャッシュ再読み込み）

**detect の動作:** `deno` が `PATH` にあるか確認します。

**clean の動作:**
1. `deno` が見つからない場合、メッセージを表示して終了します (0)。
2. `deno cache -r` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Deno CLI に委譲します。

---

## 20. `sasurahime clean docker`

**カテゴリ:** Sprint 5

**削除対象:** [Docker](https://www.docker.com/) の dangling イメージ、コンテナ、ビルドキャッシュ、ネットワーク。

**方法:** `docker system prune -f`

**detect の動作:** `docker` が `PATH` にあるか確認します。

**clean の動作:**
1. `docker` が見つからない場合、メッセージを表示して終了します (0)。
2. `docker system prune -f` を実行します（dangling イメージのみ — タグ付きイメージは保持）。
3. 解放バイト数として 0 を報告します。

**安全性:** dangling（タグなし）イメージのみ削除します。明示的に `-a` フラグを避け、タグ付きの未使用イメージを保持します。

---

## 21. `sasurahime clean downloads`

**カテゴリ:** Sprint 5

**削除対象:** `~/Downloads/` 内のファイル。

**detect の動作:**
1. `~/Downloads` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `~/Downloads` の直下の子項目を一覧表示します。
2. `chflags -R nouchg` + `remove_dir_all` で削除します。
3. **安全性:** `~/Downloads/` の直下の項目のみ削除します（再帰なし）。

---

## 22. `sasurahime clean gradle`

**カテゴリ:** Sprint 5

**削除対象:** [Gradle](https://gradle.org/) の古いバージョンキャッシュ（`~/.gradle/caches/`）。キャッシュされた各アーティファクトの最新バージョンのみ保持します。

**detect の動作:**
1. `~/.gradle/caches/` をスキャンしてバージョンディレクトリを確認します。
2. アーティファクトグループごとに最新バージョンを保持し、それ以前の全バージョンのサイズ合計を報告します。

**clean の動作:**
1. バージョン比較を使用して古いバージョンを特定します。
2. `chflags -R nouchg` + `remove_dir_all` で古いディレクトリを削除します。

**安全性:** 各キャッシュアーティファクトの最新バージョンは常に保持されます。

---

## 23. `sasurahime clean huggingface`

**カテゴリ:** Sprint 5

**削除対象:** [Hugging Face](https://huggingface.co/) モデルキャッシュ（`~/.cache/huggingface/hub/` または `$HF_HOME/hub`）。

**detect の動作:**
1. `hub/` ディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. まず CLI を試行：`huggingface-cli delete-cache --yes` を実行します。
2. CLI が PATH にない場合は、`hub/` の内容を直接削除します（削除後に `hub/` ディレクトリを再作成）。
3. `$HF_HOME` は `is_safe_delete_target` で検証されます。

**安全性:** `$HF_HOME` はシステムパスのブロックリストに対して検証されます。CLI が直接削除より優先されます。

---

## 24. `sasurahime clean jetbrains`

**カテゴリ:** Sprint 5

**削除対象:** [JetBrains IDEs](https://www.jetbrains.com/)（IntelliJ IDEA、WebStorm など）の古いバージョンキャッシュ（`~/Library/Caches/JetBrains/` 内）。

**方法:** Gradle キャッシュと同様に、各 IDE キャッシュの最新バージョンを保持します。

**detect の動作:**
1. `~/Library/Caches/JetBrains/` をスキャンして IDE 単位・バージョン単位のディレクトリを確認します。
2. 古いバージョンを報告します。

**clean の動作:**
1. IDE ファミリーごとに古いバージョンを特定します。
2. `chflags -R nouchg` + `remove_dir_all` で削除します。

**安全性:** 各 IDE キャッシュの最新バージョンは常に保持されます。

---

## 25. `sasurahime clean library-logs`

**カテゴリ:** Sprint 5

**削除対象:** `~/Library/Logs/` 下のユーザーログファイル。ヒューリスティックルールを使用して削除候補を提案します。**インタラクティブ** — `--all` が使用されない限り選択プロンプトが表示されます。

**detect の動作:**
1. `~/Library/Logs/` の直下の子項目を読み取ります。
2. 各エントリについて：`dir_size` と `last_modified` 時間を測定します。
3. 2 つのヒューリスティックルールを適用します：
   - **大容量:** サイズ > 100 MB → `[large]` タグ
   - **古い:** 最終変更 > 90 日前 → `[stale N days]` タグ
4. 少なくとも 1 つのルールに該当するエントリを結果に含めます。
5. `CrashReporter`、`DiagnosticReports`、ドットエントリを除外します。

**clean の動作:**
1. detect と同じスキャンを実行します。
2. `--dry-run`：各エントリとその理由タグを表示します。
3. `--all`：確認なしですべての候補を削除します。
4. それ以外：`dialoguer::MultiSelect` ですべてのエントリを事前選択して表示し、ユーザーが確認して削除します。

**安全性:**
- `CrashReporter` と `DiagnosticReports` は **常に** 除外されます。
- ドットファイル・ディレクトリ（`.DS_Store`、`.localized` など）はスキップされます。
- 時計のずれによる未来のタイムスタンプは `SystemTime::now()` にクランプされます。
- `--dry-run` は副作用ゼロを保証します。

---

## 26. `sasurahime clean orbstack`

**カテゴリ:** Sprint 5

**削除対象:** [Orbstack](https://orbstack.dev/) Docker ランタイムキャッシュ。

**方法:** `orb prune`

**detect の動作:** `orb` が `PATH` にあるか確認します。

**clean の動作:**
1. `orb` が見つからない場合、メッセージを表示して終了します (0)。
2. `orb prune` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Orbstack CLI に委譲します。

---

## 27. `sasurahime clean pipx`

**カテゴリ:** Sprint 5

**削除対象:** [pipx](https://pypa.github.io/pipx/) キャッシュと未使用パッケージ。

**方法:** `pipx cache purge`

**detect の動作:** `pipx` が `PATH` にあるか確認します。

**clean の動作:**
1. `pipx` が見つからない場合、メッセージを表示して終了します (0)。
2. `pipx cache purge` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の pipx CLI に委譲します。

---

## 28. `sasurahime clean poetry`

**カテゴリ:** Sprint 5

**削除対象:** [Poetry](https://python-poetry.org/) パッケージキャッシュ。

**方法:** `poetry cache clear --all`

**detect の動作:** `poetry` が `PATH` にあるか確認します。

**clean の動作:**
1. `poetry` が見つからない場合、メッセージを表示して終了します (0)。
2. `poetry cache clear --all` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Poetry CLI に委譲します。

---

## 29. `sasurahime clean pre-commit`

**カテゴリ:** Sprint 5

**削除対象:** [pre-commit](https://pre-commit.com/) フック環境キャッシュ（`~/.cache/pre-commit/`、`$PRE_COMMIT_HOME`、または `$XDG_CACHE_HOME/pre-commit`）。

**detect の動作:**
1. 環境変数からキャッシュディレクトリを解決します：`$PRE_COMMIT_HOME` → `$XDG_CACHE_HOME/pre-commit` → `~/.cache/pre-commit`。
2. 合計サイズを報告します。

**clean の動作:**
1. まず CLI を試行：`pre-commit clean` を実行します。
2. CLI が PATH にない場合はキャッシュディレクトリを直接削除します。
3. 環境変数パスは `is_safe_delete_target` で検証されます。

**安全性:** 環境変数パスは検証されます。CLI が直接削除より優先されます。

---

## 30. `sasurahime clean rustup`

**カテゴリ:** Sprint 5

**削除対象:** 未使用の [Rust](https://www.rust-lang.org/) ツールチェーンバージョン（`rustup default` や `rustup override` で選択されていないもの）。

**detect の動作:**
1. `rustup toolchain list` を実行してインストール済みツールチェーンを列挙します。
2. デフォルトツールチェーン（`(default)` マーク）とオーバーライドツールチェーン（`(override)` マーク）を特定します。
3. デフォルト **でも** オーバーライド **でもない** ツールチェーンを未使用として報告します。

**clean の動作:**
1. detect と同じ検出ロジックを使用します。
2. 未使用のツールチェーンごとに：`rustup toolchain remove <name>` を実行します。
3. 解放バイト数を報告します（rustup 出力からパース）。

**安全性:** デフォルトとオーバーライドのツールチェーンは **決して** 削除されません。

---

## 31. `sasurahime clean spm`

**カテゴリ:** Sprint 5

**削除対象:** [Swift Package Manager](https://www.swift.org/package-manager/) のビルドアーティファクトとキャッシュされたパッケージ。

**方法:** `~/Library/Caches/org.swift.swiftpm/` と `~/Library/Developer/Xcode/DerivedData/SourcePackages/` を削除します。

**detect の動作:**
1. SPM キャッシュディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. キャッシュディレクトリに対して `chflags -R nouchg` を実行します。
2. 各キャッシュディレクトリに対して `fs::remove_dir_all` を呼び出します。
3. キャッシュされたパッケージチェックアウトとリポジトリクローンが削除されます（次回ビルド時に再取得されます）。

---

## 32. `sasurahime clean trash`

**カテゴリ:** Sprint 5

**削除対象:** `~/.Trash` — **スキャンのみ**。sasurahime はゴミ箱ディレクトリのサイズを報告しますが、削除は行いません（ユーザーは Finder からゴミ箱を空にしてください）。

**detect の動作:**
1. `~/.Trash` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. **`--dry-run`:** detect スタイルのスキャンを実行し、解放されるサイズを表示します。
2. **それ以外:** Finder からゴミ箱を空にするよう指示する警告を表示します。ファイルは削除されません。

**安全性:** sasurahime は `~/.Trash` の内容を削除することを拒否します。これは意図的な安全対策です。

---

## スキャン (`sasurahime scan`)

すべてのクリーナーで `detect()` を実行し、`comfy_table` を使ってフォーマットされたテーブルを表示します。副作用はありません — ファイルの作成、変更、削除は一切行いません。

---

## インタラクティブ / 自動モード

| モード | 動作 |
|--------|------|
| `sasurahime`（引数なし、TTY） | インタラクティブなチェックボックスリストを起動します。ターゲットを選択して確認すると、選択したターゲットがクリーンされます。 |
| `sasurahime --yes`（引数なし） | 確認なしですべての削除可能ターゲットをクリーンします。何もなければ 0 で終了します。 |
| `sasurahime scan`（非 TTY） | スキャン結果の一覧のみを表示します。 |
| `sasurahime clean <target>` | 特定のターゲットを直接クリーンします。 |

</details>
