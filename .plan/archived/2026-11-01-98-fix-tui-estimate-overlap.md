出典: なぜなぜ分析 round 6（.plan/whywhy-round6-counting.md、2026-11-01、main @ 53f0b88、読み取り専用分析・コード変更なし）の §5.3 判定 (iii) A6 収束解（tier 1）。§6 実装順の 3 番目（A2 → A1 → A6）。マージ順は 96（A1）→ 98（本 PBI）

# PBI-98: TUI の "Will free approximately" の重複計上を差し引く（tier 1）

## ユーザーストーリー
sasurahime の TUI（引数なし起動）でキャッシュを選択する開発者として、confirm 前の "Will free approximately" の数字が選択した target 間の重複を二重に含まないことがほしい、なぜなら「hints」と「app-support」（または「library-caches」）を両方選ぶと、scan 表の Total reclaimable がすでに除外している同じディレクトリを TUI の合計だけが二重計上し、約束の数字が実解放より大きく出るから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 85% / Effort 0.25 人週 = スコア 10.2

- Reach 2: TUI 利用者（confirm 前の 1 行のみ。`scan` / `clean` の CLI 利用者は今日どおり対象外）
- Impact 1.5: 見積もりの信頼性。重複は pin 済みの構造的重複（hints 行と app-support / library-caches 行が同じ dir を数える、scanner.rs:48-53）で、両方選択時はほぼ必ず過大。差し引きで解消する
- Confidence 85%: 純関数（compute_selected_total）への差し引きと名前キー関数 1 つの追加のみ。検出・削除経路・freed 計測・表示文言は 1 行も変わらない
- 前提: なぜなぜ分析 round 6 の tier 1 進行裁定（今進めるべき）。PBI 96（A1、measured_bytes 1 化）が着地済みで、重複関係の「共有の居場所」の置き場所（scanner.rs）がある

## 背景/問題

1. TUI の "Will free approximately"（interactive.rs:401）は compute_selected_total（interactive.rs:52-71）の素加算。「hints」と「app-support」（または「library-caches」）を両方選ぶと、hints の選択分（Slack/Cache のような兄弟行と同じ dir を数える分）が二重計上される。scanner 側（表の total と `scan --json` の total）は同じ重複に「hints 行は合計に加算しない」規則で対処済み（counts_toward_total、scanner.rs:54-56。run_scan :187-196、run_scan_json :263-270）で、TUI に同じ規則がない locality gap
2. 同一 fact（同じ dir が 2 行に計上される）が 3 箇所に 3 形で存在する: scanner.rs:48-53 のコメント＋counts_toward_total（scan 合計用の名前除外）、src/hint/apps.rs:266-272 の hint_owned_paths（候補タグ用の path 所有者、PBI 94）、そして TUI 合計。TUI 合計だけがまだ二重計上する
3. 重要な罠: counts_toward_total の写し（除外）を TUI に入れると、hints のみ選択時に "Will free approximately 0 B" になる。hints は実際に app-support / library-caches の dir を消す（PBI 94 の管轄事実）ため、TUI に必要なのは「除外」ではなく「重複除去」
4. TUI の数字は confirm 前の 1 行だけで、実測（Total freed）と突き合わせる画面が存在しないため、過大が scan 側の Total のように指摘として来ていない（round 6 連鎖の観測）

## 発見の詳細（実コードの確認、main @ 53f0b88）

- compute_selected_total: interactive.rs:52-71。selected の各 index を selection_mapping に引き、sub が Some なら運搬済みの sub_size（PBI 59 の運搬契約）、None なら results[cleaner_idx] の Pruneable(b) を合計する。重複を判定する材料（cleaner 名の重なり関係）をどちらの経路にも持たない
- selection_mapping: SelectionEntry = (usize, Option<(&'static str, u64)>)（interactive.rs:11）。build_selection_items（interactive.rs:13-48）が運搬するのは (cleaner_idx, sub_size) のみで、path は選択の時点で捨てられている（tier 2 が要る理由）
- TUI の行の形: hints は sub_targets() が per-hint の display name + size を返す（src/hint/cleaner.rs:284-291、xcode 前例）ため sub 展開（(idx, Some)）になり、サイズは sub_size 経由。app-support と library-caches は sub_targets が trait 既定の空（src/cleaner.rs:366-368、library_caches.rs:719-722 の sub_targets_is_empty）のため単一行（(idx, None)）で、サイズは results[cleaner_idx].status の Pruneable(b) 経由
- scanner 側の対処済み重複: counts_toward_total（scanner.rs:54-56、`r.name != "hints"`）。根拠コメントは scanner.rs:48-53「The hints row overlaps the app-support / library-caches rows (the same dirs are counted in both)」。run_scan の total は :187-196 で counts_toward_total を通し、hints 行が Pruneable 表示のとき注記 1 行を出す（scanner.rs:211-221）。run_scan_json の total も同じ関数を通す（scanner.rs:263-270）
- 重複の知識の在処: scan 合計用に scanner.rs（counts_toward_total）、タグ用に hint_owned_paths（src/hint/apps.rs:266-272、PBI 94 で着地した「誰がこの dir を主張するか」の 1 所有者）に存在するが、compute_selected_total はどちらも読まない
- 名前キー: "hints"（src/hint/cleaner.rs:232-234）/ "app-support"（src/cleaners/app_support.rs:222）/ "library-caches"（src/cleaners/library_caches.rs:658 の name_is_library_caches）。1 cleaner = 1 name の registry pin（PBI-83）
- 既実装確認（フェーズ0）: compute_selected_total の差し引きと名前キー関数（overlaps_hints_row / hints_overlap_kind のような形）は src/ に存在しない（grep と実コード読みで確認済み。overlap の出現は scanner.rs の prose と counts_toward_total のみ）。未実装のため fix として書ける
- 既存テスト: compute_selected_total の単体 3 本が src/interactive.rs tests（:566-600、TestCleaner fixture :467-511 が名前と subs を可変で持つ）に隣接し、新 pin の置き場所と fixture がある。"Will free approximately" の文言を pin する test は 0 本（文言出現は src/interactive.rs:401 のみ）
- TUI の実行: 選択順に clean を実行し（interactive.rs:412 以降）、SkipConfirmGuard で二次確認を抑制（:409）。本 PBI は confirm 前の数字のみを変え、実行順・freed 計測は触れない
- PBI 96（A1）は scanner.rs に名前付き決定関数 measured_bytes（Pruneable(b) なら計上対象）を作る。本 PBI の重複関係関数はその隣に置き、重複知識の「共有の居場所」を消費する

## BDD 受け入れシナリオ

```gherkin
Scenario: TUI の合計は hints と兄弟行の重複を差し引く
  Given 「hints」と「app-support」を両方選択した（hints の行が app-support 行と同じ dir を数える）
  When  選択を確定して confirm 前の合計を表示する
  Then  "Will free approximately" は hints の選択分のサイズを含まない（兄弟行のサイズのみの合算）
  And   "Will free approximately" の文言と表示位置は今日どおりである

Scenario: hints のみ選択時の合計は保持される
  Given 「hints」のみを選択した
  When  選択を確定して confirm 前の合計を表示する
  Then  "Will free approximately" は hints の選択分のサイズを保持する（0 B にならない）

Scenario: 2 つの兄弟行と hints の同時選択でも差し引きは 1 回
  Given 「hints」「app-support」「library-caches」をすべて選択した
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は hints の選択分を 1 回だけ差し引く（兄弟行ごとの二重差し引きはない）

Scenario: 重なりがない選択では合計が今日どおり素加算と一致する
  Given 「hints」と重ならない target のみを選択した
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は各選択行のサイズの素加算と一致する（差し引きは発生しない）
```

## 受け入れ基準
- [ ] compute_selected_total（interactive.rs:52-71）に重複除去が入る: hints の選択分のサイズは、app-support / library-caches のいずれかが同時選択されているときだけ差し引かれ、hints のみ選択時は保持される
- [ ] 差し引きは 1 回（app-support と library-caches の両方の同時選択でも 1 回）
- [ ] 重複関係は名前キーで 1 箇所（scanner.rs の A1 決定関数 measured_bytes の隣、PBI 96 着地後）に置かれ、TUI がそれを読む（TUI 側に 2 箇所目の名前リテラルを作らない）
- [ ] test 2 本追加: 両方選択時の差し引き（3 選択の 1 回差し引き assert を含む）/ hints のみ選択時の保持
- [ ] "Will free approximately" の文言・表示位置は無変更（数字のみ整合）
- [ ] PBI 96 の CLI/JSON 総計一致 test は無修正で green（TUI 合計の差し引きは scanner 合計に触れない）
- [ ] tier 2（hint_owned_paths を使った path 単位の和集合）は本 PBI では実装しない（別 PBI に分離、下記 scope fence 1）
- [ ] DoD の 3 コマンドが通る

## テスト戦略（Outside-In TDD）

Red: 両方選択時の二重計上を実証する pin → Green: 差し引きで解消。

- 単体 Red（1 本）: src/interactive.rs tests に compute_selected_total_subtracts_hints_overlap_when_sibling_selected を追加。TestCleaner（:467-511）で "hints"（sub 展開、例: sub_size 250 + 300 の 2 hint）と "app-support"（sub_targets 空、Pruneable(1000)）を組み立て、両方選択の selected/mapping/results を渡し、合計が 1000（hints 分を含まない）であることを assert する。現行の素加算は 1550 を返すため先に Red になる（二重計上の実証）。同じ test の 2 番目の assert で「"library-caches"（Pruneable(400)）も足した 3 選択」を渡し、合計が 1400（差し引き 1 回）であることを pin する
- 単体（1 本）: compute_selected_total_keeps_hints_when_selected_alone を追加。「hints」のみ選択で合計が 550（sub_size の合計が保持される）であることを assert する。counts_toward_total の写し（除外）にした PR はこの test で赤くなる（0 B の防御 pin）
- テストの置き場所: 既存 compute_selected_total 3 本（src/interactive.rs:566-600）の隣。純関数なので CommandRunner モック不要（TestCleaner fixture で足りる）
- E2E: TUI の confirm 前の 1 行を pin する既存 test は 0 本（"Will free approximately" は src/interactive.rs:401 のみ）で、本 PBI の主体は既存 compute_selected_total tests と同層の単体 pin。文言・表示位置の無変更は既存 E2E（tests/interactive.rs）が無修正で green することで担保する
- 4 番目のシナリオ（重なりがない選択）は既存 compute_selected_total_mixed_selection（src/interactive.rs:590-600）が現行 pin として機能する（無修正で green）

## 実装計画

### 実装順（マージ順は 96 → 98）
1. 重複関係の名前キー関数: scanner.rs に「"hints" の行か / hints と重なる兄弟行か」の名前キー決定を 1 関数に寄せる（形は Negotiable）。measured_bytes（PBI 96）の隣に置き、根拠コメントは scanner.rs:48-53 の「the same dirs are counted in both」を引用する。TUI 側に名前リテラルを増やさない
2. compute_selected_total の差し引き: interactive.rs:52-71 に重複除去を入れる。除外（counts_toward_total の写し）ではなく、同時選択時のみの差し引き。cleaner 名の判定は results[cleaner_idx].name と 1. の関数で行い、シグネチャは変えない
3. test 2 本（Red → Green）
4. 検証ゲート（DoD の 3 コマンド）

### 1. 名前キー関数（形の例、Negotiable）

```rust
/// The name-level overlap between the hints row and its siblings: the one
/// home for "who counts the same dirs". The TUI sum reads this; the
/// scanner's counts_toward_total keeps its own pinned name join.
pub(crate) enum HintsOverlap {
    HintsRow,
    OverlappingSibling,
}

pub(crate) fn hints_overlap_kind(name: &str) -> Option<HintsOverlap> {
    match name {
        "hints" => Some(HintsOverlap::HintsRow),
        "app-support" | "library-caches" => Some(HintsOverlap::OverlappingSibling),
        _ => None,
    }
}
```

- scanner.rs の counts_toward_total（scanner.rs:54-56）は pin 済みの名前 join のまま変更しない（scope fence 3）。本関数は TUI 合計が読む新しい 1 箇所で、counts_toward_total をこの関数へ寄せる移設の判断は A1 territory（PBI 96 系）に残す

### 2. compute_selected_total の差し引き（重要な罠の再掲）

- 1 回目の走査で selected の (kind, size) を集める。size は sub が Some なら運搬済み sub_size、None なら results[cleaner_idx] の Pruneable(b)（現行と同じ参照。シグネチャ変更なし）
- kind に HintsRow と OverlappingSibling が両方あるときだけ、HintsRow 分の size 合計を合計から引く。片方しかないときは今日どおり素加算
- **counts_toward_total の写しにすると hints のみ選択時に "Will free approximately 0 B" になる。hints は実際に app-support / library-caches の dir を消す。必要なのは「除外」ではなく「重複除去」**
- 近似の開示: 差し引きは hints の選択分全体にかかる。library-logs の 100 MB 閾値未満の log dir のように「hints 行にしか載らない分」も同時選択時には引かれる。これは scanner 合計が今日行っている「hints 行を丸ごと合計から外す」近似（scanner.rs:211-221 の注記 1 行と同じ根拠）と同じで、TUI が scan 表の Total と同じ規則に揃う。path 単位の正確化は tier 2

### 対象ファイル
src/interactive.rs（compute_selected_total + tests 2 本）、src/scanner.rs（名前キー関数 1 つのみ。counts_toward_total・measured_bytes・run_scan / run_scan_json の出力は触れない）

### 期待する Red → Green
単体 Red 1 本が現行の素加算で先に Red（二重計上を実証）し、1. + 2. の実装後に全 Green。既存の compute_selected_total 3 本（:566-600）と build_selection_items 3 本（:513-563）、scanner テスト、PBI 96 の CLI/JSON 総計一致 test は無修正で green が pin（回帰の検知）

## pin（壊してはいけない契約）
- PBI-83 の registry pin: 1 cleaner = 1 name（"hints"）。名前キー関数はこの pin とセット（名前を変える PR を赤くする）
- counts_toward_total（scanner.rs:54-56）とその pin: scan 合計（表 + `scan --json`）の名前除外契約は 1 行も変わらない
- PBI-96 の決定関数契約: measured_bytes（Pruneable(b) なら計上対象）と CLI/JSON 総計一致 test は消費のみで変更しない
- PBI-94 の hint_owned_paths 契約: 完全一致のみ・prefix 比較禁止（src/hint/apps.rs:266-272）は tier 2 の材料として残置。本 PBI は触れない
- PBI 59 の sub size 運搬契約: selection_mapping が sub_size を運ぶ合計の再束縛（interactive.rs:8-10 の doc）は触れない
- pbi/README.md 設計方針: Outside-In TDD / CommandRunner モック / macOS 専用

## scope fence（絶対禁止）
1. tier 2（hint_owned_paths を使った path 単位の和集合）の実装はしない（別 PBI に分離。selection_mapping への path 運搬拡張を要する）
2. selection_mapping / SelectionEntry（interactive.rs:11）の拡張はしない（path 運搬は tier 2 の変更）
3. counts_toward_total（scanner.rs:54-56）の変更はしない（名前 join + pin 済みの scan 合計契約）
4. measured_bytes 決定関数の変更はしない（A1 territory、PBI 96。本 PBI は消費のみ）
5. app-support / library-caches の sub_targets 公開（UX shape 変更）はしない
6. scanner の表 / run_scan / run_scan_json の出力形変更はしない（注記行を含めて 1 行も変わらない）
7. src/hint/cleaner.rs への接触はしない（hints の detect / sub_targets / clean は触れない）
8. KNOWN_ENTRIES・閾値（64MB・1MB・100MB）・top5 truncate の変更はしない
9. CHANGELOG.md は触らない（統合側で一括記載）
10. TUI の実行ループ・SkipConfirmGuard・freed 計測（interactive.rs:409 以降）は触れない（confirm 前の数字のみ）
11. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。一覧は PBI 94 の DoD と同一、実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオ 4 本が自動テストで担保される（シナリオ 1 と 3 → 新単体 Red test、シナリオ 2 → 新単体保持 test、シナリオ 4 → 既存 compute_selected_total_mixed_selection が無修正で green）
- [ ] "Will free approximately" の文言・表示位置が無変更（数字のみ整合）であることを確認する
- [ ] PBI 96 の CLI/JSON 総計一致 test が無修正で green

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）。マージ順は 96 → 98（scanner.rs の「共有の居場所」を消費）。PBI 96 未着地で先に始める場合、名前キー関数の置き場所は counts_toward_total の隣でもよい（Negotiable、measured_bytes 自体は触れない）
- 並行で src/progress.rs・cleaner.rs（PBI 95）、src/cleaners/library_logs.rs・orphan_containers.rs（PBI 97/99）、prompt.rs（PBI 99）を触る別 PBI が進行中。本 PBI は src/interactive.rs（compute_selected_total + tests）と scanner.rs（新規関数 1 つのみ）で交差しない
- PBI 96 も scanner.rs を触るが領域が別（96: measured_bytes 1 化 + 5 サイト接続 + 総計一致 test、98: 新規関数 1 つのみ）。マージ順で衝突を避ける
- TUI confirm 画面に重複の注記 1 行を出すかは出典 §5.3 の任意事項（出すなら test 付き）。本 PBI では入れない（scanner 注記行との文言同期面を増やさない、scope fence 6）
- 出典 §7 の RICE は式どおりの計算（Reach 2 × Impact 1.5 × 0.85 / 0.25 = 10.2）
- tier 2（path 和集合、選択制）の PBI 化は本 PBI の着地後に判断する（hint_owned_paths の消費、selection_mapping の path 運搬拡張を要する）
