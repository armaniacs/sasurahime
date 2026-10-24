# PBI-44: 4クリーナーが CleanOptions.trash_mode を尊重する（pin 契約の残り）

## ユーザーストーリー

CleanOptions.trash_mode を pin する呼び出し側として、すべてのクリーナーが pin を尊重してほしい。なぜなら、pin 契約（cleaner.rs:124-127）は Some(v) がモードを固定すると文書化しているのに、library_caches / orphan_containers / app_support / mise が TrashMode::Global をハードコードし、run_batch の note は opts からモード導出するため、pin された Some(Permanent) で note「Permanently deleted」vs 実際は Trash の不整合が起きうるから。

## 発見元

1023a 全体レビュー Major #9（library_caches.rs:236, orphan_containers.rs:419, app_support.rs:507, mise.rs:197）。

## BDD受け入れシナリオ

```gherkin
Scenario: pin された Some(Permanent) は4クリーナーの削除を permanent にする
  Given library-caches / orphan-containers / app-support / mise が clean_with_opts を実装している
  When  opts.trash_mode = Some(Permanent) で clean する
  Then  削除は permanent パスを通る
  And   uses_trash は false を報告する

Scenario: None は現行どおり Global にフォールバックする
  Given 4クリーナーが clean_with_opts を実装している
  When  opts.trash_mode = None で clean する
  Then  挙動は現行（Global 解決）と同一である
```

## 受け入れ基準

- [ ] 4クリーナー（library_caches / orphan_containers / app_support / mise）に clean_with_opts を実装し、opts.trash_mode.unwrap_or(Global) を safe_delete に渡す
- [ ] 各クリーナーに pin テスト（Some(Permanent) / Some(Trash) / None）を追加。PBI 34 の CliCleaner/ProjectArtifacts の pin テスト形状（TEST_LOCK + TrashModeGuard、TCC 許容の seam-truth 条件）に倣う
- [ ] 現行の None→Global 呼び出しは挙動中立（全既存テスト green）
- [ ] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 単体: 各クリーナーの pin テスト（3形）。TDD red（現行ハードコードが pin を無視）→ green

## 実装者向け注記

PBI 34（commit 6887842）が CliCleaner/ProjectArtifacts を実装済み。同じ clean_with_mode + clean_with_opts パターンを踏襲する。mise は plugin/external ではなく内部削除ループを持つため対象。app_support は Terminal guard があるがテストは TrashModeGuard で書ける。既知の TCC 環境依存4件（+xcode フレーク）は回帰ではない。
