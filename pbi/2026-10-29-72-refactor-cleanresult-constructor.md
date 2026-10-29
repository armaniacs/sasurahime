# PBI: CleanResult の dead field 削除と constructor seam への収束

## ユーザーストーリー
sasurahime の保守担当者として、CleanResult の構成が constructor seam に収束し、誰にも読まれない dead field が消えることがほしい、なぜなら 24 箇所の struct literal 手書きと虚偽の doc comment が一貫性ドリフトの温床になっているから。

## 優先度
- 順位: 4 / 8
- RICE スコア: 3.2（Reach=3 / Impact=1 / Confidence=80% / Effort=0.75 人週）
- 根拠: deleted_paths は本番 reader 0 で deletion test pass（消費者ゼロの pass-through data holder）。PBI 70（ledger）が 21 箇所の data-carrying literal を吸収した後に着手。

## 発見の詳細（深掘り結果）
- struct literal は実数 50 件（本番 45 + cfg(test) 5）。本番 45 のうち empty(name) と完全同一は 24 件。残り 21 件は PBI 70 の ledger に吸収される。
- deleted_paths は本番 reader 0（唯一の read は trash.rs:618 のテスト）。cleaner.rs:161-162 の doc は 3 重に虚偽: (1) --verbose で gate されない (2) --permanent の reader 不在 (3) delete_entries 経由以外は populate されない。
- 来歴: PBI-05（commit 301c751）で #[expect(dead_code)] 付きの投機的フィールドとして誕生。実際の監査証跡は log crate と per-entry println! で出荷済み。cd5790f で #[allow] に格下げ。
- pass-through ヘルパー 3 件: library_caches.rs:197-199、app_support.rs:362-364、orphan_containers.rs:406-408（deletion test pass）。
- 却下した代替: Default + struct update は uses_trash: false を静かに選び PBI-43 seam-truth を弱める。deleted_paths を SkippedEntry に併合は意味的な補集合の混在。

## BDD受け入れシナリオ
Scenario: dead audit field が削除される
  Given deleted_paths フィールドなしの CleanResult
  When  cargo build / test / clippy / fmt を実行する
  Then  全てが green で dead_code allow が残らない

Scenario: zero-result サイトが constructor に収束する
  Given 24 箇所の empty 同等 literal
  When  CleanResult::empty(name) に置換する
  Then  exit_code 挙動は不変（全 exit-code / skip-visibility テストがパス）

Scenario: cancelled run が名前付き constructor を持つ
  Given main.rs:741 と main.rs:759 の匿名 fabricated zero（name: ""）
  When  CleanResult::cancelled() に置換する
  Then  run_clean_target テストが name policy の doc 付きでパスする

## 受け入れ基準
- [ ] deleted_paths フィールド削除（49 箇所の vec![] + producer 3 行 + trash.rs:618 assert + 虚偽 doc）
- [ ] 24 箇所の empty 同等 literal を CleanResult::empty(name) に機械置換（exit_code 挙動不変）
- [ ] pass-through ヘルパー 3 件を削除し call site が直接 CleanResult::empty を呼ぶ
- [ ] main.rs:741/759 を CleanResult::cancelled() に置換（name policy を doc で明記）
- [ ] 21 箇所の data-carrying literal は scope fence（PBI 70 の ledger に吸収済み。本 PBI では触らない）
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- 統合: exit_code（main.rs:824）、PBI-43 seam-truth、PBI-53/59、skip visibility（batch.rs / interactive.rs）、history roundtrip 全種が無修正で green
- 単体: trash.rs:618 の 1 assert はフィールド削除とともに消える（テスト自体の delete_entries_vanished_path は残る）

## 見積もり
0.75 人週（deleted_paths 削除 0.25、24 変換 0.25、ヘルパー + cancelled + doc 0.25、検証バッファ 0.25 は検証工程に吸収）

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（cleaner.rs の CleanResult doc）
