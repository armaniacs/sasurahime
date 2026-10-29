# PBI: 削除帳簿（delete ledger）の一般化と迂回 9 ファイルの回収

## ユーザーストーリー
sasurahime の保守担当者として、per-entry 削除の帳簿契約（progress、fatal/skip 分類、freed 加算、uses_trash 真実、exit_code）が 1 つの interface の裏に集中することがほしい、なぜなら 9 ファイルの手書きループで契約がドリフトし、すでに ios_backup の skipped 欠落と mise の無記録という実害が出ているから。

## 優先度
- 順位: 2 / 8
- RICE スコア: 2.0（Reach=3 / Impact=2.5 / Confidence=80% / Effort=3.0 人週）
- 根拠: 実害ドリフト（ios_backup.rs:128-132 が skipped を DROP、generic.rs:898-905 が SkippedEntry 無記録、mise.rs:289-298 が無記録）。後続の TrashMode（71）と CleanResult（72）が同じ call site 群を通るため先に着手して churn 最小化。

## 発見の詳細（深掘り結果）
- 帳簿契約が 9 ファイル・約 13 ループ本体で再実装: mise.rs:275-304、browser.rs:129-168、log.rs:184-207、custom.rs:80-109、cargo.rs:104-155、xcode.rs:224-252、generic.rs:884-933、gradle.rs、ollama.rs:190-210
- 実害: ios_backup.rs:128-132 は delete_entries の戻り値を CleanResult::freed で包み直し skipped と deleted_paths を DROP（all-skipped が exit 0、pin 契約と矛盾）。generic.rs:898-905 は unsafe path で SkippedEntry 無記録。mise.rs:289-298 は fatal/skip を eprintln + 裸 continue。
- 迂回側 6 ループは fatal で `?` 中止（browser.rs:150 ほか）→ trash.rs:661-666 の pin が「廃止済み契約」と明言する継続挙動に統一
- design-it-twice の結論: (a) record(id, size, outcome) は記録漏れを構造的に防げず PBI 68 pin を壊すリスク。採用は (b) ledger が items + per-item 削除クロージャ（FnMut(&Path, u64) -> Result<DeletionOutcome>）を受け loop を内部実行。ollama CLI 削除も同じ帳簿に乗り、seam closure（safe_delete）+ CLI closure（ollama rm）の 2 adapter で seam が本物になる。

## BDD受け入れシナリオ
Scenario: fatal per-entry エラーで継続する
  Given ledger を通る掃除で 1 エントリの削除が fatal 失敗
  When  掃除を実行する
  Then  SkippedEntry(SkipCause::Reason) が 1 件記録される
  And   残りエントリの削除が継続する

Scenario: every entry is skipped で exit 1
  Given mise の全ランタイム削除が失敗する環境
  When  `clean mise` を実行する
  Then  exit_code は 1 で SkippedEntry が全件記録される

Scenario: ollama の CLI 削除が同一帳簿に乗る
  Given `ollama rm` が失敗する環境
  When  `clean ollama` を実行する
  Then  SkippedEntry(SkipCause::Reason) に散文理由が保持される
  And   freed 加算と uses_trash false は成功時のみ

## 受け入れ基準
- [ ] 迂回 9 ファイルの削除実行部（tick・分類・freed・SkippedEntry・CleanResult 組立）が ledger に回収される
- [ ] ios_backup.rs:128-132 の wrap を外し skipped/deleted_paths が落ちない（all-skipped は exit 1）
- [ ] generic.rs:898-905 の unsafe path が SkippedEntry 記録される（clean_one は deletion test pass で削除）
- [ ] fatal は全ループで SkippedEntry + 継続（`?` 中止の統一）
- [ ] 6 seam 利用者は形状不変、PBI 68 pin（progress total = items.len()）無傷、trash.rs 4 pin 無傷
- [ ] dry-run は ledger を通らない（現行どおり）
- [ ] 範囲外を明記: hint/cleaner.rs の 2 ループ（Cleaner trait 外フロー）
- [ ] CHANGELOG: per-entry 行形式、gradle/cargo registry の progress 新規出現、all-skipped exit flip、log の per-target 接頭辞喪失
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: mise/browser/gradle の all-skipped で exit 1（tempdir HOME）
- 統合: ledger 契約テスト（fatal 継続、skippable 分類、freed、exit_code、空 items、vanished entry）
- 単体: mise all-skipped pin、generic unsafe pin。既存 pin（gradle.rs:336-415、cargo.rs:279-307、app_support.rs:1454-1486）が無傷で通ること

## 見積もり
3.0 人週（ledger 一般化 + trash.rs 契約 0.5-1.0、迂回移植 1.0-1.5、pin + ios_backup 0.5-0.75、外観検証 + CHANGELOG 0.25）

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（CHANGELOG）
