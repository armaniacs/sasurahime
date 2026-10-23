# PBI: デッドグローバルの除去（DRY_RUN / with_spinner / USE_UNICODE）

## ユーザーストーリー
sasurahime の保守担当者として、書き込み専用・本番未使用のグローバルがなく、表示スタイルのフラグが読み手の隣に置いてほしい、なぜなら `context::DRY_RUN` は set されるが本番コードから一度も読まれず、`progress::with_spinner` は本番未使用で fallible クロージャに `[OK]` を出す罠を持つシグネチャで残り、`USE_UNICODE` は history モジュールに属しながら scanner/explorer/hint が表示のために読んでいるから。

## 優先度
- 順位: 6 / 8
- RICEスコア: 80（Reach=50 / Impact=1 / Confidence=80% / Effort=0.5人日）
- 根拠: 削除テストで「複雑さが移動せず消失する」純粋削除が 2 件。USE_UNICODE の移動は局所性の修正

## BDD受け入れシナリオ
Scenario: write-only グローバルの削除で挙動が変わらない
  Given context::DRY_RUN が削除される
  When  フルテストが走る
  Then  テストは green を維持する（読み手が存在しなかった証拠）

Scenario: 本番未使用ヘルパーの削除で罠が消える
  Given progress::with_spinner が削除される
  When  フルテストが走る
  Then  [OK]-on-error 罠のシグネチャがコードベースから消失する

Scenario: 表示スタイルは読み手の隣から取得される
  Given USE_UNICODE が表示 seam に移動する
  When  scanner / explorer / hint がスタイルを決める
  Then  history モジュールをまたがない

## 受け入れ基準
- [x] `context::DRY_RUN`（write-only、`#[allow(dead_code)]` 付き）が削除される
- [x] `progress::with_spinner`（本番未使用、`[OK]`-on-error 罠）が削除される
- [x] `USE_UNICODE` の実体が表示 seam に移動し、history は保存専用になる
- [x] 削除後に clippy -D warnings と fmt --check が通る
- [x] 削除の根拠（読み手不在・本番未使用）が WHY コメントまたはコミットメッセージに残る

## テスト戦略
- 単体: 削除対象の既存テストを削除、フルスイートで regression を確認
- 統合: scan/explorer/hint の表示テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: commit 1b734dc。
- context::DRY_RUN 削除: 削除前の grep で本番の読み手が存在しないことを確認 (is_dry_run は allow(dead_code) 付き、参照は context.rs 自身と write-only の store 呼び出し main.rs:841 のみ)。static / set_dry_run / is_dry_run / テスト 2 件 / TEST_LOCK doc の言及を削除。
- progress::with_spinner 削除: grep で本番呼び出しが存在しないことを確認 (定義と自己テストのみ、with_spinner_result 系は別関数)。FnOnce() -> R で fallible クロージャに無条件に [OK] を出す罠のため削除。
- USE_UNICODE 移動: 実体を format.rs の private static + use_unicode() / set_use_unicode() に移動。history.rs は atomic 参照ゼロの保存専用ロジックに。読み手 (scanner.rs, explorer.rs x2, hint/cleaner.rs, main.rs) を新アクセサに更新。
- 検証: cargo test --lib 644 passed / 4 failed (4 件は既知の TCC 環境依存)、統合 36 ターゲット green、clippy 警告 0、fmt クリーン。
- 根拠: コミットメッセージの WHY (読み手不在・本番未使用・表示ポリシーが history モジュールを経由していた) に記録。
