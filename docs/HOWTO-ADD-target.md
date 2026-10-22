---
layout: doc
title: "Add a new clean target / 新しいクリーンターゲットの追加方法"
permalink: /HOWTO-ADD-target
---
<details open markdown="1">
<summary markdown="0"><strong>🇺🇸 English</strong></summary>

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

New standard targets appear in scan automatically but are excluded from --yes;
add the cleaner's name() to AUTO_CLEAN_TARGETS in src/main.rs only if it is safe
to run unattended.

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

</details>

<details markdown="1">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

sasurahime に新しいキャッシュクリーンターゲットを追加する手順を説明します。

## 1. まず Issue を開く

コードを書く前に、**Issue を開いて**（または既存の PR にコメントして）提案を議論してください。以下を確認します：

- 対象ツールが使用するキャッシュ
- ディスク上のキャッシュ位置
- クリーン方法（外部 CLI コマンドまたはディレクトリ削除）
- 安全性に関する懸念（不変フラグ、実行中プロセスなど）

```
Title: feat: support <tool> cache cleaning
Body:  - ツール名 / バージョン
       - キャッシュディレクトリのパス
       - クリーン方法
       - 安全性に関する懸念
```

## 2. 実装

クリーナーの複雑さに応じて 3 つのパスがあります。

### パス G：ジェネリッククリーナー（最短、キャッシュターゲットには推奨）

新しいターゲットが外部 CLI ツールへの委譲か、既知ディレクトリの削除のいずれかなら、`src/cleaners/generic.rs` の `CliCleaner` または `PathCleaner` を選びます。合計 **2 箇所** の変更で、新規ファイルは不要です：

```rust
// src/cleaners/generic.rs の impl CliCleaner 内（外部ツールがクリーンする場合）：
pub fn my_tool(runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-tool", "my_tool", &["cache", "clean"], runner)
}
// または impl PathCleaner 内（固定ディレクトリリスト、年齢/サイズフィルタ可）：
pub fn my_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-cache", vec![home.join(".my-cache")], runner)
}
```

```rust
// src/main.rs の define_cleaners! ブロック内：
MyTool : "my-tool" => "MyTool パッケージキャッシュ";
(|_home, _config| cleaners::generic::CliCleaner::my_tool(Box::new(SystemCommandRunner))),
```

CLI 委譲とパス削除の分岐は 2 つの型に閉じているため、共有ディスパッチ列挙型を理解する必要はありません。サイズをファイルシステムから計測したい場合は `CliCleaner::with_detect_dir` ファクトリを、年齢やサイズのフィルタが必要なら `PathCleaner::with_older_than` / `with_larger_than` を使います。

### パス A：標準クリーナー（推奨）

ほとんどのクリーナー（シンプルな `{ dry_run: bool }` 型で単一のファクトリ関数を持つもの）は `src/main.rs` の `define_cleaners!` マクロを使用します。追加には **3 箇所** の変更が必要です：

1. `Cleaner` トレイトを実装した `src/cleaners/<name>.rs` を **作成**
2. `src/cleaners/mod.rs` に `pub mod <name>;` を **登録**
3. `src/main.rs` の `define_cleaners!` 呼び出しに **1 行追加**：

```rust
// src/main.rs の define_cleaners! ブロック内：
MyNewTarget : "my-new-target" => "このクリーナーの説明";
(|home, _config| cleaners::my_new_target::MyNewTargetCleaner::new(home, Box::new(SystemCommandRunner))),
```

これだけです。マクロが自動生成するもの：
- `CleanTarget::MyNewTarget { dry_run: bool }` 列挙子
- `SUPPORTED_TARGETS` エントリ（名前 → 説明）
- `dispatch_clean()` のマッチアーム
- `command_name()` / `dry_run()` ディスパッチヘルパー

標準クリーナーでは `CleanTarget`、`SUPPORTED_TARGETS`、`all_cleaners()`、`main()` のマッチへの手動編集は不要です。

新しい標準ターゲットは自動で scan に出ますが --yes の対象外です。無人実行しても安全な場合だけ、src/main.rs の AUTO_CLEAN_TARGETS に name() を追加してください。

### パス B：特別ディスパッチクリーナー

一部のクリーナーはマクロが提供する以上のカスタムディスパッチロジックが必要です：

| 理由 | 例 |
|--------|------|
| `--dry-run` 以外の追加 CLI フラグ | `Logs`（`--keep-days`）、`LibraryLogs`（`--all` / インタラクティブ） |
| 複合（複数クリーナーを実行） | `Caches` |
| クリーン前の事前チェック | `Xcode`（実行中プロセスの検出） |
| 完全に異なる動作 | `Trash`（スキャンのみ、クリーン時に警告） |

これらの場合は、上記のパス A に従った上で `src/main.rs` に **手動ディスパッチ** を追加します：

1. マクロが列挙子と基本登録を処理します
2. `main()` の special-targets ブロック内にマッチアームを追加します（`Logs`、`Xcode`、`Caches`、`Trash`、`LibraryLogs` の既存ハンドラを参照）

```rust
// src/main.rs の if matches!(target, ...) { match target { ... } } ブロック内：
CleanTarget::MySpecialTarget { dry_run } => {
    let cleaner = cleaners::my_target::MyTargetCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("my-target", |dry| cleaner.clean(dry), dry_run)?;
}
```

3. `matches!()` チェックと `impl CleanTarget` のメソッド（`command_name()`、`dry_run()`）に列挙子名を追加します。

### 両方のパスに共通

4. `tests/<name>.rs` に E2E テストを追加し、必要に応じてユニットテストを追加します。
5. 品質ゲートを通過させます：

```bash
cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test
```

参考として、既存のクリーナー（標準クリーナーは `uv.rs`、特別ディスパッチクリーナーは `library_logs.rs`）を参照してください。

</details>
