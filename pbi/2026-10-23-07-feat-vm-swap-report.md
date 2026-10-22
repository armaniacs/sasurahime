# PBI: スワップ/スリープイメージの表示と再起動案内

## ユーザーストーリー
macOSユーザーとして、「システムデータ」のうち `/private/var/vm` がどれだけを占めているかを知りたい、なぜなら長期稼働で数十GBになっていても削除手段がなく、原因が分からないまま他を消してしまうから

## ビジネス価値
削除できない領域を「消せないが再起動で戻る」と明示でき、無駄な削除試行を防ぐ。測定指標は、scan で vm 領域のサイズと稼働日数が確認できること。

## 優先度
- 順位: 03 / 5
- RICEスコア: 240(Reach=300 / Impact=1 / Confidence=80% / Effort=1.0人週)
- 根拠: 実装が小さく、既存の `manual_only` パターンに乗る。

## BDD受け入れシナリオ

```gherkin
Scenario: スワップとスリープイメージのサイズを表示する
  Given /private/var/vm にスワップファイルとスリープイメージがある
  When  `sasurahime scan` を実行する
  Then  `vm-swap` が手動対応として合計サイズと稼働日数付きで表示される

Scenario: clean は何も削除しない
  Given /private/var/vm にスワップファイルがある
  When  `sasurahime clean vm-swap` を実行する
  Then  ファイルは変更されない
  And   「再起動で初期化される」旨が表示される

Scenario: 読めない場合は不明として扱う
  Given /private/var/vm の一覧が権限で取得できない
  When  `sasurahime scan` を実行する
  Then  サイズは Unknown と表示され 0 扱いされない
```

## 受け入れ基準
- [ ] 対象は `swapfile*` と `sleepimage` のメタデータ参照のみ(sudo 不要の範囲)
- [ ] `clean` は no-op で、`--yes` の対象に含めない
- [ ] 稼働時間(`sysctl kern.boottime`)を併記する
- [ ] サイズは見かけ値を採用し、その旨を注記する

## テスト戦略(t_wadaスタイル)

### E2Eテスト
- vm ディレクトリを差し替えた状態で scan と clean の出力を検証

### 統合テスト
- `CommandRunner` モックで `sysctl` 出力を差し替え、稼働日数の表示を検証
- 一覧取得失敗時に Unknown になること

### 単体テスト
- boottime 出力のパース
- 稼働日数の整形(0日・1日・複数日)
- 不正な出力のエラーハンドリング

## 実装アプローチ
- **Outside-In**: E2E から開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 各レイヤーで適用
- **リファクタリング**: `manual_only` との共通部分を整理

## 見積もり
2 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: `2026-10-22-06-feat-unknown-size-json.md` の Unknown 表現に従う
- テスタビリティ: vm ディレクトリのパスと `sysctl` 呼び出しを注入可能にする
- 非機能要件: 読み取り専用で副作用なし

## 実装者向け注記

### 現状コードの確認
確認済み: `sleepimage` と `boottime` は `src/` に存在しない(2026-10-23 時点)。
```bash
grep -rn "sleepimage\|boottime" src/
cat src/cleaners/manual_only.rs
```

### 実装手順
1. E2E: scan の vm-swap 行のテストを書く(Red)
2. 統合: `sysctl` モックと取得失敗のテストを書く(Red)
3. 単体: boottime パースのテストを書く(Red)
4. `vm-swap` を `manual_only` パターンで実装(Green)
5. 登録して `targets` に出ることを確認(Green)
6. 共通部分の整理(Refactor)

### 落とし穴
- swapfile はスパースの場合があり、見かけと実効が乖離する
- `/private/var/vm` は権限により一覧できない環境がある。0 と誤表示しない

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] E2E/統合/単体のすべてが揃っている
- [ ] コードレビュー完了
- [ ] リファクタリング完了(グリーン後)
- [ ] README のターゲット一覧を更新済み
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る
