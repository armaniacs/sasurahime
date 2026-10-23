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
- [ ] `context::DRY_RUN`（write-only、`#[allow(dead_code)]` 付き）が削除される
- [ ] `progress::with_spinner`（本番未使用、`[OK]`-on-error 罠）が削除される
- [ ] `USE_UNICODE` の実体が表示 seam に移動し、history は保存専用になる
- [ ] 削除後に clippy -D warnings と fmt --check が通る
- [ ] 削除の根拠（読み手不在・本番未使用）が WHY コメントまたはコミットメッセージに残る

## テスト戦略
- 単体: 削除対象の既存テストを削除、フルスイートで regression を確認
- 統合: scan/explorer/hint の表示テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
