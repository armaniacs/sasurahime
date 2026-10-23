# PBI: hint cleaner の仕上げ（skip 後の [OK] と osascript エスケープ）

## ユーザーストーリー
hint auto-clean を使うユーザーとして、スキップされた削除が [OK] と表示されず、AppleScript 補間が安全であってほしい、なぜなら hint/cleaner.rs:79-86 と 105-112 はスキップ後も無条件で `  [OK]` を出力し（旧実装はエラーで中断）、hint/cleaner.rs:21 は `quit app "{quit_app}"` に `"` をエスケープせず補間する（現行値はすべてコンパイル時定数で安全、latent）から。

## 優先度
- 順位: 9 / 10
- RICEスコア: 8（Reach=10 / Impact=0.5 / Confidence=80% / Effort=0.5人日）
- 根拠: latent injection はリスク軽減効果でテスト衛生（37）との同点タイブレーク勝ち。

## BDD受け入れシナリオ
Scenario: スキップされた削除は [OK] を出さない
  Given 削除がスキップされるエントリ
  When  hint auto-clean を実行する
  Then  そのエントリは警告付きで報告され、[OK] は出ない

Scenario: osascript 補間の安全性
  Given `"` を含む quit_app 値（将来の追加を想定）
  When  quit コマンドが構築される
  Then  補間はエスケープされ、任意の AppleScript が実行されない

## 受け入れ基準
- [ ] hint/cleaner.rs:79-86 と 105-112 で、skip 分岐の後に [OK] が出ない（[OK] は削除成功時のみ）
- [ ] hint/cleaner.rs:21 の osascript 補間がエスケープされる（または値の不変条件が型/ランタイムで強制される）
- [ ] 既存の hint 系テストが green を維持する

## テスト戦略
- 単体: skip 分岐の [OK] 抑制テスト、osascript エスケープテスト
- 統合: 既存の hint auto-clean テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
