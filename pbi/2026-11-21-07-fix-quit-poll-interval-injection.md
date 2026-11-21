出所: テスト手法レビュー(2026-11-21、アンカー main @ 0481291)の実測に基づく。フェーズ 0 の既実装確認済み(2026-11-21 時点でポール間隔の注入機構は未実装、定数固定)。

# PBI 2026-11-21-07: quit ポール間隔を注入可能にしてテストの実スリープを除去する

## ユーザーストーリー

開発者として、quit 待ちロジックの単体テストが実時間のスリープに依存しない挙動がほしい、なぜなら 10 秒の固定コストが反復ループのフィードバックを遅くし、テストスイートの構造的な下限を作っているから。

## ビジネス価値

- **価値**: テスト実行時間。915 テストの lib スイート(実測 13.8〜14.2 秒)が、quit タイムアウトの 1 テストのためだけに 10 秒未満にできない構造を解消する
- **測定方法**: 変更前後で `cargo test --lib` の wall 時間を記録して比較する。目標: 全 quit フロー系テストが各 1 秒未満で完了し、lib スイートが概ね半減(実測値を PR 説明に貼る)

## 実測の事実(2026-11-21 レビュー)

- `src/hint/cleaner.rs:13` の `QUIT_POLL_INTERVAL = 1s` は定数固定。ポールループ(src/hint/cleaner.rs:39-48)は sleep が先、pgrep が後
- `auto_clean_skips_if_quit_times_out`(src/hint/mod.rs:422、`RecordingRunner::new(&["Code"], usize::MAX)`)は 10 反復 = **実スリープ 10 秒**。lib スイートの wall 下限(~10 秒)をこの 1 テストが支配する
- ほかの quit フロー系テスト(gone_after=1 の 6 本)は各 2 秒、gone_after=0 は 1 秒。quit 系合計でおよそ 24 秒の実スリープが 8 ワーカーに散らばっている
- ポール間隔の注入機構は未実装。環境変数オーバーライドの前例は `SASURAHIME_PLUGIN_TIMEOUT_SECS`(src/cleaners/plugin.rs:17)に存在するが、**環境変数はプロセスグローバルであり、並列テストに PBI 06 と同型のレースを持ち込む**

## BDD 受け入れシナリオ

```gherkin
Scenario: アプリが quit しないとき、規定回数のポール後に諦める(ハッピーパス)
  Given テスト用 runner がアプリを「終了しない」と報告する設定になっている
  When  auto clean が quit 待ちを実行する
  Then  規定回数のポール後にエラーで諦め、削除コマンドを一切呼ばない

Scenario: テストは実時間を待たない(境界ケース)
  Given テストがポール間隔として 1ms を注入している
  When  quit タイムアウトを主張するテストが走る
  Then  テストは 1 秒未満で完了する
```

## 受け入れ基準

- [ ] `quit_app_and_wait` が `poll_interval: Duration` を引数として受け取る(本 PBI の設計判断: 環境変数ではなくパラメータ注入。根拠は下記「設計判断」)
- [ ] `auto_clean_hint` も `poll_interval` を受け取り、`quit_app_and_wait` へ素通しする
- [ ] 本番呼び出し元(src/hint/cleaner.rs:425 の KnownAppHintsCleaner)は `QUIT_POLL_INTERVAL`(=1s)を明示的に渡し、**本番の待ち時間は 1 ミリ秒も変わらない**
- [ ] quit フロー系テスト 8 本(`RecordingRunner::new` を使うもの全て)はテスト用定数(例: `Duration::from_millis(1)`)を注入する
- [ ] `auto_clean_skips_if_quit_times_out` の主張内容(エラー返却、`rm` 非呼び出し)は無変更で green
- [ ] bail メッセージの秒数は「反復数 × 注入間隔」から導出する(本番は従来どおり "10s"、テストは実測値になりメッセージ内容の pin は存在しないことを確認済み)
- [ ] `cargo clippy --tests -- -D warnings` と `cargo fmt --check` がクリーン

## テスト戦略(t_wada スタイル)

### E2Eテスト
- なし(本番の待ち時間は不変のため E2E の変化はない。既存の hint 系 E2E が無修正で green することを確認)

### 統合テスト
- `auto_clean_skips_if_quit_times_out`(1ms 注入): タイムアウト経路で `rm` 非呼び出しを主張(既存テストの間隔のみ変更)
- `auto_clean_deletes_paths_after_quit`(1ms 注入): quit → ledger 削除 → relaunch の統合が壊れていないこと

### 単体テスト
- `quit_app_and_wait_errors_when_entry_has_no_quit_app`: quit_app なしで即 bail(ポールに到達しない)
- `quit_app_and_wait_escapes_quotes_in_quit_app`: 1ms 注入に変更(現行 1 秒 → 数ミリ秒)
- `quit_app_and_wait_bails_when_osascript_exits_nonzero`: osascript 失敗でポール前に bail(無変更)
- 境界: 注入間隔 0 でも反復数は規定どおり(10 回)であること

## 実装アプローチ
- **Outside-In**: 統合テスト(タイムアウト 1ms 化)を先に red → シグネチャ変更で green
- **Red-Green-Refactor**: green 後、残る quit テストを順に 1ms 化して実測を取る

## 見積もり
2 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: なし(PBI 06 / 08 とファイル排他。hint/cleaner.rs と hint/mod.rs を触るのは本 PBI のみ)
- テスタビリティ: パラメータ注入により並列テストでのグローバル状態を増やさない
- 非機能要件: 本番のポール挙動(1 秒間隔 × 10 回)は 1 ミリ秒も変更しない

## 実装者向け注記

### 現状コードの確認
(2026-11-21 時点で実施済み。着手時に再確認すること)
```bash
rg -n "QUIT_POLL_INTERVAL|QUIT_TIMEOUT_SECS" src/hint/
rg -n "auto_clean_hint\(" src/
```
注入機構は未実装。`auto_clean_hint` の本番呼び出し元は KnownAppHintsCleaner(src/hint/cleaner.rs:425)のみ。hint モジュールへの include_str ソースピンは存在しない(確認済み)。

### 設計判断(実装前に読むこと)
- **パラメータ注入を採用。環境変数は採用しない**: `SASURAHIME_PLUGIN_TIMEOUT_SECS` の前例があるが、環境変数はプロセスグローバルで並列テストがレースする。リポジトリの既存方針(DeleteCtx の「mode を引数で運ぶ、no global」、PBI 102 の port 注入)と一致するのはパラメータ側
- ポールループの形状は維持する(固定反復数 × 注入間隔)。「時間予算で打ち切る」形への書き換えは本番の意味論変更になるため本 PBI では行わない

### 実装手順
1. `quit_app_and_wait(entry, runner)` → `quit_app_and_wait(entry, runner, poll_interval: Duration)` に変更
2. `auto_clean_hint` に `poll_interval` を追加し、本番呼び出し元は `QUIT_POLL_INTERVAL` を渡す
3. 統合テストを先に 1ms 化して red → green を確認し、残る quit 系テストを順に 1ms 化
4. bail メッセージの秒数を `attempts × interval` から導出する
5. `cargo test --lib hint` → `cargo test --lib` 全体の実測を PR 説明に貼る

### 落とし穴
- ポールループは **sleep が先、pgrep が後**(src/hint/cleaner.rs:39-44)。間隔 0 を入れると 10 反復でも即終了するが、「1 回は待ってから確認する」現行意味論は sleep 先頭のまま維持する
-RecordingRunner の `gone_after_pgrep_calls` は pgrep 呼び出し回数を数える。間隔を変えても呼び出し回数の契約は変わらない(`auto_clean_deletes_paths_after_quit` の chflags 3 回主張などは無修正で green するはず)
- hint 系 E2E(tests/hints.rs)は実プロセスを spawn するため本 PBI の影響を受けない。触らない

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] `cargo test --lib` の wall 時間が変更前後で記録され、quit 系テストがすべて 1 秒未満
- [ ] コードレビュー完了(GitHub PR での approve を必須)
- [ ] リファクタリング完了(green 後)
- [ ] ロールバック手段: 本番待ち時間が不変であることを実測で確認済みのため、revert で安全に戻る
- [ ] ドキュメント更新: 不要(本番挙動不変)
