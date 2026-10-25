# orphan-containers 削除 UX 改善とハング根因除去 設計書

対象: `sasurahime clean orphan-containers` の削除フロー (`OrphanContainersCleaner`)、`src/trash.rs`、`src/progress.rs`
ブランチ: `1025a` から切って実装します。

## 背景

2026-10-25、手元の `sasurahime clean orphan-containers` (TUI 経由) で、フォルダを選んで確定した後、
"Cleaning orphan-containers..." スピナーが出たまま 36 分間何も起きず、Ctrl-C で止めた。
原因は trash クレート (5.2.6) のデフォルトバックエンド Finder が
`osascript -e 'tell application "Finder" to delete { POSIX file "..." }'` を起動し、
macOS の TCC 許可ダイアログ (ターミナルの裏に隠れる) の回答待ちで止まること。

trash クレートの DeleteMethod 比較 (vendored クレートの src/macos/mod.rs ドキュメントより):

| | Finder (デフォルト) | NsFileManager |
|---|---|---|
| 追加権限 | 要求されることがある (TCC ダイアログ) | 不要 |
| 速度 | 遅い | 速い |
| "Put Back" メニュー | ある | 一部の環境で出ない (macOS のバグ) |

## 要件

1. 消そうとしているファイルパスを表示する
2. 削除中に副作用が生じると思われるときは、先に断る。生じないのなら現状のままでよい
3. 何の情報更新もないまま 10 秒以上すぎないようにする

## 設計 (採用: NsFileManager 切替 + パス表示 + 10 秒ウォッチドッグ)

### Task 1: NsFileManager バックエンド切替 (根因除去) - src/trash.rs

`DeleteCtx::delete_path` の trash 分岐 (現状 `trash::delete(path)` の呼び出し) を変更する:

```rust
let mut ctx = trash::TrashContext::new();
trash::macos::TrashContextExtMacos::set_delete_method(
    &mut ctx,
    trash::macos::DeleteMethod::NsFileManager,
);
ctx.delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
```

- osascript を起動しないので TCC ダイアログが出ず、ハングしない (36 分ハングの根因除去)
- Trash 行きは変わらず復元可能。速く、無音
- permanent モード (`--permanent` / config trash_mode = false) の `rm -rf` は変更しない
- 既存の `delete_ctx_trash_mode_moves_to_trash` がそのまま通ること (挙動不変: Trash 行き)

### Task 2: Put Back の断り - src/cleaners/orphan_containers.rs

`interactive_select` のヘッダー (println の 1 行) に追加する:

```
Moved items may not show "Put Back" in Finder's context menu. Restore by dragging out of the Trash.
```

- 権限ダイアログの副作用は Task 1 で消えるので、警告はこの 1 行のみ (要件 2 の「副作用がなければ現状のままでよい」)

### Task 3: フルパス表示 - src/cleaners/orphan_containers.rs `format_item`

`format_item` (現状はフォルダ名のみ) をフルパス表示に変更する:

- `c.path.display()` を表示する。列幅 56 を維持し、長いパスは中間を省略する
  (例: `/Users/u/Library/…/group.is.workflow.my.app`)
- サイズと種類の列は現状どおり
- dry-run の `would remove:` はすでにフルパスなので変更しない
- app_support 側の `format_candidate_label` はすでに root/sub を出すので変更しない

### Task 4: 10 秒ウォッチドッグ - src/progress.rs

`SharedSpinner` にウォッチドッグを追加する:

- 最終更新から 10 秒を超えたらメッセージ末尾に `(elapsed 1m20s)` を付けて更新する。
  1 秒間隔でチェックし、ベースメッセージは保持する
- `finish()` でウォッチドッグを停止する
- しきい値は差し替え可能にする (テストでは短時間で確認する)
- `with_parallel_scan` (スキャンフェーズ) にも同じ仕組みを適用する
- 通常の削除中は tick ごとのパス表示が優先される (従来どおり。`set_message` が最終更新時刻を更新する)

### Task 5: テスト

- `format_item` のフルパス対応: 既存の単体テスト (`format_item_contains_name_size_and_kind`) を
  パス対応に更新する
- ウォッチドッグ: しきい値 50ms で elapsed 表示を確認するテストと、整形関数 (純関数) の単体テスト
- trash: 既存テストがそのまま通ること (NsFileManager バックエンドでもテストは速い。ダイアログなし)
- E2E: `clean orphan-containers --dry-run` でフルパス表示を確認する

## 対象外

- クリーン時の再スキャン削除 (TUI の detect_all 結果の再利用): 別のパフォーマンス課題。
  ウォッチドッグで可視性は確保される
- 他クリーナーのパス表示: app_support はすでにフルパス表示済み

## 検証コマンド (着手前に確認)

```bash
grep -n "fn delete_path" src/trash.rs                       # 1 行出る
grep -n "fn format_item" src/cleaners/orphan_containers.rs  # 1 行出る
grep -n "struct SharedSpinner" src/progress.rs              # 1 行出る
grep -n "trash = " Cargo.lock                               # 1 行出る
```
