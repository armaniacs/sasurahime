# PBI: clean 結果の「moved to Trash」表示を CleanResult.uses_trash に基づかせる

## ユーザーストーリー
sasurahime のユーザーとして、ゴミ箱を使わない cleaner の結果に「Trash に移動した」と表示されないでほしい。なぜなら、実際は完全に削除またはシステム側で解放されたのに、ゴミ箱から戻せると誤解して復元を試みるから。

## ビジネス価値
- 削除が可逆か不可逆かを、実際の動作どおりに伝える (誤解による復元失敗、逆に不要な確認を防ぐ)
- 死んでいる公開フィールド `CleanResult.uses_trash` に実際の読み手ができ、cleaner が自己申告した値が表示に反映される

## BDD受け入れシナリオ

```gherkin
Scenario: ゴミ箱を使わない cleaner で解放量が出る
  Given trash モードが有効である (既定)
  And   apfs-snapshot の clean が bytes_freed = 5 GiB, uses_trash = false を返す
  When  ユーザーが `sasurahime clean apfs-snapshot --all` を実行する
  Then  "Freed: 5.0 GiB" と表示される
  And   "moved to Trash" は表示されない

Scenario: ゴミ箱を使う cleaner は従来どおり
  Given trash モードが有効である
  And   cargo の clean が bytes_freed = 1 GiB, uses_trash = true を返す
  When  ユーザーが `sasurahime clean cargo` を実行する
  Then  "Freed: 0 B (1.0 GiB moved to Trash)" と表示される

Scenario: trash モードが無効なら uses_trash の値によらず通常表示
  Given --permanent が指定されている
  When  任意の cleaner が uses_trash = true で結果を返す
  Then  "Freed: <size>" と表示される

Scenario: 解放量が 0 のとき
  Given cleaner が bytes_freed = 0 を返す
  When  clean が完了する
  Then  "Freed: 0 B" と表示される
```

## 受け入れ基準
- [ ] 表示の分岐が `is_trash_mode()` だけでなく `result.uses_trash` も条件にしている
- [ ] `uses_trash == false` の結果に "moved to Trash" が出ない
- [ ] `uses_trash == true` かつ trash モードの出力は現行と同一
- [ ] `CleanResult.uses_trash` の `#[allow(dead_code)]` が不要になれば外す
- [ ] 既存の `run_clean_target` のテストが通る

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 不要。apfs-snapshot の E2E は tmutil と root が要り、メッセージの分岐は単体で十分に検証できる

### 統合テスト
- `run_clean_target` に、`uses_trash` が true / false の `CleanResult` を返すクロージャを渡して出力を検証する。出力の捕捉ができない場合は、表示文字列を組み立てる純粋関数に切り出して検証する

### 単体テスト
- `freed_message(bytes_freed, uses_trash, trash_mode) -> String` の 2 x 2 x (0 / 非 0) の表

## 実装アプローチ
- **Outside-In**: 表示用の純粋関数のテストを先に Red にする
- **Red-Green-Refactor**: `run_clean_target` の if 条件を関数呼び出しに置き換える
- **リファクタリング**: 同じ文言が `interactive.rs` にあれば同じ関数に寄せる (下記注記で確認)

## 見積もり
1 ストーリーポイント。理由: 分岐 1 か所の条件変更で、表示関数の切り出しとテストが作業の大半。

## 技術的考慮事項
- 依存関係: なし。apfs-snapshot の `bytes_freed` が 0 以外になる変更 (ブランチ feat/apfs-snapshot-enhancement) は、現在のブランチ docs/system-data-specs の祖先として取り込み済み。この PBI の再現にその変更が必要
- テスタビリティ: 標準出力の捕捉が難しいので純粋関数に切り出す
- 非機能要件: なし

## 実装者向け注記

### 現状コードの確認
```bash
cargo test
grep -n "moved to Trash" -r src/
grep -n "uses_trash" src/main.rs src/interactive.rs src/cleaner.rs
git merge-base --is-ancestor feat/apfs-snapshot-enhancement HEAD && echo included
```

事実確認の結果 (HEAD 時点):
- `src/main.rs:521` の `run_clean_target` 内、580 行付近で `if crate::trash::is_trash_mode() && result.bytes_freed > 0 { println!("Freed: 0 B ({} moved to Trash)", ...) } else { println!("Freed: {}", ...) }` としている。`result.uses_trash` は読んでいない。
- `src/cleaner.rs:66` の `pub uses_trash: bool` は doc コメントで "Whether the operation used macOS Trash (vs permanent deletion)" と定義され、`#[allow(dead_code)]` が付く。`main.rs` と `interactive.rs` に `result.uses_trash` を読む箇所は無い (grep で確認、main.rs と interactive.rs の出現は構造体の生成のみ)。
- `src/trash.rs:12` `is_trash_mode()` は `--permanent` と config `trash_mode` から決まる全体フラグ (`main.rs:662`)。既定は true なので、既定設定では「解放した」すべての cleaner がこの文言になる。
- `src/cleaners/apfs_snapshot.rs:211` は `uses_trash: false` を返し、同じファイルの `bytes_freed` は空き容量の差分 (`apfs_snapshot.rs:184-191`) で 0 以外になり得る。その結果 `sasurahime clean apfs-snapshot --all` は "Freed: 0 B (X moved to Trash)" と表示する。
- 他にも `uses_trash: false` で bytes_freed が 0 以外になり得る cleaner がある (`generic.rs` の Command 方式 480-500 行、`rustup.rs:107,144`)。同じ誤表示が出るので、受け入れ時にまとめて確認する。
- `generic.rs:559,581,693,885` は `uses_trash: crate::trash::is_trash_mode()` を返すため、これらは変更後も現行と同じ表示になる。

### 再現手順
1. 解放できるローカルスナップショットがある機で `sasurahime clean apfs-snapshot --all` を実行する (root が要るため sudo hint が出る場合は、削除に成功する環境で)
2. 末尾が "Freed: 0 B (<size> moved to Trash)" になる。スナップショットは Trash を経由しないので誤り

### 想定する修正方針
- 推奨案 A: 条件を `is_trash_mode() && result.uses_trash && result.bytes_freed > 0` に変える。理由: 最小変更で、`uses_trash` を cleaner の自己申告として使える
- 案 B: `uses_trash` を廃止して `is_trash_mode()` で統一する。却下: cleaner ごとに Trash を使うかが違う実態に合わない
- 案 C: cleaner が表示用文字列を返す。却下: 変更が大きく、この PBI のスコープ外

### 落とし穴
- `uses_trash` を信頼する前に、Trash を使うはずの cleaner が `uses_trash: false` を返していないか確認する (例: `cargo.rs:159` は true、`device_support.rs:117,135` は false の分岐がある)。誤って false を返す cleaner があると、本来の "moved to Trash" 表示が消える
- trash モード時に "Freed: 0 B (...)" と表示する意図 (空き容量は増えないという注意) を壊さない
- `interactive.rs` の合計表示 ("Total freed") は別経路。ここは扱わない

### 検証コマンド
```bash
cargo test freed_message
cargo test run_clean_target
cargo clippy -- -D warnings
cargo fmt --check
```

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] uses_trash を false で返す cleaner の一覧を確認し、誤りがあれば記録済み
