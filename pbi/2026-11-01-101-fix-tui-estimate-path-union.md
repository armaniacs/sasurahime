出典: なぜなぜ分析 round 7（.plan/whywhy-round7-tui-tier2.md、2026-11-01、main @ be020c3、読み取り専用分析・コード変更なし）の §5.1 採用裁定 + §5.2 PBI 形状。tier 1（PBI 98）着地後の tier 2。マージ順は 98（tier 1）→ 101（本 PBI）

# PBI-101: TUI の "Will free approximately" を path 単位の共有に寄せる（tier 2）

## ユーザーストーリー
sasurahime の TUI（引数なし起動）でキャッシュを選択する開発者として、confirm 前の "Will free approximately" の数字が選択の組合せごとに方向が変わらないことがほしい、なぜなら tier 1（PBI 98）着地後も、選んだ hints の sub がどこにあるか（~/Library/Caches か ~/Library/Application Support か）でズレの方向が変わり、「思ったより消せない」過小と「思ったより多く消える」過大が同じ 1 行で交互に現れ、目安として学習不能だから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 80% / Effort 0.42 人週 = スコア ≒ 5.7

- Reach 2: TUI 利用者（confirm 前の 1 行のみ。`scan` / `clean` の CLI 利用者は今日どおり対象外）
- Impact 1.5: 見積もりの信頼性。round 7 分析の実測で、hint 所有 23 path のうち 16（≒70%）が 2 sibling の計上集合と重複し、cross-sibling 選択と Homebrew 選択で系統的・GB 級・両方向の誤差が実在。per-sibling net + sibling 側 counted 述語で解消する
- Confidence 80%: 主因（cross-sibling + Homebrew）は静的近似（両側とも KNOWN_ENTRIES / TARGETS / excluded_by 由来）で閉じるが、SelectionEntry 拡張と tier 1 pin 2 本の期待値更新を伴うため tier 1（85%）より下げた
- 前提: round 7 §5.1 の採用裁定（tier 1 よりスコアは小さいが、誤差が「目安」の範囲を超え、かつ選択パターン次第で方向が変わる（ユーザーが学習不能）ため、round 6 で A6 を PBI 化したのと同じ基準を満たす）。PBI 98（tier 1）が着地済みで、本 PBI の形状（§5.2）が確定している

## 背景/問題

tier 1（PBI 98）着地後も "Will free approximately"（interactive.rs:429）は compute_selected_total（interactive.rs:60-99）の行単位（名前キー）差し引きのまま。1 パス目で (kind, size) を集計し（interactive.rs:65-91）、`hints_selected && sibling_selected` のとき hints_size 全額を 1 回差し引く（interactive.rs:94-95）が、「どの sibling が選ばれたか」「sub がどの dir にあるか」は見ない。round 7 分析の実測 3 点（いずれも main @ be020c3 のコードで確認済み）:

1. cross-sibling 過小差し引き（under 方向）: tier 1 は「sibling が 1 つでも選ばれたら hints_size 全額」を差し引き「どの sibling が選ばれたか」を見ない。hints + library-caches のみ選択 → library-caches が数えるのは ~/Library/Caches の KNOWN 非 skip 項目だけ（scan_caches、library_caches.rs:103-146）なので、~/Library/Application Support にある AppSupport 所有 sub（Chrome Cache_Data / Code 系、GB 級。64MB 閾値の上に上限なしで育つ）の全額が過小報告。逆（hints + app-support のみ）も対称に過小（Caches 所有分 9 path の全額）
2. Homebrew の誤差し引き（under 方向）: Homebrew は hint 所有（KNOWN_ENTRIES Caches 非 skip、apps.rs:121-131）だが library-caches は excluded_by で排除（`excluded_by("Homebrew") → Some("brew")`、test pin 済み library_caches.rs:429-433）→ hints Homebrew sub + library-caches 選択で、sibling が数えていない dir のサイズを差し引く。所有（PBI 94 の path）と計上（sibling の counted 集合）は一致しない実例
3. 未 net の二重計上（over 方向）: hints Homebrew sub × brew 行が同一 ~/Library/Caches/Homebrew を数えるのに net 無し（`hints_overlap_kind("brew")` は None）。hints Logs sub × library-logs 行も未 net。hints_overlap_kind は 2 sibling（"app-support" | "library-caches"）に固定で、brew / library-logs は未 model（scanner.rs:83-89）
4. 重複の割合: hint_owned_paths() は全 23 path（Caches 10 + AppSupport 7 + Logs 6、apps.rs:266-272）。うち 16（≒70%）が 2 sibling の counted 集合と重複する（Caches 9 + AppSupport 7。Homebrew 1 件は hint 所有だが library-caches が excluded_by で排除、Logs 6 件は library-logs と重複するが未 model）→ 系統的・GB 級・両方向の実害が実在する

## 根本原因（round 7 分析、3 立場）

- 攻撃者/ハッカー視点: 「同一 dir を数える行」の判定が行単位（名前キー）で行われ、path 単位の真実（どの dir が実際に共有か）が 1 箇所の契約に無い。tier 1 の差し引き（under）と未 net の二重計上（over）が同じ行単位の判定から両方向に生まれ、3 つの重複判定（excluded_by / hints_overlap_kind / counts_toward_total）が散在している
- 半年後の保守担当者視点: path 級の共有判定の正本（どの側が持つか）と PBI 59 哲学（the size rides in the mapping）の適用範囲が doc に無い。所有（hint_owned_paths）と計上（sibling counted 集合）の区別が無いため、Homebrew のような「所有はされるが計上はされない」例外を各実装者が毎回再発見する
- エンドユーザー視点: 同じ 1 行の数字が、行単位の判定と path 単位の真実のズレで、選択の組合せごとに別の方向にズレる。dir の所在と共有関係が選択画面に可視化されず、内訳の不在が誤差を発見不能にしている

## 発見の詳細（実コードの確認、main @ be020c3）

- tier 1 差し引きの実体（アンカー再確認、ラウンド 6 着地後に行番号ずれ済み）: compute_selected_total（interactive.rs:60-99）は 1 パス目で (kind, size) を集計し、`hints_selected && sibling_selected` のとき `total - hints_size` を 1 回差し引き（interactive.rs:94-95）。差し引きのキーは行名: `hints_overlap_kind(results[*cleaner_idx].name)`（interactive.rs:82）
- SelectionEntry: interactive.rs:12、`(usize, Option<(&'static str, u64)>)`。運ぶのは sub 名と sub_size のみで、path は選択の時点で捨てられている。build_selection_items（interactive.rs:14-49）が hints 行を per-hint sub 展開（sub_size 経由）、app-support / library-caches を単一行（Pruneable(b) 経由）にする
- tier 1 pin 2 本: `compute_selected_total_subtracts_hints_overlap_when_sibling_selected`（interactive.rs:633-666、fixture は合成名 "slack" 250 / "xcode" 300 + app-support 1000 + library-caches 400）、`compute_selected_total_keeps_hints_when_selected_alone`（interactive.rs:669-686、dedup not exclusion の pin）。fixture は TestCleaner（interactive.rs:495-511、合成 Cleaner 実装）で実在の hint display_name を使っていない
- scanner 側: HintsOverlap（scanner.rs:70-78）= HintsRow | OverlappingSibling。hints_overlap_kind（scanner.rs:83-89）は名前キーで `"app-support" | "library-caches"` に固定、`"brew"` / `"library-logs"` は None（未 model）。counts_toward_total（scanner.rs:60-62）は現行維持（名前 join、pin 済み）。round 6 A1 の measured_bytes（scanner.rs:13-18）は着地済み
- hint_owned_paths: apps.rs:266-272、非 skip KNOWN_ENTRIES の path_suffixes × base_dir。全 23 path = Caches 10 + AppSupport 7（5 entry: Slack/Cache、Claude/Cache、obsidian/Cache、Code/Cache・Code/CachedExtensionVSIXs・Code/CachedData、Google/Chrome/Default/Cache_Data）+ Logs 6。MANAGED_BY_HINTS_TAG（apps.rs:275）= "  [managed by hints]"。BaseDir = Caches / AppSupport / Logs（hint/mod.rs:13-17）
- library-caches counted 集合: scan_caches（library_caches.rs:103-146）は excluded_by(&name) を known lookup の前に判定し、除外名は entry にすらならない（library_caches.rs:121-123）。excluded_by（library_caches.rs:50）は `excluded_by("Homebrew") → Some("brew")` を test pin（library_caches.rs:429-433）。detect は include_unknown=false（library_caches.rs:102 の doc）→ counted 集合 = hint 所有 Caches path 9 個（Homebrew を除く）。計測は dir_size と同一規格
- app-support counted 集合: TARGETS（app_support.rs:28-101）の各 target が hint_name を持ち、subpaths は hint 所有 path の上位集合（Slack 6 subpaths ⊇ hint 1、VSCode caches 11 ⊇ 3、Google Chrome 11 ⊇ 1、等）。scan_candidates（app_support.rs:191-212）の閾値は `hint_entry(target).map(e.threshold_bytes)` から派生し、hints と同一閾値。計測は dir_size と同一規格
- hints sub 筋: sub_targets（hint/cleaner.rs:338-345）= collect() → collect_hints（hint/detector.rs:15-50、閾値以上・top5 truncate（detector.rs:44）・dir_size 計測、sort 降順）。static_sub_hint（hint/cleaner.rs:260-284）は clean 時の静的解決（PBI 100）。find_known_entry（hint/cleaner.rs:183）は private（fn、pub でない）
- 閾値ドリフト窗口: LibraryCachesCleaner.threshold_override（library_caches.rs:238）は CLI フラグ経由でのみ設定、LibraryCachesCleaner::new は None（library_caches.rs:246）。TUI は標準 cleaners 構築を使い override の差し込み筋を持たない → TUI の選択和集合では runtime 変動が None に固定
- 未実装確認（フェーズ 0）: sibling 側 counted 述語（counts_owned）と `Cleaner::sub_owned_paths` は src/ に存在しない（grep + 実コード読みで確認済み。overlap 系の出現は tier 1 の hints_overlap_kind と counts_toward_total のみ）。未実装のため fix として書ける

## BDD 受け入れシナリオ

```gherkin
Scenario: AppSupport 所有 sub と library-caches の同時選択で過小差し引きが消える
  Given 「hints」の sub のうち ~/Library/Application Support にある sub（例: Google Chrome / VSCode caches）と「library-caches」を選択した
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は library-caches が数えない AppSupport 所有分まで差し引かない（sibling が数える分だけ差し引く）
  And   "Will free approximately" の文言と表示位置は今日どおりである

Scenario: Caches 所有 sub と app-support の同時選択でも対称に過小差し引きが消える
  Given 「hints」の sub のうち ~/Library/Caches にある sub と「app-support」を選択した
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は app-support が数えない Caches 所有分まで差し引かない

Scenario: Homebrew sub と library-caches の同時選択で誤差し引きが消える
  Given 「hints」の Homebrew sub と「library-caches」を選択した（library-caches は excluded_by で Homebrew を数えない）
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は Homebrew 分を差し引かない（sibling が数えない dir は差し引かない）

Scenario: 2 sibling 同時選択の差し引きと hints 単独選択の保持は今日どおりの意味論
  Given 「hints」「app-support」「library-caches」をすべて選択した（重複の同時選択）と、「hints」のみを選択した場合の両方
  When  選択を確定して confirm 前の合計を表示する
  Then  合計は共有分を差し引き、tier 1 の pin と同じ行単位の意味論（重複同時選択で差し引き / hints 単独選択で保持）を保つ
```

## 受け入れ基準
- [ ] sibling 側 counted 述語 2 つが新設される: `library_caches::counts_owned`（BaseDir, path）（KNOWN 非 skip Caches から excluded_by に当たる名前を除いた集合）と `app_support::counts_owned`（BaseDir, path）（TARGETS subpaths 含有）。静的・pure・test 各 1 本。hint 側のみ（hint_owned_paths）の判定は Homebrew を誤共有するため、正本は sibling 側に置く
- [ ] 新 trait メソッド `Cleaner::sub_owned_paths` が追加される: 既定 `vec![]`、hints のみ実装（find_known_entry 経由）。sub_targets の戻り値は拡張しない（CLI に波及するため避ける）
- [ ] SelectionEntry が拡張される: sub 行の所有 path 注記 `owned: Option<(BaseDir, &'static [&'static str])>` を加え、build_selection_items で運搬する。display_name → find_known_entry を sum 時に再 lookup する string re-match は採らない（PBI 59 逆行）
- [ ] compute_selected_total が per-sibling net に一般化される: tier 1 の 2 変数（hints_size / sibling_selected）を「選択された sibling の集合 × sub の所有 dir」の組ごとの差し引きに一般化。Homebrew（どちらの sibling も数えない）は差し引かれない
- [ ] tier 1 pin 2 本（interactive.rs:633 / 669）は消さず、fixture 拡張（TestCleaner に sub_owned_paths 実装）+ 期待値更新のみで green になる（行単位意味論は無修正）
- [ ] 新 test 3 本追加: cross-sibling 2 本（AppSupport sub + library-caches、対称に Caches sub + app-support）+ Homebrew 1 本
- [ ] "Will free approximately" の文言・表示位置は無変更（数字のみ整合）
- [ ] brew × hints Homebrew / library-logs × hints Logs の未 net 二重計上は本 PBI では実装しない（sibling 集合拡張の将来 PBI に分離、下記 採らない裁定に 1 行明記）
- [ ] DoD の 3 コマンドが通る

## テスト戦略（Outside-In TDD）

Red: cross-sibling 過小差し引き pin + Homebrew 誤差し引き pin → Green: per-sibling net + sibling 述語で閉じる。

- 単体 Red（2 本）: src/interactive.rs tests に cross-sibling pin（hints AppSupport sub + library-caches のみ → tier 1 が AppSupport 所有分の全額を過小差し引きする状態）と Homebrew pin（hints Homebrew sub + library-caches → tier 1 が sibling が数えない分を誤差し引きする状態）を追加。fixture（TestCleaner）に sub_owned_paths を実装し、実在の所有 path（例: AppSupport の Google/Chrome/Default/Cache_Data、Caches の Homebrew）を返す。現行の tier 1 差し引きは両方とも「hints_size 全額差し引き」のため先に Red になる
- 単体 Green: sibling 述語 2 つ（library_caches::counts_owned / app_support::counts_owned、test 各 1 本）+ compute_selected_total の per-sibling net 一般化で Red 2 本を閉じる
- 既存 pin 2 本: `compute_selected_total_subtracts_hints_overlap_when_sibling_selected`（interactive.rs:633）と `compute_selected_total_keeps_hints_when_selected_alone`（interactive.rs:669）は消さない。fixture 拡張（sub_owned_paths 実装）で注記が付くようになるため期待値のみ更新する（行単位意味論: 重複同時選択で差し引き / 単独選択で保持 は不変）
- 既存 pin: compute_selected_total_mixed_selection（interactive.rs:618）と build_selection_items 3 本、scanner テストは無修正で green が pin（回帰の検知）
- テストの置き場所: 既存 compute_selected_total tests の隣（src/interactive.rs tests）。純関数なので CommandRunner モック不要（TestCleaner fixture で足りる）
- E2E: TUI の confirm 前の 1 行を pin する既存 test は 0 本（"Will free approximately" は interactive.rs:429 のみ）で、本 PBI の主体は compute_selected_total tests と同層の単体 pin。文言・表示位置の無変更は既存 E2E（tests/interactive.rs）が無修正で green することで担保する

## 実装計画（round 7 収束解 §5.2 + §5.4 から）

### 実装順
1. sibling 側 counted 述語 2 つ: `library_caches::counts_owned`（BaseDir, path）（KNOWN 非 skip Caches から excluded_by に当たる名前を除いた集合。Homebrew 排除を消費）と `app_support::counts_owned`（BaseDir, path）（TARGETS subpaths 含有）。静的・pure・test 各 1 本。**hint 側のみの判定は Homebrew を誤共有するため、正本は sibling 側**
2. 新 trait メソッド: `Cleaner::sub_owned_paths`（既定 `vec![]`、cleaner.rs:381-383 の sub_targets と同型）を追加し、hints のみ実装（find_known_entry 経由）。**sub_targets の戻り値拡張は CLI（clean hints --sub、offer）に波及するため避ける**。既定空なら呼び出し側の変更は TUI のみ
3. SelectionEntry 拡張: sub 行の所有 path 注記 `owned: Option<(BaseDir, &'static [&'static str])>`（base_dir + path_suffixes）を加え、build_selection_items で運搬。**string re-match（display_name → find_known_entry を sum 時に再 lookup）は PBI 59 の排除対象（the sum re-binds by value, not by string re-match）の逆行のため採らない。注記は mapping に乗せる**。TestCleaner fixture 拡張（sub_owned_paths 実装）をこの段で入れ、既存 test 全部 green を確認
4. compute_selected_total の per-sibling net への一般化: tier 1 の 2 変数を「選択された sibling の集合 × sub の所有 dir」の組ごとの差し引きに一般化。Homebrew（どちらの sibling も数えない）は差し引かれない。**pin test 2 本の行単位意味論（重複同時選択で差し引き / 単独選択で保持）は不変、fixture 拡張 + 期待値更新のみ（pin は消さない）**。新 test 3 本（cross-sibling 2、Homebrew 1）
5.（任意）TUI confirm に重複注記 1 行（scan の注記行に倣う、出すなら test 付き）。必須ではない

### 落とし穴
- owned 注記に flat 一覧（hint_owned_paths の全 23 path）をそのまま和集合として使わない（全 sub の和集合になってしまい、選択単位の重複が計算できない）。entry 単位の (base_dir, path_suffixes) を sub から運搬する
- hint::BaseDir を TUI に持ち込む結合は type alias（例: OwnedPaths）で 1 箇所に寄せる。String で "Caches" 等を受ける置換は型安全性を失い、PBI 59 の「by value, not by string re-match」に反する
- 所有 path の membership は完全一致のみ・prefix 比較禁止（Default/Cache と Default/Cache_Data。PBI 94 の pin、apps.rs:295-308）

## pin（壊してはいけない契約）
- PBI-83 の registry pin: 1 cleaner = 1 name（"hints" / "app-support" / "library-caches"）。名前キー判定（hints_overlap_kind、counts_owned の導出元の名前照合）はこの pin とセット（rename で red）
- PBI 59 の sub size 運搬契約: selection_mapping が sub_size を運び、合計は string re-match しない（interactive.rs:9-11 の doc）。本 PBI はこの哲学を path 注記に広げるだけで、壊さない
- PBI 98 / tier 1 pin 2 本: `compute_selected_total_subtracts_hints_overlap_when_sibling_selected`（interactive.rs:633）と `compute_selected_total_keeps_hints_when_selected_alone`（interactive.rs:669）の行単位意味論（重複同時選択で差し引き / 単独選択で保持）は消さない・変えない（fixture 拡張 + 期待値更新のみ）
- PBI 94 の hint_owned_paths 契約: 完全一致のみ・prefix 比較禁止（apps.rs:266-272）は本 PBI でも消費のみ・変更しない。MANAGED_BY_HINTS_TAG（apps.rs:275）と dry-run タグも触れない（PBI 94 fence）
- counts_toward_total（scanner.rs:60-62）とその pin: scan 合計（表 + `scan --json`）の名前除外契約は 1 行も変わらない
- PBI 96 系の measured_bytes（scanner.rs:13-18）契約: 消費のみで変更しない
- collect_hints / sub_targets / threshold_bytes の契約: top5 truncate（hint/detector.rs:44）と閾値判定は 1 行も変わらない。mapping 注記は既存 sub 行（top5 に残った行）の付帯情報のみ

## 採らない裁定（記録）
- **完全 path 和集合（ScanResult path 運搬）は不採用**: 全 cleaner の counted path 運搬は detect 側の全拡張で effort が割に合わない。runtime 変動のうち TUI に届くのは None（threshold_override が TUI では固定）で、静的近似（両側とも KNOWN_ENTRIES / TARGETS / excluded_by 由来）で主因（cross-sibling + Homebrew）は閉じる
- **brew × hints Homebrew / library-logs × hints Logs の未 net 二重計上（over 方向）は射程外**: sibling 集合の拡張（hints_overlap_kind に brew / library-logs を足す、あるいは net 対象に加える）は将来 PBI として分離する
- **threshold_override は残り窗口として明記**: library_caches.rs:238 は CLI 専用で TUI は None に固定（LibraryCachesCleaner::new、library_caches.rs:246）。将来 TUI に override 差し込みが生えた場合、静的近似は閾値ドリフトに弱くなる
- **Logs 所有 path は注記のみ**: 本 PBI では sub 行の owned 注記まで。library-logs との net は行わない

## scope fence（絶対禁止）
1. 完全 path 和集合（ScanResult への path 運搬）の実装はしない（上記 採らない裁定）
2. ScanStatus（cleaner.rs）の変更はしない
3. sub_targets の戻り値拡張はしない（CLI に波及する。運搬は新 trait メソッド sub_owned_paths で行う）
4. collect_hints / sub_targets / threshold_bytes には触れない（top5 truncate と閾値契約は不変）
5. KNOWN_ENTRIES / TARGETS の表変更はしない（本 PBI は消費のみ）
6. explorer.rs は変更しない（MANAGED_BY_HINTS_TAG の表示は PBI 94 の現行のまま）
7. brew × hints Homebrew / library-logs × hints Logs の未 net 実装はしない（射程外、将来 PBI）
8. dry-run タグの変更はしない（PBI 94 fence）
9. CLI 側の Total reclaimable（scan 表 + `scan --json`）の変更はしない（counts_toward_total 系は触れない）
10. CHANGELOG.md は触らない（統合側で一括記載）
11. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。一覧は PBI 94 の DoD と同一、実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] tier 1 pin 2 本（interactive.rs:633 / 669）が意味論無修正で green（fixture 拡張 + 期待値更新のみ）
- [ ] "Will free approximately" の文言・表示位置が無変更（数字のみ整合）であることを確認する
- [ ] BDD 受け入れシナリオ 4 本が自動テストで担保される（シナリオ 1・2・3 → 新単体 test 3 本、シナリオ 4 → tier 1 pin 2 本の意味論不変 green）

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）
- PBI 59 哲学（the size rides in the mapping）の適用範囲を本 PBI で 1 行明文化する: **sub 行の owned 注記（path）も mapping に乗せる。sum 時の display_name → find_known_entry 再 lookup（string re-match）は採らない**。find_known_entry（hint/cleaner.rs:183）は private のまま
- round 7 分析の §5.2（PBI 形状）と §5.4（実装順）を実装計画に写した。アンカー行番号は main @ be020c3 で再確認済み（ラウンド 6 着地後のずれを含む）
- 触れるファイル: src/interactive.rs（SelectionEntry + build_selection_items + compute_selected_total + tests）、src/cleaner.rs（trait 既定メソッド 1 つ）、src/hint/cleaner.rs（sub_owned_paths 実装のみ）、src/cleaners/library_caches.rs + app_support.rs（counts_owned 述語 1 つずつ）。src/scanner.rs は触れない（共有判定の正本は sibling 側 cleaner に置く）
- RICE は式どおりの計算（Reach 2 × Impact 1.5 × 0.80 / 0.42 = 5.71… ≒ 5.7）。tier 1（PBI 98、スコア 10.2）より小さいが、round 7 分析で両方向の系統的実害が実在と実測された
