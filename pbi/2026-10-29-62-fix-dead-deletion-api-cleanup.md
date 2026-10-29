# PBI: 死んだ削除 API の整理（未呼び出し pub `delete_path` の削除 + `is_skippable_error` の統合）

## ユーザーストーリー
リポジトリの保守担当者として、削除シンクの入口が 1 つ（`safe_delete`）に整理され、分類の写像も 1 か所だけである状態がほしい。なぜなら、tripwire をバイパスする pub 削除シンク（`delete_path` 自由関数）と、kind 一覧を `classify_io_error` と重複保持する孤立関数（`is_skippable_error`）が残っていると、未来の呼び出し元が安全でない入口を誤って選び、分類の写像も静かに分岐するから。

## 優先度
- 順位: 62 / 全 9 候補（5+6 を統合）
- RICE スコア: 15.0（Reach=1 / Impact=1.5 / Confidence=100% / Effort=0.1 人週）
- 根拠: (1) 互換 free fn `delete_path`（src/trash.rs:123-133）は src/tests のどこからも呼ばれておらず（v0.4.2 でも未呼び出し）、safe_delete のスコープ tripwire と chflags 前段をバイパスする pub 削除入口。「1 つの共有削除ループ」のリリースで削除入口が 3 つ（`safe_delete`、`DeleteCtx::delete_path`、自由関数）残っている。(2) `is_skippable_error`（src/cleaner.rs:230-248）は型付きシームの置き換えで本番呼び出しが 0 になり、io kind 一覧を `classify_io_error`（cleaner.rs:134-140）と重複保持 : 2 つの真実源が静かに分岐しうる

## ビジネス価値
- 未来の呼び出し元が tripwire をバイパスする pub 入口を誤って選ぶバグのクラスが消える
- 「どの io エラーが skippable か」の真実源が 1 つになる

## BDD 受け入れシナリオ
Scenario: 削除入口が 1 つになる
  Given 自由関数 `delete_path` が削除されている
  When  削除シンクを探す
  Then  `safe_delete`（とその内部の `DeleteCtx::delete_path`）だけが残る

Scenario: 分類の写像が 1 つになる
  Given `is_skippable_error` が削除または `DeleteError::is_skippable` への委譲になっている
  When  新しい ErrorKind を分類に足す
  Then  書き換える場所は `classify_io_error` 1 か所だけですむ

## 受け入れ基準
- [x] 互換 free fn `delete_path` が src/trash.rs から削除されている
- [x] `is_skippable_error` が削除されているか、io kind 一覧を `classify_io_error` と重複しない（委譲）
- [x] 削除に伴い lib.rs の pub 再輸出が更新されている
- [x] 全テストが緑（挙動維持）

## テスト戦略
- 単体: 既存の分類テスト（is_skippable 系）が `DeleteError::is_skippable` / `classify_io_error` を指すように更新
- 検索: `grep -rn "pub fn delete_path\|fn is_skippable_error" src/` が重複を示さない

## 実装アプローチ
- 実装順: (1) `delete_path` 自由関数を削除（呼び出し 0 を確認済み）、(2) `is_skippable_error` を削除し、テストを `DeleteError::is_skippable` + `classify_io_error` に差し替え、(3) lib.rs 再輸出の更新
- 判断の根拠: `is_skippable_error` を anyhow downcast 版として残す選択肢は採らない。本番呼び出し 0 で重複保持は 2 つの真実源を作るだけ

## 見積もり
0.1 人週

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
- [x] CHANGELOG の [Unreleased] に追記
- [x] コードレビュー完了
