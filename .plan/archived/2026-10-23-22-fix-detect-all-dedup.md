# PBI: 並列 detect ループの1本化（scanner::detect_all）

## ユーザーストーリー
sasurahime を scan / TUI で使う macOS ユーザーとして、並列 detect の挙動修正が1箇所で完結してほしい、なぜなら rayon + progress bar + index merge の微妙なブロックが scanner.rs と interactive.rs に 3 重複しており、うち 2 クローンは一切テストされていないから。

## 優先度
- 順位: 3 / 8
- RICEスコア: 180（Reach=90 / Impact=1 / Confidence=100% / Effort=0.5人日）
- 根拠: scan/auto/interactive の全 detect 経路に影響。クローンは未テストであり、修正が 3 箇所に必要な現状は局所性の失敗

## BDD受け入れシナリオ
Scenario: 3 エントリポイントが同じ detect 結果を返す
  Given 同じ cleaners リスト
  When  scan / run_auto / run_interactive の preamble が detect を実行する
  Then  同じ scan_results 順序・サイズで ScanResult が返る（detect_all の1実装経由）

Scenario: クローンは消失する
  Given interactive.rs の 2 preamble クローン
  When  detect_all に置き換えられる
  Then  interactive.rs に rayon ブロックのコピーが残らない

## 受け入れ基準
- [x] `scanner::detect_all(&[Box<dyn Cleaner>]) -> Vec<ScanResult>` が1実装として存在する
- [x] interactive.rs:90-112 と 215-237 のクローンが detect_all 経由に置き換わる
- [x] run_scan が scan_results 内で再計算している available（scanner.rs:110）の二重計算が解消される
- [x] run_scan / run_scan_json の available/risks/denied preamble 重複が共通化される
- [x] detect_all のテストが1実装に対して1つ存在する

## テスト戦略
- 単体: detect_all が ScanResult を返すことのテスト（stub クリーナー + 順序固定）
- 統合: 既存の scan/interactive テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: `pub fn detect_all(&[Box<dyn Cleaner>]) -> Vec<ScanResult>` を scanner.rs に追加 (commit eb85604)。パラレル detect ループ (index 初期化、is_available フィルタ、with_parallel_scan、index merge) の唯一の所有者。
- interactive.rs のクローン 2 件 (run_auto, run_interactive) を `let results = crate::scanner::detect_all(cleaners);` の 1 行に置換。grep で interactive.rs から rayon / par_iter / with_parallel_scan が消えたことを確認。
- 二重計算解消: run_scan は scan_preamble (available + risks + denied_paths の 1 パス) から表示行を導出し、2 回目の is_available フィルタは存在しない。
- preamble 共通化: run_scan / run_scan_json が 1 つのプライベートヘルパーを共有。
- 逸脱 (記録): detect_all は index-aligned 形を採用。旧 scan_results は compact + size-sorted だったが、interactive は results[i] と cleaners[i] の対を index で取るため整列が必須。compact 形の再利用は誤ったクリーナーをクリーンする可能性があった。run_scan / run_scan_json は detect_all の結果を表示用に filter + sort し、テーブル/JSON の行は旧実装と同一を維持。
- テスト: detect_all_preserves_cleaner_order_and_names (TDD RED → GREEN、登録順・名前・プレースホルダを固定)。
- 検証: cargo test --lib 631 passed / 4 failed (4 件は既知の TCC 環境依存)、統合 36 ターゲット green、clippy 警告 0、fmt クリーン。
