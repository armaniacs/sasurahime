# PBI: 削除を1つのシームに統一する（SafeDeleter）

## ユーザーストーリー
sasurahime を `--permanent` で実行する macOS ユーザーとして、削除の安全性ルール（uchg 解除・Trash/Permanent モード・uses_trash の真実報告）が1箇所に集中してほしい、なぜなら現状 chflags 前処理が 13 ファイル 15 サイトに 3 種類の失敗セマンティクスでコピーされ、16 クリーナーがグローバルモード削除なのに `uses_trash: true` をハードコードして誤報告しているから。

## 優先度
- 順位: 1 / 8（依存優先。スコア順では 2 位だが、DeleteOutcome が他候補 2/6/8 の土台シームであり generic.rs 等のファイル重複があるため先頭）
- RICEスコア: 200（Reach=100 / Impact=3 / Confidence=100% / Effort=1.5人日）
- 根拠: 全削除経路に影響。uses_trash 誤報告（`--permanent` で恒久削除なのに Trash 使用と報告）は実測で確認済みの正しさのバグ。CLAUDE.md の uchg 規約が慣習でしか守られていない局所性の失敗

## BDD受け入れシナリオ
Scenario: 恒久モードの削除が uchg フラグを解除してから削除する
  Given `DeleteCtx::new(TrashMode::Permanent)` と uchg が立ったディレクトリ
  When  `SafeDeleter::delete` が呼ばれる
  Then  chflags -R nouchg が先に実行され、削除が成功する

Scenario: chflags 失敗は警告として記録され削除は試行される
  Given chflags コマンドが失敗する runner
  When  delete が呼ばれる
  Then  log::warn が記録され、削除の成否が DeleteOutcome に反映される

Scenario: uses_trash は削除時に解決されたモードの真実を返す
  Given TrashMode::Permanent で削除が成功した
  When  DeleteOutcome を検査する
  Then  uses_trash は false である（Trash 使用と報告しない）

Scenario: スキップ可能エラーは skip_reason として返る
  Given 削除先が存在しない等の skippable エラー
  When  delete が呼ばれる
  Then  Err ではなく skip_reason=Some を持つ DeleteOutcome が返る

## 受け入れ基準
- [ ] `SafeDeleter::delete(runner, path, mode) -> Result<DeleteOutcome>` が src/trash.rs に実装され、uchg・モード解決・is_skippable 分類を内部に持つ
- [ ] 15 サイトの chflags 重複（generic.rs:583/825/1038、mise.rs、gradle.rs:109/256、app_support.rs:511、cargo.rs:103/135、custom.rs:108、device_support.rs:153、ollama.rs:218、orphan_containers.rs:446、library_caches.rs:239、library_logs.rs:229/315、ios_backup.rs:154）が seam 経由に置き換わる
- [ ] xcode.rs / project_artifacts.rs / uv.rs（fs::remove_dir_all 直呼び）も seam 経由になる
- [ ] hint/cleaner.rs の `rm -rf` 直接実行（trash-mode シーム回避）が seam 経由になる
- [ ] 16 クリーナーの `uses_trash: true` ハードコードが削除され、DeleteOutcome.uses_trash から返る
- [ ] 85 件の CleanResult 構造体リテラルに `CleanResult::freed / not_found / empty` コンストラクタを導入（uses_trash 判断を1箇所に）
- [ ] chflags 失敗のセマンティクスは warn-and-continue に統一（mise の propagate は seam の警告ログで代替、仕様として明記）

## テスト戦略
- 単体: trash.rs に chflags 失敗 × モード × skippable の表テスト（DeleteOutcome の真理を1箇所で固定）
- 統合: 各クリーナーは「outcome を返す」ことのみを検査するテストに簡素化
- E2E: 既存の TCC 環境依存テスト（4件）は回帰ゲート外として扱う

## 見積もり
2ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
