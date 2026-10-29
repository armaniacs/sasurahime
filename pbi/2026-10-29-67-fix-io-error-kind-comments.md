# PBI: io エラー kind コメントの修正（EBUSY の Rust バージョン依存と EEXIST の理由）

## ユーザーストーリー
リポジトリの保守担当者として、`classify_io_error` の kind 写像のコメントが実際の macOS errno 対応とツールチェーン依存を正確に説明する状態がほしい。なぜなら、現在のコメントは「EBUSY (WouldBlock on macOS)」というツールチェーン依存の主張と、EEXIST が busy 扱いされる理由の未記載があり、誤ったメンタルモデルを固定するから。

## 優先度
- 順位: 67 / 全 9 候補
- RICE スコア: 5.0（Reach=1 / Impact=0.25 / Confidence=100% / Effort=0.05 人週）
- 根拠: テストコメント「EBUSY (WouldBlock on macOS)」（src/cleaner.rs:543 付近）は Rust バージョン依存の主張 : 新しいツールチェーンでは EBUSY は `ResourceBusy` に対応し、古いものでは `WouldBlock`。コードは両方の kind を照合するので挙動は安全だが、コメントが片方だけを固定している。また `AlreadyExists`（EEXIST）が busy 扱いされる理由（削除パスで EEXIST が期待される理由）が未記載

## ビジネス価値
- 保守担当者が誤ったメンタルモデル（EBUSY = WouldBlock 固定）で新しい ErrorKind を扱う事故が消える
- kind 写像の理由がコードに残る

## BDD 受け入れシナリオ
Scenario: コメントが errno 対応を正確に説明する
  Given `classify_io_error` の doc コメント
  When  読む
  Then  EBUSY がツールチェーンによって WouldBlock / ResourceBusy のどちらにも対応することが書かれている

Scenario: EEXIST の扱いに理由が書かれている
  Given `classify_io_error` の doc コメント
  When  読む
  Then  EEXIST を busy/skippable 扱いする理由が書かれている

## 受け入れ基準
- [x] `classify_io_error` の doc コメントが EBUSY のツールチェーン依存を説明する
- [x] `AlreadyExists` を busy 扱いする理由が doc に書かれている
- [x] テストコメントの「EBUSY (WouldBlock on macOS)」が修正されている
- [x] 全テストが緑（挙動変更なし）

## テスト戦略
- 検索: `grep -n "WouldBlock on macOS" src/cleaner.rs` が 0 件
- 単体: 挙動変更なし（既存の classify_io_error_maps_the_kinds が緑のまま）

## 実装アプローチ
- 実装順: (1) doc コメントに errno 対応の表（EAGAIN→WouldBlock、EBUSY→ResourceBusy/WouldBlock、EEXIST→AlreadyExists）を書く、(2) テストコメントの修正。挙動変更なし

## 見積もり
0.05 人週

## Definition of Done
- [x] コメント修正済み
- [x] コードレビュー完了
