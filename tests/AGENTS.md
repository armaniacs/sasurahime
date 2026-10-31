# AGENTS.md — tests/

Guidance for AI agents editing `tests/`. Repo-wide contract: `../AGENTS.md`. PBI process (branch/commit/archive): `../pbi/AGENTS.md`.

## Outside-In TDD

Start each PBI with an E2E test using a `tempdir` fixture, then add integration and unit tests inward.

- **E2E**: spawn the binary or call the top-level function with a `tempdir` as `HOME`; assert on exit code and stdout.
- **Integration**: construct a `Cleaner` with a fake root path; call `detect()` / `clean(dry_run, reporter, opts)` and assert on `ScanResult`.
- **Unit**: pure functions like `parse_size_str`, `version_matches_spec`, `is_older_than`.

Mock external commands by injecting a `CommandRunner` trait rather than calling `Command` directly.

## Cache-dir resolution convention (PBI 88)

Cache directories are resolved **once, in the cleaner's constructor**. Every env-dependent cleaner (huggingface, pre-commit, flutter, terraform, act) calls `crate::safety::resolve_env_dir` from its constructor and passes the resolved `PathBuf` into the shared adapter, so `detect`/`clean` reuse the resolved value and never re-read the process environment.

- Cleaner unit tests must NOT steer cache-dir resolution with `std::env::set_var` or an `EnvGuard`: build the cleaner through the `#[cfg(test)]` test factory (e.g. `HuggingFaceCleaner::with_cache_dir`) and pass the resolved cache dir explicitly, so the test has no process-env dependency.
- The resolver's env-value → result matrix (unsafe / root / unset / escaping / joined) is pinned at the unit level in `src/safety.rs` using a test-only var name; production env behaviour is pinned by the E2E spawn pins (`.env()` on the child process).
- A new env-dependent cleaner follows the generic constructor-resolution shape (`let cache = resolve_env_dir(...);` in its constructor), never a per-call resolver closure.
