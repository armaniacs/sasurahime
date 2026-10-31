出典: アーキテクチャ探索 round 6 なぜなぜ分析の収束解 (ii) A1（.plan/whywhy-round6-counting.md、読み取り専用分析・コード変更なし）。収束解 (ii) が本 PBI の実装計画で、実装順 2 に相当する

# PBI-96: scan 総計の計上決定を measured_bytes に集約する

## ユーザーストーリー
scripts や CI で `sasurahime scan --json` の総計を消費し、`scan` の表を人間が読む開発者として、CLI の Total reclaimable と JSON の `total_reclaimable_bytes` が常に同じ 1 つの計上決定から出てくることがほしい、なぜなら今日の一致は run_scan と run_scan_json が同じ決定を 6 サイトで重複実装しているだけで、片側の修正がもう片方を静かに割るから。

## 優先度
RICE: Reach 2 × Impact 1 × Confidence 85% / Effort 0.15 人週 = スコア 11.3

- Reach 2: scan 表（run_scan）と scan --json（run_scan_json）の両経路の利用者。jq で JSON を消費する scripts / CI を含む。
- Impact 1: ユーザー可視の欠陥は 0 の防衛リファクタ。片側修正が CLI と JSON を静かに割る構造を、振る舞い pin 1 本で red 化する。新 status variant 追加が 6 サイト回しから 1 関数 + 1 名前 join に縮む。
- Confidence 85%: 集約は純リファクタ（出力 byte 等価が受け入れ基準）で、counts_toward_total と PBI-83 pin は無修正。既存テストが回帰を検知する。
- PBI 98 より先に着地する理由: 計上決定と名前重複関係の「共有の居場所」（measured_bytes の隣）を先に作ると、98（TUI compute_selected_total の重複除去）が 2 箇所目を作らないで済む（whywhy round 6 実装順 2 の理由）。

## 背景（なぜなぜ分析 round 6 の発見）
1. 「Total reclaimable に何が入るか」という概念は、status レベルの決定（Pruneable → 計上、Unavailable / Unknown → 除外）+ name レベルの例外（hints → 表示するが計上しない）+ 表示関心事（size 列・estimable フラグ）の合成だが、この決定が scanner.rs の 6 サイトに if / match として再導出されている。正本の宣言がなく、どれが正かは diff を読み比べるまで分からない。
2. run_scan と run_scan_json は shallow な双エンジンで、共通の決定層を共有していない。両者は scan_preamble（scanner.rs:153-169）で risks map と denied probe は共有するが、計上決定は共有していない。deletion test が通らない: 片方の合計の complexity を削除すると、もう片方に同じ complexity が再出現する。複製であって集中ではない。
3. 片側修正が CLI と JSON を静かに割る。CLI の Total reclaimable と JSON の総計が一致することを pin する test は 0 本（run_scan と run_scan_json が別関数のため、共通 test の置き場所がない）で、割れは CI で表面化しない。
4. 新 status variant を追加すると 6 サイトの match を手で回すことになる。回し忘れてもコンパイラも test も落ちない。

### 実コードの確認（main @ 53f0b88、着手前確認を含む）
- sort_by_size（src/scanner.rs:9-22）: 内側の size()（10-15）が `match r.status { ScanStatus::Pruneable(b) => Some(b), _ => None }` で Pruneable 判定を再実装（1 サイト目）。
- status_columns（src/scanner.rs:38-46）: (size 列, status 列, counted) の 3 値タプルを返す「表示の都合」の関数で、counted: Option<u64>（39-45）に同じ match が埋め込まれる（2 サイト目）。計上の決定が表示のタプルに混在。
- counts_toward_total（src/scanner.rs:54-56）: `r.name != "hints"` の名前キー例外（3 サイト目）。doc comment（48-53）に「The name is hardcoded on purpose (PBI-83: 1 cleaner = 1 name) and pinned by the scanner tests: a rename goes red.」
- scan_json_document（src/scanner.rs:72-97）: size_bytes match（82-85）と estimable（88 の `matches!(r.status, ScanStatus::Pruneable(_))`）（4 サイト目）。
- run_scan（src/scanner.rs:171-250）: total 宣言（187）と増分ループ（190-196）が counted + counts_toward_total の合成（5 サイト目）。Unavailable カウント（197-199）と並走。
- run_scan_json（src/scanner.rs:256-272）: filter chain（263-270）が counts_toward_total + Pruneable match を filter / filter_map で再導出（6 サイト目）。
- counts_toward_total の pin: doc comment（src/scanner.rs:48-53、pin の一文は 52-53）+ 単体テスト the_hints_row_is_displayed_but_never_counted_toward_the_total（src/scanner.rs:400-421、hints は除外 / app-support は計上 / 名前 filter が status-level 計上を覆わない、の 3 assert）。PBI-83 の pin で、名前は意図的ハードコード、rename で red。
- 着手前確認（フェーズ0）: measured_bytes という名前の関数は src/ に存在しない（未実装を確認）。CLI 総計と JSON 総計の一致を pin する test は 0 本: tests/app_support.rs:110 scan_total_excludes_the_hints_row_and_notes_the_overlap は CLI 総計の hints 除外、tests/app_support.rs:160 scan_json_total_excludes_the_hints_row は jq で JSON 単体の自己整合（total == size_bytes 総和、hints 除外）を pin するだけで、CLI と JSON の突き合わせはない。.plan/archived/ に重複 PBI なし（PBI 83 は 1 cleaner = 1 name の pin、PBI 87 は hints 除外で隣接 scope、PBI 94 は hint_owned_paths で A6 の lever、PBI 98 は TUI 合計の territory）。

## BDD 受け入れシナリオ

```gherkin
Scenario: CLI の Total reclaimable と scan --json の総計が一致する
  Given TTY なしで `sasurahime scan` と `sasurahime scan --json` を同一の tempdir HOME で実行できる
  And  掃除対象のキャッシュに残骸が存在する
  When scan と scan --json を走らせる
  Then CLI の "Total reclaimable" 行のバイト数と JSON の "total_reclaimable_bytes" が同一の値を語る
  And  総計から hints 行は除外され続ける（名前 join の契約どおり）
  And  総計から unavailable 行も除外され続ける

Scenario: 集約後も出力は byte 等価（現行維持）
  Given 同一の tempdir HOME に対して集約前の scan 表と scan --json の出力を保存してある
  When measured_bytes への集約を当てて scan と scan --json を走らせる
  Then 表の出力が集約前と 1 行も変わらない
  And  JSON の出力が集約前と 1 バイトも変わらない
```

## 受け入れ基準
- [ ] src/scanner.rs に名前付き正本 fn measured_bytes(r: &ScanResult) -> Option<u64>（Pruneable(b) → Some(b)、それ以外 None）が 1 つだけ追加される
- [ ] 5 サイトが measured_bytes に接続される: sort_by_size の size()（scanner.rs:10-15）、status_columns の counted（39-45）、scan_json_document の size_bytes（82-85）と estimable（88）、run_scan の total 増分（190-196）、run_scan_json の total filter chain（263-270）
- [ ] counts_toward_total の名前 join（scanner.rs:54-56、`r.name != "hints"`）は無修正。path 所有（hint_owned_paths）への置換は「総計の意味」を除外から重複除去に変える意味論変更のため混入させない（本 PBI の 1 行明記事項）
- [ ] CLI の Total reclaimable == JSON 総計の一致 test 1 本を tests/scan.rs 付近に追加する（片側修正を red にする唯一の振る舞い pin）
- [ ] 出力 byte 等価: scan 表と JSON が現行と同一（集約前後で 1 バイトも変わらない）
- [ ] 既存 pin が無修正で green: counts_toward_total の pin テスト（scanner.rs:400-421）を含む scanner.rs 全既存単体テスト、tests/scan.rs 全既存、tests/app_support.rs 全既存、tests/hints.rs 全既存、tests/coverage.rs、tests/scan_coverage.rs

## テスト戦略
Outside-In TDD。test を先に置いてから集約を行い、Red → Green → Refactor を繰り返す。

- E2E Red 起点: tests/scan.rs 付近に 1 本。同一 tempdir HOME で `scan` と `scan --json` を走らせ、CLI の Total reclaimable 行と `total_reclaimable_bytes` が同一のバイト数を語ることを pin する（tests/plugin.rs:436 の `sasurahime::format::format_bytes` 使用形状で、JSON の生バイトを format_bytes して CLI 行の値と比較する）。現行コードでは両エンジンが同じ決定を重複実装しているため green で始まる。Red の確認は片側変異で行う: 実装前に run_scan の counted 条件か run_scan_json の filter を一時的に 1 か所だけ変えて test が落ちること（= 片側修正を red にする振る舞い pin として機能すること）を確認し、戻す。
- E2E（byte 等価）: 集約後に同一 fixture で scan 表と scan --json の出力が現行と同一であることを確認する。tests/app_support.rs:110 の CLI 総計 pin と tests/app_support.rs:160 の JSON 自己整合 pin が無修正で green するため、byte 等価の追加 assert は置かない（本 PBI で明記）。
- 単体: scanner.rs の既存 12 本（sort_by_size 3 本 286-319、status_columns 3 本 321-335 と 392-398、scan_json 2 本 337-390、counts_toward_total pin 1 本 400-421、detect_all 2 本 451-531、run_scan is_available 1 本 560-573）は無修正で green が pin。measured_bytes の単体 assert を新設するかは実装時に design-it-twice で決める（CLI/JSON 一致 test が振る舞い pin として機能するため、最低限は 1 本でよい）。
- モック: E2E は tempdir HOME + 実残骸で駆動する（pbi/README.md の設計方針どおり）。scan は外部コマンドを呼ぶため、CI と同じ PATH stub 方式（tests/scan.rs:18-37 の stub 形状）を踏襲する。

## 見積もり
0.15 人週

内訳: measured_bytes の追加と 5 サイト接続（0.05）+ CLI/JSON 総計一致 test 1 本（0.05）+ 出力 byte 等価の検証と DoD 3 コマンド（0.05）。

## 実装計画

### 実装順
1. E2E Red 起点: tests/scan.rs 付近に CLI/JSON 総計一致 test 1 本を追加する。現行（両エンジンが同じ決定を重複実装）では green のため、片側変異で 1 回 red を確認してから戻す（振る舞い pin として機能することの確認）。
2. Green: src/scanner.rs に `fn measured_bytes(r: &ScanResult) -> Option<u64>`（Pruneable(b) → Some(b)、それ以外 None）を 1 つ追加し、5 サイトを接続する:
   - sort_by_size の size()（scanner.rs:10-15）→ `measured_bytes(r)`
   - status_columns の counted（39-45）→ measured_bytes 相当（引数は ScanResult 化、whywhy round 6 判定 (ii) どおり）
   - scan_json_document の size_bytes（82-85）と estimable（88）→ `measured_bytes(r)` と `measured_bytes(r).is_some()`
   - run_scan の total 増分（190-196）→ counted を measured_bytes から取る
   - run_scan_json の total filter chain（263-270）→ filter_map を measured_bytes に置換
   **counts_toward_total の名前 join は不変** ― PBI-83 pin 済みで契約として機能する。path 所有（hint_owned_paths）への置換は「総計の意味」を除外から重複除去に変える意味論変更のため混入させない（1 行明記事項）。pin（scanner.rs:400-421）には触れない。
3. 検証: 出力 byte 等価（scan 表と JSON が現行と同一）を同一 fixture で確認し、DoD の 3 コマンド + pin 無修正 green を通す。

### 対象ファイル
src/scanner.rs（measured_bytes の追加 + 5 サイト接続）、tests/scan.rs（CLI/JSON 総計一致 test 1 本）。

### 期待する Red → Green
新 test は現行で green の pin（片側変異で red を 1 回確認）。集約後も全 green、出力 byte 等価。既存 pin と既存 E2E は無修正で green（回帰の検知）。

### design-it-twice
- Option A（採用）: scanner.rs に measured_bytes 1 化、5 サイト接続。正本の宣言が 1 箇所になり、新 status variant 追加が 1 関数回しになる。CLI/JSON 一致 test の置き場所（共通の決定関数）ができる。
- Option B（却下）: counts_toward_total を path 所有（hint_owned_paths、PBI-94）に置き換える。「総計の意味」を除外から重複除去に変える意味論変更で、名前 join は PBI-83 pin 済みの契約として機能している（whywhy round 6 攻撃者連鎖 25 段の行き止まり裁定）。純リファクタに混入させない。
- Option C（却下）: 計上決定を現状の 6 サイトのまま残す。新 status variant 追加が 6 サイト回しのままになり、片側修正が CLI と JSON を静かに割る構造も解消しない（半年後の保守担当者連鎖 15-22 段）。

## pin（壊してはいけない契約）
- counts_toward_total（scanner.rs:54-56）と doc comment（scanner.rs:48-53）: 名前ハードコード（PBI-83: 1 cleaner = 1 name）は意図的。本 PBI で無修正
- the_hints_row_is_displayed_but_never_counted_toward_the_total（scanner.rs:400-421）: hints は除外 / app-support は計上 / 名前 filter が status-level 計上を覆わない、の 3 assert。rename で red する pin。無修正で green
- scanner.rs の既存 12 本（sort_by_size 3 本 286-319、status_columns 3 本 321-335 と 392-398、scan_json 2 本 337-390、counts_toward_total pin 1 本 400-421、detect_all 2 本 451-531、run_scan is_available 1 本 560-573）は無修正で green
- 出力 byte 等価: scan 表と scan --json の出力形（ヘッダ Category / Size / Status / Risk / Target、ScanJsonEntry のキー名、hints 行の size_bytes・estimable 保持、注記行・unavailable・denied・notes の各ブロック）は現行と同一
- 既存 E2E: tests/scan.rs 全既存（scan_lists_largest_targets_first:183-212、scan_json_outputs_valid_schema:296-329、second_scan_reuses_the_persistent_cache:497-533 の Total reclaimable 行形状 513-517 など）、tests/app_support.rs 全既存（scan_total_excludes_the_hints_row_and_notes_the_overlap:110-159、scan_json_total_excludes_the_hints_row:160-196）、tests/hints.rs 全既存（scan の hints 行表示 2 本 205-243）、tests/coverage.rs、tests/scan_coverage.rs が無修正で green
- ScanStatus の 5 variant（Pruneable / Clean / NotFound / Unavailable / Unknown）と estimable の意味（Pruneable かどうかの bool）は現行維持
- hint_owned_paths（src/hint/apps.rs:266、PBI-94 着地済み）と TUI compute_selected_total（src/interactive.rs、PBI-98 の territory）には接触しない

## scope fence（絶対禁止）
1. counts_toward_total の名前 join 変更・path 所有への置換はしない（PBI-83 pin 済みの契約。path 級への置換は「総計の意味」を除外から重複除去に変える意味論変更で、純リファクタに混入させない）
2. ScanStatus の追加・変更はしない（5 variant のまま）
3. 表 / JSON の出力形変更はしない（byte 等価が受け入れ基準）
4. estimable の意味変更はしない（Pruneable かどうかの bool のまま、measured_bytes への接続で現行値を保つ）
5. hint_owned_paths への接触はしない（PBI 94 着地済み、A6 の lever）
6. TUI compute_selected_total への接触はしない（PBI 98 の territory。96 を先に着地させる理由は「共有の居場所」を先に作るため）
7. CHANGELOG.md は触らない（マージ後に本体で一括記載）
8. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 出力 byte 等価: scan 表と JSON が現行と同一
- [ ] counts_toward_total の pin（scanner.rs:400-421）が無修正で green
- [ ] BDD 受け入れシナリオが自動テストとして実装され、パスする
- [ ] pin の列挙がすべて無修正で green していることを確認する

## 備考
- 本 PBI は scanner 側の純リファクタで、clean エンジンの意味論変更ではない。Option A 段 3 の発火条件（src/batch.rs:80-83 の「a new line family, a gate move, or a gate condition change」）には当たらない（whywhy round 6 判定 (ii) の 1 行明記事項）。
- 実装は worktree で subagent-driven。対象が src/scanner.rs + tests/scan.rs 付近で pin との接触範囲が狭いため、単独タスク 1 本で着手できる。
- マージ順は 96 → 98（scanner.rs 共有）。96 が「共有の居場所」（measured_bytes の隣）を先に作ると、98（A6、compute_selected_total の重複除去）が 2 箇所目を作らないで済む。
- 並行で src/progress.rs・cleaner.rs（PBI 95）、src/cleaners/library_logs.rs・orphan_containers.rs（PBI 97 / 99）、prompt.rs（PBI 99）を触る別 PBI が進行中。本 PBI の対象とは独立ファイルのため競合しない。
- scan の注記行（scanner.rs:211-221、hints の重複説明）は現行維持。A6（PBI 98）の重複関係は measured_bytes の隣に置く想定で、本 PBI では触れない。
