# Changelog

All notable changes to sasurahime will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Fixed

- The plugin protocol's JSON extractor now tracks JSON string state
  (quotes and escapes), so a `}` inside a JSON string value can no longer
  truncate the object and leave the plugin permanently unavailable.
- Plugin-claimed `bytes` and `freed` are capped at the size sasurahime
  verified on disk, so a plugin (or anything on PATH named `sasurahime-*`)
  can no longer inflate the scan total, the "Total freed" line, or the
  history. This also fixes the overflow panic a u64::MAX claim triggered
  in the scan total.
- Every prompt call site threads the live reporter down from `clean()`
  instead of hardcoding a suppressor whose suspend is a no-op, so the
  dialoguer redraw bug can no longer reproduce at the interactive
  cleaners. A source-pin test keeps the port free of literal references.
- `sasurahime clean xcode --sub <typo>` now errors with the expected
  spellings instead of silently cleaning DerivedData only, and the CLI
  help no longer claims "(default: all)" (the actual default is
  derived-data only).
- `sasurahime clean apfs-snapshot --all --thin <size>` now errors (exit 1)
  instead of thin silently winning over `--all`, and `--urgency N` without
  `--thin` errors instead of being silently ignored. The cleaner carries
  one explicit `SnapshotMode` (delete or thin), and the command help and
  docs state the exclusivity.
- `clean ios-backup` wrapped the shared delete ledger's result in a bare
  `freed` result, so its skipped entries and deleted paths were dropped:
  an all-skipped run exited 0 and `--verbose` lost the audit trail. The
  full result now propagates, so an all-skipped run exits 1 (the same
  contract every other ledger user pins).
- A fatal per-item error in `clean uv`'s old `simple-vN` removals aborted
  the whole clean (the prune never ran); the deletions now ride the shared
  ledger, so a fatal item is recorded as a skipped entry, the remaining
  items delete, and the prune still runs. The same migration records the
  CLI-fallback fallback deletes (colima, huggingface, pre-commit),
  ollama's fallback and `clean project-artifacts`' unsafe-path refusals
  as skipped entries with a prose reason, so the skip channel and the
  exit code see them.
- `clean project-artifacts` never received the live reporter, so its
  per-entry progress ticks never printed; the reporter now threads down
  and the heaviest cleaner prints the standard progress again.
- A batch run (`clean all --include-manual`, `auto`) reported a cleaner
  cancelled at its confirm prompt as an `Error cleaning <target>` failure
  line; the batch now collects it as a separate `cancelled` row and keeps
  going, so declining colima no longer reads as a script failure. The
  single-target path's calm exit is unchanged.
- `sasurahime auto --deep-suppress` printed the `Cleaning <target>...
  [OK]` status line the single-target path suppressed; the status line's
  owner is now the reporter kind in both engines, so the two agree.
- The orphan scan read one canonical app-root list that was missing the
  `/Applications/Utilities` roots, so an app installed two folders under
  `/Applications` was invisible and its container could be reported as
  an orphan while the app was still installed. Both cleaners now read
  one root list (the superset) and the README promise matches it.
- `--dry-run` never reached the hint flow, so answering `y` at the hint
  prompt under `sasurahime --dry-run` deleted for real. The flag now
  threads down, and a deletion the hint flow performs is recorded in the
  history and reported in `stats` like every other cleaner's.
- `--permanent clean caches` dropped the resolved clean options, so the
  node-gyp fallback delete used the default `Trash` mode; the caches arm
  now forwards the resolved options and the permanent promise holds.

### Changed

- The per-entry deletion bookkeeping is now one loop behind one
  interface: `delete_ledger` owns the progress ticks, the fatal/skip
  classification, the freed sums, the `uses_trash` truth (deletion
  successes only), the deleted-path audit trail and the `CleanResult`
  assembly, and the cleaners that hand-rolled it (mise, the generic cache
  path cleaners, browsers, logs, custom paths, cargo, xcode, gradle,
  jetbrains, and ollama's `ollama rm` deletions) ride the same ledger.
  Two visible consequences:
  - a fatal per-item error (an unsafe path, a vanished directory) is
    recorded as a skipped entry with a prose reason and the remaining
    items still delete in every migrated cleaner (six of the migrated
    loops used to abort the whole clean with `?`); the generic cache path
    cleaners' unsafe-path warning is now a recorded skip;
  - `clean gradle`, `clean jetbrains` and the cargo registry cache print
    the standard per-entry progress they never had;
  - a run where every entry is skipped now exits 1 in the cleaners whose
    loop recorded nothing (mise's `{tool} {version}` removals, the
    generic path cleaners' unsafe-path refusals) ― the same all-skipped
    contract the other ledger users pin.
- Per-entry output is normalized to the ledger's standard
  `[<target>] removed: <path> (freed <size>)` line: the mise
  `{tool} {version}` label and the logs cleaner's per-target prefix fold
  into the printed path and target label, and a CLI-delegated deletion's
  failure (ollama) keeps its prose reason in
  `SkippedEntry(SkipCause::Reason)` ― never disguised as a `DeleteError`.

- `clean colima` and `clean all --include-manual` refuse the `~/.colima`
  fallback delete on a non-TTY stdin (the same floor as ios-backup and
  apfs-snapshot): colima's full deletion ― `~/.colima` with the VM's disk
  images ― is TTY-only, so an unattended run now prints the standard
  "not a terminal" remedy and deletes nothing instead of deleting
  unconfirmed. A `colima prune` with the CLI installed still runs, and the
  TTY confirm prompt is unchanged.
- `--yes --permanent` aborting at the "Are you sure?" confirmation now
  exits non-zero (it exited 0), so a script can tell an aborted run from
  a completed one. The prompt itself is unchanged, and the `--yes` help
  no longer claims "without prompting" for `--permanent` runs.
- `--yes` runs now report the targets an interactive gate skipped
  ("needs an interactive terminal") instead of only a stderr line, so an
  unattended run where ollama models were never cleaned is visible in the
  result notice. The skip behavior itself is unchanged.
- The dead deletion sinks are gone: the compatibility free function
  `delete_path` (zero callers, bypassed the scope tripwire) and the
  orphaned `is_skippable_error` (its ErrorKind list lived only in
  `classify_io_error`). Deletions reach the scanner through one seam.
- The batch reports (`auto`, `clean caches`, `clean all`) now run through
  the shared report functions the TUI already had instead of hand-written
  copies in `main.rs`. One visible consequence: "Total freed" now has the
  leading blank line every batch report prints (the three CLI arms dropped
  it). Line wording and output streams are unchanged.
- The prompt port now resolves TTY-ness itself (`CleanPrompt::
  for_current_run`): the "call `gate_interactive` first" ordering
  constraint is gone and the per-cleaner gate is deleted, so the non-TTY
  remedy line prints right before the prompt instead of at the top of the
  cleaner. An unavailable prompt is a `None` selection instead of an empty
  one, so "not a terminal" and "nothing selected" can never print
  together. `clean apfs-snapshot --all` still skips the TTY check (an
  explicit opt-in) and the colima fallback refusal is unchanged.
- The unattended target selection has one owner: `UnattendedPolicy
  { Auto, All, Yes { results } }` with `select()` in the new
  `unattended` module. `sasurahime auto`, `clean all` and `--yes` used to
  hand-write three intentionally different filter loops; they now
  classify through one function, and skipped targets carry a typed
  `SkipReason` (NeverUnattended / NeedsOptIn / NeedsExplicitSelection)
  with the target's risk. `Yes` carries its scan results in the variant,
  so the scan-driven flow cannot grow a registry-driven filter (and
  vice versa). Wording, output streams, the `auto` `[policy].present`
  gate and the interactive-gate notice are unchanged, and a matrix unit
  test pins all 48 cells (3 policies x 4 risks x opt-in x scan status).
- `CliCleaner::clean_with_mode` is split into two internal seams
  (`run_tool` and `fallback_delete_dir`), the warning for a tool that
  exited non-zero has one owner (`warn_tool_failure`), and the colima
  confirm gate runs through the new `CleanPrompt::confirm_on_tty` port.
  One visible consequence: the fallback cleaners' CLI-failure warning
  (huggingface, pre-commit) moved from the log channel to a stderr line
  with the `[<target>]` prefix the other cleaners print. Exit codes,
  gate order and the non-TTY refusal are unchanged.
- The `CleanTarget` special variants are promoted into the macro table:
  the CLI name, the descriptions, `command_name`/`dry_run`/`is_special`
  and the target lists are generated from one row per target, so adding
  a target is one line (and a missing CLI name is a compile error, not a
  runtime pin). The caches batch is generated from the table's group
  rows, the xcode runner is one function shared by the CLI arm and the
  seam, and the registry builds from one composition. CLI help, the
  `targets` output and all exit paths are unchanged.
- The installed-app concept lives in one module (`installed_apps`): the
  app-support and orphan-containers cleaners stop importing each other,
  the root list and its CI trim are single-sourced, and a bundle path is
  read with `plutil` once. Behavior is unchanged apart from the root
  list fix above.

### Added

- `sasurahime clean hints` deletes the hint-table candidates (the same
  quit → delete → relaunch flow the interactive offer runs) and
  `--sub "<name>"` restricts it to one candidate, so the hint flow rides
  the shared engine: dry-run, history, skip reporting and the ledger
  contract come for free. `targets` lists `hints` and the TUI selects
  its sub-targets like xcode's.

## [0.4.3] - 2026-10-29

### Changed

- The shared delete loop (`crate::trash::delete_entries`) changes two
  cleaners' failure semantics as part of the collapse: `clean ios-backup`
  and `clean device-support` now record skipped entries instead of only
  logging them, so a run where every entry is skipped exits 1 (it exited 0
  in 0.4.2), and a fatal per-entry error (e.g. a vanished path) is reported
  as a skipped entry with the remaining entries continuing instead of
  aborting the run. device-support's skip report moved from the log to
  stderr and its success line now reports the freed bytes. The loop-level
  contracts are pinned by tests.
- Deletion failures are classified by type instead of by message text
  (`DeleteError` / `SkipCause`): the seam's skip/fatal decision matches on
  the variant, so rewording an error message can no longer flip a skip into
  a fatal failure. Skipped entries carry the typed cause; the prose is
  display-only. The sub-target selection sum re-binds by the carried size
  instead of re-matching strings, and the Xcode sub-target spelling parser
  is one parse for every accepted spelling.
- The deletion-safety policy moved into its own module
  (`crate::safety`): `is_safe_delete_target` and `resolve_env_dir` no
  longer live in the cleaner layer, so the platform-side delete seam
  (`trash::safe_delete`) no longer calls back into it. The "CLI tool
  first, fallback delete" adapter moved to `crate::cli_fallback`.
- The per-cleaner delete loops collapsed into one shared loop
  (`crate::trash::delete_entries`): progress ticks, skip classification,
  freed-byte sums, and the deletion-mode truth now live in one place
  instead of 6 copy-pasted loops (library-caches, library-logs,
  orphan-containers, app-support, ios-backup, device-support).
- Interactive prompts now run through a single prompt port
  (`crate::prompt`): the TTY gating, the live-output suspension, and the
  dialoguer calls are owned in one place instead of 8 cleaner files, so
  the dialoguer-redraw fix (one short prompt line under a suspended
  progress line) can no longer resurface per call site. The hand-rolled
  "not a terminal" guards were replaced by one standardized gate, and the
  TUI's confirmation suppression moved into the port.
- Plugins can report what they removed by printing a JSON object
  (`{"freed": N, "uses_trash": B}`) as the last line of stdout. Plugins that
  do not print JSON keep working through the exit-code-only protocol.

### Fixed

- Review quick-win bundle. `TrashModeGuard::set` acquires the test lock
  internally and holds it until drop, which removes the hand-rolled 3-line
  lock preamble from the unit tests that pin the trash mode and closes
  the lock-forgotten flake class. `docs/HOWTO-ADD-target.md` no longer
  points at the removed `AUTO_CLEAN_TARGETS` name list: `--yes` eligibility
  is derived from `risk()` (`Risk::Safe`) and the `ManualOnly` wrap for
  non-Safe targets is automatic. A membership test pins `clean caches` to
  the 7 generic cache table rows (bun, go, pip, node-gyp, npm, yarn, pnpm),
  so dropping one of the 7 from the dispatch fails the test. colima's
  after-clean disk-image notice is printed again on the
  `detect_dir` clean path: `Freed:` measures the cache directory, not the
  VM's sparse disk image, and fstrim only reclaims unused blocks, so the
  image often does not shrink -- the notice reports that with the remedy.
- ollama: clean now skips the interactive selection when stdin is not a
  terminal, so `--yes` can clean the Safe-classified ollama target instead of
  failing with a dialoguer error.
- Cleaners now verify the exit codes of the external tools they invoke
  (mise, rustup, ollama, brew, uv, tmutil, osascript, open). A failed
  removal no longer claims the target's bytes: the failure is recorded as
  a skipped entry and the remaining items continue. `mise ls --current`
  output that cannot be parsed is fail-closed: the cleaner refuses to
  delete any runtime and scan reports the target as unavailable, so an
  output-format change can no longer mark every installed runtime
  deletable. brew warns when its freed-bytes line could not be parsed
  even though the cache directory shrank.

## [0.4.2] - 2026-10-29

### Changed

- Trash deletion uses the `NsFileManager` backend (`trashItemAtURL`) instead
  of the default Finder backend, which spawned `osascript` and could sit for
  minutes on a hidden macOS TCC permission dialog with no output. Behavior
  change: items moved to the Trash may not show "Put Back" in Finder's
  context menu -- restore by dragging them out of the Trash.
- `clean orphan-containers` shows the full path in the selection list (long
  paths ellipsize the middle at column width 56) instead of the folder name
  only, and notes the Put Back limitation before the selection.
- Spinners carry an always-visible elapsed timer (`({elapsed})`) on the
  cleaning spinner and the parallel scan bar. The delete phase enables a
  per-second steady tick so the timer updates every second, and the
  interactive selection runs with the live line hidden instead of a frozen
  `Cleaning...` label above the prompt.

### Fixed

- colima: `clean` now reclaims what the scan measures. `colima prune --all
  --force` only clears downloaded-asset caches, so the run freed 0 B while
  the scan reported the VM disk. The clean now follows up with `colima ssh
  -- sudo fstrim -av`, which asks the running VM to discard unused blocks so
  the sparse disk image's physical size shrinks. The follow-up is non-fatal:
  with the VM stopped, the remedy is printed and the prune-only freed bytes
  are still reported. The confirmation prompt now describes this
  non-destructive action instead of `colima delete`.
- The interactive confirm prompt prints the explanation as normal output and
  asks with a one-line `Proceed?` prompt. dialoguer's `clear_line` erases
  one terminal line, so a wrapped Confirm prompt left the first render's
  lines behind and the message appeared twice.

## [0.4.1] - 2026-10-25

### Security

- `cargo update` moves 92 packages to the latest semver-compatible versions
  and remediates [RUSTSEC-2026-0204](https://rustsec.org/advisories/RUSTSEC-2026-0204)
  (crossbeam-epoch 0.9.18 -> 0.9.21, reached via `rayon`), and moves off
  anyhow 1.0.102 (RUSTSEC-2026-0190 unsoundness in `Error::downcast_mut()`,
  now 1.0.104).

### Changed

- `comfy-table` 7 -> 8: stringified presets are replaced with `TableStyle`
  constants, the six `load_preset` call sites in scanner.rs and explorer.rs
  rename to `load_style`, and table formatting time improves by up to ~70%
  on large tables.
- `dirs` 6 -> 7: no code changes, `home_dir()` is signature-compatible.
- The seam tests simulate a refusing trash backend deterministically instead
  of relying on a read-only parent: the platform backends move entries out
  of read-only parents on modern macOS, so on a machine where the backend is
  not TCC-blocked the tests saw the entries deleted and their skip
  assertions failed. A test-only backend override is installed under
  TEST_LOCK, and the library-logs `clean all` test pins the global trash pin
  it previously passed via state leakage.

## [0.4.0] - 2026-10-24

### Changed

- `colima` is now classified `Risky` (its clean destroys all stopped VM disk
  data, irreversibly), so `--yes` and `auto` no longer run it unattended; the
  TUI still offers it behind a confirmation.
- `[custom]` targets are now reachable with `clean <name>`: previously the
  external plugin catch-all shadowed them, so a configured custom target
  could never be cleaned from the CLI. Name collisions with registered
  targets are warned at startup, and paths on the deletion blocklist
  (`/Volumes`, `/net`, `/Users/Shared`, `/Library`, `/opt`,
  `/private/var/vm`) refuse with exit 1.
- `auto` now treats a config as policy-driven only when the `[policy]`
  section actually exists, instead of counting `[custom]` sections.
- Per-entry skips from PathCleaner/CliCleaner paths are reported as skipped
  entries instead of being silently dropped, and `uses_trash` is only
  claimed when something was actually deleted.
- TUI sub-target cleans now report the real freed bytes; previously a
  self-re-exec fabricated `0 B`, under-counting `Total freed`.
- `ollama` model rows whose size cannot be parsed default to `0 B`
  explicitly (the listing stays best-effort) instead of silently.

- **BREAKING (`--yes`):** every target now carries a risk classification
  (`Safe` / `Moderate` / `Risky` / `Manual`) and `--yes` cleans only `Safe`
  targets. Previously a central 13-name list decided this; `docker`, `cargo`,
  `npm` and other large targets now require an explicit choice. Skipped
  targets are reported with their classification (e.g. `cargo (Moderate)`).
  Existing cron users should review the classification table in the README.
- `scan` and `scan --json` show the classification per row (`Risk` column /
  `risk` field); the TUI initial selection follows the same classification.
- Unclassified targets default to `Moderate`, so adding a cleaner can never
  silently opt it into unattended deletion.

### Added

- `Cleaner::risk() -> Risk` with the classification table covering all
  registered targets; `auto_clean_allowed()` is derived from it
  (`risk() == Safe`).
- `doctor` subcommand that diagnoses Full Disk Access problems before
  scanning, so missing FDA no longer looks like silent scan gaps.
- `scan --json` machine-readable output with resolved sizes, and
  `ScanStatus::Unavailable` for entries that cannot be measured (permission
  errors, command failures): `0 B` entries are excluded from the total
  instead of reported as cleanable.
- Persistent scan cache (`scan-cache.json`, 1-hour expiry) that makes a
  second scan of unchanged directories fast; `--no-cache` disables it.
- Global filters (`--older-than`, `--larger-than`) that apply to every
  cleaner, plus `clean all` with risk-based skipping for unattended runs.
- `clean project-artifacts` purges stale `node_modules` / `target` /
  `.build` directories that sit next to their manifest; protected by file
  mtimes only, so uncommitted changes are NOT checked.
- `clean apfs-snapshot --thin` runs `tmutil thinlocalsnapshots` with a
  decimal size and urgency (1..=4; 4 asks for an extra confirmation).
- External plugin targets: `sasurahime-*` executables on PATH answer a
  minimal scan/clean protocol, appear in `scan` / `targets` with a
  `(plugin)` marker, and are isolated by a timeout (10 s default) so one
  broken plugin cannot take a scan down.
- `auto` command: unattended policy apply driven by a `[policy]` config
  section; refuses to run without one, and irreversible targets never run
  unattended. A launchd plist example is documented in the README.
- Two-axis freed display: `clean` now reports logical and physical bytes
  separately, with restore guidance when files went to the Trash.
- `/private/var/vm` swap files are shown in `scan` with uptime guidance.
- Deletion history records the method (`trash` / `delete`) and appends are
  fsynced before rename, so an interruption cannot leave a torn file.

### Changed

- `delete_path` takes the trash mode as an argument (`DeleteCtx`) instead of
  reading a global; the global-based call sites keep working unchanged.

### Fixed

- TUI selecting `Archives` silently cleaned `DerivedData`: the old sub-target
  re-exec passed the display name where the parser expected the internal
  spelling. Both spellings now resolve to the same subcategory.
- Global `--dry-run` now reaches `External` plugin targets, and
  `clean <custom> --help` no longer runs the plugin clean.
- An empty `HOME` environment variable no longer breaks home resolution:
  it falls back to `dirs::home_dir()` everywhere instead of pointing at
  the filesystem root.
- `scan --json` flushes the scan cache before its early return, so the
  cache is not stale for the next run.

---

## [0.3.0] - 2026-10-21

### Added

- **`scan` now covers all 51 targets** (previously 15). Interactive-only and
  previously invisible targets now appear in the table.
- **`orphan-containers`** finds leftover `~/Library/Containers` and
  `~/Library/Group Containers` directories of uninstalled apps and moves them
  to the Trash (interactive only).
- **`library-caches`** lists per-app directories under `~/Library/Caches/` so
  known app caches can be removed (interactive only).
- **`app-support`** removes cache directories of known apps under
  `~/Library/Application Support/` by allow-list, and lists leftovers with a
  "reveal in Finder" prompt (interactive only).
- **Docker / Colima virtual disk size.** `scan` reports the image file size
  and prints a shrink hint instead of an unknown size.
- **Startup warning for ineffective `[cleaner.<name>]` filters.** Settings a
  cleaner will not apply are reported once on stderr, including unknown
  cleaner names.
- **`--yes` lists skipped targets.** Targets that need an explicit selection
  are named in one line instead of being dropped silently.

### Changed

- **`--yes` never runs `ios-backup` or `apfs-snapshot`.** The unattended set
  is now 13 targets; both require an explicit choice (the picker or `--all`)
  and are reported as skipped.
- **"moved to Trash" reflects the cleaner.** The message now depends on
  `CleanResult.uses_trash`, so permanent deleters such as `uv` and `rustup`
  no longer claim Trash.
- **`apfs-snapshot` validates dates** (month, day, hour, minute, second
  ranges) and reports the `tmutil` error output as the skip reason.

### Fixed

- `--yes` no longer opens a selection prompt for `ios-backup` or
  `apfs-snapshot` on a TTY.

### Internal

- Finder `open` calls are unified in `command::open_path` and tested through
  `CommandRunner`; no direct `std::process::Command::new("open")` remains.
- `cargo clippy --all-targets` passes, and tests that set environment
  variables take `TEST_LOCK`.

---

## [0.2.1] ― 2026-06-24

### Added

- **`sasurahime history` as a visible alias for `sasurahime stats`.**
  Users can now type `sasurahime history` to view deletion history and
  statistics, matching the familiar `mo history` pattern from Mole.
- **Homebrew tap `armaniacs/sasurahime` is now available.**
  Install with `brew tap armaniacs/sasurahime && brew install sasurahime`.

### Internal

- **History alias integration tests.** Added E2E coverage ensuring
  `history` produces identical output to `stats`, including `--last` filtering.

---

## [0.2.0] ― 2026-05-29

### Added

- **3 new clean targets: `gem`, `bundle`, `dotnet` (PBI-10/11/12).**
  `sasurahime clean gem` runs `gem cleanup`, `clean bundle` runs `bundle clean`,
  `clean dotnet` runs `dotnet nuget locals all --clear`. All follow the standard
  `command_cleaner` pattern with `is_available()` / `--dry-run` support.
- **`CleanOptions` struct + `clean_with_opts()` trait method (PBI-06).**
  `LibraryLogsCleaner.clean_all()` is now `clean_with_opts(opts)` on the `Cleaner`
  trait, eliminating the special-case dispatch in `main.rs`.
- **`deleted_paths` on `CleanResult` (PBI-05).** Each cleaner now records
  successfully-deleted file paths for audit trail (used in `--permanent` mode).
- **`log` + `env_logger` structured logging (PBI-05).** Runtime diagnostics
  use `log::warn!`/`log::error!` instead of raw `eprintln!`. Controlled via
  `RUST_LOG` environment variable (default: `warn`).
- **Privacy section in README (PBI-08).** Covers data access, telemetry policy,
  and `history.json` retention. Japanese version included.
- **iOS backup warning strengthened (PBI-08).** Now explicitly mentions
  personal data types (contacts, messages, photos) before deletion.

### Changed

- **`is_skippable_error` substring matching → prefix matching (PBI-01).**
  `contains("Permission denied")` → `starts_with("Permission denied")` to avoid
  false positives when keywords appear inside longer error messages.
- **GradleCleaner / JetBrainsCleaner now use `trash::delete_path` (PBI-02).**
  Previously used `fs::remove_dir_all` directly, bypassing the global
  `trash_mode` setting. Both now respect Trash / `--permanent` like all other
  cleaners.
- **17 "tool not found" E2E tests parameterized with `rstest` (PBI-03).**
  One parameterized test replaces 17 near-identical functions. Reduces test
  compilation overhead.
- **CargoCleaner / MiseCleaner cache detect results (PBI-04).**
  `std::sync::OnceLock` avoids re-running `find_target_dirs` /
  `scan_pinned_versions` when `clean()` follows `detect()` in the same process.
- **`cmd_name!` macro eliminated (PBI-07).** CLI names now come directly from
  `define_cleaners!`'s `$cli_name` literal; 118 lines removed.
- **`exit_code() != 0` duplication consolidated (PBI-07).** `run_and_exit()`
  wrapper replaces 8 manual exit-code checks.
- **`scan` stderr loss fixed (PBI-05).** `clean_cli_or_fallback` now logs
  stderr before bailing on non-zero exit.
- **`hint.rs` split into 4 sub-modules.** 1094-line monolith → `mod.rs`,
  `apps.rs`, `detector.rs`, `cleaner.rs`. No behavioral change.

### Fixed

- **`#[allow(dead_code)]` stale annotations cleaned (PBI-09).**
  14 locations reviewed: 6 removed (never-used items), 8 converted to
  `#[expect(dead_code, reason = "...")]` with intent comments.
- **TOCTOU race in env-var-derived paths.** `is_safe_delete_target` now
  re-validated at `clean()` time for `DeleteDirs` and `fallback_delete` paths.
- **`history.json` concurrent write race.** Exclusive `fs2::FileExt::lock_exclusive`
  prevents entry loss when two `sasurahime` processes run concurrently.
- **`SystemCommandRunner::exists()` no longer spawns external `which`.**
  Native `PATH` resolution via `std::env::split_paths` + `fs::metadata`.

---

## [0.1.28] ― 2026-05-28

### Added

- **Test coverage gaps ― 6 new unit test suites.** Added targeted unit tests
  for `is_skippable_error()` (C01), `GradleCleaner`/`JetBrainsCleaner`
  `find_old_caches` (H01, M02), `CargoCleaner::find_target_dirs` (H03),
  `MiseCleaner` parse/scan (M01), and `run_clean_target` dispatch (M03).
- **`--no-unicode` CLI flag.** Disables unicode and spinner characters in
  terminal output for better compatibility with log captures and non-unicode
  terminal emulators.
- **History retention cap (max 1000 entries).** `history.json` now trims to
  the 1000 most recent entries on write, preventing unbounded file growth.
  Configurable via `[history].max_entries` in `config.toml`.

### Fixed

- **`MockRunner.NotFound` order-dependency bug.** Changed from a method-based
  mock to a `bool` fallback field, fixing a test ordering issue where
  subsequent tests could see stale `NotFound` state.
- **Pre-commit test env var pollution.** Tests modifying `PRE_COMMIT_HOME` or
  `XDG_CACHE_HOME` now properly isolate via the shared `EnvGuard` guard,
  eliminating parallel-test flakiness in the `pre_commit` test suite.

### Internal

- **488 tests total** (329 unit + 159 integration/E2E across 25 test files),
  0 failures ― 46 new tests (+10.4%) since v0.1.27.
- **`src/test_helpers.rs`** introduced as a shared test infrastructure module,
  housing the `EnvGuard` Drop-based environment variable guard extracted from
  `pre_commit.rs`.
- **Archived 5 completed plan documents:** PBI A-G docs (7 files), colima
  cleaner plan, command-timeout-hint plan, and coverage-gaps test spec + plan.
- **PBI process documentation updated.** `pbi/AGENTS.md` now includes a
  detailed AI agent quick-reference guide; `pbi/PBI-process.md` streamlined
  with explicit TDD and mock guidelines.

---

## [0.1.27] ― 2026-05-27

### Added

- **`is_safe_delete_target()` path validation.** New public function in
  `src/cleaners/generic.rs` that blocks deletion of system-critical paths
  (`/`, `/etc`, `/System`, `/var/log`, etc.) while allowing macOS tempdirs
  (`/var/folders/...`) and user cache directories. Used by `CustomPathCleaner`
  and the `terraform()`/`flutter()` factory methods.
- **Downloads default 30-day age filter.** `sasurahime clean downloads` now
  skips files modified within the last 30 days by default. Override via
  config `[cleaner.downloads].older_than_days`.
- **terraform/flutter env var safety validation.** `TF_PLUGIN_CACHE_DIR` and
  `PUB_CACHE` environment variables are validated against
  `is_safe_delete_target()`. If they point to an unsafe path (e.g. `/etc`),
  the cleaner falls back to the default directory with a warning.
- **history.json schema versioning.** `HistoryEntry` gains a `version: 1`
  field with `#[serde(default)]` for forward compatibility.
- **`build_selection_items()` + `compute_selected_total()` extracted** from
  `interactive.rs` as pure functions, enabling unit testing of TUI selection
  logic without a TTY. 6 new unit tests covering sub-target rendering,
  mixed cleaners, and selected-size computation.
- **`tests/apfs_snapshot.rs`** ― new E2E test file for the APFS snapshot
  cleaner CLI entry point (dry-run and tool-not-found paths).
- **`--config <path>` documentation** added to `docs/HOWTO-USE.md` (EN + JA).

### Security

- **System path deletion protection (Checking Team: Red Team).** `terraform()`
  and `flutter()` constructors no longer blindly trust `TF_PLUGIN_CACHE_DIR`
  and `PUB_CACHE` environment variables. Unreasonable paths are rejected with
  a warning before the cleaner builds its directory list.

### Internal

- **Checking Team multi-perspective review.** 22 agents across 3 waves
  (core + specialists + test) reviewed the `pbi-2026-05-25` branch.
  Score: 82.4/100 (Rank A). Report in `plans/2026-05-27-0637-review-pbi-2026-05-25.md`.
- **Coverage gap audit.** Systematic analysis of all 24 test files.
  Score: 8.5/10. No HIGH gaps found. Report in `plans/ln-634-coverage-audit.md`.
- **442 tests total** (288 unit + 154 integration/E2E across 24 test files),
  0 failures.
- **All PBI A-G completed** (22 SP total). Sprint 5 ships PBI-D Phase 2,
  PBI-E (config.toml), PBI-F (--yes), and PBI-G (stats).
- **Docs DoD finalized.** All PBI-E/G documentation items marked complete
  (exclude, custom, per-cleaner, stats, --config documented in EN + JA).

### Fixed

- **`--permanent` help text was self-contradictory.** Flag named `--permanent`
  but claimed to "Move deleted files to Trash". Clarified to accurately describe
  the behavior: "Permanently delete files instead of moving to Trash. By default,
  files are moved to macOS Trash for safety."
- **Rust 1.95.0 clippy `items_after_test_module`.** Moved `#[cfg(test)] mod tests`
  in `src/cleaner.rs` to the end of the file, after the `pub trait Cleaner`
  definition.
- **Rust 1.95.0 clippy `needless_borrows_for_generic_args`.** Removed unnecessary
  `&` from `&log_dir.join(...)` in `tests/config.rs`.

---

## [0.1.26] ― 2026-05-26

### Added

- **Xcode subcategory TUI expansion (PBI-D Phase 2).** `XcodeCleaner` now
  overrides `sub_targets()` to expose DerivedData and Archives as expandable
  subcategories in the interactive TUI. The interactive.rs generic subcategory
  logic automatically renders indented checkboxes for each subcategory and
  dispatches `sasurahime clean xcode --sub <name>` for selected items. Non-
  existent subcategories are filtered out (not shown).
- **Config file `exclude` field (PBI-E).** `~/.config/sasurahime/config.toml`
  now supports `exclude = ["cleaner-name"]` to exclude cleaners from scan and
  TUI listing. Direct `sasurahime clean <target>` bypasses the filter.
- **`--config <path>` CLI flag (PBI-E).** Override the config file path.
  Falls back to defaults with a warning if the file doesn't exist.
- **`[[custom]]` user-defined cache targets (PBI-E).** Add arbitrary
  directories as scan/clean targets via `[[custom]]` sections in config.toml.
  `CustomPathCleaner` deletes sub-contents (not root) with macOS `uchg` flag
  handling, trash support, and progress reporting.
- **Per-cleaner config filters (PBI-E).** `[cleaner.<name>]` sections in
  config.toml support `older_than_days` and `larger_than_mb` filters. These
  apply to DeleteDirs-based cleaners (act, colima, downloads, etc.) and
  `[cleaner.logs]` (as `keep_days`). Command-based cleaners show a runtime
  warning that the filter is not supported.
- **`sasurahime stats` command (PBI-G).** New subcommand showing aggregated
  deletion history: total freed bytes, run count, and a table of recent
  cleanups. Supports `--last N` to limit output to the most recent N entries.
- **Automatic history logging (PBI-G).** Every successful clean operation
  (`bytes_freed > 0`, not `--dry-run`) appends a record to
  `~/.local/share/sasurahime/history.json` with timestamp, cleaner name,
  freed bytes, and skipped count. History writing is atomic (temp file +
  rename) and silently ignores filesystem errors.
- **`cleaner::Cleaner::sub_targets()` trait method.** Default returns empty
  vec; any cleaner can override to expose sub-targets for TUI expansion.

### Internal

- **Comprehensive test suite:** 436 tests (270 unit + 145 integration/E2E),
  22 test files, 0 failures.
- **`CustomPathCleaner`** in `src/cleaners/custom.rs` with `Cleaner` trait
  implementation, chflags nouchg handling, dry-run support, and 6 unit tests.
- **`src/history.rs`** (362 lines) with `HistoryEntry`, `StatsSummary`,
  atomic `append_history()`, `load_history()` (corruption-tolerant),
  `compute_stats()`, `format_stats()`, and 9 unit tests.
- **`GenericCleaner` builder methods:** `with_older_than()`,
  `with_larger_than()`, `with_config()` for per-cleaner filter configuration.
- **`Config::effective_logs_keep_days()`** resolves per-cleaner
  `[cleaner.logs] older_than_days` over the global `logs.keep_days`.
- **`Config::default_history_dir()`** for centralized path resolution.
- **SCM:** `.gitignore` updated for `.worktrees/` directory.
- **3 new E2E tests** for PBI-D Phase 2: `sub_targets_integration`,
  `sub_targets_returns_only_existing`, `sub_targets_filters_zero_size_entries`.
- **11 E2E tests** for PBI-E: exclude (3), --config (1), [[custom]] (2),
  per-cleaner age/size filters (3), logs age filter (1), scan display (1).
- **6 E2E tests** for PBI-G: stats aggregations, --last N, dry-run guard,
  corrupted file handling, empty state, clean auto-logging.

---

## [0.1.25] ― 2026-05-25

### Added

- **Xcode subcategory selection ― CLI core (PBI-D Phase 1).** `XcodeCleaner` now
  supports partial cleanup of Xcode caches via the `--sub` flag, letting users
  target only DerivedData, Archives, or both instead of cleaning everything.
- **`XcodeSubcategory` enum** with `DerivedData` and `Archives` variants, plus
  `from_str()` parser accepting `derived-data`, `deriveddata`, and `archives`.
- **`--sub derived-data|archives` CLI flag** on `sasurahime clean xcode`.
  Accepts comma-separated values (`--sub derived-data,archives`). When omitted,
  defaults to `DerivedData` only (backwards-compatible).
- **`Cleaner::sub_targets()` trait method.** Default returns empty vec; any
  cleaner can override to expose sub-targets for TUI expansion. The
  `interactive.rs` TUI already has generic logic to render sub-targets as
  indented checkboxes and dispatch `sasurahime clean <name> --sub <sub_name>`
  for each selected sub-target.
- **`XcodeCleaner::detect_subcategories()`** returns `Vec<SubcategoryInfo>`
  with per-subcategory path and size, used by the CLI and available for the
  future `sub_targets()` override.
- **`XcodeCleaner::with_subcategories()` builder** ― constructs an
  `XcodeCleaner` scoped to specific subcategories for targeted cleaning.
- **`XcodeCleaner::is_xcode_running()`** ― checks via `pgrep -x Xcode` for
  safe DerivedData deletion with a confirmation prompt when Xcode is active.
- **PBI-D document updated** to reflect current implementation status (Phase 1
  complete, Phase 2 TUI pending) and scope decision (Simulators handled by the
  standalone `simulator` cleaner, not an Xcode subcategory).

### Internal

- **5 E2E tests** in `tests/xcode.rs`: clean with `--sub derived-data`,
  `--sub archives`, default (no sub), `--dry-run`, and missing-data handling.
- **10 unit/integration tests** in `src/cleaners/xcode.rs`: `subcategory_all`,
  `subcategory_path`, `from_str` variants, `display_name`,
  `detect_subcategories`, `clean_selected_subcategory_only`,
  `is_xcode_running`, detection status.
- **`SubcategoryInfo` struct** for returning per-subcategory metadata from
  `detect_subcategories()`.

---

## [0.1.24] ― 2026-05-25

### Added

- **Trash warning UI (PBI-C).** After cleaning files via macOS Trash, `sasurahime`
  now shows `"Note: Moved X to Trash. Run 'Empty Trash' to reclaim disk space."`
  to inform users that Trash must be emptied to actually free disk space.
- **Large file pre-warning.** When ≧1 GB of files will be moved to Trash, a
  prominent `"Note: Files will be moved to Trash (not immediately freed)."`
  warning is shown before the clean operation begins.
- **`CleanResult.uses_trash` field.** Each cleaner reports whether it used the
  macOS Trash (`delete_path`) or permanent deletion. CLI-based cleaners (brew,
  uv, rustup, etc.) set `false`; directory-deletion cleaners set `true`.
- **`format_trash_warning()` / `format_large_trash_warning()` helpers.** Pure
  functions in `src/cleaner.rs` with 7 unit tests covering threshold boundaries,
  dry-run suppression, and trash/non-trash modes.
- **`LARGE_TRASH_THRESHOLD_BYTES`** constant (1 GiB) for threshold logic.

### Changed

- **`run_clean_target` pre/post hooks:** Added trash notice print before clean
  (when `is_trash_mode() && !dry_run`) and size-specific note after clean
  (when `is_trash_mode() && bytes_freed > 0`).

### Internal

- **All CleanResult construction sites** updated across 16+ source files to
  provide the new `uses_trash` field.
- **3 E2E tests** in `tests/trash.rs`: trash note visible, suppressed with
  `--permanent`, suppressed with `--dry-run`.

---

## [0.1.23] ― 2026-05-25

### Added

- **Robust error handling (PBI-B).** All 20+ cleaners now catch permission errors
  (`EPERM`), file-lock errors (`EBUSY`), and access errors (`EACCES`) during
  deletion instead of panicking or aborting the entire cleanup. Affected files
  are recorded as `skipped` and processing continues with remaining targets.

- **`SkippedEntry` struct + `CleanResult.skipped` field.** Each cleaner's
  `clean()` method now returns a `Vec<SkippedEntry>` listing files/directories
  that could not be deleted due to permission or lock issues, along with the
  error reason.

- **Exit code semantics.** `sasurahime clean <target>` now exits with code 1
  when ALL files failed (nothing freed, only errors), and code 0 when at least
  some data was freed or nothing was skippable. Partial success (some freed,
  some skipped) exits 0.

- **Skip summary display.** After each clean operation, a summary of skipped
  files is printed to stderr: `N file(s) skipped: /path: Permission denied`.

- **`is_skippable_error()` helper.** New public function in `src/cleaner.rs`
  that checks if an `anyhow::Error` wraps a skippable IO error
  (`PermissionDenied`, `WouldBlock`, `AlreadyExists`), with fallback message
  matching for `trash` layer errors.

### Changed

- **All deletion paths updated.** Every `crate::trash::delete_path()` call,
  `fs::remove_dir_all()` call, and `rm -rf` delegate across all cleaners
  (browser, xcode, cargo, log, uv, mise, ios_backup, library_logs,
  device_support, ollama, rustup, gradle + JetBrains, GenericCleaner)
  now handles skippable errors gracefully via the new pattern.

### Internal

- **`src/trash.rs`**: Changed `map_err` to `.with_context(…)` to preserve
  original `io::Error` in the error chain so `is_skippable_error()` can
  downcast correctly.
- **`CleanResult::exit_code()`**: Returns 1 when `bytes_freed == 0` and
  `skipped` is non-empty, 0 otherwise.
- All existing `CleanResult` construction sites updated to provide the new
  `skipped` field.

---

## [0.1.22] ― 2026-05-25

### Added

- **Parallel scan optimization (PBI-A).** `sasurahime scan` now runs cleaner
  `detect()` calls in parallel via `rayon::par_iter()`, with a consolidated
  progress bar showing scan completion count. Scan time is bounded by the
  slowest cleaner rather than the sum of all cleaners.
- **`Cleaner::is_available()` trait method.** Binary-checking cleaners (uv,
  brew, mise, rustup, apfs-snapshot, ollama, and all GenericCleaner command
  variants) now expose availability via `self.runner.exists("tool")`. The scan
  phase pre-filters unavailable cleaners, skipping `detect()` entirely for
  tools not installed ― eliminating I/O overhead for 40+ unused targets.
- **Parallel scan in interactive/auto modes.** Both `sasurahime --yes` and the
  interactive TUI scan phases use the same parallel `with_parallel_scan()`
  progress bar.

### Changed

- **Scanner progress display:** Replaced per-cleaner sequential spinners
  (`"Scanning uv..."`, `"Scanning brew..."`, …) with a single consolidated
  bar: `[{spinner} {bar:20}] Scanning... (3/12)`.
- **`with_spinner()` retained** for potential future use (marked
  `#[expect(dead_code)]`).

### Internal

- **New dependency:** `rayon = "1"` added to `Cargo.toml`.
- **New progress helper:** `with_parallel_scan(total, |pb| …)` in
  `src/progress.rs` ― accepts `&ProgressBar` for rayon-thread-safe progress
  updates.
- **3 unit tests** in `scanner.rs`: empty cleaners, mixed availability,
  default `is_available()` returns true.
- **2 integration tests** in `tests/scan.rs`: table headers, size display
  with existing cache dir.
- Updated `tests/interactive.rs` spinner assertions to match consolidated
  progress output; updated VERSION constant to `0.1.22`.

---

## [0.1.21] ― 2026-05-25

### Added

- **primary_target now available for all 17 cleaners.** The remaining 5 cleaners
  (browser, cargo, device_support, log, apfs_snapshot) now report their primary
  path when `--verbose` is active. browser → `~/.cache/puppeteer/chrome`,
  cargo → `~/.cargo/registry/cache`, device_support → `~/Library/Developer/Xcode`.
  log and apfs_snapshot explicitly return `None` (multiple targets / no user
  HOME directory).
- **14 new unit tests** covering `primary_target` behavior for all 5
  previously-missing cleaners (both verbose-on and verbose-off cases).

### Changed

- **Non-running app cache cleanup (hint.rs):** `sasurahime --yes` and interactive
  auto-clean now prompt for non-running apps with known cache directories and
  delete them directly (no quit/relaunch needed). Previously only running apps
  were offered for cleanup.
- **Double confirmation suppressed (interactive.rs):** `run_auto` and
  `run_interactive` now suppress secondary confirmation prompts inside cleaners
  via `set_skip_confirm(true)` since the TUI already asked "Proceed?".

### Internal

- **Completed plans archived:** Moved 3 completed implementation plans
  (lucky-panda, explore-command, ios-backup-apfs-snapshot) to `.plan/archived/`.

---

## [0.1.20] ― 2026-05-24

### Added

- **`sasurahime explore` ― OmniDiskSweeper-style disk explorer.** Scans
  `~/Library/Application Support/`, `~/Library/Caches/`, `~/.cache/`, and
  `~/.local/share/` at first-level depth, groups by app name, and sorts by
  size descending. Unlike `scan`, `explore` covers every app folder ―
  not just registered cleaners ― answering "who is eating my disk?" without
  prior knowledge of the culprit.

  The interactive output has two sections:

  - **Managed** ― paths owned by a registered sasurahime cleaner. Select
    entries to run `sasurahime clean <target>` in-session. After cleaning,
    the managed table is re-scanned and reprinted with updated sizes.
  - **Not managed** ― everything else. Select entries to display the full
    path (for copy-paste) and optionally open the folder in Finder.

  Options: `--top N` (default 20, per section), `--all` (show everything),
  `--dir PATH` (repeatable; replaces default roots entirely), `--dry-run`
  (forwarded to any clean subprocess). Requires an interactive TTY; exits
  with code 1 in non-TTY environments.

- **`src/explorer.rs`** new module implementing:
  - `MANAGED_PATTERNS` ― 18 hardcoded path patterns mapping cache directories
    to their `sasurahime clean <target>` names, including glob-prefix support
    (`ms-playwright*`).
  - `collect_entries` ― first-level directory scan; skips missing roots,
    permission errors, and size-0 entries silently.
  - `apply_top` ― `sort_unstable_by_key(Reverse(size))` + truncate.
  - `explore_results` ― `pub(crate)` testable core (no dialoguer dependency).
  - `run_explore` ― full interactive flow with TTY guard, `dialoguer::MultiSelect`
    for both sections, subprocess spawn via `std::env::current_exe()`.

### Documentation

- **HOWTO-USE.md** (EN + JA): added `sasurahime explore` section with
  annotated output example, two-section behaviour description, options table,
  and usage examples.

### Internal

- 18 new unit tests in `src/explorer.rs` covering all pure functions
  (`is_managed`, `default_roots`, `collect_entries`, `apply_top`,
  `explore_results`) with `tempfile::TempDir` fixtures. No real `~/Library`
  access in tests.

---

## [0.1.19] ― 2026-05-23

### Added

- **`--verbose` flag for detailed operation output.** When `--verbose` is set:
  - `sasurahime scan` displays a 4th "Target" column showing the primary cache
    directory each cleaner monitors (e.g. `~/.cache/uv`, `~/.colima`).
  - `sasurahime clean` prints per-file/dir removal details for every cleaner.
  - The flag is global and works with all subcommands (scan, clean, targets).
- **`--dry-run` global flag.** Shows what would be cleaned without deleting
  anything. Works alongside `--verbose` for previewing cleanup operations.

### Changed

- **`sasurahime targets` output is now sorted alphabetically.** Previously the
  target list appeared in declaration order (macro order + manual entries
  appended). Now all targets are sorted by name for easier browsing.
- **Scan result table now includes a "Target" column.** This column is always
  present in the table but shows `"-"` when `--verbose` is not active. Each
  cleaner reports its primary monitored path when verbose mode is on.

## [0.1.18] ― 2026-05-22

### Internal

- **Applied `cargo fmt` to fix formatting drift** across
  `src/cleaners/generic.rs` and `tests/interactive.rs` after the v0.1.16
  colima review changes.

## [0.1.17] ― 2026-05-22

### Internal

- **Archived completed design specs:** Moved 3 fully-implemented spec files
  (`ios-backup-apfs-snapshot-design`, `command-timeout-hint-design`,
  `running-process-hint-design`) from `docs/superpowers/specs/` to
  `.plan/archived/`.

## [0.1.16] ― 2026-05-22

### Added

- **colima deletion confirmation prompt:** `sasurahime clean colima` now shows a
  confirmation prompt on interactive TTYs before running `colima prune --all --force`,
  warning about deletion of all stopped VM disk data (containers, images, volumes).
  In `--yes` (non-TTY) mode the confirmation is skipped automatically.
- **colima fallback deletion:** When the `colima` CLI is not installed but
  `~/.colima/` exists, `sasurahime clean colima` now falls back to directly
  deleting the directory via Trash (with `chflags nouchg` handling).
- **colima non-zero exit warning:** If `colima prune` exits with a non-zero
  status, sasurahime now prints a warning with the exit code and stderr output
  instead of silently treating it as success.

### Changed

- **GenericCleaner now supports `confirm_message` and `fallback_delete`:**
  Two new optional fields on `GenericCleaner` enable any command-based cleaner
  to request interactive confirmation and/or fall back to direct directory
  deletion when the CLI tool is missing. `colima_prune` is the first consumer.

### Fixed

- **colima prune hangs on confirmation prompt:** `colima prune --all` was invoked
  without the `--force` flag, causing it to wait for `y/N` input indefinitely.
  Added `--force` to skip the interactive prompt and proceed non-interactively.

### Documentation

- **SUPPORTED.md note on estimate accuracy:** Added a note to the colima entry
  (EN + JA) that the scan report is an estimated maximum ― actual freed space
  depends on which VM disk images are pruned.
- **Colima target description updated:** Changed from `"Colima VM disk cache
  prune"` to `"Colima VM disk images (inactive) prune"` to more accurately
  reflect the deletion scope.
- **Documentation command examples updated:** All `colima prune --all`
  references in SUPPORTED.md, HOWTO-USE.md, and CHANGELOG.md updated to
  `colima prune --all --force`.

### Internal

- **Test version string deduplication:** `tests/interactive.rs` version
  assertions now use a shared `const VERSION` instead of 7 hardcoded strings.
- **Test refactoring:** `command_with_detect_dir_returns_not_found_when_tool_missing`
  replaced with `command_with_detect_dir_fallback_reports_pruneable_when_tool_missing`
  (using factory method) and `command_without_fallback_returns_not_found_when_tool_missing`.
- **Testing team auto-fixes:** E2E test assertion strengthened from
  `calls.contains("prune --all")` to `calls.contains("prune --all --force")`.
  Unit test args updated from `&["prune", "--all"]` to `&["prune", "--all", "--force"]`.

## [0.1.15] ― 2026-05-22

### Fixed

- **colima prune hangs on confirmation prompt:** `colima prune --all` was invoked
  without the `--force` flag, causing it to wait for `y/N` input indefinitely.
  Added `--force` to skip the interactive prompt and proceed non-interactively.

## [0.1.14] ― 2026-05-22

### Added

- **Command timeout hint:** When a command-based cleaner (colima, docker, brew,
  etc.) times out (>30s), the error message now includes the full command and a
  hint to run it manually: `You can run this command manually in another terminal:
  $ colima prune --all --force`. Applies to all 20+ cleaners that delegate to external
  CLIs.
- **`with_spinner_result()`:** New progress helper that prints `[FAILED]` instead
  of the misleading `[OK]` when a clean operation fails (e.g. due to timeout).

### Fixed

- **Misleading `[OK]` on timeout:** Previously `Cleaning colima... [OK]` was
  printed even when the command timed out and was killed. Now shows
  `[FAILED]` with the error details.

## [0.1.13] ― 2026-05-21

### Fixed

- **Rust 1.95.0 clippy warnings:** Removed unnecessary borrows (`&[0u8; N]` →
  `[0u8; N]`) in test code across `src/cleaners/rustup.rs`, `src/cleaners/uv.rs`,
  and `src/format.rs` to satisfy the new `needless_borrows_for_generic_args`
  lint.
- **Test version strings:** Updated version assertions in
  `tests/interactive.rs` from `0.1.12` to `0.1.13` to match the new version.

## [0.1.12] ― 2026-05-21

### Added

- **IosCleaner (PBI-012):** Scan and remove iOS device backups from
  `~/Library/Application Support/MobileSync/Backup/`. Interactive only (never
  runs in `--yes` mode) ― backups are sent to Trash with `chflags nouchg`
  handling. Reachable via `sasurahime clean ios-backup`.
- **ApfsSnapshotCleaner (PBI-013):** List and delete APFS local Time Machine
  snapshots via `tmutil deletelocalsnapshot`. Interactive only ― warns about
  losing local Time Machine protection. Reachable via
  `sasurahime clean apfs-snapshot`.

## [0.1.11] ― 2026-05-21

### Fixed

- **Sparse file size over-reporting (colima):** `dir_size()` now uses physical
  disk blocks (`st_blocks × 512`) instead of logical file size (`st_size`).
  This fixes `sasurahime scan` showing wildly inflated sizes for sparse VM disk
  images ― e.g. colima dropped from 100.3 GB to the correct 9.3 GB, matching
  `du`.
- **LogCleaner logical-size bug:** `detect()` and `clean()` were using
  `m.len()` (logical size) instead of `m.blocks() × 512` (physical blocks),
  inconsistent with all other cleaners after the `dir_size()` fix.
- **CommandWithDetectDir false-positive scan:** `detect()` for colima,
  simulator, maven, and flutter now checks that the cleaning tool is actually
  installed before reporting the directory as reclaimable. Previously, if the
  tool was removed but `~/.[colima|maven|flutter]` remained, scan would show a
  large pruneable size but `clean` would silently skip.
- **UvCleaner under-reporting:** `detect()` was only measuring the
  `archive-v0` subdirectory but `clean()` also frees old `simple-vN` index
  dirs. `detect()` now measures the full cache dir, matching `clean()`.
- **RustupCleaner hardcoded size estimate:** Replaced the fixed 300 MB per
  toolchain guess with actual `dir_size()` measurement of each unused
  toolchain in `~/.rustup/toolchains/<name>/`.

### Changed

- **Audited all 16+ cleaners for size-reporting accuracy.** No other
  discrepancies found beyond the above. Lower-severity observations (e.g.
  `brew` uses two different measurement sources, `huggingface` CLI path
  credits full dir size) are documented as inherent design trade-offs rather
  than bugs.

## [0.1.10] ― 2026-05-21

### Changed

- **Dependency version bumps:** Updated four crate dependencies to latest
  compatible versions ― dialoguer 0.11→0.12, dirs 5→6, indicatif 0.17→0.18,
  toml 0.8→1.1. No source code changes were required; all APIs remain
  compatible.

### Fixed

- **rustup test flake:** `rustup_not_found_skips` now restricts PATH to
  `/usr/bin:/bin` so the test does not accidentally find `rustup` installed
  on the host machine and proceed to clean real toolchains.

---

## [0.1.8] ― 2026-05-21

### Added

- **4 new clean targets:** `volta`, `sbt`, `tree-sitter`, `vscode-extensions`.
- **vscode-logs** built-in target added to LogCleaner (`~/Library/Application Support/Code/logs`).
- SUPPORTED.md updated to document all 43 clean targets (previously undocumented: `colima`, `simulator`, `vscode-extensions`, `maven`, `terraform`, `flutter`, `ollama`, `device-support`).
- HOWTO-USE.md target tables (EN + JA) updated with 11 missing entries: `colima`, `device-support`, `flutter`, `maven`, `ollama`, `sbt`, `simulator`, `terraform`, `tree-sitter`, `volta`, `vscode-extensions`.

### Changed

- Cleaner count in SUPPORTED.md: English 35 → 43, Japanese 32 → 43.
- Japanese section of SUPPORTED.md now includes translations for `volta`, `sbt`, `tree-sitter`. (These existed in English-only since 0.1.7.)

---

## [0.1.7] ― 2026-05-21

### Added

- Docs site navigation integration: language-aware nav labels in doc pages
  (`switchLang()` JS を拡張、JA表示時に「使い方」「ターゲット一覧」に切替).
- `.lang-btn` CSS class added to shared stylesheet for unified language switcher
  appearance across landing pages and doc pages.
- Top link in doc page footer for quick navigation to the top of the site.
- Cross-reference links (`See also` / `関連ドキュメント`) added to both EN and
  JA sections of HOWTO-USE.md.
- Language switcher visual unified across all pages: landing pages now use
  the same `.lang-btn` button style as doc pages (labels `EN`/`JA`).
- Footer expanded on both landing pages to link all 3 documentation pages
  (How to Use, Supported Targets, Add a Target) in the appropriate language.
- `<html dir="ltr">` attribute on all pages for explicit text direction.
- `og:url` meta tags on both landing pages for correct language-specific URLs.
- Author metadata (`authors`) added to `Cargo.toml`.
- `docs/_site/` and `.superpowers/` added to `.gitignore`.
- **Progress bar with ETA and per-file speed** for all multi-entry cleaners:
  shows `removing BloatApp, 45.2 MB/s (3/15)` during cleanup.
- `ProgressReporter` trait with three implementations: `VerboseProgress`
  (indicatif progress bar), `SuppressReporter` (spinner only),
  `DeepSuppressReporter` (silent).
- `--suppress` CLI flag: hides progress bar, keeps spinner and freed summary.
- `--deep-suppress` CLI flag: hides all output (spinner, freed line, progress).
- `suppress` / `deep_suppress` config options in `~/.config/sasurahime/config.toml`.
- `format_speed()` helper for per-file MB/s calculation in progress bar.

### Changed

- `Cleaner` trait `clean()` method now accepts `&dyn ProgressReporter` parameter.
- `run_clean_target()` updated to conditionally show spinner based on reporter.
- Documentation navigation reorganized: bilingual Markdown files consolidated,
  EN/JA language switcher added to doc pages, dark theme unified across all
  doc pages and landing pages.
- Landing page nav links renamed: `Documentation` → `How to Use`, with
  additional links to `Supported Targets` and `Add a Target`.
- Redundant `.lang-btn` CSS removed from inline `<style>` in `_layouts/doc.html`
  (moved to shared `docs/assets/style.css`).
- Redundant `.lang-switch a/span` CSS rules removed (superseded by `.lang-btn`).

---

## [0.1.6] ― 2026-05-20

### Added

- GitHub Pages site at `armaniacs.github.io/sasurahime` with landing page and documentation.
- `Cargo.toml` `repository` and `description` fields.

### Changed

- License changed from MIT to Apache-2.0.
- Landing page redesigned with dark theme, Japanese version added.
- `.gitignore` cleaned up (deduplicated `target/`, added `*.bak`, `*.log`, `tags`, `__pycache__/`, `.env`, `.playwright-mcp/`).

---

## [0.1.5] ― 2026-05-20

### Added

- `sasurahime targets` subcommand listing all supported clean targets.
- Version banner now visible in `sasurahime -h` / `--help` output and at interactive startup.
- Blog articles for Zenn (`blog/compact.md`, `blog/full.md`) covering usage walkthrough and full feature guide.

### Changed

- **Version banner moved from stdout to stderr.** All commands now print
  `sasurahime v0.1.5` on stderr at startup. Scripts using `sasurahime scan | grep`
  or similar no longer see the version string in their piped output.
- **CleanTarget match arms refactored.** 14 duplicated arms extracted into
  `run_clean_target()` helper. New target additions now need only 1 line per arm.
- **ProgressStyle cached** via `OnceLock`. Template parsing happens once, not
  per spinner instance.
- **Spinner completion marker** changed from `✓` to `[OK]` for screen-reader
  compatibility.
- **Release profile tuned:** `opt-level` changed from `z` to `s` (better CLI
  speed/size balance), `lto` changed from full to `"thin"`.
- **Xcode --yes mode:** warning printed to stderr when Xcode is running.
- **MiseCleaner non-directory entries** (`.DS_Store`, `.mise.backend`) are now
  skipped. `remove_with_uchg` errors are logged and skipped instead of stopping
  the entire clean.
- `Cargo.toml` `license = "MIT"` added.
- Repository published at `github.com/armaniacs/sasurahime`.

### Dependencies

- `windows-sys` versions deduplicated via `cargo update`.

---

## [0.1.4] ― 2026-05-20

### Added

- `--permanent` flag: bypass Trash and permanently delete files.
- `trash_mode` config option in `~/.config/sasurahime/config.toml`.

### Changed

- **Trash mode is now the default.** All deleted files are sent to macOS Trash
  instead of being permanently removed. Set `trash_mode = false` in config or
  pass `--permanent` to opt out.
- **`--yes --permanent` shows a confirmation prompt** before permanently deleting
  all pruneable targets (moved from `--yes` with old `--trash` flag).
- `--trash` flag renamed to `--permanent` (inverted semantics).
- **Release profile further optimized for binary size:** `lto = "thin"` → `true`,
  `panic = "abort"` added, `opt-level = "s"` → `"z"`. Binary reduced from
  1,349 KB to 872 KB (35% reduction).

---


## [0.1.2] ― 2026-05-18

### Added

- `sasurahime --version` / `sasurahime -V` ― prints version number.
- `sasurahime targets` ― lists all 14 supported clean targets with descriptions.
- Version banner displayed on interactive (`sasurahime`) and auto (`sasurahime --yes`) startup.
- Test coverage gap audit plan (`docs/coverage-gap-plan.md`, `docs/coverage-gap-summary.json`).
- **MiseCleaner (GAP-001):** `.mise.toml` pinning cross-check ― versions pinned in
  `~/.config/mise/config.toml` or any `.mise.toml` under HOME (max depth 5) are
  protected from deletion. Added `MiseCleaner::scan_pinned_versions` with TOML
  parser (`parse_tools_section`, `parse_toml_kv`). (`MiseCleaner` now stores a
  `home` field for robust path resolution.)
- **E2E test:** `clean_mise_pinned_version_not_deleted` ― validates that a
  `.mise.toml`-pinned version is preserved and an unpinned version is removed.
- **Unit tests:** `unused_versions_pinned_is_protected` for the `pinned` set
  intersection. `expand_tilde_tilde_alone` for `Config::expand_tilde("~")`.
  `version_key_empty_string_returns_empty` and
  `find_old_versions_skips_unparseable_dir_name` for `BrowserCleaner`.
  `find_old_versions_skips_symlinks` for browser symlink guard.
- Uv, Xcode, Log edge-case unit tests (symlink guard, `--yes` bypass E2E,
  DST/time boundaries, missing metadata).

### Changed

- **Critical** `MiseCleaner::remove_with_uchg` (GAP-002): chflags errors are now
  propagated via `?` instead of being silently ignored with `let _ = ...`.
- **Medium** `BrewCleaner::parse_size_str` (GAP-003): now accepts lowercase units
  (`gb`, `mb`, `kb`) and space-separated forms (`194.3 MB`).
- **High** `BrowserCleaner::find_old_versions` (GAP-004 / GAP-005): filters out
  empty version keys (panics guard) and symlinks before processing entries.
- **Medium** `GenericCleaner::clean` (GAP-010): `DeleteDirs` path now calls
  `chmod -R nouchg` before `remove_dir_all`.
- **Low** `UvCleaner::detect_old_indexes` (GAP-006): skips symlink entries.
- Reformatted multi-line struct initializations and assertions across
  `src/cleaners/generic.rs`, `tests/generic.rs`, and `tests/npm.rs` for improved
  readability.

### Dependencies

- `filetime` added to `Cargo.lock` (dev-dependency, used in `tests/log.rs`).

---

## [0.1.0] ― 2026-05-18

### Added

- `sasurahime scan` ― scans all known cache locations and prints a summary table with reclaimable sizes.
- `sasurahime clean <target>` ― cleans individual cache targets by name.
- `sasurahime clean <target> --dry-run` ― previews what would be removed without deleting anything.
- **uv (PBI-002):** Removes stale `simple-vN` index directories (keeps only the latest) and runs `uv cache prune --force`.
- **Homebrew (PBI-003):** Runs `brew cleanup -s --prune=all` and reports freed disk space.
- **mise (PBI-004):** Calls `mise ls --current` to determine active versions, then removes unused versions from `~/.local/share/mise/installs/` with macOS `uchg` flag handling.
- **Browsers (PBI-005):** Removes old Puppeteer Chrome / Chrome-Headless-Shell directories and old Playwright (`ms-playwright*)` builds while keeping the highest version.

### Architecture

- Introduced the `Cleaner` trait (`name`, `detect`, `clean`) as the core abstraction for all cleaners.
- Introduced the `CommandRunner` trait to allow test-time mocking of external command execution (uv, brew, mise, chflags).
- Added `sasurahime --yes` entry point for non-interactive full-cleaning (placeholder for Sprint 4 TUI integration).
- Interactive TUI via `dialoguer::MultiSelect` ― UI placeholder (PBI-008, Sprint 4).
- Full E2E test suite (assert\_cmd + tempfile fixtures) for every cleaner.
- Unit tests for pure helper functions (e.g. `parse_active_versions`, `version_key`, `find_old_versions`).
