# sasurahime — Supported Targets

sasurahime provides **32 clean targets** grouped into sprints.
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

| Mode | Behaviour |
|------|-----------|
| `sasurahime` (no args, TTY) | Opens `dialoguer::MultiSelect` checkbox list. User selects targets, confirms, and selected targets are cleaned. |
| `sasurahime --yes` (no args, any) | Cleans every pruneable target without prompting. Exits with 0 if nothing is found. |
| `sasurahime scan` (non-TTY) | Prints scan table only. |
| `sasurahime clean <target>` | Cleans a specific target directly. |
