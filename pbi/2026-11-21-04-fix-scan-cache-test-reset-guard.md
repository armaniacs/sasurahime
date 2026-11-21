出所: pbi-backlog-prioritizer（2026-11-21）で起票後、pbi-create-bdd-workspace（ryuzee × BDD × t_wada 統合）テンプレートへ書き直し（2026-11-21）。実測アンカーは main @ d206d0c。

# PBI 2026-11-21-04: scan-cache テストのグローバルリセットを決定的にする

## ユーザーストーリー

保守担当者として、scan-cache のテスト用グローバルが決定的にリセットされる挙動がほしい、なぜなら在庫量を主張するテスト（prune pin）が実行順序に依存して偶発 green / red になるフレークを防ぐから。

## ビジネス価値

- **価値**: テストスイートの信頼性。在庫を主張する pin が 1 本増えるたびにフレーク面積が増える構造を、1 つのリセットヘルパーで根絶する
- **測定方法**: `cargo test --lib scan_cache` を連続 10 回実行して全 green（順序・並列に非依存）、かつリセットヘルパーの単体 pin が決定的に通る

## 優先度

- 順位: 1 位 / 全 7 候補（pbi-backlog-prioritizer 2026-11-21）
- RICE スコア: 4.0（Reach=2 / Impact=1 / Confidence=1.0 / Effort=0.5 SP）
- 根拠: 実装コスト最小で在庫主張 pin の偶発汚染という実害を排除。round 10 台帳送りの「触る機会」トリガーを着手判断に昇格。依存なし（PBI 05 とファイル排他で並列可）

## BDD 受け入れシナリオ

```gherkin
Scenario: 同一プロセスでの 2 回目の構成（境界ケース）
  Given 前のテストがキャッシュに 1 件の在庫を入れた
  When 次のテストがリセットヘルパーで初期状態を構成する
  Then 在庫は 0 件になり、前のテストの内容が引き継がれない

Scenario: 並列実行での決定性（エラー/境界ケース）
  Given グローバルを触るテストが複数存在する
  When cargo test が並列で実行される
  Then ガードを取得したテストの間だけ状態が保証され、在庫主張 pin は偶発 green/red しない
```

## 受け入れ基準

- [ ] src/scan_cache.rs の tests モジュールにリセットヘルパーを新設する（設計は**lock なし swap** に確定: `SCAN_CACHE.get_or_init(|| Mutex::new(None))` で必ず初期化してから Mutex 内を `*guard = None` で swap。`TEST_LOCK` の流用はしない — std Mutex は非再入のため、scan_cache テスト内から TEST_LOCK を取るコード（config.rs:533-536 系）を呼ぶと deadlock の溝が残る。lock を導入しないことで残る並列競合の境界は下記トリガーで管理）
- [ ] **境界トリガーの記録**: 「グローバル SCAN_CACHE を触るテストが 2 本目になる」まで lock なし swap は決定的。2 本目のテストを書くときに test lock 導入を再判断する（本 PBI はトリガーを doc コメントで残す）
- [ ] src/scan_cache.rs:339 の `let _ = SCAN_CACHE.set(Mutex::new(None));` をリセットヘルパー経由に置き換える
- [ ] 本番コード（`SCAN_CACHE` / `init` / `flush` / `prune` / `cached_dir_size` / `record`）は **1 行も変更しない**（#[cfg(test)] 配下のみ）
- [ ] 既存テストは無修正で green（prune pin 2 本を含む）
- [ ] `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- なし（本番挙動は変更しないため、E2E の追加は過剰）

### 統合テスト
- なし（グローバルは scan_cache.rs 内完結）

### 単体テスト
- リセットヘルパーの決定性 pin 1 本（2 回連続リセット → 毎回空在庫。壁時計・実行順序に非依存）
- 既存 prune pin 2 本が並列実行でも偶発しないことの確認（フルスイート green）

## 実装アプローチ

- **Outside-In**: 決定性 pin（失敗はしない — 現行でも green のため、pin は「ヘルパーの契約を固定する」役割）→ ヘルパー実装 → 既存テストの置換 → green
- **Red-Green-Refactor**: ヘルパーの pin を先に書き（この pin は実装前は存在しないため compile RED になる）→ 実装 → green → 既存 `let _ = set` の置換

## 見積もり

0.5 SP（要チームでの見積もり）

## 技術的考慮事項

- 依存関係: なし（PBI 05 と同時実装する場合はファイル排他成立・並列可）
- テスタビリティ: OnceLock<Mutex<Option<ScanCache>>> の内側を swap する形なら、OnceLock 自体の set-once 制約に触れずに済む
- 非機能要件: なし（テスト基盤のみ。セキュリティ・性能への影響なし）
- **残存リスク（深掘り裁定）**: lock なしのため、グローバルを触るテスト同士の操作列（record → assert）とリセットのインターリーブは理論上可能。グローバルを触るテストが 1 本の間は影響なし。2 本目で test lock（scan_cache 専用）の導入を再判断する

## 実装者向け注記

### 現状コードの確認
（着手前に必ず実行すること — 2026-11-21 に実施済み・未実装を確認）
```bash
rg -n 'with_test_lock|TEST_LOCK|test_reset|reset_cache' src/
rg -n 'SCAN_CACHE.set' src/scan_cache.rs
sasurahime targets
```
- **既実装確認の結果**: `test_reset` / `reset_cache` は 0 件 → **未実装**。先例 `with_test_lock` は src/context.rs:6,24-32（config.rs:533-536 でも使用）。`SCAN_CACHE.set` の握りつぶしは src/scan_cache.rs:339 のみ。コマンド面は 51 ターゲットで、本 PBI がコマンドを追加しないことも確認済み

### 実装手順
1. src/scan_cache.rs の tests モジュールに `#[cfg(test)] static TEST_LOCK: std::sync::Mutex<()>`（context.rs:6 と同形）を置く
2. `fn with_scan_cache_reset(f: impl FnOnce() -> T) -> T` を書く: TEST_LOCK を取得 → `SCAN_CACHE.get_or_init(|| Mutex::new(None))` で必ず初期化（既に初期化済みでも get_or_init は既存の値を返す）→ `*guard = None` で swap → f()
3. :339 の `let _ = SCAN_CACHE.set(Mutex::new(None));` を `with_scan_cache_reset(|| { ... })` 形に置き換える
4. 決定性 pin: ヘルパー内で在庫 1 件入れる → リセット → 空 assert → もう 1 回リセット → 空 assert

### 落とし穴
- `SCAN_CACHE.set(...)` を try で書き直すだけ（`if SCAN_CACHE.set(...).is_ok()`）だと**3 回目以降は再び沈黙する** — 必ず「Mutex 内の swap」で決定化する
- OnceLock の get_or_init は既存値があるときクロージャを呼ばない（既存値を返す）— set-once 制約を崩さずに初期化を強制できるのはこの形
- リセットヘルパーは `#[cfg(test)]` 配下に置く（本番ビルドに dead_code 警告を出させない）

## Definition of Done

- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] cargo test green（既知 TCC 依存 4 失敗を除く許容）+ clippy 0 + fmt clean
- [ ] コードレビュー完了（統合側によるバッチ境界検証。PR 運用時は approve 必須）
- [ ] ロールバック手段: 本 PBI は 1 コミットのテスト基盤変更のみで、`git revert` で完全に切り戻せる（本番挙動への影響ゼロ）
- [ ] ドキュメント更新済み（不要 — 本番 doc / README に触れないため。台帳の経過記録のみ統合側が更新）

## scope 外（やらないこと）

- 本番の SCAN_CACHE / OnceLock 構造の変更
- history.rs 等の他グローバルへの横展開（同型脆弱点が見つかった場合のみ台帳経由で判断）
- prune pin の追加変更（round 9 で着地済み）
