出所: pbi-backlog-prioritizer（2026-11-21・台帳送り+積み残しの混在採点）における round 10 台帳送りの再評価。着手順 1 位。実測アンカーは main @ d206d0c。

# PBI 2026-11-21-04: scan-cache テストのグローバルリセットを決定的にする

- 種別: fix（テスト基盤の脆弱修正・本番コードへの影響なし）
- 規模: S
- 優先度: 1 位 / 全 7 候補（RICE 4.0 = Reach 2 × Impact 1 × Confidence 1.0 / Effort 0.5 SP）
- 根拠: round 10 台帳送りの「触る機会」トリガーを今回の優先度付けで着手判断に昇格。実装コスト最小（0.5 SP）で、在庫を主張する prune pin の偶発 green/red という潜在的な実害を構造的に排除する
- 依存: なし

## 背景（実測済みの事実。推測で書き足さない）

- テストのグローバル初期化が `let _ = SCAN_CACHE.set(Mutex::new(None));`（src/scan_cache.rs:339）で、OnceLock::set は既に初期化済みなら Err を返し **`let _` で握りつぶされる** — 2 回目以降のリセットは沈黙し、前のテストが残したキャッシュ内容が引き継がれる
- 同一プロセスで init を呼ぶテストが現状 1 本（`cached_dir_size_matches_dir_size_and_caches`）のため偶発がまだ起きていないが、round 9 の prune pin（期限切れ破棄・新鮮生存）は**在庫量を主張する**ため、前テストの在庫が残っていると偶然 green / 偶然 red になりうる（round 9 なぜなぜ .plan/whywhy-round9-scan-cache-growth.md §1.b 24-31 で根本原因確定済み）
- リポジトリには正装の先例が実在する: src/context.rs:6 の `#[cfg(test)] static TEST_LOCK: std::sync::Mutex<()>` と :24-32 の `with_test_lock()` クロージャヘルパー（並列テストのグローバル競合を直列化する）
- cargo test は 1 プロセスで並列実行するため、グローバルを触るテストが増えるほど順序依存の面積が増える

## ユーザーストーリー

As a 保守担当者, I want the scan-cache global test state to reset deterministically, so that a future test that asserts cache inventory cannot flake on test ordering.

## BDD 受け入れシナリオ

Scenario: 同一プロセスでの 2 回目の構成（境界ケース）
  Given 前のテストがキャッシュに 1 件の在庫を入れた
  When 次のテストがリセットヘルパーで初期状態を構成する
  Then 在庫は 0 件になり、前のテストの内容が引き継がれない

Scenario: 並列実行での決定性（エラー/境界ケース）
  Given グローバルを触るテストが複数存在する
  When cargo test が並列で実行される
  Then ガードを取得したテストの間だけ状態が保証され、在庫主張 pin は偶発 green/red しない

## 受け入れ基準

- [ ] src/scan_cache.rs の tests モジュールにテスト専用のリセットガードを新設する（context.rs の TEST_LOCK + with_test_lock 先例を踏襲。リセットは `*guard = None` の swap で決定的に行い、`let _ = set` の沈黙を残さない）
- [ ] src/scan_cache.rs:339 の `let _ = SCAN_CACHE.set(Mutex::new(None));` をリセットヘルパー経由に置き換える
- [ ] 本番コード（SCAN_CACHE / init / flush / prune / cached_dir_size / record）は **1 行も変更しない**（#[cfg(test)] 配下のみ）
- [ ] 既存テストは無修正で green（prune pin 2 本を含む）
- [ ] `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean
- [ ] 触れないこと: src/scan_cache.rs の #[cfg(test)] 配下以外、他ファイル全て

## テスト戦略

- E2E: 不要（本番挙動は変更なし）
- 統合: 不要
- 単体: リセットヘルパーの決定性 pin 1 本（2 回連続でリセット → 毎回空在庫になることを assert。壁時計・順序に非依存）。既存 prune pin が並列実行でも偶発しないことはフルスイート green で確認

## 見積もり

0.5 SP（ヘルパー約 10 行 + 置換 1 行）

## Definition of Done

- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] cargo test green（既知 TCC 依存 4 失敗を除く許容）+ clippy 0 + fmt clean
- [ ] コードレビュー完了（統合側がバッチ境界で検証）
- [ ] 本 PBI のアーカイブ（統合側が実施）

## scope 外（やらないこと）

- 本番の SCAN_CACHE / OnceLock 構造の変更（テスト基盤のみ）
- history.rs 等の他グローバルへの横展開（同型の脆弱点が見つかった場合のみ台帳経由で判断）
- prune pin の追加変更（round 9 で着地済み）
