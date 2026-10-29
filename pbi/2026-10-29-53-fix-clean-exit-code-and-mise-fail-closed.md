# PBI: clean() の終了コード検証 + mise fail-closed（全ランタイム削除リスクの解消）

## ユーザーストーリー
キャッシュを清掃する開発者として、清掃結果の解放量が実際の削除を反映してほしい。なぜなら、偽の数字でディスク判断をするのは無意味だから。また、ツール出力の書式が変わっても全ランタイムが削除対象になってほしくない。なぜなら、取り返しのつかない削除だから。

## 優先度
- 順位: 53 / 全 8 候補
- RICE スコア: 20.0（Reach=5 / Impact=3 / Confidence=100% / Effort=0.75 人週）
- 根拠: detect() は終了コードを丁寧に分類するのに clean() は 7 サイトで無視する（rustup.rs:114/135、ollama.rs:169、brew.rs:97、uv.rs:129、hint/cleaner.rs:21/64、apfs_snapshot.rs:426）。失敗でも freed += size と removed 表示で履歴に偽が記録される。mise.rs:61-71 は fail-open パースで、書式変更により active セットが空になり全インストール済みランタイムが削除対象になる（実検証済み）。依存: 52（ollama.rs を共有するため先に着地）の後

## ビジネス価値
- 履歴・stats の数字が実際の削除を反映し、ユーザーのディスク判断が信頼できる
- mise の書式変更が全ランタイム削除に直結するリスクを fail-closed で塞ぐ
- brew の文言・ロケール変更が silent 0 になる PBI-51 同型バグを解消

## BDD 受け入れシナリオ
Scenario: mise の出力書式が変わっても全ランタイムを削除しない
  Given mise がインストール済みで ~/.local/share/mise/installs に 3 バージョンある
  When  mise ls の出力書式が変わり active セットのパースが空になる
  Then  clean は何も削除せず Unavailable または警告で終了する

Scenario: 失敗した削除は解放量に含めない
  Given rustup がインストール済みで toolchain remove が失敗する
  When  clean rustup を実行する
  Then  freed にそのツールチェーンのサイズを含めず、SkippedEntry として記録する

Scenario: 失敗した ollama rm は removed と出力しない
  Given ollama がインストール済みで ollama rm が非ゼロで終わる
  When  clean ollama を実行する
  Then  removed 表示を出さず、SkippedEntry として記録し履歴に偽を書かない

## 受け入れ基準
- [x] mise: installs_dir が非空 && active セットが空 -> 何も削除せず Unavailable（fail-closed）
  - 実装: output_unparseable()（active 空 && stdout 非空 -> true）。clean は削除ゼロで refuse 行、detect は Unavailable。検証: mise 単体 7 本 + E2E
- [x] rustup.rs:114/135、ollama.rs:169、brew.rs:97、uv.rs:129、hint/cleaner.rs:21/64、apfs_snapshot.rs:426 が status.success() を検証する
  - 実装: rustup toolchain list/remove、ollama rm、brew cleanup、uv cache prune、osascript quit/relaunch、tmutil thinlocalsnapshots の各サイトで終了コード検証
- [x] 失敗した削除は freed に含めず SkippedEntry として記録される
  - 実装: remove_toolchain/remove_model が Ok/Err を返し、呼び出し側は SkippedEntry + continue
- [ ] brew のパース取りこぼし（"freed approximately" の文言変更）が Unavailable になる
  - **逸脱（記録）**: Unavailable にはせず警告にした。brew の stdout 以外に freed の真実源がなく、文言変更時に Unavailable を出すと実行された cleanup を「実行不能」と誤報する。cache が縮んだ事実（before/after 差分）を警告に添えて報告する（"[brew] freed-bytes line could not be parsed, but the cache shrank by ..."）
- [x] 各サイトに失敗経路のテストがある（fake runner で非ゼロ終了を注入）
  - 検証: rustup +4、ollama +2、brew +2、mise +7（非ゼロ終了・出力不解析の注入テスト）

## テスト戦略
- E2E: fake mise が空 active を出すと全ランタイムが削除されない（既存 spawn-fail テスト mise.rs:296-299 の隣に semantic-empty 版）
- 統合: fake runner で非ゼロ終了を注入し、freed に含まれないことと SkippedEntry を検証
- 単体: parse_active_versions の取りこぼし（unparsable 行が黙って落ちる）を fail-closed 判定と組み合わせて検証

## 実装アプローチ
- `ensure_success(name, &output) -> Result<()>`（または is_skippable_error 風の分類）を全 runner.run の後に呼ぶ 1 ヘルパーを generic.rs か cleaner.rs に置き、7 サイトから呼ぶ
- ollama/rustup の per-item ループは盲目的な freed += size の代わりに SkippedEntry を push
- mise は clean 冒頭に installs 非空 && active 空 -> Unavailable のガード
- 判断の根拠: batch.rs:80 の失敗機構と SkippedEntry は実在するので、新しい機構ではなく既存シームへの接続で足りる

## 見積もり
5

## 技術的考慮事項
- ensure_success の失敗は Skippable に分類するか致命的にするかはサイトごとに決める（ollama rm の個別失敗は SkippedEntry、mise ls のパース空は Unavailable）
- is_skippable_error（cleaner.rs:150-164）の文字列分類に乗せるか、型付き DeleteError（59）に乗せるか。本 PBI では既存の anyhow 文言分類を維持し、59 で置き換える

## 実装者向け注記
現状確認:
```
grep -n "status.success\|\.success()" src/cleaners/rustup.rs src/cleaners/ollama.rs src/cleaners/brew.rs src/cleaners/uv.rs src/hint/cleaner.rs src/cleaners/apfs_snapshot.rs
grep -n "freed += size\|freed += \|total += " src/cleaners/rustup.rs src/cleaners/ollama.rs
grep -n "parse_active_versions" src/cleaners/mise.rs
```
手順: (1) ヘルパーを追加、(2) 7 サイトで終了コード検証、(3) mise の fail-closed ガード、(4) per-item ループの SkippedEntry 化。落とし穴: ollama.rs と rustup.rs は 52 の TTY ゲードと同じファイルなので先に着地させる。freed += size の前に検証を置かないと偽の解放量が残る。

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
  - 検証: 各 PBI のテスト戦略に沿ったテストが実装され緑
- [x] mise の fail-closed テストが緑
  - 検証: mise 単体 7 本 + E2E 緑
- [x] 7 サイトすべてに終了コード検証がある
  - 検証: rustup/ollama/brew/uv/hint/apfs_snapshot の各サイトに status 検証
- [x] CHANGELOG の [Unreleased] に追記
  - 検証: CHANGELOG.md [Unreleased] に各バッチのエントリあり
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
