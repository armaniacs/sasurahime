# PBI: scanner の堅牢化（panic 分離 + is_available 1パス + run_auto 出力順）

## ユーザーストーリー
scan / --yes / TUI を使うユーザーとして、1つのクリーナーの detect() が panic しても他のクリーナーの結果が得られてほしい、なぜなら detect_all（scanner.rs:108）の par_iter は panic をそのまま伝播し、今や全4エントリポイントの唯一の detect 経路だから。

## 優先度
- 順位: 6 / 10
- RICEスコア: 48（Reach=30 / Impact=1 / Confidence=80% / Effort=0.5人日）
- 根拠: 堅牢化。panic する detect は現状存在しない仮定だが、唯一の detect 経路としての分離は安価。

## BDD受け入れシナリオ
Scenario: panic する detect が他を落とさない
  Given panic を投げるテスト用クリーナーと正常なクリーナー
  When  detect_all を実行する
  Then  panic したクリーナーは NotFound（または明示的なエラー状態）として扱われ、正常なクリーナーの結果は返る

Scenario: is_available の1パス
  Given run_scan の1回の呼び出し
  When  detect_all と scan_preamble が走る
  Then  is_available() の評価は1回の呼び出しにつき1パスに統一される

## 受け入れ基準
- [x] detect_all の par_iter で detect() panic が per-cleaner に分離される（catch_unwind 等、panic クリーナーは NotFound として扱う）
- [x] scanner.rs:100 と 133-135 の二重 is_available パスが1パスに統一される（scanner.rs:122 の「one pass」コメントの実態合わせ）
- [x] interactive.rs run_auto の失敗出力が batch 後まとめ出力から従来の per-cleaner 即時出力に戻る（または統一の判断が記録される）
- [x] 既存の scan 系テストが green を維持する

## テスト戦略
- 単体: panic クリーナーの分離テスト
- 統合: scan / --json の既存テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit bef2720。
- panic 分離: detect_all の per-index クロージャを std::panic::catch_unwind(AssertUnwindSafe) で包み、panic したクリーナーは NotFound に劣化し他のクリーナーは全件報告。WHY コメント付き（release プロファイルは panic=abort のため unwind ビルド（tests/debug）で有効、注意書き記録）。
- is_available 1パス: detect-status からの導出は不真実（available-but-NotFound は正常な状態）のため不採用。detect_all のシグネチャを (Vec<ScanResult>, Vec<usize>) に変更して1パスを所有させ、scan_preamble は available を受けて risks + denied のみを返す。run_scan / run_scan_json / interactive 2 caller 更新。red（is_available 評価回数 left:2 right:1）→ green（1回/call）。
- run_auto 失敗出力: option (b) — batch 後まとめ出力を維持し、統一の判断を interactive.rs のコメントで記録（option (a) のコールバックは src/batch.rs のシグネチャ変更を要するため対象外、5つの run_batch 呼び出しサイトは全て batch 後まとめ出力で統一済み）。PBI 基準の「または統一の判断が記録される」を満たす。
- 新テスト: detect_all_isolates_a_panicking_detect（panic クリーナーは NotFound、正常クリーナーは結果を返す、red で panic 伝播を証明）。
- 検証: lib 663 passed / 4 failed（既知 TCC のみ）、統合 37 ターゲット green、clippy 0、fmt クリーン。
