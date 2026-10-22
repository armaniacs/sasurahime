# PBI: 自動削除ポリシーの分散化と--yesの安全拡大

## ユーザーストーリー
macOS開発者として、`--yes` で安全なものは全部消え、危険なものだけ止まってほしい、なぜなら現状13件だけではCIで効果が薄く、手動選択に戻らされるから

## ビジネス価値
CI・cronでの実効削除量が増え、`--yes` の過剰信頼による事故も減る。03の分割を活かす出口になる。

## 優先度
- 順位: 04 / 7
- RICEスコア: 630（Reach=600 / Impact=3 / Confidence=70% / Effort=2.0人週）
- 根拠: 高スコアだが03に依存するため03の直後。依存優先ルール適用。

## BDD受け入れシナリオ
Scenario: ポリシーが各クリーナーに分散される
  Given `docker` と `uv` のクリーナーがある
  When `sasurahime --yes` を実行する
  Then `uv` は自動で消え `docker` は明示選択なしには消えず、理由が表示される

Scenario: 危険ターゲットは警告付きで止まる
  Given `ios-backup` が pruneable な状態がある
  When `sasurahime --yes` を実行する
  Then 削除されずスキップ理由が表示され、終了コードが期待通りになる

## 受け入れ基準
- [ ] `AUTO_CLEAN_TARGETS` 中央リストが廃止され各 `auto_clean_allowed()` が正となる
- [ ] `ManualOnly` ラップとテスト側の二重期待値が単一化される
- [ ] `docker/cargo/npm` 等の大物が risk 表示付きで `--yes` 可否が決まる
- [ ] 既存の不可逆2件（ios-backup/apfs-snapshot）が `--yes` で絶対に動かない

## テスト戦略
- E2E: `--yes` で安全群のみ削除・危険群スキップの結合テスト
- 統合: 全クリーナーの `auto_clean_allowed` と登録名の一意性テスト
- 単体: risk文言・終了コード分岐のテスト

## 見積もり
5 SP（要チームでの見積もり）

## 実装アプローチ
1. 03完了後に中央リストを削除し trait 側に集約
2. `ManualOnly` の責務を `risk()` 表示に寄せる
3. `--yes` のスキップ表示を `Unavailable` 対応と合わせる

## 技術的考慮事項
- 破壊度ポリシー変更は本PBIでのみ行い03では挙動不変に留める
- 設定 `exclude` との相互作用をテストする

## 実装者向け注記
- 現状確認: `grep -n "AUTO_CLEAN_TARGETS\|ManualOnly\|auto_clean_allowed" src/main.rs src/cleaner.rs src/cleaners/manual_only.rs | head -n 30`
- 着手前に `cargo test` でグリーン確認。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（README の --yes 説明を更新）
