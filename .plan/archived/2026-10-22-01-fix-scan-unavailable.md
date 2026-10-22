# PBI: scan失敗を表現できる検出契約の強化

## ユーザーストーリー
macOS開発者として、権限不足・コマンド失敗・タイムアウト時に `scan` が `Clean` と嘘をつかず `Unavailable` と表示してほしい、なぜなら測れなかったのに綺麗だと誤解すると削除判断を誤るから

## ビジネス価値
計測への信頼が回復し、`Total reclaimable` の誤読・ issue報告の手戻りが減る。後続の推定表示・Trash-aware表示の前提になる。

## 優先度
- 順位: 01 / 7
- RICEスコア: 800（Reach=500 / Impact=2 / Confidence=80% / Effort=1.0人週）
- 根拠: 全scan利用者に影響し、後続2件（05, 06）の前提。スコア1位かつ依存ルートのため先頭。

## BDD受け入れシナリオ
Scenario: 権限不足のクリーナーはUnavailableになる
  Given 読み取り権限のないキャッシュを持つクリーナーがある
  When `sasurahime scan` を実行する
  Then その行の Status は `unavailable` と表示され、合計に加算されない

Scenario: 外部コマンドがタイムアウトしたらUnavailableになる
  Given `mise ls` が30秒以内に終わらない環境がある
  When `sasurahime scan` を実行する
  Then mise行は `unavailable` となり、全体のスキャンは中断せず他行は表示される

## 受け入れ基準
- [ ] `ScanStatus::Unavailable` が追加され、`detect()` が失敗を潰さず返す
- [ ] `scanner::run_scan` が `Unavailable` を合計から除外して表示する
- [ ] `interactive::auto_clean_indices` が `Unavailable` を `--yes` 対象に含めない
- [ ] 既存の3値表示の互換性が保たれる（`Clean/NotFound/Pruneable` の描画不変）

## テスト戦略
- E2E: 権限なしディレクトリ + タイムアウトする偽コマンドをTEMP HOMEに用意し `scan` 終了コード0と `unavailable` 表示を検証
- 統合: 失敗する `CommandRunner` を注入した各クリーナーの `detect()` が `Unavailable` を返すことを検証
- 単体: `sort_by_size` が `Unavailable` を末尾に置くこと、`ScanResult` の表示分岐の検証

## 見積もり
3 SP（要チームでの見積もり）

## 実装アプローチ
1. `cleaner.rs` に `Unavailable` 追加（表示文字列は `unavailable`）
2. `scanner.rs` の合計・ソート・表描画を対応
3. `interactive.rs` の自動選択フィルタを対応
4. 各クリーナーの `detect()` 内の `NotFound` 誤用を `Unavailable` に置き換え

## 技術的考慮事項
- 外部挙動変更は表示1列のみに留め、終了コード仕様は変えない
- 文字列マッチの `is_skippable_error` とは直交させる（本PBIでは分類のみ）

## 実装者向け注記
- 現状確認: `grep -rn "ScanStatus::" src/ | head`、`grep -rn "Pruneable(0)" src/cleaners/`
- 着手前に `cargo test` でグリーン確認。`cargo clippy -- -D warnings` 必須。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（README の scan 例に unavailable 行の説明があれば）
