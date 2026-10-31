出典: なぜなぜ分析 round 5（.plan/whywhy-round5-hint-dedup.md、2026-10-31、main @ 56e96cf、読み取り専用分析・コード変更なし）の Phase 2 具体形（(i) 導出関数 + (ii) scanner 別枠 + (iii) 候補 render タグ）。§7 の PBI B（feat、Phase 2）。実装順は (i) → (iii) → (ii)

# PBI-94: hint 管轄パスを scanner と候補画面で可視化する

## ユーザーストーリー
sasurahime の scan（テーブルと `--json`）と cleaner 候補選択画面を使う開発者として、scan の Total に hint 管轄パスの二重計上がなく、候補選択画面ではそのパスが `clean hints` でも消えることが表示されることがほしい、なぜなら同一ディレクトリが hints 行と app-support / library-caches 行の 2 行に載って Total が 2 回足され「これだけ戻る」の約束が約 25% 過大になり、候補画面にも交差の表示がないため、ユーザーは 2 経路の削除同意を重ねていることに気づけないから。

## 優先度
RICE: Reach 3 × Impact 2 × Confidence 85% / Effort 0.5 人週 = スコア 11.9

- Reach 3: scan 利用者全員（テーブルの Total）+ TUI / CLI の候補選択利用者 + `scan --json` のスクリプト・CI 消費者。
- Impact 2: Total の信頼性。二重計上は pin 済みの構造的重複（AppSupport 7 パス + Caches 7 エントリ）でほぼ必ず発生し、hints 行 1.5GB・Total 6GB の日には約 25% の過大。別枠化で解消する。
- Confidence 85%: 表示追加と Total の合算除外のみで、検出・削除経路・freed 計測は 1 行も変わらない。導出関数（hint_owned_paths）に必要な情報（base_dir + path_suffixes + skip）は src/hint/apps.rs と src/hint/mod.rs:12 に既にある。
- 前提: なぜなぜ分析 round 5 の Phase 2 進行裁定（今進めるべき）。PBI 89（symlink ガード）と PBI 91（hint path display）が着地済みで、導出関数を置く場所（hint/apps.rs）と交差テストの基盤がある。

## 背景/問題

1. scanner の "Total reclaimable" は全 Pruneable を合算するため（src/scanner.rs:177-184 と :237-243）、同一ディレクトリが hints 行と app-support / library-caches 行の両方に載り、最大 2 行の二重計上になる。重なりは pin 済みの構造的重複: AppSupport 7 パス（targets_cover_every_hint_suffix、app_support.rs:882-898）+ Caches 7 エントリ（同閾値 64MB）。hints 行 1.5GB・Total 6GB の日には約 25% の過大。
2. 候補画面に hint 管轄である旨の表示がなく、ユーザーは交差に気づけない。タグは「この候補は `clean hints` でも削除対象になりうる」の交差の事実を、ユーザーが per-candidate で承認する唯一の場面（cleaner 内部の multi_select）に見せるためにある。タグは「消える約束」ではなく「管轄の重なり」の意味。
3. src/cleaners/hint.rs は実在しない。KnownAppHintsCleaner は src/hint/cleaner.rs:197-348（hint/mod.rs:5 が再公開）。実装者は存在しないファイルを探さないこと。
4. 二重計上の解消と引き換えに、hints 行にしか載らない分の合計からの消失が生じる。hint の Logs エントリは 1MB 閾値（src/hint/apps.rs:196 など）で発火するが、library-logs は 100MB・90 日（src/cleaners/library_logs.rs:25-26）でしか数えない。50MB の Claude logs のような「hints 行にしか載らない分」は行ごと外すと Total から消えるため、注記 1 行がセット。

## 発見の詳細（実コードの確認、main @ 56e96cf）

- KNOWN_ENTRIES: src/hint/apps.rs:6、22 件（Caches 11 + AppSupport 5 + Logs 6、実数確認済み）。`skip: true` は GeoServices 1 件のみ（:107-117）。multi-suffix は Code 1 件のみ（:165、3 suffix）。Google/Chrome/Default/Cache_Data は単一 suffix（:180）。`HintEntry` は src/hint/mod.rs:19-33、`BaseDir` は :12-17。
- 導出数: 非 skip 21 エントリの suffix 展開は 23 パス（Code の 3 suffix を含む、実数確認済み。出典の「約 30」は実数より大きい）。render ごとの線形走査で無視できるコスト。
- BaseDir は Debug, Clone, Copy, PartialEq のみ（src/hint/mod.rs:12）。Eq/Hash なし → `Vec<(BaseDir, &'static str)>` が derive 追加なしで使える。
- scanner: run_scan の行ループ（src/scanner.rs:180-191）と Total 計算（:177-184）。「Total reclaimable」出力は :195-197（`if total > 0` ゲート）。run_scan_json の total は :237-243（全 Pruneable を filter_map で合算）。Unavailable / Unknown の counted=None 前例（:38-46、テスト :295-308）が「表示するが数えない」の同型。既存注記群（unavailable :199-203、denied :205-214、Notes :216-225）は Total の後段に並ぶ。既存テスト `status_columns_pruneable_counts_toward_total`（:365-371）は status レベルの「Pruneable は数える」pin で、hints 行の除外は名前レベルのため共存する（新 pin のテスト名はこの名前と衝突しない）。
- hints 行の集約: KnownAppHintsCleaner::detect = collect_hints の top5 集約（src/hint/cleaner.rs:264-281、collect_hints は src/hint/detector.rs:15-46、`!e.skip` :18、top5 truncate :43-44、Logs 1MB 閾値比較 :28）。KnownAppHintsCleaner は src/hint/cleaner.rs:197-348。src/cleaners/hint.rs は実在しない（hint/mod.rs:5 が再公開）。
- 候補 render: app_support.rs の format_candidate_label（:224-232、`format!("{}/{}", c.target.root, c.sub)` と `[running]` タグ、interactive_select :405 から使用）。library_caches.rs の render_listing（:150-163、全候補の listing、タグ列 `[running]` / `[below threshold]` / `[display only]`）と render_item（:165-178、interactive_select :247 の選択行）。
- known_entry_for（library_caches.rs:62-66）は単一 suffix 形を要求（`e.path_suffixes == [name]`）。将来の multi-suffix Caches エントリで分類から外れるため、タグの情報源には使わない（hint_owned_paths() を使う。known_entry_for は分類用に残す）。
- TUI 上部の MultiSelect（build_selection_items、interactive.rs:13-48）は cleaner 名 + sub_targets() から行を組み立て、候補 render を通らないためタグは付かない。両 cleaner の clean は内部で interactive_select を必ず呼ぶ（app_support.rs:397-412 を clean_cache_dirs :550-584 から、library_caches.rs:242-258 を clean_with_mode :263-321 から）ため、タグの無い選択画面は実在しない。dry-run（app_support print_dry_run :378-395、library-caches :289-298）は候補とフルパスを表示するがタグは付けない。
- 既存 render pin: app_support.rs:1574-1582（label_marks_running_apps: `[running]` contains / !contains + `Slack/Cache` contains。Slack/Cache は hint 管轄）、library_caches.rs:528-544（render_listing_tags_each_disposition: `!lines[1].contains('[')` の否 pin を含む。fixture 名 A/B/C/D は非管轄）、:547-552（render_item_mentions_running）。fixture 名が非管轄のためタグ追加でも否 pin は赤くならないが、タグ文言の pin 更新がセット（「実装計画」に列挙）。
- E2E fixture 前例: tests/app_support.rs の OVER_THRESHOLD = 65MB（:18）と put()（:12-16）、scan_includes_app_support（:86-93）が scan の app-support 行 pin。tests/hints.rs の DEBRIS_BYTES = 2MB（:42、APFS 4096B ページ倍数で dir_size（st_blocks × 512）を正確に計測）と make_diagnostic_reports（:46-51）、hints_scan_row ヘルパ（:191-202）、scan_shows_the_hints_row_*（:205-241）が hints 行 pin。tests/scan.rs の Total pin（second_scan_reuses_the_persistent_cache :498-532 の Total 行 find、corrupt_cache_falls_back_to_a_full_scan :560-580）は tempdir HOME に hint 残骸がないため Total 変更でも赤くならない（回帰 pin として無修正で green）。
- 既実装確認（フェーズ0）: `hint_owned_paths` / `MANAGED_BY_HINTS_TAG` / "managed by hints" は src/ に存在しない（grep と実コード読みで確認済み）。未実装のため新規実装 PBI として書ける。既存テスト名は `status_columns_pruneable_counts_toward_total`（scanner.rs:366）のみが類似し、名前レベルの新 pin とは別物。

## BDD 受け入れシナリオ

```gherkin
Scenario: scan の Total は hints 行を別枠にする
  Given Slack/Cache（hint 閾値 64MB と app-support の両方に載るディレクトリ）に 65MB 以上の残骸が存在する
  When  `sasurahime scan` を実行する
  Then  hints 行に集約サイズが表示される（行とサイズ表示は残る）
  And   Total reclaimable は hints 行のサイズを含まない（app-support 行のみの合算）
  And   Total の後に注記 1 行が表示される（hints 行は上の行と重なり、100 MB 閾値未満の log ディレクトリは hints 行にのみ計上される旨）

Scenario: scan --json の total も hints 行を除外する
  Given 同じ残骸が存在する
  When  `sasurahime scan --json` を実行する
  Then  total_reclaimable_bytes は hints 行の size_bytes を含まない
  And   hints 行の targets エントリは size_bytes と estimable を保持する（消費者が自前で合算できる）

Scenario: 候補選択画面が hint 管轄を表示する
  Given Slack/Cache に app-support 閾値以上の残骸が存在する
  When  `sasurahime clean app-support` の候補選択画面を表示する
  Then  Slack/Cache の行に [managed by hints] タグが [running] の後ろに付く
  And   候補は今日どおり選択可能で、除外・削除経路・freed 計測は 1 行も変わらない

Scenario: library-caches の候補にもタグが付き、非管轄には付かない
  Given typescript キャッシュが library-caches の閾値を超えて存在する
  When  `sasurahime clean library-caches` の listing と選択行を表示する
  Then  typescript の行に [managed by hints] タグが付く
  And   hint 管轄でない名前（unknown アプリ）の行にはタグが付かない

Scenario: dry-run にはタグが付かずフルパスで追跡できる（現行維持）
  Given 同じ残骸が存在する
  When  `sasurahime clean app-support --dry-run` を実行する
  Then  dry-run 行にフルパスが表示される
  And   dry-run 行に [managed by hints] タグは出ない（タグは選択画面のみ）
```

## 受け入れ基準
- [ ] src/hint/apps.rs に `pub fn hint_owned_paths() -> Vec<(BaseDir, &'static str)>` と `pub const MANAGED_BY_HINTS_TAG: &str = "  [managed by hints]"` が追加される（非 skip エントリの suffix を base_dir と組で展開、Eq/Hash derive 追加なし）
- [ ] 導出展開の単体 pin: Code 3 suffix の展開、Google/Chrome の Cache（非管轄）と Cache_Data（管轄）の区別、GeoServices（skip）の除外、Homebrew / ms-playwright は含む（library-caches は委譲だが hint 管轄）
- [ ] app_support.rs の format_candidate_label（:224-232）と library_caches.rs の render_item（:165-178）/ render_listing（:150-163）に MANAGED_BY_HINTS_TAG を追記する（[running] の後ろ。情報源は hint_owned_paths()、known_entry_for は使わない）
- [ ] dry-run 行（app_support print_dry_run :378-395、library-caches :289-298）にはタグを付けない（フルパス表示の現行契約のまま）
- [ ] scanner.rs の run_scan 行ループ（:180-191）で `r.name == "hints"` の行は Total に加算しない（サイズ表示は残る。行名直書きは scanner テストで pin、トレイト成長 counts_toward_total は採らない）
- [ ] Total の後に注記 1 行が追加される（overlaps + log dirs below the 100 MB threshold are counted only in the hints row）
- [ ] run_scan_json も同じ除外を total に適用する（hints 行の size_bytes / estimable は残置。total_reclaimable_bytes の意味変更は breaking → CHANGELOG は統合側で記載、本 PBI では触れない）
- [ ] 既存 render pin 3 本を更新済み（タグ文言分、「実装計画」に列挙）
- [ ] KNOWN_ENTRIES 22 件・`HintEntry`・`BaseDir`・検出・削除経路・freed 計測・履歴 label は 1 行も変わらない（src/hint/mod.rs の tests と既存 E2E が無修正で green）
- [ ] DoD の 3 コマンドが通る

## テスト戦略（Outside-In TDD）

Red: 二重計上を実証する pin → Green: 別枠化で解消。

- E2E Red（二重計上の実証、1 本）: tests/app_support.rs の fixture（OVER_THRESHOLD = 65MB の Slack/Cache、scan_includes_app_support :86-93 と同じ残骸）で `scan` を実行し、Total reclaimable が hints 行のサイズを含まないこと（app-support 行のみの合算）、hints 行はサイズ表示のまま残ること、Total の後に注記 1 行が出ることを assert する。現行は Total が 2 行分を合算するため先に Red になる。Total 行の assert は tests/scan.rs:513-517 の Total 行 find と同じ形。
- E2E（JSON、1 本）: 同じ fixture で `scan --json` の total_reclaimable_bytes が hints 行の size_bytes を含まないこと、hints 行のエントリは size_bytes / estimable を保持することを assert する（jq 前例は tests/scan.rs:317）。
- 単体（導出展開、src/hint/apps.rs tests）: Code 3 suffix の展開（Cache / CachedExtensionVSIXs / CachedData の 3 組が返る）、Google/Chrome/Default/Cache_Data は含むが Cache は含まない（prefix 誤爆の防御 pin）、GeoServices（skip）は含まない、Homebrew / ms-playwright は含む、MANAGED_BY_HINTS_TAG の文言 pin。
- 単体（scanner 別枠、src/scanner.rs tests）: 行名 "hints" の行が Total に入らないことの pin（Unavailable / Unknown の counted=None 前例 :295-308 と同型の「表示するが数えない」。StubCleaner 前例 :375-399 で行を組み立てられる）。hints 名を変える PR がこのテストで赤くなる（行名直書きの pin）。
- 単体（render タグ）: hint 管轄名にタグが付く / 非管轄名に付かない / [running] の後ろに付く の pin。既存 render pin 3 本は更新（タグ文言分、「実装計画」に列挙）。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針）。本 PBI は表示追加と合算除外のみなので、モックの既存契約は変わらない。

## 実装計画

### 実装順（(i) → (iii) → (ii)）
1. (i) 導出関数 + 定数: src/hint/apps.rs に hint_owned_paths() と MANAGED_BY_HINTS_TAG を追加する。導出展開の単体 pin（Red → Green）を先に置く。
2. (iii) 候補 render のタグ: 2 cleaner の render 関数に MANAGED_BY_HINTS_TAG を追記し、既存 render pin 3 本を更新する。dry-run には付けない。
3. (ii) scanner 別枠: run_scan の行ループと run_scan_json の total に除外を入れ、注記 1 行を追加し、scanner テストと E2E pin を置く。CHANGELOG は統合側で記載（本 PBI では触れない）。
4. 検証ゲート（DoD の 3 コマンド）。

### (i) 導出関数: src/hint/apps.rs に 1 関数 + 1 定数

```rust
/// The paths the hint cleaner owns: non-skip KNOWN_ENTRIES suffixes paired
/// with their base dir. One owner for the "who else claims this dir"
/// question, shared by the app-support / library-caches candidate tags and
/// the cross-reference tests. Exact-match membership only -- prefixes must
/// never match (Default/Cache vs Default/Cache_Data).
pub fn hint_owned_paths() -> Vec<(BaseDir, &'static str)> {
    KNOWN_ENTRIES.iter()
        .filter(|e| !e.skip)
        .flat_map(|e| e.path_suffixes.iter().map(move |s| (e.base_dir, *s)))
        .collect()
}

/// Render tag for a cleaner candidate that the hint cleaner also claims.
pub const MANAGED_BY_HINTS_TAG: &str = "  [managed by hints]";
```

- `Vec<(BaseDir, &'static str)>` で Eq/Hash derive 追加不要（BaseDir は Debug, Clone, Copy, PartialEq、hint/mod.rs:12）。23 パスの線形走査（render ごとに呼んでも無視できるコスト）。
- 完全一致（Vec contains）でのみ比較する。starts_with による prefix 比較は Default/Cache（app-support 候補）と Default/Cache_Data（hint 管轄）の prefix 関係で誤爆するため禁止。
- テスト pin: Code 3 suffix の展開、Google/Chrome の Cache（非管轄）vs Cache_Data（管轄）の区別、GeoServices（skip）除外、Homebrew / ms-playwright は含む。

### (iii) 候補 render のタグ（候補の除外は不採用のまま）

- app_support.rs format_candidate_label（:224-232）: hint_owned_paths() の BaseDir::AppSupport 部分集合に `format!("{}/{}", c.target.root, c.sub)` が完全一致したら MANAGED_BY_HINTS_TAG を追記（[running] の後ろ）。interactive_select（:405）から使われるため CLI / TUI 両方の選択画面に付く。
- library_caches.rs render_item（:165-178）と render_listing（:150-163）: hint_owned_paths() の BaseDir::Caches 部分集合に e.name が完全一致したらタグ追記。**情報源は hint_owned_paths()**（known_entry_for :62-66 は単一 suffix 形を要求するため、将来の multi-suffix Caches エントリでタグ漏れを起こす。known_entry_for は分類用に残す）。
- dry-run 行（app_support print_dry_run :378-395、library-caches :289-298）には付けない（フルパス表示で人手追跡可能、文言同期面を増やさない）。

### (ii) scanner 別枠（run_scan + run_scan_json の両方）

- run_scan の行ループ（scanner.rs:180-191）: `r.name == "hints"` の行は total に加算しない（サイズは表示）。Unavailable / Unknown の counted=None（:38-46、テスト :295-308）と同型の「表示するが数えない」。status_columns は名前を受けないため、除外は行ループ側（status_columns のシグネチャは変えない）。行名直書きは scanner テストで pin（トレイト成長 counts_toward_total は採らない、全 cleaner への波移）。
- Total の後（:197 付近）に注記 1 行: 「Known app hints (shown above) overlap the app-support / library-caches rows and are excluded from the total; log dirs below the 100 MB log threshold are counted only in the hints row.」注記は `if total > 0` ゲート（:195）に紐付けず、hints 行が Pruneable（サイズ表示あり）で表示されたときに出す。
- run_scan_json（:237-243）: 同じ除外を total に適用。hints 行の size_bytes / estimable はそのまま残す（消費者が自前で合算できる）。**total_reclaimable_bytes の意味変更は breaking → CHANGELOG は統合側で一括記載（本 PBI では触れない）。**

### 既存 render pin の更新（タグ文言分、列挙）

- src/cleaners/app_support.rs:1574-1582（label_marks_running_apps）: Slack/Cache は hint 管轄のため、non-running 分岐の label に `[managed by hints]` の assert を追加する（既存の `[running]` contains / !contains と `Slack/Cache` contains はそのまま green）。
- src/cleaners/library_caches.rs:528-544（render_listing_tags_each_disposition）: fixture 名 A/B/C/D は非管轄のため既存 assert は green のまま。hint 管轄名（例: typescript）の行にタグが付く assert を追加する。`!lines[1].contains('[')` の否 pin は非管轄 fixture のままで意味を保つ（管轄名の Deletable 非行は `[` を含むようになるため、否 pin に管轄名を使わない）。
- src/cleaners/library_caches.rs:547-552（render_item_mentions_running）: 同様に管轄名 fixture のタグ assert を追加する（running contains / !contains はそのまま green）。
- 無修正で green が pin（触れない）: tests/app_support.rs:117 と tests/library_caches.rs:37 の display only E2E、tests/hints.rs の hints 行 pin（:205-241）、tests/scan.rs の Total pin（:498-532、:560-580。tempdir HOME に hint 残骸がないため Total 変更でも green）。

### 対象ファイル
src/hint/apps.rs（導出関数 + 定数 + 単体 pin）、src/cleaners/app_support.rs（format_candidate_label + render pin 更新）、src/cleaners/library_caches.rs（render_item / render_listing + render pin 更新）、src/scanner.rs（Total 別枠 + 注記 + テスト）、tests/app_support.rs または tests/scan.rs（E2E Red / JSON pin）。

### 期待する Red → Green
導出展開の単体 pin と scanner 別枠の単体 pin が先に Red（hint_owned_paths 未実装・Total が hints を含む）、E2E が二重計上を実証して Red、実装後に全 Green。既存 E2E と単体テストは render pin 更新箇所を除き無修正で green が pin（回帰の検知）。

## pin（壊してはいけない契約）
- PBI-83 の registry pin: 1 cleaner = 1 name（"hints"）。行名直書きの scanner テストはこの pin とセット（名前を変える PR を赤くする）
- PBI-80 / 91 の hint 表示契約: offer_auto_clean のプロンプトと両 dry-run 行のパス表示の単一所有者は hint_paths_display（src/hint/cleaner.rs:356-370）。本 PBI は文言を触れない
- PBI-87 の unattended 除外契約: quits_running_apps() = true、auto / clean all からの hints 除外は不変（src/hint/mod.rs:640-664 と tests/auto_policy.rs / tests/clean_all.rs）
- KNOWN_ENTRIES 22 件の vetted 契約: src/hint/apps.rs:6 の表（文言・フィールド値・件数・skip 値）と `HintEntry`（src/hint/mod.rs:19-33）は触らない
- scanner の counted=None 契約: Unavailable / Unknown は表示するが数えない（scanner.rs:38-46、テスト :295-308）。hints 行の除外はこれと同型で、status_columns のシグネチャは変えない
- tests/scan.rs の永続キャッシュ契約: second_scan_reuses_the_persistent_cache（:498-532）と changed_target_is_remeasured_by_the_cache（:535-557）は無修正で green
- pbi/README.md 設計方針: Outside-In TDD / CommandRunner モック / macOS 専用

## scope fence（絶対禁止）
1. 3 cleaner 統合（KnownAppHintsCleaner / AppSupportCleaner / LibraryCachesCleaner の統合）はしない
2. KNOWN_ENTRIES の表統合・config 化・外部ファイル化はしない（22 件の vetted 契約、round 4 裁定の継続）
3. top5 truncate / 閾値（64MB・1MB）の変更はしない（detector.rs:28-44 と KNOWN_ENTRIES の threshold_bytes）
4. src/cleaners/explorer.rs への接触はしない（explorer は src/explorer.rs。cleaners/ 配下に explorer.rs は実在しない）
5. TUI build_selection_items（interactive.rs:13-48）の変更はしない。"Will free approximately"（interactive.rs:399-401）の交差差し引きは app-support の sub_targets 公開 = UX shape 変更を伴うため別 PBI
6. library-caches 分類（Deletable / BelowThreshold 判定）の fresh 化はしない（永続キャッシュ pin、tests/scan.rs:498-532。分類の古さは別軸）
7. dry-run 行へのタグ追加はしない（フルパス表示の現行契約）
8. src/hint/cleaner.rs のプロンプト / dry-run 文言変更はしない（PBI 91 着地済み。hint_paths_display と両 dry-run 行は触れない）
9. CHANGELOG.md は触らない（統合側で一括記載。run_scan_json の total 意味変更は breaking のため CHANGELOG 記載が必須、本 PBI では記載しない）
10. 候補の除外（タグを見た cleaner 側の自動除外）はしない（`clean app-support` が Slack/Cache のような最大 subpath を失う回帰。タグは「載せたまま見せる」）
11. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオ 5 本が自動テストとして実装され、パスする
- [ ] 既存 render pin の更新（上記 3 本）と導出展開 / scanner 別枠の pin が green、pin の列挙が無修正で green していることを確認する

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）。PBI 93（fix、freed を削除時計測に統一）と src/cleaners/app_support.rs・library_caches.rs を共有するが別関数領域（PBI 93 は apply_selection:509 と delete_entries:231-233 の計測、本 PBI は format_candidate_label / render_item / render_listing の表示行）、マージ順は 93 → 94。
- 出典の展開数「約 30 パス」は実数より大きい: 非 skip 21 エントリの suffix 展開は 23 パス（Code の 3 suffix を含む、実数確認済み）。Vec 線形走査の判断は変わらない。
- 出典 §7 のスコア 11.9 は式どおりの計算（Reach 3 × Impact 2 × 0.85 / 0.5）だと 10.2 になる（Reach 3.5 換算で 11.9）。どちらを採るかは実装時のチーム見積もりで確定してよい（Negotiable）。
- run_scan_json の total_reclaimable_bytes の意味変更は breaking。CHANGELOG は統合側で一括記載するため、本 PBI の DoD には含めない。
- 候補の除外（タグによる自動除外）は前ラウンド裁定の再確認で不採用。TUI は選択順に実行し、後続 cleaner は clean 時に再スキャンするため消えたパスは候補から落ちる。競合は seam の NotFound 分類（src/trash.rs）が prose skip に変える。
- タグの見え方（文言・[running] との間隔・listing と item で揃えるか）は Negotiable（実装時に確定してよい）。ただし文言は MANAGED_BY_HINTS_TAG 定数 1 か所に寄せる。
