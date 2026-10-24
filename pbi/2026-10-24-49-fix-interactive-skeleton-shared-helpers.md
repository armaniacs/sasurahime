# PBI-49: interactive スケルトンの共有化（confirm / skip-confirm ブラケット / バッチ報告）

## ユーザーストーリー

保守担当者として、run_auto と run_interactive の重複が 1 つのヘルパーにまとまっていてほしい。なぜなら y/N 確認、set_skip_confirm ブラケット、post-batch の failures/skipped/total 印刷が 2 箇所でほぼ同一（約 40-50 行）で、片方だけ直すともう片方の挙動がずれるから。

## 発見元

アーキテクチャ deepening 後日候補（pbi/2026-10-23-27 索引、2026-10-24 再評価）。RICE: Reach 20 / Impact 1 / Conf 80% / Effort 0.5 = 32。バッチシーム（batch::run_batch）と純粋ヘルパー抽出は PBI 23/25 の実装で済んでおり、残りはこの 3 区画のみ。

## BDD受け入れシナリオ

```gherkin
Scenario: 2 つの interactive 入口が同じ確認・報告ヘルパーを使う
  Given run_auto と run_interactive が共通ヘルパーを呼ぶ
  When  ユーザーがそれぞれの入口で y を答える
  Then  set_skip_confirm のブラケット意味論が同一になる
  And   バッチ後の failures + skipped + Total freed の表示形式が同一になる

Scenario: ユーザーが n と答えると両フローとも中断する
  Given 共通の y/N 確認ヘルパーがある
  When  ユーザーが n を入力する
  Then  どちらの入口でも掃除は開始されない
```

## 受け入れ基準

- [ ] y/N stdin 確認が 1 つのヘルパーに統一される（run_auto:133-143 / run_interactive:232-241、可能なら xcode.rs:171-177 の stderr 変体も）
- [ ] set_skip_confirm(true)/(false) ブラケットが 1 つのヘルパーに統一される（147+187 / 246+308）
- [ ] post-batch の failures + skipped + total 印刷が 1 つの報告ヘルパーに統一される（174-185 / 293-304、total 189/310）
- [ ] interactive.rs:167-173 の PBI-33 決定（failures は run_batch コールバックでなく呼び出し側で印刷）を尊重する
- [ ] 既存テスト（interactive.rs:364-505、run_auto test 494-533）green、全ゲート green

## テスト戦略（t_wadaスタイル）

- 統合: 既存 run_auto/run_interactive テストの維持
- 単体: ヘルパーの n 中断・報告形式ケース（純粋部分は既存の抽出パターンに倣う）

## 実装者向け注記

### 現状コードの確認（2026-10-24 調査済み）
- src/interactive.rs:88-191 run_auto / 195-312 run_interactive
- 重複: confirm 133-143/232-241、bracket 147+187/246+308、failures 印刷 174-176/293-295、skipped 印刷 177-185/296-304、total 189/310
- 第 3 の確認変体: src/cleaners/xcode.rs:171-177（stderr 出力）

### 実装手順
1. confirm_or_abort ヘルパーを追加（出力先をパラメータ化すれば xcode.rs の stderr 変体も吸収）
2. skip-confirm ブラケット ヘルパーを追加
3. report_batch_summary ヘルパーを追加（failures + skipped + Total freed）
4. 2 入口から呼び出す

### 落とし穴
- PBI-33 決定の記録（interactive.rs:167-173）に反して failures を run_batch 側へ移動しないこと
- explorer.rs / library_logs.rs / app_support.rs の label → MultiSelect スケルトンは別物（項目と操作が違う）― 統一対象にしない
