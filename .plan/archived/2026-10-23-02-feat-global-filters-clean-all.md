# PBI: 明示的全消去とグローバルフィルタ統一

## ユーザーストーリー
macOS開発者として、`--older-than / --larger-than` を一度指定すれば全部のターゲットに効いてほしい、なぜなら現状は `PerCleanerFilters::None` が大半で `[cleaner.*]` に書いても効かず、起動時警告でごまかされているから

## ビジネス価値
設定の一貫性が回復し、「書いたのに効かない」混乱が消える。`clean --all --include-manual` による明示的全消去で、手動選択に戻らずに済む場面が増える。

## 優先度
- 順位: 02 / 4（新規分のみ）
- RICEスコア: 350（Reach=500 / Impact=2 / Confidence=70% / Effort=2.0人週）
- 根拠: 設定不信の解消効果が大きい。キャッシュPBI（01）とは独立に着手でき、ポリシーエンジン（03）の前提になる。

## BDD受け入れシナリオ
Scenario: グローバルフィルタが全部に効く
  Given `--older-than 30d --larger-than 100MB` を指定した
  When `clean --all` を実行する
  Then 全クリーナーが同じ年齢・サイズ条件で絞り込み、条件外はスキップ理由と共に表示される

Scenario: 明示しないと危険群は消えない
  Given `docker / ios-backup` が pruneable な状態がある
  When `clean --all` を実行する（`--include-manual` なし）
  Then 危険群はスキップされ、`--include-manual` 付きでのみ削除対象になる

Scenario: 効かない設定がなくなる
  Given 旧版で警告が出ていた `[cleaner.*]` 設定がある
  When 新版で起動する
  Then 「効かない」警告が出ず、設定が実際に適用される

## 受け入れ基準
- [ ] `--older-than / --larger-than` が全クリーナーに適用される（CLI委譲系の扱いも定義される）
- [ ] `clean --all [--include-manual]` が追加され、既存の `clean <target>` 挙動は不変
- [ ] `ineffective_per_cleaner_settings` の警告対象がゼロになるか、残る場合は理由が文書化される
- [ ] `AUTO_CLEAN_TARGETS` 廃止PBI（2026-10-22-04）との整合性が保たれる

## テスト戦略
- E2E: TEMP HOME＋偽コマンドでフィルタ絞り込みと `--include-manual` 分岐を検証
- 統合: 全クリーナーのフィルタ適用有無マトリクステスト
- 単体: 年齢・サイズ判定の境界値テスト

## 見積もり
5 SP（要チームでの見積もり）

## 実装アプローチ
1. `PerCleanerFilters` を全クリーナー対応に拡大（CLI委譲系は「検出ディレクトリ」基準を定義）
2. `clean --all` ディスパッチを追加し、`ManualOnly` との相互作用を整理
3. 起動時警告ロジックを更新

## 技術的考慮事項
- 外部コマンド系（`brew cleanup` 等）は年齢フィルタを自前で持てないため、検出パス基準に倒す等の定義が必要
- 破壊的変更のため `--yes` との関係を明記する

## 実装者向け注記
- 現状確認: `grep -rn "PerCleanerFilters\|ineffective_per_cleaner" src/ | head -n 30`
- 着手前に `cargo test` でグリーン確認。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（README のフィルタ説明を更新）
