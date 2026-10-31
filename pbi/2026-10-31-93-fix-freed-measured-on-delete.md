# PBI-93: app-support と library-caches の freed を削除時の実測に統一する

出典: なぜなぜ分析「hint 重複 Phase 2/3」(.plan/whywhy-round5-hint-dedup.md、ラウンド 5、2026-10-31、main @ 56e96cf、読み取り専用分析・コード変更なし)。分析の Phase 3(freed 計測の削除時再計測統一)が本 PBI の実装計画。Phase 2(hint_owned_paths + scanner 別枠 + タグ)は別 PBI 94。

## ユーザーストーリー
sasurahime で `clean library-caches` / `clean app-support` を使う macOS 開発者として、freed に表示される値と履歴に記録される値が削除の瞬間のディレクトリ実測値であることがほしい、なぜなら library-caches 経路は scan 時の値を流用して永続キャッシュを通るため、freed が最大 1 時間前の値になりうる唯一の経路になっているから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 90% / Effort 0.25 人週 = スコア 10.8

- Reach 2: `clean app-support` / `clean library-caches` の利用者と、freed の説明を読む docs 読者(計測時点の説明が 3 文から 1 文に落ちる)。
- Impact 1.5: library-caches の 1 時間窓の解消(3 計測時点で唯一の実害)と freed metric の単一化。非 breaking(freed の値が真実に寄るだけで、表示契約や削除経路は変わらない)。
- Confidence 90%: 差し替えは 2 call site(app_support.rs:509、library_caches.rs:231-233)で、計測追加は削除直前の 1 walk のみ。ledger 契約(caller-owned size、src/trash.rs:293-295)は不変。
- 前提: PBI 89(symlink スコープ検査の統一)着地後。なぜなぜ分析の裁定で、scan 時の分類の fresh 化と 3 cleaner 統合は不採用。本 PBI は Phase 3 のみ。

## 背景と問題(実コード裏取り済み、main @ 56e96cf)

1. 計測時点が 3 種: hint 経路は削除時点で format::dir_size を再計測する(src/hint/cleaner.rs:151-154、PBI-80 が寄せた契約)。app-support は scan 時の Candidate.size を流用する(src/cleaners/app_support.rs apply_selection:509 `items.push((path, cand.size))`)。同一 run 内の scan→delete 窓は数秒だが、quit 待ちの間の書き込みを反映しない(items ループは prepare_app の quit 待ち後に回るため、scan 時値のままではその間の変化が載らない)。library-caches は scan 時の CacheEntry.size を流用し(src/cleaners/library_caches.rs delete_entries:231-233 `(e.path.clone(), e.size)`)、scan_cache::init は全コマンドで呼ばれる(src/main.rs:1302-1305)ため、clean library-caches の scan も mtime キーの永続キャッシュ(MAX_AGE_SECS = 60*60、src/scan_cache.rs:14)を通る。サブディレクトリ内部の変化はトップレベルの mtime を動かさないため、freed が最大 1 時間前の値になりうる唯一の経路。「Deletion paths never consult the cache」(scan_cache.rs:10)は削除呼び出し自体の話で、サイズの来歴は scan 時のまま。
2. 実害の所在: 3 計測時点のうち library-caches だけが実害を持つ。app-support の窓は数秒(interactive_select + confirm の間)で、running アプリは blocked になるため書き込み主体がいない。hint は削除時再計測で正しい。library-caches の分類(Deletable/BelowThreshold)も同じキャッシュ値で行われる(library_caches.rs:119 の cached_dir_size、:128 の classify)が、1 時間以内に 64MB を跨いだキャッシュが BelowThreshold のままになる副次は scan 精度の別軸で本 PBI 対象外。
3. ledger 契約は不変: delete_ledger は caller 提供の size をそのまま freed に足す(src/trash.rs:293-295 の doc「the caller owns the per-item size computation... (the ledger never re-walks a path)」、:332 `freed += size`)。呼び出し側が新鮮な値を渡すだけで契約は変わらず、freed は成功分だけの合計(:285 の doc)という既存の来歴も変わらない。
4. 既実装チェック済み(2026-10-31、pbi/AGENTS.md の着手前手順): src/cleaners/ 配下の削除経路に dir_size 再計測は実在しない(app_support.rs:509 は cand.size、library_caches.rs:232 は e.size のまま)。本 PBI は新規実装。

## BDD 受け入れシナリオ

```gherkin
Scenario: library-caches の freed は削除直前の実測を報告する
  Given ~/Library/Caches/typescript の残骸が scan で計上されたあとに大きくなっている
  And   永続キャッシュはサブディレクトリ内部の変化を反映しない(mtime キーはトップレベルのみ)
  When  `clean library-caches` で該当エントリの削除が走る
  Then  freed は削除直前のディレクトリ実測値で報告される
  And   scan 時の値(最大 1 時間前の永続キャッシュ値)は freed に使われない

Scenario: app-support の freed は quit 待ち後の実測を報告する
  Given 対象アプリが走っていて quit 待ちの間にキャッシュへの書き込みが起きる
  When  `clean app-support` で該当候補の削除が走る
  Then  freed は quit 待ち後(削除直前)のディレクトリ実測値で報告される
  And   scan 時に見えた値は freed に使われない

Scenario: 変化のないキャッシュは従来どおりの freed で記録される
  Given 残骸のサイズが scan 後に変化していない
  When  `clean library-caches` で該当エントリの削除が走る
  Then  freed は従来どおり成功した削除の合計で履歴に記録される
  And   ledger の契約(caller 提供の size、ledger はパスを再計測しない)は変わらない

Scenario: scan 時の分類はキャッシュのまま保たれる
  Given 永続キャッシュに保存された scan 結果がある
  When  ファイルの変更なしで再度 scan が走る
  Then  分類(Deletable 判定)と Total はキャッシュから配られ変わらない
  And   削除経路の freed だけが削除時の実測に寄る
```

## 受け入れ基準
- [ ] src/cleaners/app_support.rs の apply_selection が items 構築の同位置(:509)で dir_size 再計測を行い、quit 待ち後の書き込みが freed に反映される
- [ ] src/cleaners/library_caches.rs の delete_entries が削除直前(:231-233 の同位置)で dir_size 再計測を行い、永続キャッシュの古さが freed に混入しない
- [ ] scan 時の分類(Deletable 判定)はキャッシュのまま(永続キャッシュ pin を壊さない)
- [ ] ledger 契約(caller-owned size、ledger は never re-walks)は無変更
- [ ] drift pin 2 本(library-caches、app-support)が Red → Green 反転を確認済み
- [ ] 文書化 3 行が README.md または docs/SUPPORTED.md に記載済み
- [ ] cargo test --no-fail-fast が green(既知の TCC 依存 4 失敗を除く)
- [ ] cargo clippy --tests -- -D warnings が 0、cargo fmt --check が通る

## テスト戦略
Outside-In TDD。外側の drift pin で Red を確認してから、内側の差し替えで Green にする。

- E2E/統合: tests/library_caches.rs に drift pin 1 本。tempdir HOME に既知キャッシュディレクトリ(例: ~/Library/Caches/typescript)をサブディレクトリ構成で作る。scan を 1 回走らせて永続キャッシュに載せたあと、サブディレクトリ内にファイルを追加してサイズを動かす(トップレベル mtime は動かさない = 永続キャッシュが生きる構成)。clean を走らせて、freed が追加分を含むことを検証する。Red は「現行の実装が scan 時の値(永続キャッシュ値)を freed に報告する」ことで実証する。binary 経由では freed の確認が表示経由になるため、必要なら cleaner 直呼び出しに寄せる(既存の delete_entries pin 群と同型、Negotiable)。
- 統合: src/cleaners/library_caches.rs の tests モジュールに drift pin。scan で CacheEntry を得たあとディレクトリに追加してから delete_entries を呼び、freed が追加分を含むことを assert する。
- 単体: src/cleaners/app_support.rs の tests モジュールに app-support の drift pin。apply_selection pin 群(apply_deletes_only_selected_candidates など)と同型で、Candidate.size を実ディレクトリより小さく差し替えた fixture(または scan 後のファイル追加)で freed が実測側を報告することを assert する。現行は cand.size をそのまま渡すため Red。
- モック: 外部コマンドは CommandRunner 注入(pbi/README.md の設計方針)。削除の fs 操作は tempdir で駆動し、実 Trash 操作は既存テストと同じ backend override で固定する。

## 見積もり
0.25 人週
内訳: drift pin 2 本(Red 再現を含む) 0.1 / 2 call site の差し替えと検証 0.1 / 文書化 3 行 0.05

## 実装計画

### 実装順
1. Red: tests/library_caches.rs(または src/cleaners/library_caches.rs の tests モジュール)に library-caches の drift pin を追加する。削除前にディレクトリサイズを変える fixture で、scan 時値が freed に報告されることを実証する。
2. Red: src/cleaners/app_support.rs の tests モジュールに app-support の drift pin を追加する。Candidate.size と実ディレクトリの差で scan 時値が freed に報告されることを実証する。
3. Green: src/cleaners/app_support.rs apply_selection:509 の `items.push((path, cand.size))` を同位置で dir_size 再計測に差し替える(`let size = crate::format::dir_size(&path); items.push((path, size));`)。items ループは prepare_app(quit 待ち最大 10 秒)の後に回るので、quit 待ち後の書き込みも計測に反映される。
4. Green: src/cleaners/library_caches.rs delete_entries:231-233 の size を dir_size 再計測に差し替える(`entries.iter().map(|e| { let size = crate::format::dir_size(&e.path); (e.path.clone(), size) })`)。split_running 後の削除直前なので、1 時間窓の古さが消える。
5. 文書化 3 行(README.md または docs/SUPPORTED.md。README.md は Safety first :129 の freed 表示行 :131-146 付近と Deletion history :224-229 付近が既存の近接先):
   1. 「freed は削除時点の format::dir_size 実測(全経路共通 metric)」
   2. 「quit timeout は hint 経路では exit 0(prose error)、app-support では blocked が SkippedEntry になり all-skipped なら exit 1(pin 済み契約)」
   3. 「履歴の Cleaner 列: hint 経路は accepted エントリ毎に "hints"、クリーナーは各 cleaner 名」
6. 検証ゲート(DoD の 3 コマンド)と Red → Green 反転の確認。

### 対象ファイル
src/cleaners/app_support.rs(apply_selection:509 の差し替えのみ + tests モジュールの drift pin)、src/cleaners/library_caches.rs(delete_entries:231-233 の差し替えのみ)、tests/library_caches.rs(drift pin 1 本)、README.md または docs/SUPPORTED.md(文書化 3 行)。

### 期待する Red → Green
drift pin 2 本が先に Red(現行は scan 時の値を freed に報告する)。実装後に全 Green。既存テスト(delete_entries pin 群、app_support の guard/apply テスト群、hint 契約、永続キャッシュ pin)は無修正で green が pin(回帰の検知)。

### design-it-twice
- Option A(採用): 呼び出し側 2 か所で dir_size を削除直前に呼ぶ。ledger 契約(caller-owned size、trash.rs:293-295)を壊さず、hint 経路(hint/cleaner.rs:151-154)と同型の統一になる。計測追加は削除直前の 1 walk のみ。
- Option B(却下): delete_ledger 側で re-walk する。caller-owned size の doc と「the ledger never re-walks a path」契約を破る(scope fence 1 番)。
- Option C(却下): CacheEntry.size を clean 時に fresh 化する。scan 時の分類(Deletable 判定)の来歴まで変わり、永続キャッシュ pin(tests/scan.rs:497-528)と衝突する(scope fence 2 番)。

### 落とし穴
- dir_size は空ディレクトリで 0 を返しうる。library-caches は scan 時に size == 0 のエントリを載せない(library_caches.rs:120-122)ため、削除直前の計測で 0 が出てもエントリ自体は残る(seam の NotFound 分類で prose skip)。freed は成功分だけの合計(:285 の doc)なので 0 計上は契約に反しない。
- app-support の計測位置は apply_selection のループ内の正しい位置(resolve_allowed の後、:509 の同位置)に置くこと。ループ外(選択直後)に置くと quit 待ちの間の書き込みを逃す。
- drift pin の fixture はトップレベル mtime を動かさない(サブディレクトリ内への追加)こと。トップレベルを動かすと永続キャッシュが無効化されて drift を再現できない(tests/scan.rs の changed_target_is_remeasured_by_the_cache が mtime 不一致で full walk に落とす契約)。
- dry-run は ledger に到達しない(trash.rs:297-298 の doc)。drift pin は削除実行でのみ有効で、dry-run の表示は本 PBI の対象外。
- 既存の apply_selection pin(apply_progress_total_counts_deletable_items_only、app_support.rs:1395)は progress_init(items.len())の契約で、計測の差し替えは items の長さを変えないため影響しない。

## pin(壊してはいけない契約)
- 永続キャッシュ pin: tests/scan.rs:497-528 の second_scan_reuses_the_persistent_cache が無修正で green(scan 時の分類と Total はキャッシュのまま)
- ledger 契約 pin: delete_ledger の doc(src/trash.rs:280-300、caller-owned size は :293-295)と `freed += size`(:332)は無修正。delete_entries pin 群(src/cleaners/library_caches.rs tests の delete_entries_removes_and_sums_bytes、delete_entries_records_failures_as_skipped、delete_entries_continues_past_fatal_permanent_delete)が無修正で green
- exit code 契約: hint = quit timeout でも prose error・skip なし・exit 0(src/hint/cleaner.rs:338-343)、app-support = blocked → SkippedEntry → all-skipped なら exit 1(apply_blocks_running_app_without_approval、app_support.rs:1347-1368)
- app_support.rs の既存 guard/apply テスト群(apply_rejects_dir_replaced_by_symlink_after_scan、apply_does_not_follow_symlinks_inside_cache、apply_never_deletes_forged_unlisted_sub など)が無修正で green
- tests/library_caches.rs の既存 pin(dry_run_lists_deletable_and_hides_excluded、non_tty_clean_deletes_nothing、scan_includes_library_caches_with_deletable_total_only など)が無修正で green
- 削除 seam の分類: NotFound → permanent は fatal で prose skip / TrashRefused → trash は skippable、の現行分類は無変更
- pbi/README.md の設計方針(dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用)

## scope fence(絶対禁止)
1. ledger の never re-walks 契約の変更はしない(caller-owned size の doc と `freed += size`、delete_ledger の re-walk 化)
2. scan 時の分類(Deletable 判定)の fresh 化はしない(永続キャッシュ pin tests/scan.rs:497-528 を壊す)。分類の古さは scan 精度の別軸(なぜなぜ分析の裁定で別 PBI)
3. 削除経路のその他の変更はしない(delete_entries、delete_ledger、DeleteError 分類、DeleteCtx、TrashMode、chflags 経路、seam の NotFound / TrashRefused 分類)
4. exit code 契約の変更はしない(hint exit 0 / app-support all-skipped exit 1 は pin 済み)
5. KNOWN_ENTRIES(src/hint/apps.rs)・TARGETS・EXCLUDED の表、閾値、top5 truncate は触れない
6. src/hint/cleaner.rs には接触しない(既に削除時再計測で正しい)
7. TUI の build_selection_items(src/interactive.rs:13-48)には接触しない
8. CHANGELOG.md は触らない(統合側で一括記載)
9. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green(既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash)
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 永続キャッシュ pin(tests/scan.rs の second_scan_reuses_the_persistent_cache)が無修正で green
- [ ] ledger 契約 pin(delete_ledger doc + delete_entries pin 群)が無修正で green
- [ ] drift pin 2 本(library-caches、app-support)が Red → Green 反転を確認済み(実装前の Red 実行記録つき)
- [ ] BDD 受け入れシナリオ 4 本が自動テストとして実装され、パスする
- [ ] 文書化 3 行が README.md または docs/SUPPORTED.md に記載済み

## 備考
- 実装は worktree で subagent-driven(AGENTS.md の実装ワークフロー)。ブランチ案: fix/freed-measured-on-delete
- PBI 94 と src/cleaners/app_support.rs・library_caches.rs を共有するが別関数領域(本 PBI は apply_selection:509 と delete_entries:231-233、94 は format_candidate_label:224-232 と render_listing/render_item:150-178)。マージ順は 93 → 94
- quit timeout の exit code と履歴 label は pin 済み契約のためコード変更はしない(文書化のみ)
