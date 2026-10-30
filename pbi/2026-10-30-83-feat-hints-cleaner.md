出典: /improve-codebase-architecture ラウンド 2 候補 1 Phase C（深掘り 2026-10-30）

# PBI-83: hint 掃除を hints cleaner として registry に登録する

## ユーザーストーリー
sasurahime を使う開発者として、hint 掃除（対象アプリの終了 → キャッシュ削除 → アプリ再起動）を `sasurahime clean hints` と TUI から他のクリーナーと同じ方法で実行できることがほしい、なぜなら現行は hint 掃除だけが Cleaner trait フローの外にあり、scan のテーブルと `sasurahime targets` に載らず、dry_run と履歴記録の契約も他のクリーナーと揃っていないから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 70% / Effort 1.5 人週 = スコア 1.4（本ラウンド 7 位・任意。PBI 80 と PBI 82 の着地後）

- PBI 80 が hint の実行部を ledger + 履歴に寄せ、PBI 82 が define_cleaners! 表の special 行形式を確立するため、残る作業は「表に 1 行 + cleaner 1 実装」に縮み、Effort 1.5 人週で収まる。
- Reach 2 は hint 掃除の経路 1 本だが、registry / coverage / scan 表示への横断修正が伴う。registry の外に置かれた cleaner は pin の穴になり続けるため、網羅性を先に固める価値がある。
- Confidence 70% は quit / relaunch を伴う外部アプリ操作（osascript 経由）のテスト容易さに不確実性が残るため。

## 発見の詳細（深掘り結果）

**前提**: この PBI は PBI 80（hint: dry_run + twin 統合 + ledger 移行 + 履歴統合）と PBI 82（CleanTarget special 行昇格）の着地後に着手する。PBI 80 が hint の実行部を ledger + 履歴に寄せ、PBI 82 が define_cleaners! 表を拡張するため、この PBI は「表に 1 行 + cleaner 1 実装」に縮む。

**問題**: hint 掃除は Cleaner trait フローの外にあり、scan / TUI / coverage の registry から見えない:

- hint は CleanTarget ではない（variant がない）→ dry_run 契約が arm 解決から漏れた（PBI 80 が修正）
- TUI から hint 掃除を選択できない（offer_auto_clean は scan のおまけの tip として独立）
- coverage.rs（tests/coverage.rs、targets 出力 ⇔ CLI サブコマンド対応を強制する 2 テスト: all_targets_are_valid_cli_subcommands / no_duplicate_targets）に hint が載っていない
- 場所モデルの四重化: hint KNOWN_ENTRIES 22 件、cleaners/* の detect パス、explorer.rs MANAGED_PATTERNS 18 件、app_support の installed_apps 動的許可リスト

**解（design-it-twice の決定）**: **KNOWN_ENTRIES 全体の 1 cleaner + sub_targets() を採用。app ごとの 22 cleaners は却下**（targets 51→73 行・coverage.rs チェック 22 増・scan テーブル肥大。display_name 22 件は重複なし確認済み → sub 名として使える。GeoServices は skip=true で detector.rs:18 が除外済み）:

- `KnownAppHintsCleaner`: name() = "hints"、detect() = collect_hints 集約（Pruneable(合計)）、clean() = quit → ledger → relaunch（PBI 80 が作る統合関数を呼ぶ）、sub_targets() = display_name + size（xcode の sub_targets() 先例 interactive.rs:33-44、main.rs:845-849）、risk() = Moderate（Safe にしない）、interactive_gated() = false（prompt は entry point 側の仕事、cleaner 内に prompt を持たない）
- main.rs: define_cleaners! に `Hints : "hints" => ...` + CleanTarget::Hints { dry_run }（PBI 82 の special 行形式）。tests/coverage.rs は自動で通る（CLI サブコマンドも同時追加）
- TUI は clean_sub_target と同じ CleanByName seam（main.rs:832-849）に寄せる: `clean_by_name("hints", Some(display_name), dry_run, reporter, &clean_opts)`
- offer_auto_clean は選択収集に縮退（実行部は PBI 80 で CleanByName seam に寄せ済み）
- quit / relaunch メタデータ（threshold / quit_app / relaunch_app / commands / path_suffixes）は現行の HintEntry 構造体 + KNOWN_ENTRIES 静的テーブル（hint/apps.rs）に残留 ― 深いデータの正しい形状。registry の define_cleaners! ファクトリシグネチャ（|home, config|）はこれら 4 軸を運べないのでテーブルには載せない
- detect / sub_targets の二重 walk は PBI-04 の OnceLock キャッシュ（scan_cache）戦略とともに design-it-twice で決める（未解決の疑問として明記）

## BDD 受け入れシナリオ
Scenario: CLI で hints を掃除する
  Given KNOWN_ENTRIES の候補に該当する残骸が存在する
  When  `sasurahime clean hints` を実行する
  Then  該当 app に対して quit → 削除 → relaunch が実行される
  And   履歴に実行記録が 1 件書かれる

Scenario: --sub で該当 app のみ掃除する
  Given KNOWN_ENTRIES の複数候補に残骸が存在する
  When  `sasurahime clean hints --sub "Claude logs"` を実行する
  Then  "Claude logs" に対応する app のみが削除される
  And   他の候補は削除されない

Scenario: dry_run では何も削除しない
  Given KNOWN_ENTRIES の候補に該当する残骸が存在する
  When  `sasurahime clean hints --dry-run` を実行する
  Then  quit も削除も relaunch も実行されない
  And   削除予定の一覧だけが表示される

Scenario: targets に hints が載る
  When  `sasurahime targets` を実行する
  Then  出力に hints が含まれる
  And   tests/coverage.rs の all_targets_are_valid_cli_subcommands と no_duplicate_targets が green する

Scenario: TUI から hints を選ぶ
  Given TUI でユーザーが hints の sub_target を選択した
  When  掃除が実行される
  Then  実行は CleanByName seam 経由で行われる
  And   dry_run と履歴記録が Cleaner trait フローの契約に従う

Scenario: 候補が見つからないとき
  Given KNOWN_ENTRIES のどの候補にも残骸が存在しない
  When  `sasurahime scan` を実行する
  Then  hints の行が Not found として表示される

## 受け入れ基準
- [ ] KnownAppHintsCleaner が Cleaner trait を実装している（name() = "hints"、detect() = collect_hints の集約（Pruneable(合計)）、clean() = quit → ledger → relaunch の統合関数呼び出し）
- [ ] sub_targets() が display_name + size の一覧を返す（xcode の先例 interactive.rs:33-44 / main.rs:845-849 に合わせる）
- [ ] risk() が Moderate を返す（Safe にしない）
- [ ] interactive_gated() が false を返し、cleaner 内に prompt を持たない（prompt は entry point 側の仕事）
- [ ] main.rs の define_cleaners! に `Hints : "hints" => ...` が登録され、CleanTarget::Hints { dry_run } が追加されている
- [ ] `sasurahime clean hints` が KNOWN_ENTRIES の候補に対して quit → 削除 → relaunch を実行する
- [ ] `sasurahime clean hints --sub <display_name>` が該当 app のみを掃除する
- [ ] TUI からの hints 実行が clean_sub_target と同じ CleanByName seam（main.rs:832-849）経由になっている
- [ ] offer_auto_clean が選択収集に縮退している（実行部は CleanByName seam に寄せる）
- [ ] quit / relaunch メタデータ（threshold / quit_app / relaunch_app / commands / path_suffixes）が HintEntry 構造体 + KNOWN_ENTRIES 静的テーブル（hint/apps.rs）に残留している
- [ ] tests/coverage.rs の all_targets_are_valid_cli_subcommands / no_duplicate_targets が hints を含んだ状態で green する
- [ ] 既存の pin が壊れていない（explorer.rs MANAGED_PATTERNS 18 件 / KNOWN_ENTRIES 22 件の文言 / UnattendedPolicy 3 種）

## テスト戦略
Outside-In TDD で外側から内側へ書く。

- E2E: tempdir を HOME として binary を spawn（またはトップレベル関数を呼び出し）し、`sasurahime clean hints` の exit code と stdout を検証する。`sasurahime targets` に hints が載ることも E2E で確認する。
- 統合: CommandRunner モックで quit → 削除 → relaunch の呼び出し順序を検証する。KnownAppHintsCleaner の detect() と clean(dry_run=true) / clean(dry_run=false) を KNOWN_ENTRIES を差し替えられる fixture で検証する。
- 単体: sub_targets() の display_name + size の組み立て、detect() の Pruneable(合計) 集約、--sub の絞り込みロジックを検証する。

## 見積もり
1.5 人週（1.0 から 2.0）

## 実装計画
実装順:

1. KnownAppHintsCleaner の実装（hint/cleaner.rs に追記するか、新 src/hint/cleaner_impl.rs を作るかは design-it-twice で決める）。name() = "hints"、detect() = collect_hints の集約（Pruneable(合計)）、clean() = PBI 80 が作る quit → ledger → relaunch の統合関数を呼ぶ。risk() = Moderate、interactive_gated() = false。
2. define_cleaners! 登録 + CleanTarget::Hints { dry_run }（PBI 82 の special 行形式）。CLI サブコマンド `sasurahime clean hints` を同時に配線する。
3. sub_targets()（display_name + size）+ clean_sub_target seam 接続。TUI は `clean_by_name("hints", Some(display_name), dry_run, reporter, &clean_opts)` で CleanByName seam（main.rs:832-849）に寄せる。offer_auto_clean は選択収集に縮退する。
4. tests/coverage.rs 自動対応の確認（CLI サブコマンド追加と同時に all_targets_are_valid_cli_subcommands / no_duplicate_targets が通る）。
5. 検証ゲート（DoD の 3 コマンド）。

設計メモ:

- KNOWN_ENTRIES 全体を 1 cleaner + sub_targets() として扱う。app ごとの 22 cleaners は却下（targets が 51→73 行に増え、coverage.rs のチェックが 22 増え、scan テーブルが肥大する。display_name 22 件は重複なし確認済みのため sub 名として使える。GeoServices は skip=true で detector.rs:18 が除外済み）。
- quit / relaunch メタデータ（threshold / quit_app / relaunch_app / commands / path_suffixes）は現行の HintEntry 構造体 + KNOWN_ENTRIES 静的テーブル（hint/apps.rs）に残留させる。registry の define_cleaners! ファクトリシグネチャ（|home, config|）はこれら 4 軸を運べないためテーブルには載せない。
- 未解決の疑問（design-it-twice で決める）: KnownAppHintsCleaner の配置（hint/cleaner.rs 追記 vs 新 src/hint/cleaner_impl.rs）。detect() と sub_targets() の二重 walk を PBI-04 の OnceLock キャッシュ（scan_cache）戦略で潰すかどうか。

## pin（壊してはいけない契約）
- registry pin「1 cleaner = 1 name」: main.rs unique / 表順序 / non-overlap / 翻訳不要 / 登録網羅 + coverage.rs no_duplicate_targets
- tests/coverage.rs: all_targets_are_valid_cli_subcommands / no_duplicate_targets ― hints が載った状態で green
- PBI 80 の契約: hint の dry_run pin / 履歴統合（write_history_entry_if_needed）/ is_yes_answer 共有 / KNOWN_ENTRIES 22 件の文言
- explorer.rs: MANAGED_PATTERNS 18 件は触らない
- PBI 74 契約: UnattendedPolicy は 3 のまま（hints は TTY 限定 interactive で無人実行と方向が逆）
- PBI 75 契約: hint は PromptPort にしない
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence
1. app ごとの 22 cleaners 化・個別 CleanTarget 登録はしない
2. UnattendedPolicy への hint 統合はしない
3. KNOWN_ENTRIES の config 化・外部ファイル化はしない
4. hint の top5 truncate（detector.rs:44）と threshold_bytes は触れない
5. explorer.rs / MANAGED_PATTERNS は触れない
6. hint 経路と app_support/library_caches cleaner の機能重複の解消はしない
7. Linux/Windows 対応なし
8. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
