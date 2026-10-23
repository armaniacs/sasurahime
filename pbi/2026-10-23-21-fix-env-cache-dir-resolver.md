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
- [ ] `resolve_env_dir(var, subpath, default, label) -> PathBuf` が generic.rs に実装され、結合前のベース検証 + 警告 + フォールバックを1箇所に持つ
- [ ] 6 コピー（generic.rs:242-255/310-323/344-357、huggingface.rs:22-36、pre_commit.rs:23-33/34-44）が seam 経由に置き換わる
- [ ] `HF_HOME=/` のフォールバック単体テスト（EnvGuard + TEST_LOCK 規約）が通る ― PBI 19 の受け入れ基準を満たす
- [ ] huggingface / pre_commit / generic の EnvGuard テストスイートがリゾルバ経由の1スイートに統合される
- [ ] PBI 19（pbi/2026-10-23-19-fix-hf-home-root-path.md）が検証・チェックボックス更新・アーカイブで閉じられる

## テスト戦略
- 単体: リゾルバの `/etc`・`/tmp`・相対パス・HOME祖先ケースを1スイートで（EnvGuard + TEST_LOCK）
- 統合: `clean huggingface` が `/hub` を削除しないこと

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
