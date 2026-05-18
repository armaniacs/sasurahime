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

Issue で合意が取れたら、既存の cleaner を参考に実装します：

1. `src/cleaners/<name>.rs` に `Cleaner` トレイトを実装
2. `src/cleaners/mod.rs` に `pub mod <name>;` を追加
3. `src/main.rs` の `CleanTarget` enum / `all_cleaners()` / `main()` match に配線
4. E2E テストを `tests/<name>.rs` に追加（必要に応じてユニットテストも）
5. 品質ゲート通過: `cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test`

既存の cleaner（`uv.rs`、`brew.rs`、`mise.rs` など）の実装パターンを参考にしてください。
