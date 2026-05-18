# sasurahime — Supported Targets

sasurahime provides **14 clean targets** grouped into three categories:
**Sprint 1** (core language tools), **Sprint 2** (runtime/browser tooling),
and **Sprint 3** (system caches and logs).

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
