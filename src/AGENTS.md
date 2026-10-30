# AGENTS.md — src/ implementation

Guidance for AI agents editing `src/`. Repo-wide contract (commands, safety rules, subagent-driven workflow): `../AGENTS.md`. PBI process and branch/commit rules: `../pbi/AGENTS.md`.

## Cleaner trait (`cleaner.rs`)

Every cleaner implements the shared `Cleaner` trait:

```rust
pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;   // read-only, never deletes anything
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter,
             opts: &CleanOptions) -> Result<CleanResult>;
    fn is_available(&self) -> bool { true }    // false = skipped during scan
    fn risk(&self) -> Risk { Risk::Moderate }  // only Risk::Safe runs under --yes
}
```

- `detect()` must never delete anything. `clean(dry_run: true)` must never delete either.
- There is exactly one `clean` per cleaner and every impl must honour `opts` (filters + trash mode); no fallback path that ignores it.
- Deletion safety (enforced scope check, `uchg` flag clearing, skip/fatal classification) concentrates in `trash.rs` / `safety.rs`; external tools delete themselves outside this seam via `command::CommandRunner`.

## Cleaners (`cleaners/`, one module per target family)

Core cleaners from the early PBIs:

| Module | What it touches |
|---|---|
| `uv` | `~/.cache/uv/` — `uv cache prune --force`, removes old `simple-vN` dirs |
| `brew` | runs `brew cleanup -s --prune=all`, parses freed bytes from stdout |
| `mise` | `~/.local/share/mise/installs/` — reads global + per-project `.mise.toml` before removing |
| `browser` | `~/.cache/puppeteer/` and `~/Library/Caches/ms-playwright*/` — keeps highest build number only |
| `generic` | delegates to `bun pm cache rm`, `go clean -cache`, `pip cache purge`, removes `node-gyp` dirs |
| `log` | `~/.local/share/kilo/log/` — deletes `*.log` older than N days, skips `dev.log` |

Full target list: `sasurahime targets` or `docs/SUPPORTED.md`.

## Entry points (`main.rs`)

`sasurahime scan [--json]`, `sasurahime clean <target>`, `sasurahime targets`, `sasurahime explore`, `sasurahime stats` (alias `history`), `sasurahime doctor`, `sasurahime auto [--dry-run]`, `sasurahime --yes` (non-interactive full clean), no args = interactive TUI.
