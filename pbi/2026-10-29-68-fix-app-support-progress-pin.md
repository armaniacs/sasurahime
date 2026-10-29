# PBI: app_support の progress 総計を pin する

## ユーザーストーリー
TUI でクリーニングするユーザーとして、進捗バーの総計が何を数えているかが安定していてほしい。なぜなら、app_support の削除前準備をループ外に移した際（PBI 56）、`progress_init` の総計が「全選択候補」から「削除可能のみ」に変わった挙動変更が行われ、それを固定するテストが無いから。

## 優先度
- 順位: 68 / 全 9 候補
- RICE スコア: 2.0（Reach=1 / Impact=0.25 / Confidence=80% / Effort=0.1 人週）
- 根拠: app_support.rs:481-527 の前段フィルタ（Blocked/resolve_allowed → items + pre-skips）により、`progress_init` が `items.len()`（削除可能のみ）を数える。v0.4.2 は `selected.len()`（全選択候補）を数えていた。ブロックされた候補が進捗総計に現れなくなり、UX の意味が変わった。どちらが正かを決めて pin するテストが無い

## ビジネス価値
- 進捗バーの意味（何を数えているか）が明示され、固定される
- 「pure extraction」に紛れた UX 意味の変更が意図的だと証明される

## BDD 受け入れシナリオ
Scenario: 進捗総計が削除可能のみを数える（現行を pin）
  Given 選択 3 候補のうち 1 つが blocked で pre-skip される
  When  clean を実行する
  Then  progress_init の総計は 2（削除可能のみ）であり、テストで固定される

Scenario: 進捗総計が全選択候補を数える（変更する場合）
  Given 選択 3 候補のうち 1 つが blocked
  When  clean を実行する
  Then  progress_init の総計は 3 であり、pre-skip は tick の外で報告される

## 受け入れ基準
- [ ] 進捗総計の意味（削除可能のみ vs 全選択）を決めて PBI に記録する
- [ ] 決めた挙動を固定するテストが 1 本追加されている
- [ ] 全テストが緑

## テスト戦略
- 統合: tests/app_support.rs に blocked 候補を含む clean の progress 総計主張を追加
- 単体: pre-skip 報告のテスト

## 実装アプローチ
- 実装順: (1) 現行（items.len()）が意図的か確認（PBI 56 の受入基準に記録なし）、(2) 意図的なら pin するテスト、意図的でなければ selected.len() に戻す、(3) テスト
- 判断の根拠: 進捗バーは「これから削除するもの」を数えるのが自然なので現行（items.len()）を pin する方向で検討。pre-skip は削除報告（SkippedEntry）で見える

## 見積もり
0.1 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
