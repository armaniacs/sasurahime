# PBI: 計測の一本化とmise走査の高速化

## ユーザーストーリー
macOS開発者として、どのコマンドでも同じ基準でサイズが測られ、miseの `scan` が速く正確になってほしい、なぜなら二重計上と見落としがあると合計を信用できないから

## ビジネス価値
合計表示の二重計上・取りこぼしが減り、スキャン体感速度が上がる。Quick Winとして単独で価値が出る。

## 優先度
- 順位: 02 / 7
- RICEスコア: 560（Reach=700 / Impact=2 / Confidence=80% / Effort=2.0人週）
- 根拠: mise利用者を含む大半に影響。依存なしで着手でき、後続のpolicy・表示改善の土台。

## BDD受け入れシナリオ
Scenario: サイズ計測が一本化される
  Given `~/Library/Caches` 配下に既知アプリのキャッシュがある
  When `sasurahime scan` と `explore` で同じパスを測る
  Then 両者のByte数が一致し、合計に二重計上されない

Scenario: miseの深いmonorepoでも取りこぼさない
  Given 6階層目に `.mise.toml` があるmonorepoがある
  When `sasurahime scan` を実行する
  Then 使用中バージョンが保護され、かつ `node_modules/.git/target` 配下は走査されない

## 受け入れ基準
- [ ] `hint/detector.rs` の自前 `dir_size_bytes` が廃止され `format::dir_size` 委譲になる
- [ ] `explorer.rs` の `MANAGED_PATTERNS` と cleaner登録名の対応表テストが追加される
- [ ] `mise.rs` の手書きTOML行パースが `toml` クレート利用に置き換わる
- [ ] `mise` 走査が除外ディレクトリ対応し、depthが設定可能になる

## テスト戦略
- E2E: TEMP HOMEに6階層 `.mise.toml` + `node_modules` を用意し保護・除外を検証
- 統合: 両dir_size実装の一致テスト、TOML `[tools]` 複雑系（コメント・引用符なし）のパーステスト
- 単体: 除外フィルタ・depth解決の境界値テスト

## 見積もり
5 SP（要チームでの見積もり）

## 実装アプローチ
1. `hint/detector.rs:14` を `format::dir_size` 委譲に置換
2. `explorer.rs:15-80` と `main.rs` ターゲット名の対応テスト追加
3. `mise.rs:135-169` を `toml` クレートで置換、`scan_pinned_versions` に除外・depth導入

## 技術的考慮事項
- `Cargo.toml:11` の `toml` 依存を再利用し新規依存を増やさない
- `follow_links(false)` は維持し symlink ループを作らない

## 実装者向け注記
- 現状確認: `grep -rn "dir_size" src/ | head -n 20`、`grep -rn "MANAGED_PATTERNS" src/explorer.rs | head`
- 着手前に `cargo test` でグリーン確認。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
