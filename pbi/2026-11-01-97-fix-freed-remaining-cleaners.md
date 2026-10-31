# PBI-97: library-logs と orphan-containers の freed を削除時の実測に統一する

出典: なぜなぜ分析「freed 計測・hint flow の深掘り」(.plan/whywhy-round6-freed.md、ラウンド 6、2026-10-31、main @ 53f0b88、読み取り専用分析・コード変更なし)。収束解 (i) B1(残り 2 cleaner の freed を削除時計測に統一)と (ii) B3(freed 意味論の契約を 1 箇所に集約)が本 PBI の実装計画。(iii) A3(hint flow の静的解決)と (iv) docs 未掲載は PBI 99。drift pin の同型前例は PBI 93(.plan/archived/2026-10-31-93-fix-freed-measured-on-delete.md)。PBI 93 の scope 漏れを完成させる PBI。

## ユーザーストーリー
sasurahime で `clean library-logs` / `clean orphan-containers` を使う macOS 開発者として、freed に表示される値と履歴に記録される値が削除の瞬間のディレクトリ実測値であることがほしい、なぜなら library-logs 経路は scan 時の値を流用し、選択画面が人間の応答をタイムアウト無しで待つため、freed が「画面を読んでいる時間」のぶんだけ実際の解放量からズレる実害経路になっているから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 90% / Effort 0.25 人週 = スコア 10.8

- Reach 2: `clean library-logs` / `clean orphan-containers` の利用者と、「scan は 3GB、Total freed はなぜ 1.2GB?」を切り分ける docs 読者と保守担当者(freed 意味論の検証が 6 ファイル往復から 1 hop に落ちる)。
- Impact 1.5: library-logs の人間待ち窓の解消(対象は running アプリの常時書かれるログで drift 確率最大、drift pin 無しの regression 面)と ledger rider 全経路の metric 単一化。非 breaking(freed の値が真実に寄るだけで、表示契約や削除経路は変わらない)。
- Confidence 90%: 差し替えは 2 call site(src/cleaners/library_logs.rs:159-160、src/cleaners/orphan_containers.rs:418-419)で、計測追加は削除直前の 1 walk のみ。library_caches.rs:274-280(PBI 93 済み)と同一形状に寄せる。ledger 契約(caller-owned size、src/trash.rs:293-295)は不変。
- 前提: PBI 93 着地後。なぜなぜ分析の裁定で、scan 時の分類の fresh 化と cli_fallback CLI 筋の freed 意味変更(差分への変更)は不採用。本 PBI は収束解 (i)+(ii) のみで、docs-only 部分(表・相互参照・stale 修正)を同時に着地させる。

## 背景と問題(実コード裏取り済み、main @ 53f0b88)

1. 「freed は削除時の実測」という契約が 2 cleaner にだけ着地した: PBI 93 は契約コメントと drift pin を library_caches.rs(:268-280 と drift pin :826-874)と app_support.rs(:516-521 と drift pin :1382-1421)に着地させたが、同一形状の残り 2 cleaner が残存。src/cleaners/library_logs.rs の delete_entries は items 構築(:159-160 `entries.iter().map(|e| (e.path.clone(), e.size))`)で scan_logs(:81 の scan 時 dir_size)の e.size をそのまま ledger へ。src/cleaners/orphan_containers.rs の delete_selected も同形状(:418-419 `selected.iter().map(|c| (c.path.clone(), c.size))`)で scan(:197 の scan 時 dir_size)の c.size をそのまま ledger へ。
2. library-logs の窓は人間待ちで無上限: clean の流れは scan(:232) → interactive_select(:254 呼び出し → :274-313 定義、src/prompt.rs:154-175 の dialoguer `MultiSelect::interact()`、タイムアウト無しの人間待ち) → 削除(:261)。scan → 削除の窓は「画面を読んでいる時間」で上限がない。defaults は全選択(:302)なので Enter 1 つでも早いが、large/stale タグを読む時間がそのまま窓になる。`--all` 筋(:245-247)はプロンプト無しで窓は数ミリ秒、無上限窓は interactive 筋のみ。
3. 実害は数字のズレだけ: 対象は running アプリ(Claude/zoom/LM Studio)の常時書かれるログで、窓の間に成長・回転する。freed 行(:262-265)は scan 時サイズの和を報告する(src/trash.rs:332 `freed += size`)。削除対象集合は選択時点で固定され、競合は NotFound/TrashRefused 分類で prose skip になるため、データ破壊には至らない(なぜなぜ分析の裁定)。
4. library-logs は「drift pin 無し + 契約コメント無し」の唯一残存: テスト群は `bytes_freed > 0` と skip 分類のみ assert し、drift を pin するテストが実在しない。delete_entries の doc(:148-152)は seam 真実の OR と skipped channel の契約だけを書き、サイズの来歴について沈黙している。drift pin が無いため、scan_logs の計測を scan_cache::cached_dir_size 参照に差し替えるような「高速化」regression が全テスト緑で通る(scan_cache::init は全コマンドで呼ばれ、キャッシュは最大 1 時間生きるため、freed が 1 時間前の値を報告できる)。
5. orphan-containers は同形状で drift 確率は低い: 対象は uninstalled app のフォルダで書き手がいないため構造的に低いが、PBI 93 の一般形コメントが 2 cleaner 限定なことが folklore 化を生む(「自分は scan 時のままで正しい」と読める)。含めると全 ledger rider が「削除時 dir_size」で統一され、freed 意味論 (a) が例外無しの契約になる(なぜなぜ分析の推奨)。
6. freed 意味論 4 種の知識は各 cleaner のコメントに散在: (a) ledger 総和(src/trash.rs:293-295 の caller-owned size 契約 + :332 `freed += size`)、(b) 差分(src/cleaners/generic.rs:640 `freed = size_before.saturating_sub(size_after)` / src/cleaners/uv.rs:158)、(c) 実行前全量(src/cli_fallback.rs:147 + :168)、(d) plugin cap(src/cleaners/plugin.rs:431-438)。「scan は 3GB、Total freed はなぜ 1.2GB?」の検証に 6 ファイル(trash.rs / generic.rs / uv.rs / cli_fallback.rs / plugin.rs / main.rs)往復が発生する。解釈側の freed_message(src/main.rs:762-776)の doc(:762-764)は uses_trash 軸だけを書き、来歴の表が 1 か所にも無い。
7. stale コメント: src/cleaners/orphan_containers.rs:1020-1024 の doc が「`clean_with_opts` resolves `opts.trash_mode.unwrap_or(Global)`」と旧契約を記述。実コードは :453-459 で `opts.trash_mode` を直接 thread し、統合 clean 契約(src/cleaner.rs:300-304「every impl must honour what `opts` says instead of falling back to a default」)で unwrap_or は存在しない(grep 実測: `trash_mode.unwrap_or` の一致は src/config.rs:186 の別 bool とこのコメントのみ)。統合 merge で pin テスト(:973-1016)は直ったが doc コメントだけ残った唯一の残骸。
8. 既実装チェック済み(2026-11-01、pbi/AGENTS.md の着手前手順): library_logs.rs / orphan_containers.rs の削除経路に dir_size 再計測は実在しない。本 PBI は新規実装。

## BDD 受け入れシナリオ

```gherkin
Scenario: library-logs の freed は選択画面の待ち時間の成長を反映する
  Given ~/Library/Logs の残骸が scan で計上されたあとに大きくなっている
  And   interactive_select は人間の応答をタイムアウト無しで待つ
  When  `clean library-logs` で該当エントリの削除が走る
  Then  freed は削除直前のディレクトリ実測値で報告される
  And   scan 時の値は freed に使われない

Scenario: orphan-containers の freed は削除直前の実測を報告する
  Given orphan フォルダが scan で計上されたあとにサイズが変化している
  When  `clean orphan-containers` で該当候補の削除が走る
  Then  freed は削除直前のディレクトリ実測値で報告される
  And   scan 時に見えた値は freed に使われない

Scenario: 変化のないエントリは従来どおりの freed で記録される
  Given 残骸のサイズが scan 後に変化していない
  When  `clean library-logs` で該当エントリの削除が走る
  Then  freed は従来どおり成功した削除の合計で履歴に記録される
  And   ledger の契約(caller 提供の size、ledger はパスを再計測しない)は変わらない

Scenario: scan 時の分類と選択リストの表示は変わらない
  Given scan で計上されたエントリがある
  When  interactive_select の選択リストが表示される
  Then  large/stale タグと各行の表示サイズは scan 時の値のまま変わらない
  And   削除経路の freed だけが削除時の実測に寄る
```

## 受け入れ基準
- [ ] src/cleaners/library_logs.rs の delete_entries が items 構築の同位置(:159-160)で dir_size 再計測を行い、選択画面の待ち時間の間の成長・回転が freed に反映される
- [ ] src/cleaners/orphan_containers.rs の delete_selected が items 構築の同位置(:418-419)で dir_size 再計測を行い、scan 時値が freed に混入しない
- [ ] 両 cleaner の items 構築に library_caches.rs:268-273 と同型の契約コメント 1 行が付く
- [ ] drift pin 2 本(library-logs、orphan-containers)が Red → Green 反転を確認済み(実装前の Red 実行記録つき)
- [ ] src/main.rs freed_message の doc(:762-776)に 4 意味論の表(各 1 行 + 実装 anchor)が置かれている(cli_fallback CLI 筋の freed 意味は表で明示するだけで変更しない)
- [ ] src/trash.rs delete_ledger doc(:293-295)に freed_message doc への相互参照 1 行が付く
- [ ] src/cleaners/orphan_containers.rs:1020-1024 の stale コメント(`trash_mode.unwrap_or(Global)` の旧契約)が cleaner.rs:300-304 参照に修正され、grep でコード側の `trash_mode.unwrap_or` 一致が 0 になる
- [ ] scan 時の分類(large/stale タグ、選択リストの表示サイズ)は scan 時値のまま無変更
- [ ] ledger 契約(caller-owned size、ledger は never re-walks)は無変更
- [ ] cargo test --no-fail-fast が green(既知の TCC 依存 4 失敗を除く)
- [ ] cargo clippy --tests -- -D warnings が 0、cargo fmt --check が通る

## テスト戦略
Outside-In TDD。外側の drift pin で Red を確認してから、内側の差し替えで Green にする。

- 統合: src/cleaners/library_logs.rs の tests モジュールに library-logs の drift pin 1 本。tempdir HOME に dir エントリ(例: ~/Library/Logs/<名前>/logs/a.log、サブディレクトリ構成)を作り、scan_logs で LogEntry を得たあと、サブディレクトリ内部にファイルを追加してサイズを動かす(トップレベル mtime は動かさない、PBI 93 の落とし穴節と同型)。delete_entries を呼び、freed が追加分を含む(== 削除時 dir_size)ことを assert する。Red は「現行の実装が scan 時の e.size を freed に報告する」ことで実証する。既存の seam pin 群と同型。
- 統合: src/cleaners/orphan_containers.rs の tests モジュールに orphan-containers の drift pin 1 本。write_container で orphan フォルダを作り、dir_size で scan 時の c.size を決めてからフォルダ内にファイルを追加し、delete_selected を呼び、freed が実測側(== 削除時 dir_size)を報告することを assert する。orphan_candidate_cleaner(:1025-1036)と delete_selected pin 群(:942、:1039)と同型。
- 単体(docs-only 部分は赤にならない): freed_message 表と trash.rs 相互参照、stale コメント修正は pin の対象外(コメントを見るテストは無い)。stale コメント修正は grep で検証する(`trash_mode.unwrap_or` のコード一致が 0、src/config.rs:186 の別 bool を除く)。
- モック: 外部コマンドは CommandRunner 注入(pbi/README.md の設計方針)。削除の fs 操作は tempdir で駆動し、実 Trash 操作は既存 pin 群と同じ TrashMode::Permanent で固定する。

## 見積もり
0.25 人週
内訳: drift pin 2 本(Red 再現を含む) 0.1 / 2 call site の差し替えと検証 0.05 / freed_message 表 + 相互参照 + stale コメント修正(docs-only) 0.05 / 検証ゲートと Red → Green 記録 0.05

## 実装計画

### 実装順
1. Red: src/cleaners/library_logs.rs の tests モジュールに library-logs の drift pin を追加する。サブディレクトリ内のファイル追加で、scan 時の e.size が freed に報告されることを実証する。
2. Red: src/cleaners/orphan_containers.rs の tests モジュールに orphan-containers の drift pin を追加する。c.size と実ディレクトリの差で scan 時値が freed に報告されることを実証する。
3. Green: src/cleaners/library_logs.rs delete_entries:159-160 を library_caches.rs:274-280 と同一形状に差し替える(`entries.iter().map(|e| { let size = crate::format::dir_size(&e.path); (e.path.clone(), size) })`)+ 同型契約コメント 1 行。dir_size は :3 で既に import 済み。削除直前の計測なので、選択画面の待ち時間の間の成長・回転が freed に反映される。
4. Green: src/cleaners/orphan_containers.rs delete_selected:418-419 を同一形状に差し替える + 契約コメント 1 行。全 ledger rider が「削除時 dir_size」で統一され、freed 意味論 (a) が例外無しの契約になる。
5. docs-only: src/main.rs freed_message(:762-776)の doc(:762-764)に 4 意味論の表(下記「4 意味論の表」、各 1 行 + 実装 anchor)を置く。cli_fallback CLI 筋の freed 意味は表で明示するだけで変更しない。plugin cap pin(:878、:930)は「意味変更の PR を赤くする」契約として表から参照する。
6. docs-only: src/trash.rs delete_ledger doc(:293-295)に「来歴の表は freed_message doc を参照」の相互参照 1 行を足す。
7. docs-only: src/cleaners/orphan_containers.rs:1020-1024 の stale コメントを修正する(「`clean_with_opts` resolves `opts.trash_mode.unwrap_or(Global)`」→「`opts.trash_mode` is the resolved deletion mode(統合 clean 契約、src/cleaner.rs:300-304)」)。
8. 検証ゲート(DoD の 3 コマンド)と Red → Green 反転の確認。CHANGELOG のユーザー向け 1 行(備考の案)は統合側で記載する。

### 4 意味論の表(実装順 5 の内容、docs-only)

freed_message doc に置く表の内容(各 1 行 + 実装 anchor):

| 種 | 意味 | anchor |
|---|---|---|
| (a) ledger 総和 | caller 提供の per-item size(本 PBI 着地後は全 ledger rider が削除時 dir_size)の和、成功分だけ | src/trash.rs:293-295 + :332 |
| (b) 差分 | 外部ツール実行前後の dir_size 差 | src/cleaners/generic.rs:640 / src/cleaners/uv.rs:158 |
| (c) 実行前全量 | CLI 実行成功時に実行前のディレクトリ全量を freed として報告 | src/cli_fallback.rs:147 + :168 |
| (d) 検証済み上限 | plugin の claimed freed を削除後の残り実測サイズで上限切り | src/cleaners/plugin.rs:431-438 |
| 解釈 | Trash 軸の分離表示(Logical/Physical) | src/main.rs:765-776(本 doc) |

### 対象ファイル
src/cleaners/library_logs.rs(delete_entries:159-160 の差し替え + 契約コメント + tests モジュールの drift pin)、src/cleaners/orphan_containers.rs(delete_selected:418-419 の差し替え + 契約コメント + :1020-1024 の stale コメント修正 + tests モジュールの drift pin)、src/main.rs(freed_message doc の表)、src/trash.rs(delete_ledger doc の相互参照 1 行)。

### 期待する Red → Green
drift pin 2 本が先に Red(現行は scan 時の値を freed に報告する)。実装後に全 Green。既存テスト(PBI 93 drift pin 群、library_logs/orphan_containers の seam pin 群、永続キャッシュ pin、exit code 契約)は無修正で green が pin(回帰の検知)。

### design-it-twice
- Option A(採用): 呼び出し側 2 か所で dir_size を削除直前に呼ぶ。ledger 契約(caller-owned size、src/trash.rs:293-295)を壊さず、library_caches.rs:274-280 / app_support.rs:516-521(PBI 93)と同型の統一になる。計測追加は削除直前の 1 walk のみ。
- Option B(却下): delete_ledger 側で re-walk する。caller-owned size の doc と「the ledger never re-walks a path」契約を破る(scope fence 1 番)。
- Option C(却下): orphan-containers を fence して library-logs だけにする。freed 意味論 (a) が例外 1 つ残り、(ii) の表が複雑になる。+1 行 +1 pin で統一できるため含める(なぜなぜ分析の推奨)。
- Option D(却下): scan 時の分類(Deletable 判定)も fresh 化する。library-logs の classify_entry と orphan の scan 分類の来歴まで変わり、scan 精度の別軸と衝突する(なぜなぜ分析の裁定で不採用)。

### 落とし穴
- dir_size は空ディレクトリ/消失パスで 0 を返しうる。library_logs の scan_logs は size == 0 のエントリを載せない(:82-84)ため、削除直前の計測で 0 が出てもエントリ自体は残る(seam の NotFound 分類で prose skip)。orphan_containers も同様(:198-200)。freed は成功分だけの合計(src/trash.rs:285 の doc)なので 0 計上は契約に反しない。
- 編集位置は delete_entries の items 構築(:159-160)と drift pin に限る。interactive_select 領域(:254-313)は並行で進行中の PBI 99 が触る。選択リストの各行の表示サイズ(:296)は scan 時値(e.size)のままで正しい。
- orphan_containers.rs の編集は delete_selected(:418-419)、drift pin、:1023 コメントに限る。interactive_select/pin 領域(:973-1016)は PBI 99 が触る。pin テスト selection_runs_without_a_live_spinner_and_deletion_ticks_per_second(:974)は delete_selected のソース(include_str 検証)を pin するため、差し替え後も無修正で green すること(enable_live_elapsed と reporter 経由の契約は変えない)。
- library-logs の LogEntry.path は dir と file の両方を取りうるが dir_size は両方を測れるため、fixture は「dir エントリ + 内部ファイル追加」で PBI 93 と同じ形にできる。
- drift pin の fixture はトップレベル mtime を動かさずサブディレクトリ内部のみ変更(PBI 93 の落とし穴節と同型)。library-logs/orphan-containers は永続キャッシュを通らないが、同型 fixture で drift の再現が安定する。
- 既知の TCC 失敗 1 本(cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries)は本 PBI 対象のファイル内にあるが無関係(TCC 依存)。drift pin の Red の再現と混同しない。
- dry-run は ledger に到達しない(src/trash.rs:297-298 の doc)。drift pin は削除実行でのみ有効で、dry-run の表示は本 PBI の対象外。
- 4 意味論の表は docs-only。cli_fallback CLI 筋(:147+168)の freed 意味(実行前全量)を表で明示するだけで、コード・差分意味論(generic.rs:640 / uv.rs:158)・plugin cap の意味は変更しない。

## pin(壊してはいけない契約)
- PBI 93 drift pin: src/cleaners/library_caches.rs:826-874 の delete_entries_freed_is_remeasured_after_the_scan_drifts と src/cleaners/app_support.rs:1382-1421 の apply_freed_is_remeasured_after_the_scan_drifts が無修正で green
- 永続キャッシュ pin: tests/scan.rs:498 の second_scan_reuses_the_persistent_cache が無修正で green(scan 時の分類と Total はキャッシュのまま)
- ledger 契約 pin: delete_ledger の doc(src/trash.rs:279-300、caller-owned size は :293-295)と `freed += size`(:332)は無修正。相互参照 1 行の追加は doc のみ。delete_entries pin 群(delete_entries_removes_and_sums_bytes、delete_entries_records_failures_as_skipped、delete_entries_continues_past_fatal_permanent_delete、library_caches.rs:732/:759/:780)が無修正で green
- exit code 契約 pin: app-support = blocked → SkippedEntry → all-skipped なら exit 1(apply_blocks_running_app_without_approval、app_support.rs:1359)、hint = quit timeout でも prose error・skip なし・exit 0(src/hint/cleaner.rs:338 付近の契約)が無修正で green
- plugin cap pin: src/cleaners/plugin.rs:878 の cap_claimed_at_verified_is_min_of_claimed_and_verified と :930 の clean_caps_the_claimed_freed_at_the_verified_size が無修正で green(4 意味論の表はこの契約を参照する)
- src/cleaners/library_logs.rs の既存 pin(clean_via_trait_processes_entries_with_interactive、clean_all_via_opts_merges_older_than_days_stricter_wins、clean_all_via_opts_honors_pinned_trash_mode、clean_skips_interactive_select_without_a_terminal)が無修正で green
- src/cleaners/orphan_containers.rs の既存 pin(delete_selected_records_failures_as_skipped、orphan_containers_is_never_auto_cleaned、selection_runs_without_a_live_spinner_and_deletion_ticks_per_second、clean_with_opts_pinned_permanent_deletes_for_real、clean_with_opts_pinned_trash_claims_trash、clean_options_default_carries_trash)が無修正で green
- src/hint/cleaner.rs と TUI(src/interactive.rs)には接触しない(既に削除時再計測で正しい / PBI 99 の領域)
- pbi/README.md の設計方針(dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用)

## scope fence(絶対禁止)
1. ledger の never re-walks 契約の変更はしない(caller-owned size の doc と `freed += size`、delete_ledger の re-walk 化)
2. scan 時の分類(Deletable 判定)の fresh 化はしない(なぜなぜ分析の裁定で別軸)
3. 削除経路のその他の変更はしない(delete_entries、delete_selected、delete_ledger、DeleteError 分類、DeleteCtx、TrashMode、chflags 経路、seam の NotFound / TrashRefused 分類)
4. exit code 契約の変更はしない(hint exit 0 / app-support all-skipped exit 1 は pin 済み)
5. cli_fallback CLI 筋の freed 意味(src/cli_fallback.rs:147 + :168 の実行前全量)の変更はしない(表で明示するだけ)
6. uv/generic の差分意味論(generic.rs:640 / uv.rs:158)の変更はしない
7. KNOWN_ENTRIES(src/hint/apps.rs)・TARGETS・EXCLUDED の表、閾値、top5 truncate は触れない
8. src/hint/cleaner.rs には接触しない(既に削除時再計測で正しい)
9. TUI の build_selection_items(src/interactive.rs:13-48)には接触しない
10. src/cleaners/library_logs.rs の interactive_select 領域(:254-313)には接触しない(並行で進行中の PBI 99 が触る。編集は delete_entries の items 構築 :159-160 と drift pin に限る)
11. src/cleaners/orphan_containers.rs の interactive_select/pin 領域(:973-1016)には接触しない(PBI 99 が触る。編集は delete_selected :418-419、drift pin、:1023 コメントに限る)
12. CHANGELOG.md は触らない(統合側で一括記載)
13. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green(既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash)
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] drift pin 2 本(library-logs、orphan-containers)が Red → Green 反転を確認済み(実装前の Red 実行記録つき)
- [ ] ledger never re-walks 契約 pin(src/trash.rs の doc と `freed += size`)が無修正で green
- [ ] exit code 契約 pin(hint exit 0 / app-support all-skipped exit 1)が無修正で green
- [ ] PBI 93 既存 drift pin(library_caches.rs:826-874、app_support.rs:1382-1421)が無修正で green
- [ ] BDD 受け入れシナリオ 4 本が自動テストとして実装され、パスする
- [ ] freed_message doc の 4 意味論表、trash.rs 相互参照、stale コメント修正が完了(grep でコード側の `trash_mode.unwrap_or` 一致が 0)
- [ ] CHANGELOG のユーザー向け 1 行案が本 PBI に記載済み(記載自体は統合側)

## 備考
- 実装は worktree で subagent-driven(AGENTS.md の実装ワークフロー)。ブランチ案: fix/freed-remaining-cleaners
- PBI 99 と src/cleaners/library_logs.rs・orphan_containers.rs を共有するが別関数領域(本 PBI は delete_entries:159-160 と delete_selected:418-419、99 は interactive_select 領域 :254-313 / :973-1016)。マージ順は 97 → 99
- CHANGELOG ユーザー向け 1 行案: 「Freed の意味は対象によって異なる: 削除時計測の和(全 ledger rider)/外部ツール実行前後の差分/CLI 実行成功時の実行前全量/plugin claimed の検証済み上限」
- 文書文字種規約(SHIFT-JIS 表現可能文字)に従う。docs-charset スキルで検証済み。
