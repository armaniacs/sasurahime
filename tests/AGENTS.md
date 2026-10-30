# AGENTS.md — tests/

Guidance for AI agents editing `tests/`. Repo-wide contract: `../AGENTS.md`. PBI process (branch/commit/archive): `../pbi/AGENTS.md`.

## Outside-In TDD

Start each PBI with an E2E test using a `tempdir` fixture, then add integration and unit tests inward.

- **E2E**: spawn the binary or call the top-level function with a `tempdir` as `HOME`; assert on exit code and stdout.
- **Integration**: construct a `Cleaner` with a fake root path; call `detect()` / `clean(dry_run, reporter, opts)` and assert on `ScanResult`.
- **Unit**: pure functions like `parse_size_str`, `version_matches_spec`, `is_older_than`.

Mock external commands by injecting a `CommandRunner` trait rather than calling `Command` directly.
