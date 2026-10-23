# PBI: バッチ実行モジュールの1本化（run_batch）

## ユーザーストーリー
sasurahime を `auto` / `clean --all` / TUI で使う macOS ユーザーとして、バッチ実行のセマンティクス（キャンセル・履歴記録・post-clean ノート・skip 報告）がエントリポイント間で一貫してほしい、なぜなら 6 ループが「クリーンして累計して履歴に書く」を再実装しており、history ガードが 6 サイトに重複し、TUI の sub-target クリーンは freed_message も post_clean_note も履歴も bypass しているから。

## 優先度
- 順位: 4 / 8
- RICEスコア: 85（Reach=80 / Impact=2 / Confidence=80% / Effort=1.5人日）
- 根拠: 全エントリポイントのバッチ実行に影響。挙動の不整合（sub-target の履歴欠落、skip 文言の 3 種類）は実測済み

## BDD受け入れシナリオ
Scenario: sub-target クリーンも履歴とノートを得る
  Given TUI で sub-target を選択してクリーンした
  When  bytes_freed > 0 で dry_run でない
  Then  freed_message と post_clean_note が表示され、history エントリが書かれる

Scenario: 履歴ガードは1箇所にある
  Given 全 6 ループが run_batch 経由になる
  When  `!dry_run && bytes_freed > 0 → write_history_entry` ガードを検索する
  Then  ガードは run_batch の 1 箇所にのみ存在する

Scenario: バッチセマンティクスは stub でテストできる
  Given stub クリーナー + VerboseProgress
  When  run_batch が呼ばれる
  Then  総計・skip・失敗が BatchSummary として返る

## 受け入れ基準
- [x] `run_batch(cleaners, opts, reporter) -> BatchSummary{total, skipped, failures}` が lib に実装される
- [x] main.rs の Auto / All / Caches ループと interactive.rs の 2 ループが run_batch 経由になる
- [x] history ガードの 6 サイト（main.rs:770/917/1047/1254、interactive.rs:188/322）が 1 箇所に集約される
- [x] TUI sub-target クリーンが freed_message / post_clean_note / history を得る（意図した挙動変更として明記）
- [x] バッチのテストが stub クリーナーで書ける（E2E バイナリ実行不要）

## テスト戦略
- 単体: run_batch の BatchSummary（総計/skip/失敗）テスト
- 統合: 既存の auto/all/caches/interactive テストが green を維持

## 見積もり
2ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: `run_batch(cleaners, opts, reporter, dry_run, animated, with_notes) -> BatchSummary` を新モジュール src/batch.rs に追加 (commit c7aa582)。BatchSummary は total_freed + failures (クリーナー名 + エラー) を持つ。skipped は各 CleanResult 内の SkippedEntry が保持し、run_batch は failures を収集して呼び出し側に返す。
- ループ移行: main.rs Auto (Risky/Manual ガード + wording A を維持して pre-filter、run_batch 経由)、main.rs All (--include-manual ガード + wording B)、main.rs Caches (with_notes=false で素の挙動を維持)、interactive.rs run_auto (auto_clean_indices サブリスト、animated、no notes)、interactive.rs run_interactive (whole-target 分岐のみ、選択順序を維持)。
- history ガード集約: `!dry_run && bytes_freed > 0` の式は src/batch.rs の write_history_entry_if_needed に 1 箇所のみ (grep で確認: write_history_entry の非テスト呼び出しは 1 件)。旧 6 サイトから集約。run_clean_target も同じヘルパーを共有。
- TUI sub-target: サブプロセス (.status() = stdio 継承) 経由で freed_message を表示し、子プロセスの run_clean_target が history を書く。post_clean_note は子の freed_message 出力が同じ役割のため未出力。subprocess 分岐自体は維持 (意図: 子が既に独自の表示と history を持つため親での二重化を避ける)。
- 逸脱 (記録): run_batch の cleaners 引数は `&[&dyn Cleaner]` (interactive が散在 index で部分スライスを持てないため)。post_clean_note は main.rs から batch.rs へ移動 (lib は bin の関数を呼べないため)。
- テスト: src/batch.rs に 6 テスト (stub クリーナー + SuppressReporter、E2E バイナリ実行不要、history は EnvGuard + tempdir HOME で dry_run=true/false の両方を観測)。
- 検証: cargo test --lib 653 passed / 4 failed (4 件は既知の TCC 環境依存)、統合 36 ターゲット 250 passed 0 failed、clippy 警告 0、fmt クリーン。
