# PBI: uv / hint / apfs の非ゼロ終了分岐のテスト追加

## ユーザーストーリー
リポジトリの保守担当者として、PBI 53 で追加した終了コード検証の全分岐がテストで固定されている状態がほしい。なぜなら、uv/hint/apfs の非ゼロ終了分岐にテストが無く、fire-and-forget 呼び出しへの回帰がテストスイートを通ってしまうから。

## 優先度
- 順位: 65 / 全 9 候補
- RICE スコア: 3.2（Reach=1 / Impact=1 / Confidence=80% / Effort=0.25 人週）
- 根拠: PBI 53 で終了コード検証を追加したサイトのうち、uv.rs:129-138（prune 失敗は log::warn! のみ）、hint/cleaner.rs:22-31（osascript quit 失敗の bail）、hint/cleaner.rs:75-83（relaunch 失敗で [OK] を出さない分岐）、apfs_snapshot.rs:434-441（thinlocalsnapshots 非ゼロ分岐）にテストが無い。brew/rustup/mise/ollama/cargo は covered（2026-10-29 の coverage コミット ba759b8）

## ビジネス価値
- 終了コード検証の回帰（fire-and-forget への戻り）がテストで弾かれる
- PBI 53 の「各サイトに失敗経路のテストがある」受入基準が全サイトで真になる

## BDD 受け入れシナリオ
Scenario: uv の prune 失敗が検知される
  Given fake runner が `uv cache prune` に非ゼロ終了を返す
  When  uv の clean を実行する
  Then  失敗が記録され、freed に含まれない

Scenario: hint の osascript 失敗で bail する
  Given fake runner が `osascript` に非ゼロ終了を返す
  When  hint の clean を実行する
  Then  bail し、後続の削除に進まない

Scenario: apfs の tmutil 失敗が検知される
  Given fake runner が `tmutil thinlocalsnapshots` に非ゼロ終了を返す
  When  apfs の clean を実行する
  Then  失敗が記録され、freed に含まれない

## 受け入れ基準
- [x] uv.rs の prune 非ゼロ終了分岐にテストが 1 本追加されている
- [x] hint/cleaner.rs の osascript quit/relaunch 分岐にテストが 2 本追加されている
- [x] apfs_snapshot.rs の thinlocalsnapshots 非ゼロ分岐にテストが 1 本追加されている
- [x] 全テストが緑

## テスト戦略
- 単体: MockRunner（with_exit_code）で非ゼロ終了を注入する 4 本
- 検索: 各分岐の行を通るテストが存在することを cargo test --lib <name> で確認

## 実装アプローチ
- 実装順: (1) uv、(2) hint（quit/relaunch）、(3) apfs の順で MockRunner 注入テストを追加。既存の rustup/mise/ollama/brew/cargo のパターン（with_exit_code/with_not_found）を踏襲

## 見積もり
0.25 人週

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
