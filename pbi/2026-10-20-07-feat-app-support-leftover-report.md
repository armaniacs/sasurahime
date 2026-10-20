# PBI: アンインストール済みアプリの Application Support 残骸を表示し、Finder で開けるようにする (app-support 第2段)

## ユーザーストーリー
macOS の開発者として、`~/Library/Application Support` にあるアンインストール済みアプリの残骸らしきフォルダを一覧し、気になるものを Finder で開いて自分で確認したい。なぜなら、ユーザーデータを含む可能性があり、ツールが自動で消すのは危険だが、どこに何が残っているか分からないと容量の整理ができないから。

## ビジネス価値
- 「システムデータ」の中身が分からない問題に対し、残骸候補をサイズ付きで見える化する (削除はしない)
- 削除という不可逆な操作を避け、Finder 起動までに留めて誤削除のリスクを無くす
- 測定方法: 残骸が一覧に出ること、`detect()` の合計に含まれないこと、何も変更されないこと

## BDD受け入れシナリオ

```gherkin
Scenario: インストール済みアプリのフォルダは残骸にならない
  Given /Applications に Spotify.app があり、Application Support に Spotify と GoneApp (各 2 MiB、60 日前に更新) がある
  When  sasurahime clean app-support --dry-run を実行する
  Then  残骸一覧に GoneApp が出る
  And   Spotify は出ない

Scenario: 照合は名前と bundle ID の両方で行う
  Given Application Support に slackmacgap があり、インストール済みに bundle ID com.tinyspeck.slackmacgap がある
  When  残骸を検出する
  Then  末尾要素の一致により slackmacgap は残骸にならない
  And   "Google Chrome" と "zoom.us" のような綴りの揺れは正規化 (googlechrome, zoomus) で一致する

Scenario: 除外条件に当たるものは出さない
  Given 1 MiB 未満のフォルダ、30 日以内に更新されたフォルダ、シンボリックリンク、通常ファイル、com.apple.* と AddressBook、ベンダー親フォルダ (Google, Microsoft, Adobe, JetBrains) がある
  When  残骸を検出する
  Then  どれも一覧に出ない

Scenario: 残骸は detect の合計に含まれない
  Given 残骸が 1 件以上ある
  When  detect() を呼ぶ
  Then  合計は許可リストのキャッシュ分だけで、残骸のサイズを含まない

Scenario: dry-run は何も変更せず、コマンドも実行しない
  When  --dry-run を実行する
  Then  残骸は表示だけされ、open も削除も行われない

Scenario: 5 件までは確認なしで Finder に開く
  Given 利用者が残骸を 3 件選んだ
  When  Finder で開く
  Then  runner 経由で open が 3 回呼ばれ、Confirm は出ない

Scenario: 6 件以上は開く前に 1 回確認する
  Given 利用者が残骸を 6 件選んだ
  When  Finder で開く
  Then  "This opens 6 Finder windows. Continue?" の Confirm (既定 No) が 1 回出る
  And   拒否すると 1 件も開かず "[app-support] nothing opened" と表示される

Scenario: 0 件選択では何も起きない
  Given 利用者が何も選ばなかった
  When  Finder で開く
  Then  確認も open も出ない
```

## 受け入れ基準
- [ ] 残骸は表示のみ。削除、移動、書き込みを一切しない。`detect()` の合計に含めない
- [ ] 残骸一覧の MultiSelect の既定は全部未選択。選んだものを `open <path>` で開く
- [ ] `open` は `self.runner.run("open", ...)` 経由で呼ぶ (`explorer::open_in_finder` を呼ばない。`src/explorer.rs` を変更しない)
- [ ] `OPEN_CONFIRM_THRESHOLD: usize = 5` と純関数 `open_needs_confirmation(selected) -> bool` (`selected > 5`) で判定する。0 件は確認も open もなし、1 から 5 件は確認なしで全件、6 件以上は 1 回 Confirm (既定 No)、拒否なら 1 件も開かない
- [ ] `Info.plist` は自分で読まない。02 の `collect_app_identities` が返す `AppIdentity` (bundle ID、name、display_name) だけを使う。値を println/log に出さない。`plutil` を直接呼ばず `serde_json` も使わない
- [ ] 除外条件: 1 MiB 未満、30 日以内に更新、シンボリックリンク、通常ファイル、Apple 製、ベンダー親フォルダ、第1段の既知アプリ
- [ ] README は変更しない (既存の Data access の約束に収まることを grep で確認する)。SUPPORTED.md に除外条件の記述を足す

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `dry_run_shows_leftovers_without_deleting` など。HOME は tempdir で `/Applications` は実機を読み取りのみ (`ZzzNoSuchApp12345` に一致するアプリは無い前提)。`Confirm` は TTY が必要なので E2E では出さない

### 統合テスト
- `leftovers_skips_small_and_recent_and_future`、`leftovers_skips_excluded_names`、`leftovers_skips_files_and_symlinks`、`leftovers_never_modify_anything`、`duplicate_app_names_still_match_once_or_more`、`cleaner_leftovers_use_injected_app_roots`、`collect_installed_apps_falls_back_to_stem_when_plutil_fails`、`collect_installed_apps_never_takes_other_plist_values`、`detect_ignores_leftovers`、`dry_run_touches_nothing_and_runs_no_commands`

### 単体テスト
- `normalize_name`、`is_excluded_dir_name`、`matches_installed` (設計書 4.2 節の入出力例の表)、`open_confirmation_is_needed_only_above_five`、`reveal_paths_runs_open_once_per_path_through_runner`、`reveal_paths_with_nothing_selected_runs_no_command`、`leftover_line_shows_path_and_size`

## 実装アプローチ
- **Outside-In**: Task 12 の clean 組み込みの観点 (一覧と Finder 起動) から逆算し、設計書の順 (Task 8 の前提確認、Task 9 の純関数、Task 10 の収集、Task 11 の検出) で内側から積む
- **Red-Green-Refactor**: 1 タスク 1 コミット (計 6 コミット)
- **リファクタリング**: 判定を純関数に分け、`Confirm` の対話部分を薄く保つ

## 見積もり
5 ストーリーポイント (根拠: 設計書のタスク 6 本 (Task 8 から 13)。純関数中心で削除を伴わず、既存の `AppIdentity` を再利用するため PBI-06 より低リスク。Task 12 の対話組み込みがやや重い)

## 技術的考慮事項
- 依存関係: PBI-04 (`AppIdentity` と `collect_app_identities`) と PBI-06 (第1段の `AppSupportCleaner`)。第2段は第1段の完了後に同じ流れで続けて実装する
- テスタビリティ: `with_app_roots` で `/Applications` を注入する。付けないと実機を読み `plutil` 呼び出しが記録されてテストが失敗する
- 非機能: Finder のウィンドウが大量に開かないよう 6 件以上で確認。explorer との二重実装は意図したもので、共通化は PBI-08

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-05-app-support-design.md`: 前提、1.3 節 (第1段と第2段の境界)、4.2 節 (入出力例)、4.3 節 (ふるまい。Finder 起動の閾値)、Task 8 から Task 13、7 章 (エッジケース)、8 章 (やりがちな間違い、禁止事項、完了条件)
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` の 3.1, 9 章, 11.2
- `docs/superpowers/specs/2026-10-20-02-orphan-containers-design.md` の 3 章 (`AppIdentity` の定義)

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"                         # 全件 ok
cargo run -q -- targets 2>/dev/null | grep -c '^app-support'                  # 1 (PBI-06 完了)
cargo run -q -- targets | wc -l                                               # 51
grep -n "pub struct AppIdentity" src/cleaners/orphan_containers.rs            # 1 行
grep -n "pub fn collect_app_identities" src/cleaners/orphan_containers.rs     # 1 行
grep -n "fn find_leftovers\|OPEN_CONFIRM_THRESHOLD\|fn open_needs_confirmation" src/cleaners/app_support.rs   # 出力なしなら未実装
grep -n "Info.plist" README.md                                                # 2 行
```
どれか期待と違えば推測で合わせず止まって報告する (Task 8 の前提確認の方針)。

### 前提となる PBI
PBI-04 と PBI-06。

### 実装の進め方
設計書の Task 8 から Task 13 を順に、1 タスク 1 コミット。Task 8 の前提確認が通らなければ作業を止める。Task 13 で README の確認 (変更はしない) と最終ゲートを行う。

### 落とし穴
- 本書のコードで `Info.plist` を読まない (`plutil -convert json`、`serde_json`、`fs::read` を使わない)。`AppIdentity` の値を `println!`、`eprintln!`、`log::*!` に出さない
- `crate::explorer::open_in_finder` を呼ばない、`src/explorer.rs` を変更しない
- 閾値 5 を定数を使わず直書きしない。6 件以上で確認を省かない、5 件以内で確認を出さない。拒否されたときに最初の 5 件だけ開く実装にしない
- `Confirm` を単体テストや E2E で動かそうとしない (TTY が必要で止まる)。判定関数と `reveal_paths` だけをテストする
- 単体テストで `with_app_roots` を付けずに第2段を動かさない
- 残骸の削除や `detect()` の合計への算入をしない
- E2E で `--yes` を実行しない

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo run -q -- targets | wc -l                                              # 51
grep -n 'run("plutil"\|serde_json\|fs::read(' src/cleaners/app_support.rs    # 出力なし
git diff --stat HEAD -- src/explorer.rs src/hint/detector.rs                 # 差分なし
cargo run -q -- clean app-support --dry-run                                  # 実機は dry-run のみ
git status --short
```
`--dry-run` なしの `clean app-support` と `sasurahime --yes` は実機で実行しない。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする (Confirm の対話自体を除く)
- [ ] 設計書 8.3 の完了条件のうち第2段 (Task 8 から 13) の項目を満たす
- [ ] 6 コミット、日本語 Conventional Commits、個別 add
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告
- [ ] コードレビュー完了
- [ ] リファクタリング完了
- [ ] ドキュメント更新済み (SUPPORTED.md に除外条件の記述。README は変更なし)
