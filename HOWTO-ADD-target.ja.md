# HOWTO: 新しいクリーンターゲットを追加する

sasurahime に新しいキャッシュ削除対象を追加する手順です。

## 1. まず Issue を立ててください

コードを書き始める前に、**必ず Issue（または PR のコメント）で提案を議論** してください。
以下の項目を確認します：

- 対象ツールが使うキャッシュの種類
- ディスク上のパス
- 削除方法（外部CLIコマンド or ディレクトリ削除）
- 安全上の懸念（イミュータブルフラグ、実行中プロセスなど）

```
Title: feat: support <tool> cache cleaning
Body:  - ツール名 / バージョン
       - キャッシュディレクトリのパス
       - 削除方法
       - 安全上の懸念
```

## 2. 実装

クリーナーの複雑さに応じて 2 つの実装パスがあります。

### パス A: 標準クリーナー（推奨）

ほとんどのクリーナー（シンプルな `{ dry_run: bool }` バリアント + 単一の
ファクトリ関数）は `define_cleaners!` マクロを使用します。必要な変更は
**3 箇所**だけです：

1. **作成** `src/cleaners/<name>.rs` に `Cleaner` トレイトを実装
2. **登録** `pub mod <name>;` を `src/cleaners/mod.rs` に追加
3. **1行追加** `src/main.rs` の `define_cleaners!` マクロ呼び出し内：

```rust
// src/main.rs の define_cleaners! ブロック内：
MyNewTarget : "my-new-target" => "説明文";
(|home, _config| cleaners::my_new_target::MyNewTargetCleaner::new(home, Box::new(SystemCommandRunner))),
```

これだけで、マクロが以下を自動生成します：
- `CleanTarget::MyNewTarget { dry_run: bool }` enum バリアント
- `SUPPORTED_TARGETS` エントリ（名前 → 説明）
- `dispatch_clean()` の match arm
- `command_name()` / `dry_run()` のヘルパー

標準クリーナーでは `CleanTarget`、`SUPPORTED_TARGETS`、`all_cleaners()`、
`main()` match の手動編集は不要です。

### パス B: 特殊ディスパッチクリーナー

一部のクリーナーはカスタムディスパッチロジックが必要です：

| 理由 | 例 |
|--------|----------|
| `--dry-run` 以外の追加フラグ | `Logs`（`--keep-days`）、`LibraryLogs`（`--all` / 対話的） |
| 複数クリーナーの統合 | `Caches` |
| 削除前チェック | `Xcode`（実行中プロセス検出） |
| 完全に異なる動作 | `Trash`（スキャン専用、削除時に警告） |

この場合、パス A に加えて手動ディスパッチを追加します：

1. マクロが enum バリアントと基本登録を生成する
2. `main()` の特殊ターゲットブロックに match arm を追加：

```rust
// src/main.rs の if matches!(target, ...) { match target { ... } } ブロック内：
CleanTarget::MySpecialTarget { dry_run } => {
    let cleaner = cleaners::my_target::MyTargetCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("my-target", |dry| cleaner.clean(dry), dry_run)?;
}
```

3. `matches!()` チェックと `impl CleanTarget` メソッド
（`command_name()`、`dry_run()`）にバリアント名を追加

### 両パス共通

4. E2E テストを `tests/<name>.rs` に追加（必要に応じてユニットテストも）
5. 品質ゲート通過：

```bash
cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test
```

既存の cleaner を参考にしてください。標準クリーナーは `uv.rs`、
特殊ディスパッチは `library_logs.rs` が参考になります。
