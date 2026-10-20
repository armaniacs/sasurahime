# PBI: Application Support の既知アプリのキャッシュだけを許可リスト方式で安全に削除する (app-support 第1段)

## ユーザーストーリー
macOS の開発者として、`~/Library/Application Support` にある Slack、Claude、Obsidian、VS Code、Chrome のキャッシュ系サブディレクトリだけを、一覧から選んで安全に削除したい。なぜなら、これらは数 GB になって「システムデータ」を圧迫するが、同じ階層に設定やログイン情報、IndexedDB などのユーザーデータもあり、手作業や雑な `rm -rf` では取り返しのつかない事故になりうるから。

## ビジネス価値
- 肥大化の大きな原因を、ユーザーデータに触れずに回収できる
- 許可リスト方式、パストラバーサルとシンボリックリンクへの対策、削除直前の再検証により、削除範囲を構造的に限定する
- 実行中アプリは勝手に終了せず、利用者の承認を得る
- 測定方法: 許可リスト回避の攻撃的テスト (トラバーサル、リンク、偽造) がすべて拒否されること、`--dry-run` が副作用ゼロであること

## BDD受け入れシナリオ

```gherkin
Scenario: 許可リストのキャッシュだけが候補になる
  Given AS/Slack に Cache (50 MB)、Code Cache (20 MB)、Local Storage (500 MB)、IndexedDB (500 MB) がある
  When  sasurahime clean app-support --dry-run を実行する
  Then  候補は Cache と Code Cache だけである (合計 70 MB が閾値 64 MiB を超えるため出る)
  And   Local Storage と IndexedDB は一覧に出ない
  And   何も削除されない

Scenario: 合計が閾値以下のアプリは出さない
  Given Slack の許可サブパスの合計が 64 MiB 以下である
  When  --dry-run を実行する
  Then  Slack の候補は出ない

Scenario: パストラバーサルと絶対パスは拒否される
  When  resolve_allowed に "../Code/Cache" や "/etc" や "Local Storage" を渡す
  Then  すべて Err(NotAllowed) になる (実在していても)

Scenario: シンボリックリンクは辿らない
  Given AS/Slack が外部へのシンボリックリンクである
  When  resolve_allowed(base, slack, "Cache") を呼ぶ
  Then  Err(Symlink) になり候補にならない
  And   キャッシュ内にリンクがあっても、リンク先は削除されない

Scenario: 走査後に差し替えられたディレクトリは削除されない
  Given 走査後に AS/Slack/Cache がシンボリックリンクに差し替えられた
  When  削除を実行する
  Then  削除前の再検証で拒否され、skipped に載る

Scenario: 偽造された候補では許可リスト外を消せない
  Given Candidate の path や sub を許可リスト外 (Local Storage) に偽造した
  When  削除を実行する
  Then  何も削除されない (削除に使うのは resolve_allowed の戻り値だけ)

Scenario: 実行中アプリは承認が無ければ削除しない
  Given Slack が実行中で、利用者が終了を承認しない
  When  Slack のキャッシュを選んで実行する
  Then  Slack の候補は skipped (理由 "Slack is running") になり、osascript は呼ばれない

Scenario: 承認すると終了、削除、再起動の順に行う
  Given Obsidian が実行中で、利用者が終了を承認する
  When  実行する
  Then  quit_app_and_wait が呼ばれ、削除後に Obsidian だけ open -a で再起動される
  And   実行中でなかったアプリは再起動されない

Scenario: --yes では何も消えない
  When  run_auto が対象を選ぶ
  Then  app-support は auto_clean_allowed() が false のため clean() が呼ばれない

Scenario: 非 TTY では何も削除しない
  Given 標準入力が TTY でない
  When  dry-run なしで clean app-support を実行する
  Then  "not a terminal" と表示され何も削除されず終了コード 0
```

## 受け入れ基準
- [ ] CLI 名 `app-support`。マクロ表の標準エントリ 1 つで登録され、`targets` が 51 行になる
- [ ] 許可リスト (設計書 3.1 節の表) の外は一切削除できない。禁止例 (Local Storage、IndexedDB、Partitions、Cookies、Session Storage、User、Backups、Workspaces、Default/Login Data、Default/Extensions、Default/Service Worker 全体) を入れない。許可サブパス同士に祖先関係がない
- [ ] `resolve_allowed` が NotAllowed、Symlink、NotDirectory、NotFound、Unreadable を返し、root から末端まですべての構成要素のリンクを拒否する
- [ ] 削除は候補ごとに `resolve_allowed` を再実行した結果だけを使う (`Candidate.path` を使わない)。`chflags -R nouchg` の後 `crate::trash::delete_path`
- [ ] 閾値、pgrep 名、quit_app、relaunch_app は hint の `HintEntry` から引き、表と一致することをテストで保証する
- [ ] `detect()` は軽量 (pgrep も plutil も呼ばない)、`--dry-run` は副作用ゼロ (osascript、chflags、削除、Finder 起動なし)
- [ ] `auto_clean_allowed()` が `false`。MultiSelect の既定は全部未選択。終了コードは `CleanResult::exit_code()` に従う
- [ ] `hint::auto_clean_hint` は呼ばない。終了と再起動の部分だけ `quit_app_and_wait` / `relaunch_app` として抽出し共有する (既存 hint テストが回帰テスト)
- [ ] SUPPORTED.md の件数 51、新節 `## 51.`、stats は `## 52.`、HOWTO-USE.md の表に 1 行。README は変更しない (既存の Data access の約束に収まるため)

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `tests/app_support.rs`: `--dry-run` と非 TTY のみ (`non_interactive_clean_deletes_nothing`、Application Support が無い場合の成功を含む)。HOME は tempdir。`--yes` を E2E で実行しない

### 統合テスト
- `scan_returns_only_allowlisted_dirs`、`scan_skips_app_below_threshold`、`scan_excludes_symlinked_cache_dir`、`scan_skips_empty_dirs_and_missing_base`、`apply_*` 系 (`apply_blocks_running_app_without_approval`、`apply_quits_approved_running_app_then_deletes_and_relaunches`、`apply_does_not_relaunch_app_that_was_not_running`、`apply_does_not_follow_symlinks_inside_cache`、`apply_rejects_dir_replaced_by_symlink_after_scan`、`apply_never_deletes_forged_unlisted_sub`、`apply_ignores_forged_path_field`)。ゴミ箱を使うテストは先頭で `set_trash_mode(false)`、末尾で `true`

### 単体テスト
- `resolve_rejects_path_traversal_and_absolute`、`resolve_rejects_symlinked_root`、`resolve_rejects_symlinked_leaf`、`resolve_rejects_symlinked_middle_component`、`resolve_reports_unreadable_parent`、`targets_cover_every_hint_suffix`、許可リスト定数の検査、`detect_is_not_found_without_application_support`、`app_support_is_never_auto_cleaned`、`app_support_is_registered_but_not_auto_cleaned`

## 実装アプローチ
- **Outside-In**: Task 6 の E2E の振る舞いを意識しつつ、設計書の順 (Task 1 の抽出、Task 2 の定数、Task 3 のパス検査) で内側から固める。セキュリティ上重要な Task 3 と Task 5 は攻撃的テストを先に書いて失敗させる
- **Red-Green-Refactor**: 1 タスク 1 コミット (計 7 コミット)
- **リファクタリング**: Task 1 の抽出後は既存 hint テストで回帰を確認

## 見積もり
8 ストーリーポイント (根拠: 設計書のタスク 7 本。Task 5 の Cleaner 本体が約 640 行相当と最大で、パス検査と実行中アプリの扱い、hint の関数抽出を含む高リスクのため、スプリント上限相当)

## 技術的考慮事項
- 依存関係: PBI-05 (library-caches)。`hint::is_running` の `pub(crate)` 化と `pub(crate) use` は PBI-05 の Task 1 で済んでいること。本 PBI は `is_running` の可視性を変えない。ロードマップ上の固定順 (03 -> 05)
- セキュリティ: パス検査は文字列ではなく構成要素ごとに行い、リンクを辿らない。TOCTOU を削除直前の再検証で緩和する
- 非機能: ユーザーデータの保全が最優先。許可リストへの追加は 3.2 節の規則 (hint/apps.rs へ先に足す) に従う

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-05-app-support-design.md`: 前提、1 章 (特に 1.2 非目標と 1.3 の境界)、3 章 (許可リスト)、4 章 (公開する型、入出力例、ふるまい)、5 章の共通ルールと Task 1 から Task 7、6 章、7 章 (エッジケース)、8 章 (やりがちな間違い、禁止事項)
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` の 1.4, 2.2, 3.1, 4.4, 7.3, 7.4

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"                      # 全件 ok
grep -n "LibraryCaches :" src/main.rs                                        # 1 行 (PBI-05 完了)
grep -n "pub(crate) fn is_running" src/hint/detector.rs                      # 1 行
grep -n "pub(crate) use detector::is_running;" src/hint/mod.rs               # 1 行
grep -n "pub struct AppIdentity" src/cleaners/orphan_containers.rs           # 1 行 (PBI-04 完了)
cargo run -q -- targets | wc -l                                              # 50
ls src/cleaners/app_support.rs                                               # No such file なら未実装
grep -rn "app-support\|AppSupport :\|quit_app_and_wait" src | head           # 出力なしなら未実装
```
どれか期待と違えば着手せず報告する。

### 前提となる PBI
PBI-05 (library-caches。その前提として PBI-04 も完了済み)。

### 実装の進め方
設計書の Task 1 から Task 7 を順に、1 タスク 1 コミット。Task 1 では `auto_clean_hint` の呼び出し順と出力文言を変えない。テストでファイルを消すものは `set_trash_mode(false)` を先頭で呼び末尾で戻す (設計書のコードには書かれていないので足す)。第2段 (Task 8 以降) は PBI-07。

### 落とし穴
- 削除に `Candidate.path` を使わない (必ず `resolve_allowed` の戻り値)
- `hint::cleaner::auto_clean_hint` を呼ばない (`rm -rf` で許可リストを無視する)
- 許可リストに `Service Worker` や `Default` を丸ごと入れない (Database、Login Data を含む)
- `slice.contains(&sub)` は寿命でコンパイルエラーになる。`iter().any(|s| *s == sub)` を使う
- fake runner の失敗終了コードは `from_raw(256)` (1 はシグナル終了の意味になる)
- E2E で `--yes` を実行しない (他の cleaner が実機のツールを呼ぶ)。`--yes` 非実行は単体テストで確認
- マクロの標準エントリに加えて特殊変種にも登録しない (clap が panic)。`AUTO_CLEAN_TARGETS` に足さない。除外リストを `main.rs` に作らない
- `mod cleaner;` と `mod detector;` を `pub(crate) mod` にしない。`is_running` の可視性を変えない。`explorer.rs` を変更しない
- 実機の `~/Library` を触るテストを書かない

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo run -q -- targets | wc -l                                  # 51 (第2段込みの最終値。第1段のみなら 51 のまま)
cargo run -q -- targets 2>/dev/null | grep -c '^app-support'     # 1
cargo run -q -- clean app-support --dry-run                      # 実機は dry-run のみ
git status --short
```
`--dry-run` なしの `clean app-support` と `sasurahime --yes` は実機で実行しない。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 設計書 8.3 の完了条件のうち第1段 (Task 1 から 7) の項目を満たす
- [ ] 7 コミット、日本語 Conventional Commits、個別 add
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告
- [ ] コードレビュー完了 (特にパス検査と削除経路)
- [ ] リファクタリング完了
- [ ] ドキュメント更新済み (SUPPORTED.md、HOWTO-USE.md)
