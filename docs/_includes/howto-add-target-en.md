<details{% unless page.lang == 'ja' %} open{% endunless %} markdown="1" data-lang="en" lang="en">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

<div class="doc-body" markdown="1">

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

There are three paths depending on the cleaner's complexity.
**Plugins are preferred for new targets**: they keep the binary lean and do
not need a code review of the core. Write an external `sasurahime-*`
executable and ship it yourself:

### Path P: External plugin (preferred for new targets)

Build an executable named `sasurahime-<name>` on PATH that answers:

```sh
sasurahime-<name> scan --json
# stdout: {"status": "pruneable|clean|notfound|unavailable", "bytes": <n>}
# exit 0
sasurahime-<name> clean [--dry-run]
# performs (or plans) the clean; exit 0 on success
```

Then `sasurahime scan` shows a `<name>` row, `sasurahime targets` lists it
with a `(plugin)` marker, and `sasurahime clean <name>` runs the plugin.
Failures, invalid output and hangs (over a 10 s timeout, override with
`SASURAHIME_PLUGIN_TIMEOUT_SECS`) are isolated: the row becomes
`unavailable` and the rest of the scan is unaffected. See
`examples/sample-plugin.sh` for a working sample. Security note: plugins are
arbitrary binaries found on PATH; only install plugins you trust.

### Path G: Generic cleaner (built-in, for cache targets already maintained here)

If the new target either delegates to an external CLI tool or just deletes
known directories, pick `CliCleaner` or `PathCleaner` in
`src/cleaners/generic.rs`. That is **2 touches** total, no new file:

```rust
// In src/cleaners/generic.rs, inside impl CliCleaner (external tool does the cleaning):
pub fn my_tool(runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-tool", "my_tool", &["cache", "clean"], runner)
}
// or inside impl PathCleaner (fixed directory list, optional age/size filters):
pub fn my_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-cache", vec![home.join(".my-cache")], runner)
}
```

```rust
// In src/main.rs, inside the define_cleaners! block:
MyTool : "my-tool" => "MyTool package cache";
(|_home, _config| cleaners::generic::CliCleaner::my_tool(Box::new(SystemCommandRunner))),
```

The CLI-delegation and path-deletion branches are closed into the two types,
so no shared dispatch enum needs to be understood. Use `CliCleaner::with_detect_dir`
factories when the size should be measured from the filesystem, and
`PathCleaner::with_older_than` / `with_larger_than` for age or size filters.

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

New standard targets appear in scan automatically. To make a target eligible
for --yes (unattended clean), override `risk()` on the cleaner to return
`Risk::Safe`: `--yes` eligibility is derived from `risk()` via
`auto_clean_allowed()`, and non-Safe targets are wrapped in `ManualOnly`
automatically. There is no name list to maintain.

### Path B: Special dispatch cleaner

Some cleaners need custom dispatch logic beyond what the macro provides:

| Reason | Examples |
|--------|----------|
| Extra CLI flags beyond `--dry-run` | `Logs` (`--keep-days`), `LibraryLogs` (`--all` / interactive) |
| Composite (runs multiple cleaners) | `Caches` |
| Pre-check before cleaning | `Xcode` (running process detection) |
| Completely different behavior | `Trash` (scan-only, warns on clean) |

For these, add a **special row** to the `define_cleaners!` table plus
**manual dispatch** in `src/main.rs`:

1. Add one special row to the table. The row header carries the variant,
   the CLI name, the clap doc, the `special` marker, the `targets`
   listing text and the `dry_run` marker; the fields stay verbatim:

```rust
// In src/main.rs, in the special rows section of the define_cleaners! table:
MySpecialTarget : "my-target"
    => "Description of what this cleans" ;
special ;
{
    #[arg(long)]
    dry_run: bool,
} ;
"targets listing text" ;
dry_run,
```

2. Add a match arm inside the special-targets block in `main()` (see existing
   handlers for `Logs`, `Xcode`, `Caches`, `Trash`, and `LibraryLogs`):

```rust
// In src/main.rs, inside the if target.is_special() { match target { ... } } block:
CleanTarget::MySpecialTarget { dry_run } => {
    let cleaner = cleaners::my_target::MyTargetCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("my-target", |dry| cleaner.clean(dry), dry_run)?;
}
```

That's it. The macro auto-generates from the row:
- `CleanTarget::MySpecialTarget { ... }` enum variant (clap attrs from the header)
- `SPECIAL_TARGETS` entry and its `ALL_TARGETS` union (the `targets` output)
- `command_name()` / `dry_run()` / `is_special()` match arms, exhaustive
  without a `_` arm: removing the row's CLI name drops the build
- the `is_special()` gate in `main()` picks the variant up automatically

No manual edits to `impl CleanTarget`, `SUPPORTED_TARGETS`, or the
`matches!()` check are needed for special cleaners. The run arm stays
manual: the special bodies are genuinely heterogeneous (validation, exit
paths), so only the arm body is hand-written.

### Both paths

4. Add E2E test in `tests/<name>.rs`; add unit tests as needed
5. Pass quality gates:

```bash
cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test
```

See existing cleaners (`uv.rs` for a standard cleaner, `library_logs.rs` for a
special-dispatch cleaner) for reference.


</div>

</details>
