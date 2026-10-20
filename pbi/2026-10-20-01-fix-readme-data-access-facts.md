# PBI: README Data access を実際の挙動に合わせる

## ユーザーストーリー
sasurahime を使う開発者として、README の Data access 節が「ツールが実際に読むファイル」を正確に書いていてほしい。なぜなら、「ファイルの内容は読まない」という約束を信じて導入したのに、実際は mise の設定ファイルなどを読んでいたら、プライバシー上の判断を誤るから。

## ビジネス価値
- README の約束と実コードの食い違い (信頼性の欠陥) を解消する
- 後続の PBI (02, 04 など) が「内容を読むファイルの一覧」に 1 行ずつ足すだけで済む土台を作る (共通規約 1.6 の規則の前提)
- 測定方法: README の英日両方に「内容を読むファイル」の一覧があり、旧文言が残っていないことを grep で確認できる

## BDD受け入れシナリオ

```gherkin
Scenario: 英語の Data access 節が内容を読むファイルを列挙する
  Given README.md の Data access 節に "It does **not** read the contents of your files." という旧文言がある
  When  P0 の書き換えを適用する
  Then  "It reads the contents of only the following files" で始まる文と、mise の設定ファイルの項目が README.md に 1 行ずつある
  And   sasurahime 自身の設定ファイルと削除履歴を読むことが一覧の外の文で述べられている

Scenario: 日本語の Data access 節も同じ内容になる
  Given README.md の日本語節に "ファイルの内容を読み取ることはありません" という旧文言がある
  When  P0 の書き換えを適用する
  Then  "ファイルの内容を読み取るのは次のファイルだけ" で始まる文と、mise の設定ファイルの項目が 1 行ずつある

Scenario: ドキュメントだけの変更で挙動が変わらない
  Given 基準コミットで cargo test が全件成功し、sasurahime targets が 48 行を出す
  When  README.md だけを変更してコミットする
  Then  git diff --stat に README.md 以外が出ない
  And   cargo test が全件成功し、targets が 48 行のままである
```

## 受け入れ基準
- [ ] 旧文言 (英語 "It does **not** read the contents of your files"、日本語 "ファイルの内容を読み取ることはありません") が README.md に残っていない
- [ ] 英日それぞれ、内容を読むファイルの一覧に mise の設定ファイル (`~/.config/mise/config.toml` と、ホームから 5 階層以内の `.mise.toml`) がある
- [ ] sasurahime 自身の設定 (`~/.config/sasurahime/config.toml` または `--config`) と削除履歴 (`~/.local/share/sasurahime/history.json`) を読むことが、一覧の外の文に書かれている
- [ ] `Info.plist` の項目は書かない (まだ実装されていないため。PBI-04 が足す)
- [ ] 変更ファイルは README.md だけ。src/ と tests/ は変更しない

## テスト戦略（t_wadaスタイル）

ドキュメントだけの変更のため、自動テストの追加はない。grep による検証を「テスト」とみなす。

### E2Eテスト
- 追加なし。既存の `cargo test` が全件成功したままであることを確認する

### 統合テスト
- 追加なし

### 単体テスト
- 追加なし。設計書 7.5 節の grep 5 本 (旧文言が 0 行、新文言が各 1 行、mise 項目が 2 行) を実行し、出力が期待どおりであることを確認する

## 実装アプローチ
- **Outside-In**: 先に設計書 7.5 節の grep を実行して「旧文言が 1 行出る」状態 (Red) を確かめ、書き換え後に期待の出力 (Green) になることを確認する
- **Red-Green-Refactor**: 英日の文面は設計書 7.2 と 7.3 の文をそのまま使う。言い換えない
- **リファクタリング**: 不要

## 見積もり
1 ストーリーポイント (根拠: README.md の 2 か所を設計書の文面で置き換えるだけで、コード変更とテスト追加がない)

## 技術的考慮事項
- 依存関係: なし (前提 PBI なし)
- 事実の根拠: `src/cleaners/mise.rs` (global config と `.mise.toml` の `read_to_string`)、`src/config.rs` (設定ファイルの読み込み)、`src/history.rs` (履歴の読み込み)
- 文字種: ドキュメントは Shift-JIS で表せる文字だけ。EM DASH、不等号の合字、波線イコールを使わない。日本語の導入文末尾のコロンは半角

## 実装者向け注記

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"
grep -n "It does \*\*not\*\* read the contents of your files" README.md      # 1 行出れば未着手
grep -n "ファイルの内容を読み取ることはありません" README.md                   # 1 行出れば未着手
grep -n "It reads the contents of only the following files" README.md       # 出力なしなら未実装
grep -rn "read_to_string" src/cleaners/mise.rs src/config.rs src/history.rs
```

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-01-roadmap-and-order.md` の 7 章 (P0)。7.2 が英語の置換文、7.3 が日本語の置換文、7.5 が確認コマンド、7.6 がコミットメッセージ
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` の 1.6 (README の約束を変える変更は同じタスクで README を直す)

### 前提となる PBI
なし。ただし、本 PBI は PBI-02 より前に完了させる (ロードマップ 1 章の固定順)。

### 実装の進め方
1. 上の確認コマンドで旧文言があることを確かめる
2. 設計書 7.2 の置換後の文を README.md の英語節に、7.3 の文を日本語節に適用する (Edit で旧 1 行を新しい 5 行に置き換える)
3. 設計書 7.5 の grep を実行する
4. 検証コマンドを実行し、1 コミットにする: `git add README.md` してから `git commit -m "docs(readme): Data access を実際に内容を読むファイルに合わせて書き直す"`

### 落とし穴
- `Info.plist` の項目を先回りして書かない (実装されていない挙動を README に書くことになる。PBI-04 の Task 6 が足す)
- 「読み取った値をログにも削除履歴にも書き出さない」の対象は一覧のファイルだけ。sasurahime 自身の設定と履歴は一覧に入れない (履歴は書き戻すため矛盾する)
- 設計書の置換文を自分の言葉に直さない。「40+」「40 以上」の記述や Cargo.toml は変更しない
- `git add -A` / `git add .` を使わない。`docs/superpowers/` をコミットに含めない

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"   # 全件 ok
cargo run -q -- targets | wc -l                           # 48
git diff --stat                                           # README.md だけ
```
`sasurahime clean ...` と `sasurahime --yes` は実行しない。

## Definition of Done
- [ ] 設計書 7.5 の grep がすべて期待どおり
- [ ] 英日の文面が設計書 7.2 と 7.3 に一致する
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` が基準コミットと同じ結果、`targets` が 48 行
- [ ] 変更ファイルが README.md だけ
- [ ] コードレビュー完了
- [ ] 文字種 (Shift-JIS で表せない文字がない) を確認済み
