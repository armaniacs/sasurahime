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
  → テスト: `cleaners::huggingface::tests::cache_dir_falls_back_when_hf_home_is_root` + `resolve_env_dir_falls_back_when_value_is_root`（実装前に `/hub` で赤、実装後に緑）

Scenario: 通常の HF_HOME は影響を受けない
  Given `HF_HOME=/Users/u/hf` が設定されている
  When  `cache_dir()` が呼ばれる
  Then  `/Users/u/hf/hub` が返る（現行どおり）
  → テスト: `cleaners::huggingface::tests::cache_dir_returns_joined_path_when_hf_home_is_safe` + `resolve_env_dir_returns_joined_path_when_value_is_safe`

## 受け入れ基準
- [x] `cache_dir`（huggingface）が `HF_HOME=/` に対してデフォルトにフォールバックする
- [x] 仕様の決定: ルート直下（親が `/`）のパスを unsafe とみなすか、`cache_dir` 側で HF_HOME 自体を検証するかをレビューで決める
- [x] 単体テストが `HF_HOME=/` のフォールバックを固定する

## 決定記録（PBI 21 実装時に確定）
- 仕様決定: **HF_HOME（オーバーライド値そのもの）を結合前に検証する方式を採用**。`resolve_env_dir(var, subpath, default, label)`（generic.rs）がオーバーライド値を先に `is_safe_delete_target` で検証し、安全な場合のみ `subpath` を結合するため、`HF_HOME=/` は `/hub` を生成せずデフォルトにフォールバックする。ルート直下（親が `/`）のパスを個別に unsafe とみなす方式は採用しなかった（全クリーナーがリゾルバ経由で同一規則を共有するため不要）。
- 実装: PBI 21 の `resolve_env_dir` 1本化（flutter/terraform/act/huggingface/pre-commit の 6 箇所を移行）で本 PBI をクラスとして閉じた。

## テスト戦略
- 単体: `HF_HOME=/` のフォールバック（EnvGuard + TEST_LOCK 規約どおり）
- 統合: `clean huggingface` が `/hub` を削除しないこと（既存の `tests/huggingface.rs` の env 安全テストがリゾルバ経由で実行される）

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
  - 注: 別PRのレビューは実施していません。safe_delete/resolve_env_dir リファクタ中の行単位レビューで検証 — verified via line-level review during the safe_delete/resolve_env_dir refactor
- [x] ドキュメント更新済み
  - 注: 本ファイルの決定記録と `resolve_env_dir` の doc コメントが仕様を記録。docs/SUPPORTED.md の HF_HOME 検証の記載はこの変更後も正しい
