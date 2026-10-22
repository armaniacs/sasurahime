# PBI: GenericCleanerの責務分割とdispatch整理

## ユーザーストーリー
コントリビューターとして、新ターゲット追加が1箇所の factory 追加で完結してほしい、なぜなら1489行の分岐理解と4箇所同期が追加のたびにバグを生むから

## ビジネス価値
追加コスト・修正漏れが減り、後続のpolicy分散（04）の前提が整う。直接の利用者価値は小さいが拡張性のボトルネック解消になる。

## 優先度
- 順位: 03 / 7
- RICEスコア: 80（Reach=100 / Impact=3 / Confidence=80% / Effort=3.0人週）
- 根拠: スコアは低いが04の依存元のため先行。依存はスコアより優先のルール適用。

## BDD受け入れシナリオ
Scenario: CLI委譲とパス削除が分離される
  Given 新規に `foo` キャッシュを追加したい
  When `CliCleaner` または `PathCleaner` のどちらかを選んで登録する
  Then もう一方の分岐に影響せず `targets` に1行増える

Scenario: 既存振る舞いが変わらない
  Given 分割前の `scan` 結果がある
  When 分割後に同じHOMEで `scan` する
  Then 全行の Name/Size/Status が一致する

## 受け入れ基準
- [ ] `CliCleaner` と `PathCleaner` に分割され、`CleanMethod` の3分岐が各構造体に閉じる
- [ ] `main.rs` の supported/extra/auto の対応表テストが1箇所の真実に集約される
- [ ] `cocoa-pods`→`cocoapods` 読み替えハックが消える
- [ ] 外部挙動（CLI名・scan表・終了コード）が不変

## テスト戦略
- E2E: 分割前後の `targets`・`scan` 出力のゴールデン比較
- 統合: 各factoryが正しい型（Cli/Path）に解決されることの登録テスト
- 単体: `per_cleaner_filters`・`is_available` の分岐テストが各型に移動していること

## 見積もり
8 SP（要チームでの見積もり）

## 実装アプローチ
1. `generic.rs:44-60` を `CliCleaner`/`PathCleaner` に分割し factory を振り分け
2. 共通の `detect` ヘルパー（dir_size合算・フィルタ適用）のみ共有化
3. `main.rs:110-237,448-462,414-438` の4箇所を単一テーブル化

## 技術的考慮事項
- 過度な共通化を避け「変わる理由が同じ」ものだけ共有する
- `#[allow(dead_code)]` の復活を禁止し clippy clean を維持

## 実装者向け注記
- 現状確認: `wc -l src/cleaners/generic.rs src/main.rs`、`grep -n "AUTO_CLEAN_TARGETS\|extra_targets\|SUPPORTED_TARGETS" src/main.rs | head`
- 着手前に `cargo test` でグリーン確認。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（HOWTO-ADD-target.md の手順が短縮される）
