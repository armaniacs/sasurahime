# PBI: リスク4段階分類による自動削除ポリシーの分散化

## ユーザーストーリー
macOS開発者として、各ターゲットが「安全に再生成されるもの」か「ユーザーデータを含みうるもの」かを一目で区別でき、`--yes` では安全なものだけが消えてほしい、なぜなら現状は中央リストの13件だけで CI での効果が薄く、50以上のターゲットを名前だけで判断して全選択すると事故につながるから

## ビジネス価値
CI・cron での実効削除量が増え、`--yes` の過剰信頼による事故も減る。`--yes` の可否、TUI の初期選択、scan の表示が、同じ分類1つから決まる。測定指標は、全ターゲットに分類があり、`--yes` が Safe のみを実行すること。

## 優先度
- 順位: 04 / 7
- RICEスコア: 360(Reach=600 / Impact=3 / Confidence=70% / Effort=3.5人週)
- 根拠: ポリシー分散化(旧 04)とリスク分類(旧 2026-10-23-08)は同じ `--yes` 判定を触るため1件に統合した。03の分割に依存するため03の直後。

## BDD受け入れシナリオ

```gherkin
Scenario: scan にリスク列を表示する
  When  `sasurahime scan` を実行する
  Then  各行に Safe / Moderate / Risky / Manual のいずれかが表示される

Scenario: --yes は Safe のみを対象にする
  Given `uv` は Safe、`docker` は Moderate、`ios-backup` は Risky である
  When  `sasurahime --yes` を実行する
  Then  `uv` は削除される
  And   `docker` と `ios-backup` は実行されず、スキップ理由が表示される

Scenario: 不可逆な対象は --yes で絶対に動かない
  Given `ios-backup` と `apfs-snapshot` に削除可能なデータがある
  When  `sasurahime --yes` を実行する
  Then  どちらも削除されず、終了コードは期待通りになる

Scenario: TUI の初期選択は Safe のみ
  When  引数なしで起動する
  Then  Safe のターゲットだけが初期選択される
  And   Risky は未選択で警告表示になる

Scenario: 分類が未定義のターゲットは Moderate になる
  Given 新規ターゲットがリスクを明示していない
  When  `sasurahime scan` を実行する
  Then  そのターゲットは Moderate と表示され `--yes` では動かない
```

## 受け入れ基準
- [ ] `Cleaner` に `risk() -> Risk`(Safe / Moderate / Risky / Manual)を追加する。既定値は Moderate とし、付け忘れで Safe にならないようにする
- [ ] `auto_clean_allowed()` は `risk() == Safe` から導かれ、`AUTO_CLEAN_TARGETS` 中央リストは廃止される
- [ ] `ManualOnly` ラップとテスト側の二重期待値が単一化され、`ManualOnly` は Manual として表示される
- [ ] `docker` / `cargo` / `npm` 等の大物が、リスク表示付きで `--yes` の可否を決められる
- [ ] 既存の不可逆2件(`ios-backup` / `apfs-snapshot`)は Risky で、`--yes` では絶対に動かない
- [ ] scan 表、`--json`、TUI のすべてで同じ分類値を使う
- [ ] 分類基準を README に表で記載する
- [ ] 設定 `exclude` との相互作用がテストされている

## テスト戦略(t_wadaスタイル)

### E2Eテスト
- `--yes` で Safe のみ削除・それ以外スキップされることの結合テスト
- scan の表示にリスク列が出ること

### 統合テスト
- 全クリーナーを列挙し、分類が期待表と一致すること(未分類の検出)
- 登録名の一意性
- TUI の初期選択ロジックが Safe のみを返すこと

### 単体テスト
- `Risk` の順序比較と既定実装(Moderate)
- スキップ理由の文言と終了コード分岐
- `risk()` から `auto_clean_allowed()` への導出

## 実装アプローチ
- **Outside-In**: E2E から開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 各レイヤーで適用
- **リファクタリング**: 中央リストと二重期待値を除去し、判定の入口を `risk()` に一本化

## 見積もり
8 SP(要チームでの見積もり)。上限ちょうどのため、膨らむ場合は「分類の導入と表示」と「`--yes` 対象の拡大」で分割する。

## 技術的考慮事項
- 依存関係: 03(`GenericCacheCleaner` の分割)の後。03 では挙動不変に留め、破壊度ポリシーの変更は本PBIでのみ行う
- テスタビリティ: 分類表を全件テストで検証する
- 非機能要件: 分類変更は `--yes` の挙動変更になるため、CHANGELOG に記載して既存の cron 利用者に周知する

## 実装者向け注記

### 現状コードの確認
確認済み: `src/` に `Risk` 型は存在しない(2026-10-23 時点)。
```bash
grep -n "AUTO_CLEAN_TARGETS\|ManualOnly\|auto_clean_allowed" src/main.rs src/cleaner.rs src/cleaners/manual_only.rs | head -n 30
grep -n "trait Cleaner" -A 30 src/cleaner.rs
```
着手前に `cargo test` でグリーン確認。

### 実装手順
1. E2E: `--yes` の対象と scan のリスク列のテストを書く(Red)
2. 統合: 全クリーナーの分類網羅と TUI 初期選択のテストを書く(Red)
3. 単体: `Risk` の順序・既定値・導出のテストを書く(Red)
4. `Risk` enum と既定実装を追加(Green)
5. 各クリーナーに分類を付与し、`--yes`・scan・TUI の判定を `risk()` に切り替える(Green)
6. 中央リストと `ManualOnly` の二重期待値を除去(Refactor)
7. `--yes` のスキップ表示を `Unavailable` 対応と合わせる

### 落とし穴
- Safe を既定にすると、新規ターゲットが無審査で `--yes` に入る
- 分類の見直しで `--yes` の対象が減る場合がある。既存の cron 利用者に影響する
- 03 より先に着手すると、`GenericCacheCleaner` が1つのリスクに丸められる

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] E2E/統合/単体のすべてが揃っている
- [ ] コードレビュー完了
- [ ] リファクタリング完了(グリーン後)
- [ ] README(`--yes` の説明と分類表)と CHANGELOG を更新済み
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る
