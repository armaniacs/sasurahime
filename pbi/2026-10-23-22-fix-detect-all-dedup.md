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
- [ ] `scanner::detect_all(&[Box<dyn Cleaner>]) -> Vec<ScanResult>` が1実装として存在する
- [ ] interactive.rs:90-112 と 215-237 のクローンが detect_all 経由に置き換わる
- [ ] run_scan が scan_results 内で再計算している available（scanner.rs:110）の二重計算が解消される
- [ ] run_scan / run_scan_json の available/risks/denied preamble 重複が共通化される
- [ ] detect_all のテストが1実装に対して1つ存在する

## テスト戦略
- 単体: detect_all が ScanResult を返すことのテスト（stub クリーナー + 順序固定）
- 統合: 既存の scan/interactive テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
