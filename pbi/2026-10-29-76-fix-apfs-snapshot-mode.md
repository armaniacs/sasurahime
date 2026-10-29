# PBI: apfs-snapshot の mode を型に明示化し --all/--thin 競合をエラー化

## ユーザーストーリー
sasurahime を使う開発者として、`clean apfs-snapshot --all --thin` のような競合フラグが明示エラーになることがほしい、なぜなら現行は thin が勝って --all が黙って無視され、全削除を期待した操作が静かに別の操作に変わるから。

## 優先度
- 順位: 8 / 8
- RICE スコア: 1.33（Reach=1 / Impact=1 / Confidence=80% / Effort=0.6 人週）
- 根拠: Speculative から格上げ（深掘りで `--all --thin` の黙って無視と `--urgency` 単独の黙って無視という実害を発見）。scope は 2 module 分割から「mode enum + CLI 競合エラー化 + 共有ヘルパ」に修正済み（2-Cleaner 分割は registry pin 5 箇所を壊し thin 側に死に interface を作るため却下）。

## 発見の詳細（深掘り結果）
- `--all --thin 10GB` は構築可能で apfs_snapshot.rs:194 `if self.thin.is_some()` が先行するため thin が勝ち --all は黙って無視される（main.rs:1314-1325 は with_all(all) を無条件に呼ぶ）。help text に排他性の記載なし。
- `--urgency N` 単独（--thin なし）も main.rs:1324 の unwrap_or(2) が if let Some(spec) = thin 内でのみ評価されるため黙って無視される。
- clean_thin は self.all を読まない（apfs_snapshot.rs:374-460 全体に参照なし）= interface の shallow さの実体。
- より大きい重複は zero CleanResult の 6 サイト約 35 行（:198-204 ほか）と free-space delta 計算 4 行 x 2（:269-280、:442-448）。
- 2-Cleaner 分割は却下: registry は「1 クリーナ 1 名」を 5 箇所で pin（main.rs:1744 unique、:1734/:1727 legacy、:1869 翻訳、coverage.rs:10-33）。thin 型を登録すると unique 違反、異名で legacy/翻訳/coverage 違反、登録しないと detect 等が死に interface。

## BDD受け入れシナリオ
Scenario: --all と --thin の同時指定がエラーになる
  Given `clean apfs-snapshot --all --thin 10GB`
  When  実行する
  Then  明示エラー（exit 1）で終了し tmutil は呼ばれない

Scenario: --urgency 単独がエラーになる
  Given `clean apfs-snapshot --urgency 3`（--thin なし）
  When  実行する
  Then  明示エラー（exit 1）で終了する

Scenario: CLI は 1 コマンドのまま
  Given 既存 E2E 7 件（tests/apfs_snapshot.rs）
  When  全テストを実行する
  Then  CleanTarget::ApfsSnapshot variant と対応 pin が無変更で green

## 受け入れ基準
- [ ] ApfsSnapshotCleaner は SnapshotMode（Delete { all: bool } / Thin { target_bytes, urgency }）を 1 フィールドに持ち clean() は match self.mode で分岐（all: bool 単独フィールド、thin: Option<...>、:194 の分岐が消える）
- [ ] `clean apfs-snapshot --all --thin 10GB` は明示エラー（exit 1、tmutil 未呼び出し）
- [ ] `clean apfs-snapshot --urgency N`（--thin なし）は明示エラー（help に排他性を追記、docs 更新）
- [ ] free-space delta 計算が 1 箇所のヘルパになり :269-280 と :442-448 の重複が消える（guard の report.deleted == 0 / output.status.success() は意図が異なるため各呼び出し側に残す）
- [ ] zero CleanResult の 6 サイトが apfs_snapshot.rs 内の 1 コンストラクタに置換
- [ ] TM 警告が 1 定数になる（:219-221、:394-396）
- [ ] CLI は 1 コマンドのまま、CleanTarget::ApfsSnapshot variant と対応 pin（main.rs:1601-1710、:1734、:1869、extra_targets、coverage.rs）は無変更で green
- [ ] E2E 既存 7 件は無変更で green
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: 競合 2 パターンの新規 pin + 既存 7 件が無修正で green
- 単体: ヘルパ 1 件（delta 計算、欠落 reading は 0）
- 書き換え: unit 14 件の機械的 constructor 置換（with_all x 8 / with_thin x 6、assert 無変更）

## 見積もり
0.6 人週（mode enum 変換 0.5 人日、CLI 競合エラー化 + E2E 0.5 人日、ヘルパ 3 件 + unit 書き換え 0.5-1 人日、ゲート 0.5 人日）

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（help text + docs の排他性記載）
