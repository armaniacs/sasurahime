# PBI: scan に標準ターゲット全件を表示し、--yes は従来の15件に限る

## ユーザーストーリー
sasurahime のユーザーとして、`sasurahime scan` で docker、npm、cargo など標準ターゲット全件のサイズや状態を見たい。なぜなら、現在は 39 件の標準ターゲットのうち 15 件しか scan に出ず、macOS の「システムデータ」を何が占めているのか全体像をつかめないから。ただし、新しく出る 31 件は `--yes` で勝手に実行されてはならない。

## ビジネス価値
- scan が `sasurahime targets` の標準ターゲットを網羅し、肥大化の原因を 1 回の scan で把握できる
- `all_cleaners()` をマクロ表から生成することで、後続の cleaner (orphan-containers など) が「表に 1 エントリ足すだけ」で scan と TUI に出る
- `--yes` の対象を既存 15 件に固定し、`docker system prune` や `~/Downloads` 削除などの意図しない無人実行を構造的に防ぐ
- 測定方法: `all_cleaners` の名前集合がマクロ表と一致する単体テストと、`--yes` 対象が 15 件であることを固定する単体テスト

## BDD受け入れシナリオ

```gherkin
Scenario: 新規の標準ターゲットが scan と TUI の一覧に載る
  Given マクロ表の標準ターゲットが 39 件ある
  When  all_cleaners(home, config) を呼ぶ
  Then  標準 39 件と特殊 7 件の名前がすべて含まれる
  And   名前に重複がない

Scenario: --yes は従来の 15 件だけを対象にする
  Given all_cleaners に docker、npm、cargo、downloads を含む全標準ターゲットが載っている
  When  run_auto が対象を選ぶ (auto_clean_indices)
  Then  対象は uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support, ios-backup, apfs-snapshot の 15 件だけである
  And   docker や downloads は Pruneable(0) でも対象に入らない

Scenario: ManualOnly で包んでも委譲が壊れない
  Given ManualOnly で包んだ cleaner がある
  When  name、detect、clean を呼ぶ
  Then  内側の cleaner と同じ結果が返り、auto_clean_allowed() だけが false になる

Scenario: config.exclude に書いた cleaner は scan に出ない
  Given config.exclude に "docker" がある
  When  all_cleaners を呼ぶ
  Then  docker は含まれない

Scenario: colima の per_cleaner 警告が維持される
  Given colima に older_than_days を設定した config がある
  When  マクロ表の colima factory から cleaner を構築する
  Then  with_config を通る (警告の判定 unsupported_filter_warning が Some を返す) 
  And   detect() の結果は設定なしのときと変わらない
```

## 受け入れ基準
- [ ] `Cleaner::auto_clean_allowed()` (既定 `true`) と `Cleaner::advisory()` (既定 `None`) が trait にある
- [ ] `ManualOnly` が name、detect、clean、clean_with_opts、sub_targets を内側に委譲し、`auto_clean_allowed()` だけ `false` を返す
- [ ] `run_auto` は `auto_clean_indices` で `auto_clean_allowed()` が `true` のものだけを対象にする
- [ ] `all_cleaners()` は `standard_cleaners()` (マクロ生成) と `special_cleaners()` (手書き 7 件) から組み立てる。`AUTO_CLEAN_TARGETS` 外の標準ターゲットは `ManualOnly` で包まれる
- [ ] 既存 15 件の `--yes` 挙動が変わらない。custom cleaner は従来どおり `--yes` 対象
- [ ] マクロ表の colima の行だけ `.with_config(config)` を付ける。他の行の factory は変えない
- [ ] `sasurahime targets | wc -l` が 48 のまま
- [ ] `exclude` は `name()` で照合する (`cocoapods` は CLI 名 `cocoa-pods` と異なる) 挙動が保たれる

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- S2: `tests/interactive.rs` の `yes_flag_never_runs_manual_only_targets` (偽の cleaner が `--yes` で実行されないこと。実機のツールを呼ばないよう PATH と HOME を差し替える)
- scan に標準ターゲット全件が出る E2E (`scan_lists_every_standard_target`) は PBI-03 の T4 で追加する。本 PBI では下の単体テストで網羅を保証する

### 統合テスト
- `src/main.rs` の `mod tests`: `every_clean_target_is_registered_in_all_cleaners`、`standard_cleaners_follow_the_macro_table_order`、`standard_and_special_cleaners_do_not_overlap`、`only_legacy_targets_are_auto_clean_allowed`、`destructive_targets_are_not_auto_clean_allowed`、`config_exclude_removes_generated_targets`
- S1 の回帰テスト: `all_cleaners_keeps_legacy_targets`、`all_cleaners_names_are_unique` (HEAD で成功する。切替前後で同じであること)

### 単体テスト
- `default_cleaner_allows_auto_clean`、`manual_only_blocks_auto_clean`、`manual_only_delegates_name_detect_and_clean`、`auto_clean_indices_skips_manual_only`
- `src/cleaners/generic.rs`: `colima_with_filter_config_takes_the_warning_branch`、`unsupported_filter_warning_is_none_without_entry_or_for_delete_dirs`、`colima_with_config_keeps_detect_result`

## 実装アプローチ
- **Outside-In**: S1 で現状を固定するテスト、S2 で E2E (`--yes` 保護) を先に書いて失敗を確認し、内側の trait とラッパーを実装する
- **Red-Green-Refactor**: 設計書のタスクごとに「失敗するテスト、失敗確認、実装、成功確認、コミット」
- **リファクタリング**: S3 でマクロの doc コメントを実態に合わせる程度に留める

## 見積もり
5 ストーリーポイント (根拠: 設計書のタスク 3 本 (S1, S2, S3)。S3 は `define_cleaners!` マクロの拡張を含み難度が高く、S2 の順序を誤ると `--yes` が破壊的になる)

## 技術的考慮事項
- 依存関係: PBI-01 (README の事実化) が完了していること
- 順序制約: S2 を必ず S3 より先に完了させる。逆にすると S3 の時点で `--yes` が docker prune や `~/Downloads` 削除を実行する
- テスタビリティ: 偽の cleaner と tempdir の HOME を使う。実機のツールを呼ぶ経路を作らない
- 非機能: scan が遅くなりうる (cargo の HOME 走査など)。並列実行のため最も遅い cleaner に律速される

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-04-docker-and-scan-coverage-design.md`: 2 章 (HEAD の事実。2.5 が最重要)、5 章 (決定事項)、6 章 (依存順序)、7 章 (Task S1)、8 章 (Task S2)、9 章 (Task S3)、15 から 17 章 (エッジケース、やりがちな間違い、禁止事項)
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` (特に 1.4 の dead_code の許容範囲、2.2)
- `docs/superpowers/specs/2026-10-20-01-roadmap-and-order.md` の 2 から 4 章

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"             # 全件 ok
grep -n "fn auto_clean_allowed" src/cleaner.rs                      # 出力なしなら未実装
grep -n "fn standard_cleaners\|const AUTO_CLEAN_TARGETS" src/main.rs  # 出力なしなら未実装
ls src/cleaners/manual_only.rs                                      # No such file なら未実装
cargo run -q -- targets | wc -l                                     # 48
grep -n "It reads the contents of only the following files" README.md   # 1 行 (PBI-01 完了の確認)
```

### 前提となる PBI
PBI-01 (README Data access の事実化)。

### 実装の進め方
設計書のタスクを順に S1, S2, S3 とし、1 タスク 1 コミットにする。各タスクは設計書の「失敗するテストを書く、実行、実装、コミット」の手順に従う。コミットは日本語の Conventional Commits、ファイルは個別に `git add`。

### 落とし穴
- S3 を S2 より先に行わない (`--yes` が破壊的になる)
- `all_cleaners` を手書きの `vec!` に戻さない。標準ターゲットを `special_cleaners` に重複登録しない (重複テストが検出する)
- `ManualOnly` で `clean_with_opts` と `sub_targets` の委譲を忘れない (xcode のサブターゲットや apfs-snapshot のオプションが壊れる)
- 新規 31 件を `AUTO_CLEAN_TARGETS` に足さない。既存 15 件の `--yes` 挙動を変えない
- colima 以外の行に `.with_config(config)` を付けない (29 件の `_config` は別 PBI。PBI-09)
- `unsupported_filter_warning` を `pub` にしない、警告文面を変えない、ログを捕まえるためのロガーを足さない
- `exclude` に CLI 名ではなく `name()` を使う
- 設計書のコードは未コンパイルの部分がある。コンパイルエラーが設計書由来で 2 回の修正で直らなければ止まって報告する
- 途中で許容される警告は `ManualOnly` 未使用 (S2 の間) と `advisory` 未使用だけ。それ以外の警告が出たら止まる

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings      # S3 完了時はゼロ。S2 の間は上記の dead_code のみ許容
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo run -q -- targets | wc -l  # 48
./target/debug/sasurahime --no-unicode scan     # 実機の読み取りのみ。docker、npm、cargo などの行が増える
./target/debug/sasurahime clean docker --dry-run  # dry-run のみ
git status --short
```
`sasurahime clean ...` (`--dry-run` なし) と `sasurahime --yes` は実機で実行しない。E2E は tempdir の HOME と差し替えた PATH だけを使う。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 設計書 18 章の完了条件 1, 3, 8 を満たす (2 と 4 から 6 は PBI-03 で満たす)
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告で通る
- [ ] S1, S2, S3 が個別にコミットされている
- [ ] コードレビュー完了
- [ ] リファクタリング完了 (グリーン後)
- [ ] ドキュメントは PBI-03 の T5 でまとめて更新する (本 PBI では README と docs を変更しない)
