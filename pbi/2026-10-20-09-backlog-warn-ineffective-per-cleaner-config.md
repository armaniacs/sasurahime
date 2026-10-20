# PBI: 効かない per-cleaner 設定を起動時にまとめて警告する

## ユーザーストーリー
sasurahime のユーザーとして、設定ファイルに書いた `[cleaner.<name>]` の `older_than_days` / `larger_than_mb` が効かないときは、その場で知りたい。なぜなら、効いていると思ったまま `clean` を実行し、期待より多くを削除してしまうから。

## ビジネス価値
- 「age 30 日以上だけ消す」つもりの設定が黙って無視される事故を防ぐ
- 警告が cleaner ごとに散らばらず、起動時に 1 回だけ読める形で出る
- マクロ表の factory が設定を渡すかどうかの不整合に利用者が影響されなくなる

## BDD受け入れシナリオ

```gherkin
Scenario: コマンド方式の cleaner に効かない設定がある
  Given config.toml に [cleaner.docker] older_than_days = 30 がある
  And   docker は CleanMethod::Command の cleaner である
  When  ユーザーが `sasurahime scan` を実行する
  Then  "older_than_days" が docker には効かない旨の警告が 1 回だけ stderr に出る
  And   scan の結果表は通常どおり出る

Scenario: ディレクトリ削除方式の cleaner は警告しない
  Given config.toml に [cleaner.node-gyp] older_than_days = 30 がある
  And   node-gyp は CleanMethod::DeleteDirs の cleaner である
  When  ユーザーが `sasurahime scan` を実行する
  Then  警告は出ない

Scenario: 存在しない cleaner 名の設定
  Given config.toml に [cleaner.no-such] larger_than_mb = 10 がある
  When  ユーザーが `sasurahime scan` を実行する
  Then  "no-such" という cleaner は存在しない旨の警告が 1 回出る

Scenario: 複数の効かない設定がまとまって出る
  Given docker と npm の 2 件に効かない設定がある
  When  ユーザーが `sasurahime scan` を実行する
  Then  警告は 1 つのまとまり (見出し 1 行と 2 件の箇条書き) で出る
```

## 受け入れ基準
- [ ] 効かない設定 (Command / CommandWithDetectDir 方式に対する `older_than_days` / `larger_than_mb`) が起動時に 1 回だけ、cleaner 名つきでまとめて警告される
- [ ] 警告の有無は、マクロ表の factory が `.with_config(config)` を呼ぶかどうかに依存しない
- [ ] DeleteDirs 方式の cleaner と、設定が空の cleaner では警告しない
- [ ] 未知の cleaner 名は別の文言で警告する
- [ ] 警告は削除や scan の結果を変えない (警告だけ。設定の適用有無は現状どおり)
- [ ] `clean <target>` / `scan` / `--yes` / TUI のどの入口でも同じ警告が 1 回だけ出る

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 一時 HOME に `config.toml` を書き、`sasurahime scan` を実行して stderr に警告が 1 回含まれることを検証する
- 設定なしの HOME で警告が出ないことを検証する

### 統合テスト
- 全 cleaner の一覧と設定を渡して「効かない設定のリスト」を返す純粋関数を、実際の `all_cleaners()` に対して検証する

### 単体テスト
- 判定関数: (method の種別, 設定の有無) の組み合わせ表
- 文面生成: 0 件なら None、複数件なら 1 つの文字列にまとまる

## 実装アプローチ
- **Outside-In**: E2E (警告が出る) を Red にしてから判定関数へ
- **Red-Green-Refactor**: まず現状の `with_config` の警告を判定関数に移して挙動を保つ
- **リファクタリング**: 重複する警告ログを削除する

## 見積もり
3 ストーリーポイント。理由: 判定に必要な「method の種別」を `Cleaner` trait 越しに取る設計判断があり (trait 拡張か、設定側から cleaner 名で引くか)、全入口で 1 回だけ出す配線と E2E が必要。

## 技術的考慮事項
- 依存関係: 設計書 04 (docker-and-scan-coverage) の S3 で `all_cleaners()` がマクロ表から生成される。その後に着手すると、全 cleaner を 1 か所で列挙できて簡単になる。S3 の前でも手書きの `all_cleaners()` に対して実装できるが、後で組み直しになる
- 設計書 04 の 2.7 節との関係: S3 は colima の 1 行だけ `.with_config(config)` を付けて scan と clean の両方で警告が出るようにする。他の約 29 件と、起動時のまとめ警告は本 PBI の担当と明記されている。本 PBI を実装したら colima の個別 `with_config` 警告は重複するので、個別の `log::warn!` を撤去するか、まとめ警告に一本化する
- テスタビリティ: `log::warn!` はテストで捕まえにくい (設計書 04 の 2.7 節末尾)。判定と文面は純粋関数に切り出し、出力先は呼び出し側に置く
- 非機能要件: 起動時間を増やさない (設定が空なら判定をスキップ)

## 実装者向け注記

### 現状コードの確認
```bash
cargo test
grep -n "with_config" src/main.rs src/cleaners/generic.rs
grep -n "per_cleaner" -r src/ | grep -v "^src/cleaners/generic.rs.*fn with_config_"
sed -n 350,380p src/cleaners/generic.rs
```

事実確認の結果 (HEAD 時点):
- `src/cleaners/generic.rs:355` の `GenericCleaner::with_config` は、`config.per_cleaner.get(self.name())` に `older_than_days` / `larger_than_mb` があり、かつ `method` が `CleanMethod::Command` か `CleanMethod::CommandWithDetectDir` のとき `log::warn!("per-cleaner filters not supported for '{}': command-based cleaner", name)` を出す。そのうえで値をフィールドに設定する。値が実際に使われるのは `DeleteDirs` の分岐だけ。
- `src/main.rs` のマクロ表 `define_cleaners!` の行数は 39 件 (設計書 04 の 2.1 節と一致するか、`grep -c` ではなく表を数えて確認すること)。そのうち `.with_config(config)` を呼ぶ行は 9 件: act (101 行), node-gyp (125), spm (152), downloads (194), vscode-extensions (197), terraform (203), volta (209), sbt (212), tree-sitter (215)。
- 手書きの `all_cleaners()` では act (471 行), colima (488 行) に `.with_config(config)` があり、`clean node-gyp` の特殊経路 (745 行付近) にもある。colima のマクロ行 (149 行) は `|home, _config|` で設定を無視する。このため colima は経路によって警告が出たり出なかったりする。
- 設定を使わない/無視する行は、colima を除くと約 29 件で、これらは警告も出ない。また `HuggingFaceCleaner` など専用 struct の cleaner は `with_config` 自体を持たない。
- `PerCleanerConfig` は `src/config.rs:8`、`Config.per_cleaner: HashMap<String, PerCleanerConfig>` は `config.rs:62`。

### 再現手順
1. `~/.config/sasurahime/config.toml` (パスは `src/config.rs` で確認) に `[cleaner.npm]` / `older_than_days = 30` を書く
2. `RUST_LOG=warn sasurahime scan` を実行する。npm は factory が設定を無視するので警告は出ない
3. 同じ設定を `[cleaner.act]` に書くと、act は `with_config` を通るが、DeleteDirs か Command かで挙動が変わる

### 想定する修正方針
- 推奨案 A: 起動時 (設定読み込み直後) に、`per_cleaner` の各キーを cleaner 一覧と突き合わせる純粋関数 `ineffective_cleaner_settings(config, cleaners) -> Vec<String>` を作る。method の種別は `Cleaner` trait を増やさず、「per-cleaner フィルタに対応する cleaner 名の集合」を `generic.rs` が公開関数で返す方式にする (DeleteDirs の名前一覧を `GenericCleaner` から取る)。理由: trait を触らない。設計書 04 の S2 が trait を変更するため衝突を避けられる。
- 案 B: `Cleaner` trait に `supports_filters(&self) -> bool` を追加する。判定は自然だが、全 cleaner 実装に既定値を足す変更になり、04 と同じ trait を触る。
- 案 C: すべての factory に `.with_config(config)` を付けて警告を個別に出す。却下: 29 件を触るうえ、warn が cleaner ごとに散らばる要件に反する。

### 落とし穴
- `all_cleaners()` は scan / TUI / `--yes` で使われ、`clean <target>` は `dispatch_clean` (マクロ) を使う。入口が複数あるので、警告は `main` の設定読み込み直後の 1 か所に置かないと重複または漏れが出る
- `log::warn!` は `RUST_LOG` 未設定だと出ない可能性がある。ユーザー向けの警告は `eprintln!` にするかを決める
- 未知の cleaner 名の判定には「CLI 名」と `Cleaner::name()` の対応が要る (colima のように enum 名と文字列が違うものがある)。`targets` コマンドの一覧と同じ元データを使う
- `with_config` の既存テスト (`generic.rs` 1295 行付近) を壊さない

### 検証コマンド
```bash
cargo test per_cleaner
cargo test --test '*' warn
cargo clippy -- -D warnings
cargo fmt --check
```

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] ドキュメント (設定ファイルの説明) に「DeleteDirs 方式のみ有効」が明記されている
