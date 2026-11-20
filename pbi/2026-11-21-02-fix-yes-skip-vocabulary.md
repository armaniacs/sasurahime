出所: holistic-code-improvement round 10（RICE 2 位）。round 9 台帳送りの再検討トリガー発火（PBI 03 着地により共有レンダラの `Some(advice)` 経路が実在）を実装する PBI。実測アンカーは main @ 892ae1d。

# PBI 2026-11-21-02: --yes の skip 行に語彙を付与する（PBI 74 の語彙統一を完成）

- 種別: fix（意図的な出力変更 — byte-identical 規律の「pin してから意図的に変える」を適用）
- 規模: S
- 優先度: round 10 中 2 位（RICE 2.0 = Reach 4 × Impact 0.5 × Confidence 1.0 / Effort 1 SP）
- 依存: 2026-11-21-01（同一ファイル src/interactive.rs を直列に触るため後）

## 背景（実測済みの事実。推測で書き足さない）

- `interactive::run_auto` の skip 描画は `SkipAdvice` を全 `None` で構築し、汎用 1 行 `"{name} ({risk})"` を出す（src/interactive.rs:352-360、PBI 03 で byte 等価維持のために残した状態）
- Yes policy の skip は `NeedsExplicitSelection` のみ（src/unattended.rs:375-386 の matrix 実測。テスト `yes_skips_non_auto_clean_targets_with_an_explicit_selection_reason` も pin）
- 共有レンダラの advised 形 `" {name} ({risk}): {advice}"` は PBI 03 で実装済みで、--yes 経路だけが未使用（round 9 台帳送りの「共有レンダラの足場」）
- 現行の --yes skip 出力 pin は contains 形で耐性がある: `stdout.contains("Skipped") && stdout.contains("cargo (Moderate)")`（tests/scan.rs:474-477）— 語彙追記でも green を維持するが、**新語彙を主張する pin は実在しない**
- auto は `"set [policy] include_manual = true to include"`、clean all は `"needs --include-manual"` と語彙を持ち（src/main.rs:1292-1298、:1472-1483）、--yes だけが「なぜ動かなかったか」を語らない（PBI 74 の「per-flow advice」趣旨の未完部分）
- 先例: round 9 なぜなぜ（.plan/whywhy-round9-entry-dispatch.md §2.a 7-13）が「--yes の語彙沈黙は新変数の温床」と裁定し、足場だけ先に着地させた

## ユーザーストーリー

As a --yes ユーザー, I want the skipped targets to state why they did not run, so that the unattended output tells the same story as `auto` and `clean all`.

## BDD シナリオ

1. **語彙の付与**: Given Moderate target（cargo）に cache が存在する, When `sasurahime --permanent --yes` を実行する, Then stdout の skip 行が `Skipped cargo (Moderate): needs an explicit selection` 形になり、既存 pin の contains 主張（"cargo (Moderate)"）も維持される
2. **他経路への非波及**: Given auto / clean all を実行する, Then 両者の skip 語彙は現行と byte 等価（本 PBI は --yes 経路のみを変える）

## 受け入れ基準

- [ ] `interactive::run_auto` の `SkipAdvice` を `needs_explicit_selection: Some("needs an explicit selection")` に変更する（`never_unattended` / `needs_opt_in` は Yes policy が生成しないため `None` のまま、doc コメントでその理由を 1〜2 行記す）
- [ ] 変更**前に**新語彙を主張する E2E pin を追加する（`stdout.contains("cargo (Moderate): needs an explicit selection")` 相当）。pin は現行実装では RED、変更後に GREEN になる順序で Red→Green を記録する
- [ ] 既存 pin（tests/scan.rs:474-477、:505 付近、tests/ollama.rs の gate_skipped 系）は無修正で green（gate_skipped は SkipReason ではなく TTY 観測の別系統、unattended.rs:38-44 の記録どおり）
- [ ] auto / clean all の skip 出力は byte 等価（tests/auto_policy.rs、tests/clean_all.rs 無修正で green）
- [ ] unattended.rs には触れない（rendering は unattended.rs に入れない PBI 74 決定。96 セル matrix pin 無傷）
- [ ] `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean
- [ ] 触れないこと: src/batch.rs（レンダラ本体）、src/unattended.rs、src/main.rs

## テスト戦略

- Red→Green 先行: 新語彙 pin を現行実装で RED 確認 → `SkipAdvice` を Some 化して GREEN（parity pin 規律の意図的変更版）
- 意図的な出力変更なので、変更内容と理由を CHANGELOG（Unreleased 節の Changed に 1 行: --yes の skip 行が理由を語るようになった旨）に記録する
- 既存 E2E 無修正 green が「他経路への非波及」の証拠

## 見積もり

1 SP（SkipAdvice 1 箇所 + E2E pin 1 本 + CHANGELOG 1 行）

## DoD

- [ ] cargo test が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] cargo build が green
- [ ] 新語彙 pin の Red→Green 実測を本ファイルに記録
- [ ] PBI アーカイブ（統合側が実施: git mv → `.plan/archived/`、INDEX 追記）

## scope 外（やらないこと）

- auto / clean all の advice 語彙変更（byte 等価の対象外）
- gate_skipped 行の変更（TTY 観測の別系統、PBI 74 決定）
- NeedsOptIn / NeverUnattended に対する --yes 固有の advice 語彙（Yes policy が生成しないため None 維持。将来 policy が変わるならその時の判断）
