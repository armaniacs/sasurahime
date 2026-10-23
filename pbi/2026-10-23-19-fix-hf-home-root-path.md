# PBI: HF_HOME=/ のキャッシュパスがルート直下を指す問題

## ユーザーストーリー
`HF_HOME` を設定する macOS ユーザーとして、`HF_HOME=/` のような値がキャッシュパスの安全チェックを通らないでほしい、なぜなら `cache_dir()` が `"hub"` を結合した後 (`/hub`) に安全判定するため、ルート直下のディレクトリが削除対象と判定されうるから（PBI 13 の実装中に実測で確認: `/hub` は `is_safe_delete_target` を通過する）。

## 優先度
- 順位: 8 / 8
- RICEスコア: 160（Reach=100 / Impact=2 / Confidence=80% / Effort=1.0人日）
- 根拠: PBI 13 の実装中に実測で発見（`/` 保持版テストが `left: "/hub"` で赤）。実害には `HF_HOME=/` という珍しい設定と `/hub` の存在が必要だが、削除対象判定の穴として閉じる価値がある

## BDD受け入れシナリオ
Scenario: ルート直下に結合されたキャッシュパスは拒否される
  Given `HF_HOME=/` が設定されている
  When  `cache_dir()` が呼ばれる
  Then  `/hub` は削除対象と判定されず、デフォルトにフォールバックする

Scenario: 通常の HF_HOME は影響を受けない
  Given `HF_HOME=/Users/u/hf` が設定されている
  When  `cache_dir()` が呼ばれる
  Then  `/Users/u/hf/hub` が返る（現行どおり）

## 受け入れ基準
- [ ] `cache_dir`（huggingface）が `HF_HOME=/` に対してデフォルトにフォールバックする
- [ ] 仕様の決定: ルート直下（親が `/`）のパスを unsafe とみなすか、`cache_dir` 側で HF_HOME 自体を検証するかをレビューで決める
- [ ] 単体テストが `HF_HOME=/` のフォールバックを固定する

## テスト戦略
- 単体: `HF_HOME=/` のフォールバック（EnvGuard + TEST_LOCK 規約どおり）
- 統合: `clean huggingface` が `/hub` を削除しないこと

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
