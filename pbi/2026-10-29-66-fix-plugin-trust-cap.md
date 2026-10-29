# PBI: plugin 申告値の検証（スキャンサイズに対する cap とドキュメント）

## ユーザーストーリー
プラグインを実行するユーザーとして、プラグインが申告する `bytes`/`freed` が実在のサイズを超えて報告されない状態がほしい。なぜなら、PATH 上の `sasurahime-*` 実行ファイルが申告値をそのまま scan 総計・history・「Total freed」に出力でき、監査痕跡が偽装可能だから。

## 優先度
- 順位: 66 / 全 9 候補
- RICE スコア: 3.2（Reach=1 / Impact=1 / Confidence=80% / Effort=0.25 人週）
- 根拠: プラグイン申告の `bytes` は `ScanStatus::Pruneable(bytes)` として scan 総計に加算され（plugin.rs:97、tests/plugin.rs の plugin_appears_in_scan_and_total で固定）、`freed` は `bytes_freed` → 「Total freed」→ history → 「(in Trash)」注記に直行する（plugin.rs:127-132、:354-360、:381-388）。cap もファイルシステム照合も無い。信頼境界としては文書化済み（plugin.rs:399-401 の advisory、Risk::Moderate で `--yes` 除外）だが、ツール自身のレポートが偽装可能なまま

## ビジネス価値
- プラグインの暴走・虚偽申告でレポートが膨らむ事故が封じられる
- 監査痕跡（history）の信頼性が上がる

## BDD 受け入れシナリオ
Scenario: 申告値が実在サイズを超えない
  Given プラグインが `{"bytes": 18446744073709551615}` を申告する
  When  scan を実行する
  Then  申告値は実在のディレクトリサイズ（または 0）に cap され、u64::MAX が報告されない

Scenario: 申告 freed が削除前に照合される
  Given プラグインが `{"freed": 1000000}` を申告するがキャッシュディレクトリは 0 B
  When  clean を実行する
  Then  freed は実在サイズに cap される（または申告が棄却される）

## 受け入れ基準
- [x] プラグイン申告の `bytes` がスキャンサイズ（実在ディレクトリサイズ）に対して cap される
- [x] プラグイン申告の `freed` が実在サイズに対して cap される
- [x] cap のテスト（u64::MAX 申告、虚偽申告）が 2 本以上追加されている
- [x] 信頼境界の文書（advisory）が cap 挙動を説明する
- [x] 全テストが緑

## テスト戦略
- 統合: tests/plugin.rs に u64::MAX 申告と虚偽申告の E2E
- 単体: cap 計算（min(申告, 実在)）のテスト

## 実装アプローチ
- 実装順: (1) 申告 `bytes`/`freed` を実在ディレクトリサイズと min する cap を plugin.rs に追加、(2) cap を advisory の文書に追記、(3) E2E
- 判断の根拠: 署名やチェックサムによる検証は採らない（過剰）。実在サイズ照合が最小で十分

## 見積もり
0.25 人週

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
- [x] CHANGELOG の [Unreleased] に追記
- [x] コードレビュー完了
