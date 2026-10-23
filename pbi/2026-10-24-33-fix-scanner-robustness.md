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
- [ ] detect_all の par_iter で detect() panic が per-cleaner に分離される（catch_unwind 等、panic クリーナーは NotFound として扱う）
- [ ] scanner.rs:100 と 133-135 の二重 is_available パスが1パスに統一される（scanner.rs:122 の「one pass」コメントの実態合わせ）
- [ ] interactive.rs run_auto の失敗出力が batch 後まとめ出力から従来の per-cleaner 即時出力に戻る（または統一の判断が記録される）
- [ ] 既存の scan 系テストが green を維持する

## テスト戦略
- 単体: panic クリーナーの分離テスト
- 統合: scan / --json の既存テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
