出所: holistic-code-improvement round 9（pbi/2026-11-20-00-backlog-holistic-round9.md の RICE 採点表 候補 2、実行順 Batch 2）+ なぜなぜ分析 round 9（.plan/whywhy-round9-entry-dispatch.md、2026-11-20、main @ c107011）。本 PBI は同文書「## 1. 候補 2」の 3 立場根本原因と「## 3. 裁定(候補 2・3)」を骨子とする。アンカー行番号は main @ c107011 で実測し、PBI 作成時に src/main.rs で再確認済み。

# PBI 2026-11-20-02: main() の 11 フロー抽出 + flush 1 箇所化 + dry-run 解決 1 箇所化

## メタ情報

- PBI 名: 2026-11-20-02-refactor-main-command-flows
- 種別: refactor（挙動不変。出力 / 終了コード / stream の byte 等価が前提）
- 規模: M
- 優先度: 3 位（RICE 1.6 = Reach 6 / Impact 1 / Confidence 0.8 / Effort 3 SP）。実行順は Backlog 00 の依存優先ルールにより純 RICE 順位から逸脱し、候補 3（2026-11-20-03）より先行する
- 後続依存: 2026-11-20-03（無人バッチ骨格共有化）が本 PBI で抽出した `fn run_<flow>`（特に `fn run_auto_flow` / `fn run_all_flow`）を当て先にする。同一ファイル・同一領域のため本 PBI の直後に直列実行（diff 最小）
- 対象ファイル: src/main.rs（arm 抽出・flush 寄せ・dry-run 解決 1 箇所化・tests）

## 優先度（RICE 入力の内訳）

- Reach 6: main.rs は全ラウンドで最頻接触・全記録経路（新ターゲット追加・文言修正・フラグ追加のすべてがこのファイルを通る。round 2〜7 で 35 本の PBI がここに触れた実測あり）
- Impact 1: 規範化（重複削減・構造規約の不変条件化。ユーザー可視の変化はゼロ）
- Confidence 0.8: opts 渡しの形状など設計判断が残る（source pin の設計を含む）
- Effort 3 SP（見積もりセクションと同一値）

## ユーザーストーリー

main.rs に新特殊ターゲットの arm を追加したり、既存フローの文言・フラグを修正する保守担当者として、11 フローの本体が `fn run_<flow>` の名前付き関数としてシンボルジャンプ・diff 走査可能で、arm の雛形（cleaner 構築 → `run_and_exit`）を 634 行の match 深部から探さずに済むことがほしい。なぜなら、main() は 634 行・11 フローまで成長しており、無名ブロックのままだと走査・diff 解読・雛形探しのコストと「opts 渡し 3 種・dry-run 無視の先例・flush 2 箇所」という構造の緩みが「次の early return」「次の arm」のたびに再生産されるから。

本 PBI は挙動変更を含まない: 全フローの出力（stdout / stderr の別を含む）と終了コードは現行と byte 等価、ユーザー可視の変化はゼロ。価値は将来の flag 混入事故・diff 解読コスト・flush 取りこぼしという温床の構造的解消に集中する。

## 背景（実測確定事項）

src/main.rs（main @ c107011 で再確認済み）:

1. main() は 634 行（main.rs:1255-1888）で 11 フロー（Doctor / Auto / Scan(+--json) / Targets / None(TUI) / Explore / Clean 特殊 12 arm + 標準 dispatch / Stats）を 1 関数に保持する。特殊 arm 群が 1473-1855（約 380 行）。標準ターゲットは既に main.rs:1856-1865 で `run_and_exit` + `dispatch_clean` に統一済みで、重複の本体は特殊 arm 側にある
2. Scan --json arm は `scan_cache::flush` を手動で呼んで `return Ok(())` する（main.rs:1404-1413）。末尾の flush（main.rs:1883-1885）はその early return を迂回できないため、flush 呼び出しが 2 箇所に複製されている
3. Trash arm は `let _ = dry_run;`（main.rs:1532、Report-only in both modes のコメント付き）で dry-run を意図的に無視し、常に `clean(true, ...)`（main.rs:1533-1534、`CleanOptions::default()` 渡し）を走らせる
4. Xcode arm は `--yes` の Note 判定のために `XcodeCleaner::new(...)` を本処理（run_xcode_clean）とは別に 1 回構築する（main.rs:1508-1516）
5. opts 渡しが 3 種並存する: LibraryLogs arm の arm 内再束縛（main.rs:1558-1562、all フラグ付き）、Trash arm の `CleanOptions::default()`（main.rs:1534）、他 arm の `clean_opts`
6. External arm 内の dry-run 解決 `cli.dry_run || args.iter().any(|a| a == "--dry-run")` が custom 経路（main.rs:1821）と plugin 経路（main.rs:1844）の 2 箇所にある
7. arm 内の `std::process::exit()` 散在（1276、1331、1610、1615、1623、1784、1796、1815、1836。1796 のみ --help 短絡の `exit(0)`、残り 8 箇所は `exit(1)`）と `?` の混在。exit は main() 末尾の scan_cache::flush（main.rs:1883-1885）を迂回する
8. arm 本体の unit test は無く、seam 関数（`run_clean_target` / `clean_by_name` / `dispatch_clean`）と E2E spawn で pin されている（main.rs:1891 以降の tests）

## 根本原因（なぜなぜ round 9「## 1. 候補 2」、3 立場）

- 攻撃者 / 回帰を仕込む者: arm が関数境界を持たないため、「3 種の opts 渡し」「dry-run 無視の先例（`let _ = dry_run;`）」「early return に伴う flush 複製」という構造の緩みが、byte 等価 pin の下で静かに育つ。新規 arm の作者は既存 arm を写すため、明文化された意図的無視（Trash）が意図しない無視の複製を生む誘因になる。抽出は緩みを不変条件（opts 渡し 1 規約・flush 1 箇所・early return 撤去）に変える機会である
- 半年後の保守担当者: arm が無名ブロックであることで、走査・diff 解読・雛形探しのコストが毎回発生し、flush 複製や opts 3 種の暗黙規約が「次の early return」「次の arm」のたびに判断負荷を再生産する
- エンドユーザー: ユーザー可視の変化はゼロが正解で、価値は「将来の flag 混入事故・diff 解読コスト・flush 取りこぼし」という温床の構造的解消に集中する（新ターゲット対応 PR が小さくなり、レビューが速くなる前払い）

## BDD 受け入れシナリオ

```gherkin
Scenario: BDD1: `clean trash --dry-run` の出力は現行と byte 等価
  Given `sasurahime clean trash --dry-run` の現行出力・終了コードが parity pin されている（棚卸しで不足なら抽出の前に補完済み）
  When  11 フローの抽出（Trash arm 本体の `fn run_<flow>` 移設を含む）を適用する
  Then  `sasurahime clean trash --dry-run` の stdout / stderr / 終了コードが現行と 1 byte も変わらない（pin 走査で実測）
  And   dry_run を意図的に無視する契約（`let _ = dry_run;` と Report-only コメント）は移設後もそのフロー関数内に残る

Scenario: BDD2: `scan --json` 実行後も scan-cache.json が保存される
  Given `sasurahime scan --json` を --no-cache なしで実行する
  When  Scan --json arm の early return（main.rs:1413）を撤去し、scan_cache::flush を main() 末尾 1 箇所（main.rs:1883-1885）に寄せる
  Then  scan-cache.json が保存されている
  And   flush の網羅が pin される（全フローの終了経路が main() 末尾の flush 1 箇所を通ること。src/main.rs 上の flush 呼び出し箇所が 1 つであることの pin を含む）
```

## 受け入れ基準

- [ ] 11 フロー（Doctor / Auto / Scan(+--json) / Targets / None(TUI) / Explore / Clean 特殊 12 arm + 標準 dispatch / Stats）を `fn run_<flow>` 自由関数へ抽出する。reporter / home / config / opts を引数で明示し、フロー関数内でスコープ共有変数（`cli` 全体・`clean_opts`・`scan_cache_path` 等）を直接参照しない。標準 dispatch（main.rs:1856-1865）は既に統一済みのため `run_and_exit` + `dispatch_clean` の 1 行 arm に留めてよい
- [ ] Scan --json の early return（main.rs:1413）を撤去し、`scan_cache::flush` を main() 末尾 1 箇所（main.rs:1883-1885）に寄せる。byte 等価 pin（BDD2）で証明する。main.rs:1407-1409 の cache warming コメント契約（--json 経路でも detect 結果を保存する）は唯一の flush 呼び出し箇所へ移して維持する
- [ ] External arm の dry-run 解決（main.rs:1821 custom 経路 / 1844 plugin 経路）を 1 箇所にまとめる
- [ ] opts 渡し形状の pin（「flag 持ち arm はローカル構築、それ以外は clean_opts」）を source pin（include_str! grep）で固定する。Clean 特殊 arm のうち opts をローカル構築してよいのは flag 持ち arm（例: LibraryLogs の all フラグ付き再束縛、main.rs:1558-1562）のみで、Trash arm の `CleanOptions::default()`（main.rs:1534）は Report-only 契約（main.rs:1530-1532）とセットの文書化済み例外として pin に含める。source pin は tests/AGENTS.md の規約（最後の手段・定義行アンカー・pin 自己参照回避）に従う
- [ ] 全 E2E pin 無傷（出力 byte 等価の実測を記録）+ `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean
- [ ] 抽出の前に既存 E2E pin の棚卸し（auto / all / --yes の skip 出力・stream を含む）を実施し、不足分を先に足す

## テスト戦略（parity pin 先行）

1. 棚卸し: src/main.rs tests（main.rs:1891 以降）と tests/ の E2E spawn pin を走査し、(a) auto / clean all / --yes の skip 出力（auto は stderr、all / --yes は stdout という stream の非対称を含む）、(b) 終了コード、(c) `clean trash --dry-run` の Report-only 出力、(d) `scan --json` の scan-cache.json 保存、の 4 観点で不足を洗い出す
2. pin 補完（抽出の前）: 不足分を現行出力・終了コード・stream の pin として先に足す。parity pin は「現行出力を pin してからリファクタ」の順序で初めて意味を持ち、pin を後から足すと「抽出で壊したものを後付け pin で気づく」順序になり byte 等価の主張が弱まる
3. 抽出: `fn run_<flow>` への移設は移動のみ。文言・stream・exit の位置は現行のまま（byte 等価 pin が red にならない範囲）。early return の撤去は Scan --json の 1 箇所のみ
4. 抽出後の効果: `fn run_<flow>` は reporter を注入できる自由関数になるため、現行では E2E spawn でしか歩いていない「arm のフラグ解釈 → seam 呼び出し」の結合部分を、arm 単位の unit pin として書けるようになる。spawn E2E は 1 本あたりの起動コストが高く arm × フラグの直積を置けないため、unit pin がその補完になる。本 PBI では足場の確保までを範囲とする（unit pin の全面整備は後続ラウンドの判断事項）

## 見積もり

3 SP（RICE の Effort 入力と同一、Backlog 00 の Batch 2）。実装順は pin 棚卸し・補完 → 抽出 → flush / dry-run 解決の 1 箇所化 → 検証・記録（テスト戦略と受け入れ基準の順に対応）。

## DoD

- [ ] `cargo test --no-fail-fast` が green（既知の TCC 依存 4 失敗を除く許容: cleaners::huggingface::tests::clean_fallback_deletes_dir / cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries / cleaners::pre_commit::tests::clean_fallback_deletes_dir / trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗一覧に合わせて注記する）
- [ ] `cargo clippy --tests -- -D warnings` が 0
- [ ] `cargo fmt --check` が通る
- [ ] byte 等価の実測記録: BDD1（`clean trash --dry-run` の pin 走査）と BDD2（`scan --json` の scan-cache.json 保存 + flush 1 箇所 pin）の実測結果を本ファイルに追記する
- [ ] PBI アーカイブ: 統合側が実施する（git mv で .plan/archived/ へ移動、.plan/00-INDEX.md への追記。サブエージェントは git 操作・INDEX 編集・pbi/ 編集禁止）

## scope 外（やらないこと）

- 文言・stdout / stderr（stream）の統一: 挙動変更のため。auto の skip が stderr、all / --yes が stdout という現行の非対称（PBI 74 の記録決定による呼び出し側の事実）は E2E pin の対象であり 1 byte も変えない
- arm の table 化（arm 宣言の束ね）: YAGNI。arm 数は 12 で安定しており、CleanTarget::command_name() / dry_run() の先例がある table 化は次回以降の判断事項
- arm 内 `std::process::exit()` の全面 `Result` 化: byte 等価の範囲で最小に留める。early return の撤去は Scan --json の 1 箇所のみとし、exit が残る arm は「末尾の scan_cache::flush を迂回する」旨のコメントを残す
- opts 渡しの値の変更: Trash arm の `CleanOptions::default()` など現行の opts 値は 1 つも変えない（pin は形状規約の固定であり、値の統一は行わない）

## pin（壊してはいけない契約）

- PBI 71 の one mode resolution: trash_mode の解決（main.rs:1293-1306）は 1 箇所のままとし、フローへは解決済み値を引数で流す（現行契約の維持）
- PBI 74 の記録決定: rendering（文言・stdout / stderr 選択）は呼び出し側に置く（unattended.rs:38-44）。本 PBI は arm の抽出のみで、UnattendedPolicy の純粋性と rendering の所有に触れない
- Trash arm の Report-only 契約: dry-run を無視して常に `clean(true, ...)`（main.rs:1530-1534、sasurahime never empties the Trash itself）。挙動・コメントとも維持
- Scan --json の cache warming 契約: --json 経路でも detect 結果を保存する（main.rs:1407-1409）。flush 1 箇所化はこの契約を維持したまま網羅性だけ構造化する
- Xcode arm の Note 判定: `--yes` 時のみ `is_xcode_running()` を見る throwaway 構築（main.rs:1508-1516）をそのまま移設する（seam は Note を出さない現行契約）
- 標準 dispatch の統一済み形状: `run_and_exit` + `dispatch_clean`（main.rs:1856-1865）は byte 等価のまま維持
- 全 E2E pin 群: 出力・終了コード・履歴記録（src/main.rs tests、main.rs:1891 以降 + tests/）

## 備考

- 実装は subagent-driven（AGENTS.md の実装ワークフロー）。git 操作・INDEX 編集・pbi/ 編集は統合側が実施する
- 触れるファイルは src/main.rs が主対象。parity pin の補完で必要になった場合の tests/ への pin 追加のみ許容（既存 pin の修正・削除はしない）。pbi/ 配下の他ファイル・.plan/00-INDEX.md・README には触れない
- 後続 PBI 2026-11-20-03 が当て先にする `fn run_auto_flow` / `fn run_all_flow` は、本 PBI の arm 抽出時にこの名前で用意する（後続の diff 最小化）
- アンカー行番号は main @ c107011 実測値。着地時点で行番号がずれている場合は file:line を読み替え、契約の内容は変えない
