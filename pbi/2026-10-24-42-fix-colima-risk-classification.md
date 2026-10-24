# PBI-42: colima のリスク分類を Risky/Moderate に修正

## ユーザーストーリー

`--yes` や `auto` でスクリプト運用するユーザーとして、破壊的・不可逆なターゲットが無人実行の対象にならないようにしてほしい。なぜなら、colima は「停止VMの全ディスクデータ（containers, images, volumes）を削除する」と表示しながら Risk::Safe に分類され、--yes（skip_confirm 抑制）と非TTYの auto の両方で無人到達可能だから。

## 発見元

1023a 全体レビュー Major #8（generic.rs:205-214, main.rs:1417 RISK_EXPECTED, interactive.rs:147）。

## BDD受け入れシナリオ

```gherkin
Scenario: Risky に分類されたターゲットは --yes でも確認を求める
  Given colima が Risky（または Moderate 相当の確認付き）に分類されている
  When  ユーザーが sasurahime --yes を実行する
  Then  colima の clean は無人で実行されない（確認または除外される）

Scenario: TUI/interactive では従来どおり選択可能
  Given colima が Risky 分類に変更されている
  When  ユーザーが TUI で colima を選択して実行する
  Then  確認プロンプトが表示され、承認時のみ実行される
```

## 受け入れ基準

- [ ] main.rs の RISK_EXPECTED（auto 対象のリスク表）で colima を branch 分類基準「破壊的・不可逆」に従うカテゴリに変更
- [ ] 変更に対応する auto_policy/generic テストを更新（colima が auto の無人対象から外れることを assert）
- [ ] 分類根拠をコメントで記録（clean メッセージの「ALL stopped VM disk data」主張と分類の整合）
- [ ] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 統合: auto_policy の RISK_EXPECTED 対応表更新、--yes での colima 除外
- 単体: 分類関数のケース更新

## 実装者向け注記

分類の変更だけで clean メッセージ（generic.rs:205-214）は触らない。generic のメッセージは colima の実挙動を正しく記述している。RISK_EXPECTED は main.rs 側。tests/auto_policy.rs の対応表も更新が必要。
