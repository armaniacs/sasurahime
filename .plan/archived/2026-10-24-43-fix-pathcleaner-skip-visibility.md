# PBI-43: PathCleaner/CliCleaner の skip 報告を可視化（SkippedEntry 記録 + uses_trash 真実化）

## ユーザーストーリー

`clean <target>` や auto を実行するユーザーとして、権限拒否などの削除失敗が失敗として報告されてほしい。なぜなら、PathCleaner/CliCleaner の seam-skip が `Ok(0)` + `log::warn!` のみで CleanResult の skipped がハードコード `vec![]` により可視 failure が消え、exit 0 になるから（main では `?` で可視だった）。

## 発見元

1023a 全体レビュー Major #6 + Minor（generic.rs:798-805, 769, 566-577）。main 側の伝播は `git show main:` で確認済み。

## BDD受け入れシナリオ

```gherkin
Scenario: 権限拒否の削除は skipped として報告される
  Given node-gyp ディレクトリが権限拒否になる環境がある
  When  ユーザーが clean node-gyp を実行する
  Then  削除失敗が CleanResult の skipped に記録される
  And   CLI のスキップ報告（"N file(s) skipped"）に現れる

Scenario: ゼロ削除の実行は Trash を主張しない
  Given PathCleaner の全対象が存在しない
  When  clean を実行する
  Then  uses_trash は false を報告する（ゼロ削除で Trash 主張なし）
```

## 受け入れ基準

- [x] PathCleaner::clean_one/clean_filtered が seam-skip を SkippedEntry として CleanResult に記録（`skipped: vec![]` ハードコードの除去）
- [x] PathCleaner の `uses_trash: mode.resolve()` をゼロ削除/dry-run/tripwire-refuse で false に（seam outcome 由来のみ true）
- [x] CliCleaner の seam-skip return（566-577）が `outcome.uses_trash` を報告しないよう整合（tripwire-case コメント 552-555 と矛盾しない）
- [x] 下流（note/history/freed_message は bytes_freed > 0 条件）の挙動中立を維持
- [x] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 単体: PathCleaner の skip 記録テスト（権限拒否を POSIX で決定論化、chmod 0o555 親）、ゼロ削除 uses_trash テスト。TDD red → green

## 実装者向け注記

seam 分類（is_skippable_error → skip_reason）は変更しない。CleanResult の skipped チャネルへの記録のみ。library_caches などの既存 SkippedEntry 形状に倣う。bytes_freed が 0 の場合の下流挙動は変わらないことを既存テストで確認。

## 検証記録 (2026-10-24)

- 実装コミット: e702a31 (wt-skip、1023a にマージ済み)
- 内容: `PathCleaner::clean_one` を `Result<(u64, bool, Option<SkippedEntry>)>` に変更し、seam-skip (outcome.skip_reason Some) を SkippedEntry として記録。`clean_filtered` が accumulated skipped を CleanResult に運搬 (ハードコード `vec![]` を除去)。uses_trash は削除 outcome の OR-in のみで、ゼロ削除/dry-run/tripwire-refuse/seam-skip は false。CliCleaner の seam-skip return も `uses_trash: false` に整合。
- 備考: 下流中立を確認 (post_clean_note は bytes_freed == 0 → None、history は !dry_run && bytes_freed > 0)。可視化は main.rs の "N file(s) skipped" 報告と CleanResult::exit_code → exit 1 経由。TDD テスト (permission-denied + 全非存在) は generic.rs の新規セクション。
- 検証: 1023a HEAD 64c44a1 で lib 失敗 ⊆ 既知 TCC セット / bin 30/30 / 統合 38 ターゲット green / clippy 0 warnings / fmt clean。
