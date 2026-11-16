<details{% unless page.lang == 'ja' %} open{% endunless %} markdown="1" data-lang="en" lang="en">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

<div class="doc-body" markdown="1">

sasurahime provides **55 clean targets**. Every target supports both `detect`
(read-only, no side effects) and `clean` (removal). All `clean` subcommands
accept `--dry-run`. Only caches and old versions are removed ― active runtimes
and packages are never touched.

The target list is generated from the `define_cleaners!` definition table in
`src/main.rs` (single source of truth: `command_name` / `dry_run` /
`is_special` / `ALL_TARGETS` and this listing all derive from its rows). See
`docs/HOWTO-ADD-target.md` for how to add one.

### Quick Reference

| Target | What it removes |
|--------|-----------------|
| `uv` | Stale `simple-vN` index dirs + `uv cache prune --force` |
| `brew` | `brew cleanup -s --prune=all` |
| `mise` | Unused runtime versions (cross-checks `.mise.toml` first) |
| `browsers` | Old Puppeteer/Playwright builds (keeps latest per family) |
| `cargo` | Cargo registry cache + `target/` dirs |
| `bun` | `bun pm cache rm` |
| `go` | `go clean -cache` |
| `pip` | `pip cache purge` |
| `npm` | `npm cache clean --force` |
| `yarn` | `yarn cache clean` |
| `pnpm` | `pnpm store prune` |
| `deno` | `deno cache -r` |
| `rustup` | Unused Rust toolchains |
| `flutter` | `dart pub cache clean` |
| `conda` | `conda clean --all -y` |
| `pipx` | `pipx cache purge` |
| `poetry` | `poetry cache clear --all` |
| `gem` | `gem cleanup` |
| `bundle` | `bundle clean` |
| `dotnet` | `dotnet nuget locals all --clear` |
| `caches` | All of: bun, go, pip, node-gyp, npm, yarn, pnpm |
| `node-gyp` | `~/.cache/node-gyp/` |
| `xcode` | Xcode DerivedData project dirs |
| `device-support` | Old Xcode DeviceSupport dirs (keeps recent N) |
| `simulator` | `xcrun simctl delete unavailable` |
| `spm` | SwiftPM cache dirs |
| `cocoa-pods` | `pod cache clean --all` |
| `gradle` | Old Gradle version caches |
| `maven` | `mvn dependency:purge-local-repository` |
| `sbt` | Scala/sbt + Ivy cache |
| `jetbrains` | Old JetBrains IDE caches |
| `vscode-extensions` | `~/.vscode/extensions/` |
| `docker` | `docker system prune -f` (dangling only) |
| `colima` | `colima prune --all --force` |
| `orbstack` | `orb prune` |
| `huggingface` | HF model cache (`hub/`) |
| `ollama` | Ollama model cache (interactive selection) |
| `act` | `~/.cache/act/` |
| `pre-commit` | pre-commit hook env cache |
| `terraform` | Terraform provider plugin cache |
| `tree-sitter` | tree-sitter parser cache |
| `volta` | `~/.volta/cache/` |
| `logs` | Log files older than N days |
| `library-logs` | `~/Library/Logs/` (heuristic, interactive) |
| `downloads` | `~/Downloads/` old files |
| `ios-backup` | iOS backups ― interactive only, never in `--yes` |
| `apfs-snapshot` | APFS local Time Machine snapshots ― interactive, `--all`, or `--thin` |
| `orphan-containers` | Leftover `~/Library/Containers` and `~/Library/Group Containers` of uninstalled apps - interactive only, never in `--yes` |
| `library-caches` | `~/Library/Caches/` per-app listing; known app caches removable - interactive only, never in `--yes` |
| `app-support` | Cache dirs of known apps in `~/Library/Application Support/` - interactive only, never in `--yes` |
| `trash` | Reports `~/.Trash` size (scan only) |

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
   `BrewCleaner::parse_size_str` (supports `GB`, `MB`, `KB` ― case-insensitive
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

**Safety (per AGENTS.md §Safety rules):**
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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches

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

**Category:** Sprint 3 ― Generic caches (aggregate)

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
| vscode-logs  | `~/Library/Application Support/Code/logs` | _(none)_           |

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
- Project directories are identified by reading `read_dir` ― only
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
1. `~/.cargo/registry/cache/` ― downloaded crate archives.
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
2. Runs `docker system prune -f` (dangling images only ― tagged images are
   preserved).
3. Reports 0 freed bytes.

**Virtual disk note:**
`sasurahime scan` prints the size of the Docker Desktop virtual disk
(`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw`, plus the
older layouts) under the table: bytes actually on disk and the maximum size.
`docker system prune` frees space inside the VM but the file does not always
shrink. After `clean docker` the on-disk size is compared before and after; if
it did not shrink, shrink it from Docker Desktop > Settings > Resources > Disk
image size, or Troubleshoot > Clean / Purge data (deletes all images,
containers and volumes). Detect never talks to the Docker daemon, so scan does
not hang when the daemon is stopped.

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
3. `$HF_HOME` is validated against `is_safe_delete_target` ― unsafe paths cause
   a fallback to the default.

**Safety:** `$HF_HOME` is validated against a blocklist of system paths.
CLI takes precedence over direct deletion.

---

## 24. `sasurahime clean jetbrains`

**Category:** Sprint 5

**What it removes:** Old version caches from
[JetBrains IDEs](https://www.jetbrains.com/) (IntelliJ IDEA, WebStorm, etc.)
under `~/Library/Caches/JetBrains/`.

**Method:** Similar to Gradle cache version pruning ― keeps the most recent
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
rules to suggest which entries to delete. **Interactive** ― opens a selection
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
3. Env var paths are validated via `is_safe_delete_target` ― unsafe paths
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

**What it removes:** `~/.Trash` ― **scan only**. sasurahime reports the size
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

**Safety:** sasurahime refuses to delete `~/.Trash` contents ― this is an
intentional safety measure.

---

## 33. `sasurahime clean volta`

**Category:** Sprint 5

**What it removes:** [Volta](https://volta.sh/) Node.js version manager
cache (`~/.volta/cache/`).

**Method:** Directory deletion.

**How detect works:**
1. Checks if `~/.volta/cache/` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Removes the `~/.volta/cache/` directory via `fs::remove_dir_all`.
2. Reports total freed bytes.

**Safety:** Only the `cache/` subdirectory is deleted; `~/.volta/` itself and
its toolchain binaries are preserved.

---

## 34. `sasurahime clean sbt`

**Category:** Sprint 5

**What it removes:** [sbt](https://www.scala-sbt.org/) build cache
(`~/.sbt/`) and [Ivy](https://ant.apache.org/ivy/) dependency cache
(`~/.ivy2/cache/`).

**Method:** Directory deletion.

**How detect works:**
1. Checks if `~/.sbt/` or `~/.ivy2/cache/` exist.
2. Reports the total `dir_size` of existing directories as pruneable.

**How clean works:**
1. Removes `~/.sbt/` and `~/.ivy2/cache/` directories via `fs::remove_dir_all`.
2. Reports total freed bytes.

**Safety:** Directories are deleted entirely ― dependencies will be re-fetched
on next `sbt compile`.

---

## 35. `sasurahime clean tree-sitter`

**Category:** Sprint 5

**What it removes:** [tree-sitter](https://tree-sitter.github.io/) parser
compilation cache (`~/.cache/tree-sitter/`).

**Method:** Directory deletion.

**How detect works:**
1. Checks if `~/.cache/tree-sitter/` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Removes the `~/.cache/tree-sitter/` directory via `fs::remove_dir_all`.
2. Reports total freed bytes.

**Safety:** Parsers will be recompiled on next editor use (Neovim, Helix, etc.).

---

## 36. `sasurahime clean simulator`

**Category:** Sprint 5

**What it removes:** iOS Simulator cache ― unavailable simulator runtimes
via `xcrun simctl delete unavailable`.

**How detect works:**
1. Checks if `~/Library/Developer/CoreSimulator` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. If `xcrun` is not in `PATH`, prints a message and exits (0).
2. Runs `xcrun simctl delete unavailable`.
3. Reports freed bytes from the simulator cache size change.

**Safety:** Delegates to the official `xcrun` CLI for safe simulator management.

---

## 37. `sasurahime clean device-support`

**Category:** Sprint 5

**What it removes:** Old Xcode DeviceSupport directories under
`~/Library/Developer/Xcode/* DeviceSupport/`. Keeps the N most recent major
versions per platform (default: 2).

**Scanned locations:**
- `~/Library/Developer/Xcode/iOS DeviceSupport/`
- `~/Library/Developer/Xcode/watchOS DeviceSupport/`
- Any other `* DeviceSupport/` directory

**Version comparison:**
- For each platform directory, subdirectories named by version (e.g. `17.0`,
  `18.0`) are collected. The major version (first number before `.`) is used
  for comparison.
- Versions are sorted descending by major version; the top N are kept.

**How detect works:**
1. Scans `~/Library/Developer/Xcode/` for `* DeviceSupport/` directories.
2. For each, collects version subdirectories and sorts them.
3. Reports the sum of disk sizes for all versions beyond the N most recent.

**How clean works:**
1. Same scan as detect.
2. If `--dry-run`: prints what would be removed per directory.
3. Otherwise: runs `chflags -R nouchg` then deletes via trash (or permanent).
4. Reports total freed bytes.

**Safety:**
- The N most recent major versions per platform are always kept.
- The `DeviceSupport` root directories are never deleted.

---

## 38. `sasurahime clean vscode-extensions`

**Category:** Sprint 5

**What it removes:** VS Code extensions cache (`~/.vscode/extensions/`).

**Method:** Directory deletion.

**How detect works:**
1. Checks if `~/.vscode/extensions/` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Removes `~/.vscode/extensions/` via `fs::remove_dir_all`.
2. Extensions will be re-downloaded on next VS Code launch.

**Safety:** Only the extensions cache is removed; VS Code configuration and
settings are preserved.

---

## 39. `sasurahime clean maven`

**Category:** Sprint 5

**What it removes:** Maven local repository cache
via `mvn dependency:purge-local-repository`.

**How detect works:**
1. Checks if `~/.m2/repository` exists.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. If `mvn` is not in `PATH`, prints a message and exits (0).
2. Runs `mvn dependency:purge-local-repository`.
3. Reports freed bytes from the repository size change.

**Safety:** Delegates to the official Maven CLI. Dependencies will be
re-downloaded on next build.

---

## 40. `sasurahime clean terraform`

**Category:** Sprint 5

**What it removes:** Terraform provider plugin cache
(`~/.terraform.d/plugin-cache/` or `$TF_PLUGIN_CACHE_DIR`).

**Method:** Directory deletion.

**How detect works:**
1. Resolves the plugin cache directory: `$TF_PLUGIN_CACHE_DIR` if set,
   otherwise `~/.terraform.d/plugin-cache/`.
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. Removes the plugin cache directory via `fs::remove_dir_all`.
2. Providers will be re-downloaded on next `terraform init`.

**Safety:** `$TF_PLUGIN_CACHE_DIR` is validated against `is_safe_delete_target`.
Only the provider cache is removed; Terraform state files and configurations
are preserved.

---

## 41. `sasurahime clean flutter`

**Category:** Sprint 5

**What it removes:** Flutter/Dart pub cache
via `dart pub cache clean`.

**How detect works:**
1. Checks if the pub cache directory exists (`$PUB_CACHE` or
   `~/.pub-cache/`).
2. Reports the total `dir_size` as pruneable.

**How clean works:**
1. If `dart` is not in `PATH`, prints a message and exits (0).
2. Runs `dart pub cache clean`.
3. Reports freed bytes from the cache size change.

**Safety:** Delegates to the official Dart CLI. Published packages will be
re-downloaded on next `dart pub get` or `flutter pub get`.

---

## 42. `sasurahime clean colima`

**Category:** Sprint 5

**What it removes:** [Colima](https://github.com/abiosoft/colima) VM disk
cache via `colima prune --all --force`.

**How detect works:**
1. Checks if `~/.colima/` exists.
2. Reports the total `dir_size` as pruneable.
   *Note: This is an estimated maximum ― actual freed space depends on
   which VM disk images are pruned.*

**How clean works:**
1. If `colima` is not in `PATH`, falls back to deleting `~/.colima/` directly
   via Trash (with `chflags -R nouchg` handling). Reports freed bytes.
2. If `colima` is found:
   - On interactive TTYs: shows a confirmation prompt warning that all stopped
     VM disk data (containers, images, volumes) will be deleted. Aborts if
     the user declines.
   - In `--yes` (non-TTY) mode: skips the confirmation prompt automatically.
3. Runs `colima prune --all --force`.
4. If `colima prune` exits with a non-zero status, prints a warning with the
   exit code and stderr output instead of silently treating it as success.
5. Reports freed bytes from the directory size change.

**Safety:**
- Confirmation prompt on interactive TTYs prevents accidental deletion.
- Fallback to direct deletion when CLI is missing ensures the cache is still
  cleaned even without the `colima` CLI installed.

---

## 43. `sasurahime clean ollama`

**Category:** Sprint 5

**What it removes:** [Ollama](https://ollama.com/) model cache
(`~/.ollama/models/`).

**How detect works:**
1. If `ollama` CLI is available, runs `ollama list` and parses model sizes
   from the output. Reports the sum of model sizes as pruneable.
2. Falls back to `dir_size` of `~/.ollama/models/` if the CLI is not found.

**How clean works:**
1. If `ollama` CLI is available:
   - Lists all installed models.
   - Opens an interactive `dialoguer::MultiSelect` prompt for the user to
     select models to remove (unless `--dry-run`).
   - Runs `ollama rm <model>` for each selected model.
2. If `ollama` CLI is not available:
   - Deletes `~/.ollama/models/` directory directly with
     `chflags -R nouchg` + `remove_dir_all` (or trash).
3. Reports total freed bytes.

**Safety:**
- Interactive selection prevents accidental deletion of large models.
- CLI takes precedence over direct deletion for proper model management.
- In `--dry-run` mode, models are listed but never removed.

---

## 44. `sasurahime clean ios-backup`

**Category:** Sprint 6

**What it removes:** iOS device backups from
`~/Library/Application Support/MobileSync/Backup/`. Each device appears as a
UUID-named subdirectory containing backup data (Manifest.db, etc.).

**How detect works:**
1. Checks if the backup directory exists under `MobileSync/Backup/`.
2. Reads subdirectories (UUID-named entries), measures each with `dir_size`.
3. Reports the total size of all backup directories as reclaimable.

**How clean works:**
1. Lists all backup directories with their sizes.
2. Opens an interactive `dialoguer::MultiSelect` prompt to select which backups
   to remove (unless `--dry-run`).
3. Runs `chflags -R nouchg` to clear macOS immutable flags.
4. Moves selected backups to Trash via `crate::trash::delete_path`.
5. Reports total freed bytes.

**Safety:**
- iOS backups are **irreversible** ― once deleted they cannot be restored
  (Trash only helps locally; backup data is gone).
- **Interactive only:** never runs in `--yes` mode. Non-TTY invocations are
  safely skipped with a message.
- In `--dry-run` mode, backups are listed but never removed.
- macOS immutable flags (`uchg`) are handled automatically.

---

## 45. `sasurahime clean apfs-snapshot`

**Category:** Sprint 6

**What it removes:** APFS local Time Machine snapshots via
`tmutil deletelocalsnapshots <date>` (`<date>` is `YYYY-MM-DD-HHMMSS`).

**How detect works:**
1. Checks if `tmutil` is available on the system.
2. Runs `tmutil listlocalsnapshots /` and parses output via
   `parse_snapshot_names` (only `com.apple.TimeMachine.<date>.local` lines are
   kept; the `Snapshots for disk /:` header is ignored). Dates outside the
   calendar ranges (month 1-12, day 1-31, hour 0-23, minute/second 0-59) are
   dropped.
3. Measures `/.MobileBackups` disk usage (if present) as a size estimate.
4. Returns `Pruneable` with the measured size (or 0 if `/.MobileBackups` does
   not exist).

**How clean works:**
1. Resolves exactly one mode from the flags: delete mode (default) or thin
   mode (`--thin`). `--all` and `--thin` are mutually exclusive, and
   `--urgency` requires `--thin`; each conflict fails with an explicit error
   (exit 1) before any snapshot command runs.
2. Delete mode lists all local snapshots from `tmutil listlocalsnapshots`.
   Without `--all`, it opens an interactive `dialoguer::MultiSelect` prompt
   (unless `--dry-run`). With `--all`, every snapshot is selected without
   prompting, and a non-TTY invocation is allowed. It then runs
   `tmutil deletelocalsnapshots <date>` for each selected snapshot.
3. Thin mode runs `tmutil thinlocalsnapshots / <target-bytes> <urgency>`
   (urgency defaults to 2; 4 needs an extra confirmation), letting macOS
   decide which snapshots to delete.
4. Reports freed bytes as the increase in free space on `/` between before and
   after the run. The value can be smaller than the real amount because
   purgeable space is reflected with a delay.

**Privileges:** `tmutil deletelocalsnapshots` requires root. sasurahime never
runs `sudo`; for each snapshot that fails to delete, it prints the
`sudo tmutil deletelocalsnapshots <date>` command to run manually. Only
Time Machine snapshots (`com.apple.TimeMachine.<date>.local`) are listed; other
snapshots are ignored. Failed deletions are returned as skipped entries whose
reason is `tmutil`'s own error output (or the spawn error), and the exit code
is 1 when no snapshot could be deleted.

**Safety:**
- Deleting snapshots disables local Time Machine protection until the next
  backup.
- **One mode per run:** `--all` and `--thin` are mutually exclusive and
  `--urgency` requires `--thin`; conflicting flags fail with an explicit error
  instead of silently picking one.
- **Interactive by default:** never runs in `--yes` mode. Without `--all`,
  non-TTY invocations are safely skipped with a message.
- In `--dry-run` mode (with or without `--all`), snapshots are listed but never
  removed.

---

## 46. `sasurahime clean gem`

**Category:** Sprint 5

**What it removes:** [RubyGems](https://rubygems.org/) old gem versions via
`gem cleanup`.

**How clean works:**
1. Runs `gem cleanup` to remove outdated gem versions from `~/.gem/`.
2. On next `gem install`, the latest version is fetched automatically.

**Safety:** Delegates to the official `gem` CLI. Gem sources are unaffected.

---

## 47. `sasurahime clean bundle`

**Category:** Sprint 5

**What it removes:** [Bundler](https://bundler.io/) cache via `bundle clean`.

**How clean works:**
1. Runs `bundle clean` to clear Bundler's cached gem files.
2. Cached gems are redownloaded on next `bundle install`.

**Safety:** Delegates to the official `bundle` CLI. Only cached/stale gems
are removed; `Gemfile.lock` is not modified.

---

## 48. `sasurahime clean dotnet`

**Category:** Sprint 5

**What it removes:** [.NET SDK](https://dotnet.microsoft.com/) NuGet cache via
`dotnet nuget locals all --clear`.

**How clean works:**
1. Runs `dotnet nuget locals all --clear` to purge all NuGet caches
   (http-cache, packages-cache, temp, plugins-cache).
2. Packages are re-downloaded on next `dotnet restore`.

**Safety:** Delegates to the official `dotnet` CLI. Project-level `packages/`
(check-in directories) are not affected.

---

## 49. `sasurahime clean orphan-containers`

**Category:** Sprint 6

**What it removes:** Folders under `~/Library/Containers` and
`~/Library/Group Containers` whose app is not installed. Selected folders are
moved to the Trash (use `--permanent` to delete them instead).

**How detect works:**
1. Collects the `CFBundleIdentifier` of every `*.app` directly under
   `/Applications`, `~/Applications` and `/System/Applications`, plus one level
   of sub-folders (for example `/Applications/Utilities`). Bundles whose
   `Info.plist` cannot be read are ignored.
2. A `Containers` folder is kept when its name equals an installed ID or is a
   dot-separated child of one (`com.foo.App.Helper` belongs to `com.foo.App`;
   `com.foo.AppX` does not). Matching is case-insensitive.
3. For `Group Containers`, a leading team ID (10 uppercase letters or digits
   plus `.`) and then a leading `group.` / `groups.` are removed before the same
   matching. If any folder of a team matches an installed app, every folder of
   that team is kept.
4. `com.apple.*` (including `group.com.apple.*`) and `com.docker.*` are never
   reported.
5. Returns `Pruneable` with the total size of the candidates, or `Clean` when
   there are none. Folders that cannot be read (no Full Disk Access) are not
   counted.

**How clean works:**
1. `--dry-run` lists the candidates and removes nothing.
2. Without `--dry-run`, a `dialoguer::MultiSelect` prompt opens with nothing
   selected. Only the chosen folders are moved to the Trash.
3. There is no `--all`. With a non-TTY stdin, or under `--yes`, nothing is
   removed.
4. When some folders could not be read, the number of them is printed together
   with a hint to grant Full Disk Access to your terminal. sasurahime never
   changes permissions.

**Limits:** Containers of iOS apps running on Apple silicon (`Wrapper/*.app`)
are not matched to an installed app and may be listed; review the list before
selecting.

**Safety:**
- Nothing is selected by default; `--yes` never runs this cleaner.
- `--dry-run` has no side effects.

---

## 50. `sasurahime clean library-caches`

**Category:** Sprint 6

**What it removes:** Caches of known apps under `~/Library/Caches/` (the entries
in the built-in app table whose base directory is `Caches`, for example
`typescript`, `gopls`, `electron`, `Microsoft Edge`, `BraveSoftware`).
**Interactive only.** Unknown apps are listed but never removed.

**Difference from `caches`:** `sasurahime clean caches` runs the CLI-based
cleaners (bun, go, pip, node-gyp, npm, yarn, pnpm). `library-caches` looks at
the per-app directories directly under `~/Library/Caches/`.

**How detect works:**
1. Reads the immediate children of `~/Library/Caches/`.
2. Skips dot-entries, symlinks, and directories owned by another cleaner
   (see below). Only known apps are measured.
3. Reports the sum of known caches larger than their threshold (64 MB)
   as pruneable. Unknown apps are not counted.

**How clean works:**
1. Lists every entry by size, with tags `[running]`, `[below threshold]`,
   `[display only]`.
2. **If `--dry-run`:** prints `would remove:` for each deletable entry and stops.
3. If stdin is not a terminal, prints a message and removes nothing.
4. Otherwise opens `dialoguer::MultiSelect` with nothing pre-selected, then
   moves the selected entries to the Trash (`--permanent` deletes them).
5. Entries whose app is running are skipped and reported; the app is never
   quit automatically.

**Excluded (owned by other cleaners, never listed):**

| Directory | Cleaner |
|---|---|
| `Homebrew` | `brew` |
| `ms-playwright*` | `browsers` |
| `node-gyp` | `node-gyp` |
| `org.swift.swiftpm` | `spm` |
| `JetBrains` | `jetbrains` |
| `CocoaPods` | `cocoa-pods` |
| `Yarn` | `yarn` |
| `pip` | `pip` |
| `go-build` | `go` |
| `pypoetry` | `poetry` |
| `deno` | `deno` |
| `com.apple.*` | OS managed |

**Safety:**
- Nothing is selected by default; `--yes` never runs this cleaner.
- Unknown apps are display-only.
- `--dry-run` guarantees zero side effects.

---

## 51. `sasurahime clean app-support`

**Category:** Sprint 6

**What it removes:** Cache subdirectories of known apps (Slack, Claude desktop,
Obsidian, VS Code, Google Chrome) under `~/Library/Application Support/`.
Only paths on a fixed allowlist can be removed (`Cache`, `Code Cache`,
`GPUCache`, `Service Worker/CacheStorage`, `logs`, and similar). Settings,
logins, IndexedDB, Local Storage and workspaces are never touched.

**How detect works:** Sums the allowlisted directories per app. An app is
reported only when its total exceeds 64 MiB. The leftover list described below
is not included in the total.

**How clean works:**
1. `--dry-run` prints the candidate directories and exits.
2. Otherwise a `dialoguer::MultiSelect` opens with nothing selected.
3. For a running app, a confirmation asks whether to quit it. Without
   approval the app's directories are skipped. After deletion, an app that
   sasurahime quit is relaunched if its hint entry configures a relaunch app.
4. Selected directories are moved to the Trash (`--permanent` deletes).

**Leftovers (display only):** Folders directly under `Application Support`
that match no installed `.app` by name or bundle ID tail are listed. Nothing
is deleted; selected entries are opened in Finder. Selecting more than 5
entries asks for confirmation before any window is opened.

Entries smaller than 1 MiB, modified within the last 30 days, symbolic links,
Apple-provided folders and vendor parent folders (`Google`, `Microsoft`,
`Adobe`, `JetBrains`, and similar) are not listed.

**Safety:**
- Allowlist only; `..`, absolute paths and symbolic links are rejected.
- Never runs under `--yes`; a non-TTY invocation does nothing.
- `--dry-run` has no side effects.

---

## 52. `sasurahime stats`

**Category:** Sprint 5

**What it shows:** Aggregated deletion history and statistics.

**How it works:**
1. Every successful `sasurahime clean <target>` (where `freed_bytes > 0` and not `--dry-run`) appends a
   `HistoryEntry` to `~/.local/share/sasurahime/history.json`.
2. Each entry contains: `timestamp`, `cleaner`, `freed_bytes`, `skipped_count`.
3. The history file uses atomic writes (temp file + rename) to prevent corruption.
4. `sasurahime stats` reads the file, computes aggregates, and prints a summary table.
5. If the history file is missing or corrupted, a friendly message is shown and the program
   exits with code 0 (no crash).

**Supported flags:**
- `--last N` ― show only the most recent N entries (default: 10).

**Output example:**
```
$ sasurahime stats
╔══════════════════════════════════════╗
║  sasurahime Statistics              ║
╠══════════════════════════════════════╣
║  Total freed:  12.5 GB              ║
║  Runs:         15                   ║
╚══════════════════════════════════════╝

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-27 10:30   uv             500.0 MB
  2  2026-05-26 22:15   brew           1.2 GB
```

**Implementation:** `src/history.rs` (362 lines), 9 unit tests + 6 E2E tests.

---

## Configuration File: `~/.config/sasurahime/config.toml`

**Category:** Sprint 5

The config file is optional ― if absent, sensible defaults are used.

**Example:**
```toml
# Exclude cleaners from scan/TUI listing
exclude = ["huggingface", "ollama"]

# User-defined cache directories
[[custom]]
name = "my-project"
path = "~/work/.cache"

# Per-cleaner age/size filters (DeleteDirs cleaners only)
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

# Log retention
[logs]
keep_days = 14
```

**Config fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `trash_mode` | boolean | `true` | Send deleted files to Trash by default |
| `exclude` | string[] | `[]` | Cleaners to hide from scan/TUI |
| `[[custom]]` | table[] | `[]` | User-defined cache directories |
| `custom[].name` | string | required | Display name |
| `custom[].path` | string | required | Path (supports `~`) |
| `[cleaner.<name>]` | table | ― | Per-cleaner filter settings |
| `cleaner.<name>.older_than_days` | integer | unset | Only delete entries older than N days |
| `cleaner.<name>.larger_than_mb` | integer | unset | Only delete entries larger than N MB |
| `[logs]` | table | ― | Log retention settings |
| `logs.keep_days` | integer | `7` | Global log retention period |
| `[[logs.targets]]` | table[] | `[]` | Extra log directories |
| `logs.targets[].name` | string | required | Display name |
| `logs.targets[].path` | string | required | Path (supports `~`) |
| `logs.targets[].exclude` | string[] | `[]` | Filenames to never delete |

**CLI flag override:**
```bash
sasurahime scan --config /path/to/config.toml
```

---

## Scan (`sasurahime scan`)

Runs `detect()` on every cleaner and prints a formatted table via
`comfy_table`. Rows are ordered by size, largest first; targets without a
size (clean or not found) come last. No side effects ― never creates,
modifies, or deletes files.

---

## Interactive / Auto

| Mode | Behavior |
|------|----------|
| `sasurahime` (no args, TTY) | Opens an interactive checkbox list. Select targets, confirm, and selected targets are cleaned. |
| `sasurahime --yes` (no args, any) | Cleans every pruneable target in the unattended set (uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support) without prompting. All other targets (docker, npm, cargo, downloads, ...) appear in `scan` and the interactive list but are never run by `--yes`. Exits with 0 if nothing is found. |
| `sasurahime scan` (non-TTY) | Prints the scan table only. |
| `sasurahime clean <target>` | Cleans a specific target directly. |


</div>

</details>
