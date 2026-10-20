# PBI: アンインストール済みアプリの Containers 残骸を検出してゴミ箱へ送る

## ユーザーストーリー
macOS の開発者として、アンインストール済みアプリが `~/Library/Containers` と `~/Library/Group Containers` に残した残骸を一覧し、自分で選んだものだけをゴミ箱へ送りたい。なぜなら、Containers は数百件になり、アプリを消しても残骸が残って「システムデータ」を圧迫するのに、どれが不要か手作業では判別できないから。

## ビジネス価値
- 見えにくい残骸領域を、誤検出を避けつつ (誤検出より見逃しを選ぶ) 候補化する
- `--yes` では絶対に実行せず、利用者が選んだものだけを削除する安全設計
- `AppIdentity` (bundle ID とアプリ名のみ) を共通部品として用意し、PBI-06/07 (Application Support) が再利用できる
- 測定方法: 実機の `--dry-run` で候補が列挙される。E2E 5 件と単体テストで判定を固定

## BDD受け入れシナリオ

```gherkin
Scenario: インストール済みアプリの子孫と保護対象は候補にならない
  Given インストール済み bundle ID が com.foo.app だけで、Containers に com.foo.App, com.foo.App.Helper, com.foo.AppX, com.apple.Safari, com.docker.docker, com.bar.Gone がある
  When  sasurahime clean orphan-containers --dry-run を実行する
  Then  候補は com.foo.AppX と com.bar.Gone だけである
  And   何も削除されない

Scenario: Group Containers のチーム ID 接頭辞を扱う
  Given インストール済みが com.tinyspeck.slackmacgap で、Group Containers に BQR82RBBHL.com.tinyspeck.slackmacgap, BQR82RBBHL.slack, ZZZZZ99999.group.com.example.gone がある
  When  --dry-run を実行する
  Then  候補は ZZZZZ99999.group.com.example.gone だけである
  And   同じチーム ID のインストール済みアプリがある BQR82RBBHL.slack は除外される

Scenario: ドット境界で別ベンダを誤一致させない
  Given インストール済みが com.foo.app で、Containers に com.foo.AppX と com.applesauce.x がある
  When  --dry-run を実行する
  Then  com.foo.AppX と com.applesauce.x は候補になる (com.apple と com.docker だけが子孫まで保護される)

Scenario: --yes では実行されない
  Given orphan-containers が登録されている
  When  run_auto が対象を選ぶ
  Then  auto_clean_allowed() が false のため orphan-containers は対象外である

Scenario: --all は存在しない
  When  sasurahime clean orphan-containers --all を実行する
  Then  引数エラーで終了コードが 0 以外になる

Scenario: 読めないフォルダは候補に出さず案内する
  Given Full Disk Access が無く一部のフォルダが読めない
  When  clean を実行する
  Then  読めなかった件数の案内が出て、それらは候補に含まれない

Scenario: Info.plist の他の値は出力にもログにも出ない
  Given Info.plist に CFBundleURLTypes と任意のキーがある
  When  RUST_LOG=trace で --dry-run を実行する
  Then  bundle ID とアプリ名以外の値が stdout にも stderr にも現れない
```

## 受け入れ基準
- [ ] CLI 名 `orphan-containers`。マクロ表の標準エントリ 1 つだけで登録され、`targets` が 49 行になる
- [ ] `AppIdentity { bundle_id, name, display_name }` の 3 フィールドだけ。`Info.plist` から取り出す値はこの 3 つに限る。任意のキーを取る公開関数を作らない
- [ ] plist は `plist` クレートを使わない。XML は文字列検索、バイナリは `plutil -convert xml1 -o -` (runner 経由) を使う。新規依存なし
- [ ] `auto_clean_allowed()` が `false`、`--all` なし、`sub_targets()` なし、非 TTY では何も削除しない
- [ ] 削除は `chflags -R nouchg` の後 `crate::trash::delete_path` (既定でゴミ箱)。失敗は skipped、全件失敗なら終了コード 1
- [ ] README の Data access に Containers の 1 行 (英日) と `Info.plist` の項目 (英日) を足す (同じ PBI 内で行う)
- [ ] `docs/SUPPORTED.md` の件数 49、新節 `## 49.`、stats は `## 50.`、`docs/HOWTO-USE.md` の表に 1 行

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `tests/orphan_containers.rs` の 5 件 (tempdir の HOME、`--dry-run` のみ。`rejects_all_flag` と `info_plist_values_other_than_identity_never_reach_output_or_logs` を含む)

### 統合テスト
- `scan_*` (シンボリックリンク、サイズ 0、読めない root、チーム内一致)、`detect_without_roots_is_not_found`、`collect_installed_ids_scans_top_level_and_one_subfolder`、`delete_selected_records_failures_as_skipped`

### 単体テスト
- 設計書 3.1 節の入出力例の表をそのままテスト化: `normalize_container_id`、`is_protected_id`、`is_owned_by_installed`、`find_orphan_names`、`parse_app_identity_from_xml`
- main.rs の `orphan_containers_is_registered_but_not_auto_cleaned`、cleaner の `orphan_containers_is_never_auto_cleaned`

## 実装アプローチ
- **Outside-In**: Task 5 の E2E の振る舞い (dry-run の一覧) を意識しつつ、設計書のとおり Task 1 の純関数から順に積む。各タスクは失敗するテストから
- **Red-Green-Refactor**: 1 タスク 1 コミット (計 6 コミット)
- **リファクタリング**: plist の読み取りを `collect_app_identities` の 1 か所に保つ

## 見積もり
8 ストーリーポイント (根拠: 設計書のタスク 6 本。名前正規化の規則が多く、plist 読み取り、走査、削除、E2E、README を含む最大級の新 cleaner。1 スプリントに収まる上限)

## 技術的考慮事項
- 依存関係: PBI-02 (S2 と S3 が前提)。ロードマップの固定順により、PBI-03 (04 の全タスク) の完了後に着手する (docs の件数 48 と README の Docker の行が前提のため)
- テスタビリティ: `with_app_dirs` (`#[cfg(test)]`) で `/Applications` を注入。外部コマンドは `CommandRunner` で fake 化
- 非機能: detect は `println!`/`eprintln!` を出さない (並列 scan が崩れる)。読めない件数の案内は clean だけ

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-02-orphan-containers-design.md`: 前提と未検証の仮定、1.3 節 (実機の事実)、2.1 節 (設計判断)、3.1 節 (入出力例)、Task 1 から Task 6、6 章 (エッジケース)、7 章 (禁止事項とやりがちな間違い)、8 章 (完了条件)
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` (1.4, 1.6, 2.2, 11.1, 11.2)
- ロードマップ 7.4 節 (README の完成形)

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"            # 全件 ok
grep -n "fn auto_clean_allowed" src/cleaner.rs                     # 1 行
grep -n "fn standard_cleaners" src/main.rs                         # 1 行
cargo run -q -- targets | wc -l                                    # 48
grep -n "48 clean targets" docs/SUPPORTED.md                       # 1 行
grep -n "com.docker.docker/Data/" README.md                        # 2 行
grep -n "^- mise configuration files" README.md                    # 1 行
ls src/cleaners/orphan_containers.rs                               # No such file なら未実装
grep -rn "AppIdentity\|orphan" src | head                          # 出力なしなら未実装
```
どれか期待と違えば着手せず報告する。

### 前提となる PBI
PBI-02 (コード上の前提)。固定順により PBI-03 の完了後。

### 実装の進め方
設計書の Task 1 から Task 6 を順に、1 タスク 1 コミット。Task 1, 3, 4 のコードは設計書から写す。Task 2 と Task 5 の main.rs 変更は未コンパイルなので、エラーが設計書由来で 2 回の修正で直らなければ止まって報告する。

### 落とし穴
- `AppIdentity` にフィールドを足さない。plist の値を `log::*!`、`println!`、`eprintln!` に渡さない
- `plutil -convert` は必ず `-o -` を付ける (付けないと元の Info.plist が書き換わる)
- 前方一致は `starts_with` だけにせず、ドット境界を見る (`com.foo.AppX` の誤一致)
- チーム ID は「ちょうど 10 文字の英大文字と数字」だけ。文字列を `&s[..n]` で切らず `str::get` を使う (非 ASCII で panic する)
- `with_app_dirs` から `#[cfg(test)]` を外さない (dead_code で clippy が落ちる)
- `--all`、`sub_targets()`、`AUTO_CLEAN_TARGETS` への追加、`main.rs` への除外処理を作らない。特殊変種として二重登録しない (clap が panic する)
- `detect()` で出力しない。`clean(false)` を単体テストで呼ばない (対話が始まる)。`--dry-run` なしの E2E を書かない
- `README` の更新を別 PBI に回さない (Task 6 で行う)
- 非 TTY 案内文は ASCII のハイフンにする

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo test --test orphan_containers
cargo run -q -- targets | wc -l                                    # 49
cargo run -q -- clean orphan-containers --dry-run                  # 実機の ~/Library を読むだけ。dry-run のみ
grep -n "Info.plist" README.md                                     # 2 行
git log --oneline -6
git status --short
```
`sasurahime clean orphan-containers` を `--dry-run` なしで実行しない。`sasurahime --yes` も実行しない。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 設計書 8 章の完了条件チェックリストをすべて満たす
- [ ] 6 コミット (Task 1 から 6)、日本語 Conventional Commits、個別 add
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告
- [ ] コードレビュー完了
- [ ] リファクタリング完了
- [ ] ドキュメント更新済み (README 英日、SUPPORTED.md、HOWTO-USE.md)
