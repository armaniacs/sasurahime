# PBI: 環境変数キャッシュディリゾルバの1本化（resolve_env_dir）

## ユーザーストーリー
`TF_PLUGIN_CACHE_DIR` / `ACT_CACHE_DIR` / `HF_HOME` 等を設定する macOS ユーザーとして、環境変数由来のキャッシュパスの安全検証が全クリーナーで同じタイミング・同じ規則で行われてほしい、なぜなら env→検証→警告→フォールバックの ~17 行ブロックが 6 箇所にコピーされ、huggingface のコピーだけが `hub` 結合後に検証するため `HF_HOME=/` が `/hub` を削除対象と判定しうる（PBI 19 の実測ギャップ）。

## 優先度
- 順位: 2 / 8（スコア順では 1 位だが、SafeDeleter が土台で generic.rs のファイル重複があるため 2 番手）
- RICEスコア: 240（Reach=40 / Impact=3 / Confidence=100% / Effort=0.5人日）
- 根拠: PBI 19 のクラス修正。6 コピーのうち 1 つだけが結合後検証という実測済みの非対称。TOCTOU 再検証（clean 時）が 6 すべてに欠けている

## BDD受け入れシナリオ
Scenario: ルート直下の環境変数値は結合前に拒否される
  Given `HF_HOME=/` が設定されている
  When  リゾルバが呼ばれる
  Then  ベース `/` が is_safe_delete_target で拒否され、デフォルトにフォールバックする

Scenario: 不安全な値は警告とともにフォールバックする
  Given `ACT_CACHE_DIR=/etc` が設定されている
  When  リゾルバが呼ばれる
  Then  log 警告が出てデフォルトにフォールバックする

Scenario: 通常の値はそのまま使われる
  Given `HF_HOME=/Users/u/hf` が設定されている
  When  リゾルバが呼ばれる
  Then  `/Users/u/hf/hub` が返る（現行どおり）

## 受け入れ基準
- [x] `resolve_env_dir(var, subpath, default, label) -> PathBuf` が generic.rs に実装され、結合前のベース検証 + 警告 + フォールバックを1箇所に持つ
- [x] 6 コピー（generic.rs:242-255/310-323/344-357、huggingface.rs:22-36、pre_commit.rs:23-33/34-44）が seam 経由に置き換わる
- [x] `HF_HOME=/` のフォールバック単体テスト（EnvGuard + TEST_LOCK 規約）が通る ― PBI 19 の受け入れ基準を満たす
- [x] huggingface / pre_commit / generic の EnvGuard テストスイートがリゾルバ経由の1スイートに統合される
- [x] PBI 19（pbi/2026-10-23-19-fix-hf-home-root-path.md）が検証・チェックボックス更新・アーカイブで閉じられる

## テスト戦略
- 単体: リゾルバの `/etc`・`/tmp`・相対パス・HOME祖先ケースを1スイートで（EnvGuard + TEST_LOCK）
- 統合: `clean huggingface` が `/hub` を削除しないこと

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: `resolve_env_dir(var, subpath, default, label) -> PathBuf` を generic.rs に追加 (commit 968beda)。オーバーライド値そのものを結合前に `is_safe_delete_target` で検証し、不正値は警告 + フォールバック。
- 6 コピー置換: generic.rs flutter/terraform/act、huggingface.rs cache_dir()、pre_commit.rs PRE_COMMIT_HOME/XDG_CACHE_HOME の全コピーが 1 行呼び出しに置換。pre_commit は set-but-empty の優先順位を保つため is_ok() プリチェックを維持。
- `HF_HOME=/` のフォールバック単体テスト: RED (left: "/hub" vs right: デフォルト) → GREEN。PBI 19 の受け入れ基準を満たす。
- EnvGuard テストスイート: generic.rs に 5 つの新リゾルバテスト (root / /etc / 正常値 / 空サブパス / 未設定)、huggingface.rs に root 受け入れ + 正常値結合テスト、既存の per-cleaner テストは resolve_env_dir + cache_dir() を通す形に統合。意図は維持。
- PBI 19: 検証・チェックボックス更新・アーカイブで閉じた (commit 3853340)。
- 検証: cargo test --lib 637 passed / 4 failed (4 件は既知の TCC 環境依存)、統合 36 ターゲット green、clippy 警告 0、fmt クリーン。
