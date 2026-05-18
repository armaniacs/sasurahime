# HOWTO: Add a new clean target

Steps to add a new cache-cleaning target to sasurahime.

## 1. Open an issue first

Before writing any code, **open an Issue** (or comment on an existing PR) to
discuss the proposal**. We need to confirm:

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

Once the issue is approved, follow the existing pattern:

1. Implement `Cleaner` trait in `src/cleaners/<name>.rs`
2. Add `pub mod <name>;` to `src/cleaners/mod.rs`
3. Wire into `src/main.rs`: `CleanTarget` enum, `all_cleaners()`, `main()` match
4. Add E2E test in `tests/<name>.rs`; add unit tests as needed
5. Pass quality gates: `cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test`

See existing cleaners (`uv.rs`, `brew.rs`, `mise.rs`, etc.) for reference.
