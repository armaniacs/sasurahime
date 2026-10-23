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
- [ ] `run_batch(cleaners, opts, reporter) -> BatchSummary{total, skipped, failures}` が lib に実装される
- [ ] main.rs の Auto / All / Caches ループと interactive.rs の 2 ループが run_batch 経由になる
- [ ] history ガードの 6 サイト（main.rs:770/917/1047/1254、interactive.rs:188/322）が 1 箇所に集約される
- [ ] TUI sub-target クリーンが freed_message / post_clean_note / history を得る（意図した挙動変更として明記）
- [ ] バッチのテストが stub クリーナーで書ける（E2E バイナリ実行不要）

## テスト戦略
- 単体: run_batch の BatchSummary（総計/skip/失敗）テスト
- 統合: 既存の auto/all/caches/interactive テストが green を維持

## 見積もり
2ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
