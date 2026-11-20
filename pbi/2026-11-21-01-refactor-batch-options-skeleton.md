出所: holistic-code-improvement round 10（pbi/2026-11-21-00-backlog-holistic-round10.md の RICE 1 位）+ code-review round 10 指摘。実測アンカーは main @ 892ae1d。

# PBI 2026-11-21-01: BatchOptions の部分適用（無人 3 経路の named 引数化）+ bounds debug_assert

- 種別: refactor（byte 等価）
- 規模: S
- 優先度: round 10 中 1 位（RICE 4.0 = Reach 4 × Impact 1 × Confidence 1.0 / Effort 1 SP）
- 依存: なし。後続 PBI 2026-11-21-02 が本 PBI の interactive.rs 変更後に直列実行される（同一ファイル）

## スコープ裁定（ユーザー確認済み）

`run_batch` 本体（src/batch.rs:91）の引数リスト変更は round 7 の記録契約（src/batch.rs:82-90「a `run_batch` argument-list change takes the form enum plus stage 3 in the same PR」）に抵触するため、**本 PBI では run_batch 本体に触れない**。BatchOptions を適用するのは round 9 の新シーム `run_selected_batch`（src/batch.rs:189）とその呼び出し面のみ。全面移行は台帳送り（stage 3 統合 PR と同時）。

## 背景（実測済みの事実。推測で書き足さない）

- `run_batch` の署名は位置引数 bool 3 つ: `(cleaners, opts, reporter, dry_run, animated, with_notes)`（src/batch.rs:91-98）
- `run_selected_batch` も同型の位置引数 bool 3 つを受け、内部で `run_batch` にそのまま流す（src/batch.rs:189-206）
- 本 PBI の対象呼び出し面（無人 3 経路）: `run_auto_flow`（src/main.rs:1304、`dry_run, false /* animated */, true /* with_notes */`）、`run_all_flow`（src/main.rs:1492、同型）、`interactive::run_auto`（src/interactive.rs:414、`false /* dry_run */, true, false`）
- 対象外（run_batch 直呼び・PBI 03 で scope-out 済みの 2 箇所）: `run_caches_batch`（src/main.rs:1018）、`run_interactive` の whole-target 分岐（src/interactive.rs:520）
- 誤配線のリスク: `animated` と `with_notes` の入れ替えは spinner の見た目にしか現れず、E2E（非 TTY で spinner が出ない）では観測不能 — round 9 なぜなぜ（.plan/whywhy-round9-entry-dispatch.md §2.a 15-17）で「位置 bool が攻撃面」として特定済み
- `run_selected_batch` は `selection.run` のインデックスで `cleaners[i]` を索引する（src/batch.rs:198-202）: 範囲外インデックスで panic する契約は「selection は同一スライスの select 産物」という doc のみで、debug ビルドでの検査が実在しない（code-review round 10 指摘）

## ユーザーストーリー

As a 保守担当者, I want the unattended batch call sites to pass named options instead of positional bools, so that an animated/with_notes swap becomes a compile error instead of a silent spinner regression.

## BDD シナリオ

1. **byte 等価**: Given 現行の auto / clean all / --yes, When 各フローを実行する, Then 出力・終了コード・履歴記録が現行と byte 等価（既存 E2E pin 無修正で green）
2. **named 引数の強制**: When `run_selected_batch` を呼び出すコードが `BatchOptions` のフィールド名を省略できない形で書き直される, Then `animated` と `with_notes` の入れ替えは build error になる（Then: 構造が記録される。source pin またはユニット pin で位置引数の残存を検査）

## 受け入れ基準

- [ ] `BatchOptions { dry_run, animated, with_notes }` を src/batch.rs に新設（derive は最小限。`Default` は全 false で実装してよい）
- [ ] `run_selected_batch` の署名を `BatchOptions` 受け取りに変更し、内部の `run_batch` 呼び出し（src/batch.rs:203）で展開する。**`run_batch` 本体の署名は 1 行も変えない**（round 7 契約の維持）
- [ ] 無人 3 経路の呼び出し（src/main.rs:1304、:1492、src/interactive.rs:414）を named フィールドに置き換える
- [ ] `run_selected_batch` に bounds `debug_assert` を追加: `selection.run` と `selection.skipped` の全インデックスが `cleaners.len()` 未満であること（debug ビルドで契約違反が発火する）
- [ ] batch.rs tests 内の `run_selected_batch` 直接呼び出し（src/batch.rs:829）を新署名に置き換える。`run_batch` の呼び出しテスト（:408, :446, :481, :514, :547, :589, :621, :692, :703）は**無修正で green**（run_batch 本体を触らないことの証明）
- [ ] 全 E2E pin 無傷（byte 等価）+ `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean
- [ ] 触れないこと: `run_batch` の署名・本体、`run_caches_batch`（src/main.rs:1018）、`run_interactive` whole-target 分岐（src/interactive.rs:520）、src/unattended.rs

## テスト戦略

- byte 等価: 既存 E2E（auto_policy.rs、clean_all.rs、scan.rs、interactive.rs）が無修正で green であることが主証拠
- 構造 pin は新設しない（named 引数化はコンパイラが守る。source pin 最後の手段規約に該当しない）
- bounds debug_assert のテスト: 通常 pin は不要（debug_assert は契約の自己文書化）。ただし `debug_assertions` 有効の cargo test で既存テストが発火しないことを全テスト green で確認

## 見積もり

1 SP（構造体 1 個 + 署名 1 本 + 呼び出し 3 箇所 + debug_assert 1 行 + batch.rs tests 1 箇所）

## DoD

- [ ] cargo test が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] cargo build が green
- [ ] run_batch 本体の無変更を実測で記録（git diff に run_batch 署名の変更が無いこと）
- [ ] PBI アーカイブ（統合側が実施: git mv → `.plan/archived/`、INDEX 追記）

## scope 外（やらないこと）

- `run_batch` 本体の引数リスト変更・form enum・stage 3 統合（round 7 契約により台帳送り: stage 3 統合 PR と同時）
- `run_caches_batch` と `run_interactive` whole-target 分岐の BatchOptions 化（PBI 03 の scope-out 引継。stage 3 統合時に判断）
- `CleanOptions` との統合検討（別概念: 実行オプション vs 表示制御）
