# HOWTO: Add a new clean target

Steps to add a new cache-cleaning target to sasurahime.

## 1. Open an issue first

Before writing any code, **open an Issue** (or comment on an existing PR) to
discuss the proposal. We need to confirm:

- Which cache the tool uses
- Where it is located on disk
- How to clean it (external CLI command or directory deletion)
- Any safety concerns (immutable flags, running processes, etc.)

```
Title: feat: support <tool> cache cleaning
Body:  - Tool name / version
       - Cache directory path(s)
       - How to clean
       - Safety concerns
```

## 2. Implementation

There are two paths depending on the cleaner's complexity.

### Path A: Standard cleaner (recommended)

Most cleaners (simple `{ dry_run: bool }` variants with a single factory
function) use the `define_cleaners!` macro in `src/main.rs`. Adding one requires
touching **3 places**:

1. **Create** `src/cleaners/<name>.rs` implementing the `Cleaner` trait
2. **Register** `pub mod <name>;` in `src/cleaners/mod.rs`
3. **Add one line** to the `define_cleaners!` invocation in `src/main.rs`:

```rust
// In src/main.rs, inside the define_cleaners! block:
MyNewTarget : "my-new-target" => "Description of what this cleans";
(|home, _config| cleaners::my_new_target::MyNewTargetCleaner::new(home, Box::new(SystemCommandRunner))),
```

That's it. The macro auto-generates:
- `CleanTarget::MyNewTarget { dry_run: bool }` enum variant
- `SUPPORTED_TARGETS` entry (name → description)
- `dispatch_clean()` match arm
- `command_name()` / `dry_run()` dispatch helpers

No manual edits to `CleanTarget`, `SUPPORTED_TARGETS`, `all_cleaners()`, or
`main()` match are needed for standard cleaners.

### Path B: Special dispatch cleaner

Some cleaners need custom dispatch logic beyond what the macro provides:

| Reason | Examples |
|--------|----------|
| Extra CLI flags beyond `--dry-run` | `Logs` (`--keep-days`), `LibraryLogs` (`--all` / interactive) |
| Composite (runs multiple cleaners) | `Caches` |
| Pre-check before cleaning | `Xcode` (running process detection) |
| Completely different behavior | `Trash` (scan-only, warns on clean) |

For these, follow Path A above plus add **manual dispatch** in `src/main.rs`:

1. The macro handles the enum variant and basic registration
2. Add a match arm inside the special-targets block in `main()` (see existing
   handlers for `Logs`, `Xcode`, `Caches`, `Trash`, and `LibraryLogs`)

```rust
// In src/main.rs, inside the if matches!(target, ...) { match target { ... } } block:
CleanTarget::MySpecialTarget { dry_run } => {
    let cleaner = cleaners::my_target::MyTargetCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("my-target", |dry| cleaner.clean(dry), dry_run)?;
}
```

3. Add the variant name to the `matches!()` check and to the `impl CleanTarget`
   methods (`command_name()`, `dry_run()`).

### Both paths

4. Add E2E test in `tests/<name>.rs`; add unit tests as needed
5. Pass quality gates:

```bash
cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test
```

See existing cleaners (`uv.rs` for a standard cleaner, `library_logs.rs` for a
special-dispatch cleaner) for reference.
