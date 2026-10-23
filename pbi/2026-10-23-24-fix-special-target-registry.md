# PBI: 特殊ターゲット登録の一元化（TargetSpec）

## ユーザーストーリー
sasurahime に新しいターゲットを追加する開発者として、1 箇所の編集だけで登録・構築・dispatch が揃ってほしい、なぜなら 13 特殊ターゲットが 5 箇所の同期ポイント（matches! ガード、~300 行 dispatch、special_cleaners、command_name/dry_run 手書き実装、name+desc テーブル）に散らばり、registry と CLI が同じターゲットに対して異なる設定のクリーナーを構築しているから（DeviceSupport: registry は keep=2、CLI は --keep）。

## 優先度
- 順位: 5 / 8
- RICEスコア: 48（Reach=60 / Impact=2 / Confidence=80% / Effort=2.0人日）
- 根拠: 特殊ターゲット 13/56 と registry/CLI 一致に影響。full macro 拡張は L だが、構築統一 + 重複テーブル削除に絞れば達成可能

## BDD受け入れシナリオ
Scenario: registry と CLI が同じ factory を使う
  Given DeviceSupport の factory が 1 実装になる
  When  registry（scan）と CLI（clean --keep 5）が構築する
  Then  同じ factory 経由で、引数の差だけが残る

Scenario: 重複した name+desc テーブルは消失する
  Given cleaner.rs:222-258 の重複テーブル
  When  TargetSpec に統一される
  Then  名前と説明の定義は 1 箇所にのみ存在する

Scenario: 特殊ターゲットの対応テストが自明になる
  Given 5 sync points が 1 に統一される
  When  対応テスト（every_clean_target_is_registered_in_all_cleaners）が走る
  Then  テストは構造的に必ず成立する

## 受け入れ基準
- [ ] 特殊ターゲットの factory が 1 実装に統一され、registry と CLI dispatch の両方から使われる
- [ ] special_cleaners（main.rs:493-549）と CLI dispatch の重複構築が解消される
- [ ] cleaner.rs:222-258 の name+desc 重複テーブルが削除される
- [ ] DeviceSupport の keep=2 乖離が意図的（detect では keep 無関係）であることがテストまたはコメントで固定される
- [ ] 対応テストが追加の編集なしに green を維持する

## テスト戦略
- 統合: registry/CLI 一致の対応テスト
- 単体: factory が同じ型を返すことのテスト

## 見積もり
2ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
