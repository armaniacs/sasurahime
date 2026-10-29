# PBI-82: CleanTarget の special 変種をマクロ表に昇格して identity を全生成する

出典: /improve-codebase-architecture ラウンド 2 候補 3（深掘り 2026-10-30）

## ユーザーストーリー
sasurahime の開発者として、special 変種の target 定義行（マクロ表）を command_name / dry_run / is_special / ALL_TARGETS など identity 系消費者の唯一の源に昇格して全生成させたい。なぜなら現行の identity は main.rs と cleaner.rs の 8 箇所に分散し、新変種追加時に docs 規約の「Path B = 8 手順の手動同期」が必要で、その同期漏れが caches arm の TrashMode drift という実バグ 1 件を産んだから。

## 優先度
RICE: Reach 3 × Impact 2 × Confidence 95% / Effort 2.75 人週 = スコア 2.1（本ラウンド 6 位。絶対値は低いが分散が実バグ 1 件を産んだ証拠つき。Phase 0 のバグ修正は即時着手）
- 根拠: 分散は docs/superpowers/specs/2026-10-20-00-common-conventions.md:340-409 と docs/HOWTO-ADD-target.md に「Path B = 8 手順の手動同期」として規約化済みで、事故でなく認知されたコストとして運用されてきた。しかし run_batch 統合（c7aa582）で caches arm が `CleanOptions::default()` を渡す drift が実際に混入し、`--permanent clean caches` でも node-gyp の fallback 削除だけ Trash 行きになる実バグになった。pin による実行時検知では防げない drift を表現不可能にする価値を、この分散自体が証明している。
- 根拠: standard 側 identity は既に dispatch 系マクロ（main.rs:63-93）で統合済みで、残存する分散は special 側のみ。影響範囲が絞れているため Confidence 95%。Phase 0 の caches バグ修正は 0.25 人週以下で先行単独出荷できる。

## 発見の詳細（深掘り結果）
- **問題**: 「どの target が存在し、どう実行されるか」の identity が main.rs に 8 箇所で分散: (1) macro 表（:97-322、standard 42 行 + special verbatim 13 + External）(2) command_name 手動 match（:429-449）(3) dry_run 手動 match（:451-471）(4) matches! ゲート（:1144-1159）(5) special_cleaners（:530-576）(6) caches arm vec（:1163-1189、表 113-131 の Bun/Go/Pip/NodeGyp/Npm/Yarn/Pnpm を手写し）(7) cleaner.rs:356-394 extra_targets（名前+説明 12 件）(8) correspondence pin（:1650-1759、現行 55 件）。registry 構築も 6 箇所。
- **evidence の現行差分（深掘りで確認済み）**:
  - マクロは既に dispatch 系（dispatch_clean / dispatch_command_name / dispatch_dry_run）を生成済み（main.rs:63-93）。standard 側 identity は統合済み。残存する分散は special 側のみ
  - command_name()/dry_run() の特殊腕は production-dead（run は標準 else 節でしか呼ばず、特殊は arm が自前 destructure。特殊腕の唯一の消費者は pin と library_caches テスト）
  - caches arm バグ実証済み: main.rs:1199 が `&cleaner::CleanOptions::default()` を渡す。TrashMode::default() == Trash で PathCleaner は opts.trash_mode を解釈するため `--permanent clean caches` / `config trash_mode=false` でも node-gyp の fallback 削除だけ Trash 行き。main.rs:956-965「resolved value flows down as an argument and nothing re-reads a process-global mode」の契約がこの arm だけ破れている。CLI クリーナーは外部 CLI に自己削除させるため影響は node-gyp 1 本のみ（安全側の向きだが --permanent の約束を破る）。バグは c7aa582（run_batch 統合）で混入。1190-1194 のコメントは opts 合併前の bare clean() との behavior-identity を主張しており新しい契約との不整合に気づいていない
  - 規約自体が分散を文書化済み: docs/superpowers/specs/2026-10-20-00-common-conventions.md:340-409 と docs/HOWTO-ADD-target.md が「Path A（標準）= 1 行完結、Path B（特殊変種）= 8 手順の手動同期」を規約として列挙。分散は事故ではなく認知されたコストとして運用されている
- **5 Whys の根本原因**: 「target 定義」概念が型化されておらず、clap（derive）と registry（文字列 key）の 2 系統を文字列で手動 join している。対策が常に「drift を実行時に検知する pin」であり「drift を表現不可能にする」設計ではなかった。解 = 定義行を唯一の源に昇格し identity 系消費者を生成へ、runtime join は clean_by_name の 1 箇所に閉じる（現状維持、PBI 59 判断どおり）

## BDD 受け入れシナリオ
Scenario: --permanent clean caches が node-gyp の fallback 削除まで permanent 設定に従わせる
  Given node-gyp の fallback ディレクトリが存在する
  When  `--permanent clean caches`（config trash_mode=false も同様）を実行する
  Then  fallback 削除は ~/.Trash に移動せず即座に削除され TrashMode の契約（main.rs:956-965）が caches arm でも破られなくなる

Scenario: special 変種を表に 1 行追加すると identity がコンパイル時に揃う
  Given マクロ表に special 変種を 1 行追加する
  When  cargo build を実行する
  Then  command_name / dry_run / is_special / ALL_TARGETS がその行から生成され手動 match の追記は不要になる（`_` 腕なしの完全一致 match）

Scenario: special 変種から cli-name を削除するとコンパイルが落ちる
  Given special 変種の 1 行から cli-name を削除する
  When  cargo build を実行する
  Then  runtime pin を待たずコンパイルエラーになる

Scenario: caches グループのメンバーシップは既存 pin で維持される
  Given マクロ表 7 行（Bun/Go/Pip/NodeGyp/Npm/Yarn/Pnpm）に `@group caches` マーカーを付けた
  When  tests/dry_run.rs:134 clean_caches_dispatches_every_generic_cache_table_row を実行する
  Then  caches グループが全 7 行を含むことが pin され続ける

Scenario: xcode の unknown sub エラー文言が CLI 腕と seam で一致する
  Given `clean xcode --sub <未知の値>` を実行する
  When  エラーが返る
  Then  CLI 腕と seam で同一の文言になり seam_unknown_target_and_unknown_sub_are_errors pin（tests/xcode.rs）が green

Scenario: filter_support の plugin 除外は doc コメントで意図的に明記される
  Given targets 出力に plugin 名（[cleaner.*]）が並ぶ
  When  filter_support が plugin を除外する
  Then  動作は無変更のまま doc コメントに意図的な除外として明記される

## 受け入れ基準
- [ ] Phase 0: main.rs:1199 が `&clean_opts` を渡すようになり `--permanent clean caches` / `config trash_mode=false` でも node-gyp の fallback 削除が Trash に落ちない
- [ ] Phase 0: caches 腕が `run_caches_batch(cleaners, opts, reporter, dry_run)` に抽出され依存を引数で受ける（テストから stub 注入可）
- [ ] Phase 0: Permanent opts を渡すと node-gyp 2 dir が消える統合 pin が 1 件増え、既存 `--permanent clean caches` E2E（tests/generic.rs:147-157）は無変更で green
- [ ] Phase 1: xcode が `run_xcode_clean(home, config, subs, dry_run, reporter, opts)` に共通化され main.rs:868-888 と :1227-1260 の重複が消える
- [ ] Phase 1: xcode の unknown sub エラー文言が CLI 腕版に統一され seam pin 2 件（seam_unknown_target_and_unknown_sub_are_errors、tests/xcode.rs）が同時更新される
- [ ] Phase 2: マクロ表 7 行に `@group caches` マーカーが付き `caches_group_cleaners()` がマクロから生成され、caches 腕（:1163-1189）は 1 行に削減される
- [ ] Phase 3: special 行と external 行がマクロ表の行ヘッダ（`<Variant> : "<cli-name>" => <desc> ; special ; ... ; dry_run`）に昇格し #[doc]/#[command(name)] が verbatim 側から移動する
- [ ] Phase 3: command_name / dry_run / is_special の完全一致 match（`_` 腕なし）と SPECIAL_TARGETS / ALL_TARGETS（= SUPPORTED_TARGETS ∪ SPECIAL_TARGETS）がマクロから生成される
- [ ] Phase 3: 手動 impl（:427-472）、matches! ゲート（:1144-1159）、cleaner.rs:356-394 extra_targets、main.rs:476-478 ラッパが削除される
- [ ] Phase 3: correspondence pin（:1650-1759 の 55 件手書き）が名前集合 pin 1 件に縮小され、新変種追加時の落下がコンパイルエラーになる
- [ ] Phase 3: docs/HOWTO-ADD-target.md の Path B、docs/superpowers/specs/2026-10-20-00-common-conventions.md:340-409、docs/SUPPORTED.md が更新されている
- [ ] Phase 4: `core_cleaners(home, config)`（standard ∪ special）が 1 つになり all_cleaners / clean_by_name / filter_support が使う。registry 構築サイトは 6 から 3 になる
- [ ] Phase 4: filter_support が plugins を漏らす現状は doc コメントに意図的な除外として明記される（動作変更なし）
- [ ] run 腕（:1162-1538）の clap 引数検証・exit 1 パス・UnattendedPolicy は genuinely heterogeneous として無変更のまま残る

## テスト戦略
（Outside-In TDD）
- E2E: tests/generic.rs に `--permanent clean caches` で node-gyp fallback 削除が Trash に落ちない pin を 1 件（tempdir HOME）。既存 E2E（tests/generic.rs:147-157、tests/apfs_snapshot.rs 7 件、tests/xcode.rs、tests/dry_run.rs）は無修正で green
- Integration: run_caches_batch に stub（reporter / opts）を注入して batch arms が shared reporters（report_batch_failures + report_total_freed、main.rs:2284-2308）を通ることを pin。xcode は seam 経由で unknown sub エラー文言の一致を pin
- Unit: マクロ生成の完全一致 match は `_` 腕なしで網羅性をコンパイル時に保証し、名前集合 pin 1 件に縮小した runtime pin で裏取り。tests/coverage.rs:10-33（targets 出力全名の CLI subcommand 有効性 + 重複なし）が Phase 3 の clap attr 移動後も green であること

## 見積もり
2.75 人週（2.5 から 3.0。Phase 0: 0.25 / Phase 1: 0.25 / Phase 2: 0.25 / Phase 3: 1.5 から 2.0 / Phase 4: 0.25）

## 実装計画
### Phase 0 ― caches バグ修正（先行単独出荷可、0.25 人週以下）
1. main.rs:1199 `&cleaner::CleanOptions::default()` → `&clean_opts`（1 行）
2. 腕を `fn run_caches_batch(cleaners: &[&dyn Cleaner], opts: &CleanOptions, reporter: &dyn ProgressReporter, dry_run: bool)` に抽出（依存を引数で受ける → テストで stub 注入可）
3. pin: 統合テスト 1 件（tests/generic.rs か tests/dry_run.rs）― Permanent opts を渡すと node-gyp 2 dir が消える。既存 `--permanent clean caches` E2E（tests/generic.rs:147-157）green 維持
4. PBI 73 ソース pin（main.rs:2284-2308「batch arms ride the shared reporters」）の report_batch_failures + report_total_freed 呼び出しを保持

### Phase 1 ― xcode 1 関数化（0.25 人週）
`fn run_xcode_clean(home, config, subs, dry_run, reporter, opts)` ― parse（xcode_subcategory 経由）+ with_subcategories + run_clean_target。main.rs:868-888 と :1227-1260 の共通化。`cli.yes && is_xcode_running` ノートは call site 残留。エラー文言統一（CLI 腕版に寄せる ― deriveddata スペリング列挙が有益）+ pin 2 件同時更新（seam_unknown_target_and_unknown_sub_are_errors、tests/xcode.rs）。visible change として CHANGELOG に記載（本体で一括）

### Phase 2 ― caches group（0.25 人週）
マクロ表 7 行（113-131 の Bun/Go/Pip/NodeGyp/Npm/Yarn/Pnpm）に `@group caches` マーカーを付け、マクロが `fn caches_group_cleaners() -> Vec<Box<dyn Cleaner>>`（factory 式から生成）を吐く。腕は :1163-1189 を削り 1 行（run_caches_batch に渡す、clean_opts 引き続き）。tests/dry_run.rs:134 `clean_caches_dispatches_every_generic_cache_table_row` がメンバーシップを pin し続ける

### Phase 3 ― identity 表への昇格（本体、1.5 から 2 人週）
1. マクロ拡張: special 行（`<Variant> : "<cli-name>" => <desc> ; special ; { verbatim variant tokens }; dry_run マーカー`）と external 行。verbatim 側から #[doc]/#[command(name)] を外し行ヘッダへ移動（clap derive の重複 attr はエラー）
2. 生成: command_name / dry_run / is_special の完全一致 match（`_` 腕なし = コンパイル時 exhaustive）、SPECIAL_TARGETS、ALL_TARGETS = SUPPORTED_TARGETS ∪ SPECIAL_TARGETS
3. 削除: 手動 impl（:427-472）、matches! ゲート（:1144-1159 → target.is_special()）、cleaner.rs:356-394 extra_targets（lib 内消費者なし、main.rs 8 箇所を ALL_TARGETS に）、main.rs:476-478 ラッパ
4. pin 縮小: :1650-1759 の 55 件手書きリスト → 名前集合 pin 1 件に縮小（runtime pin → compile-time。新変種追加時の落下がコンパイルエラーになる）。RISK_EXPECTED（:1810-1870）/ LEGACY_SCAN_TARGETS（:1761-1777）/ NOT_SCANNED（:1806）は意味 pin として保持
5. docs 更新: docs/HOWTO-ADD-target.md の Path B、docs/superpowers/specs/2026-10-20-00-common-conventions.md:340-409、docs/SUPPORTED.md
6. run 腕（:1162-1538）は残す ― clap 引数検証・exit 1 パス・UnattendedPolicy は genuinely heterogeneous でデータ行に入らない

### Phase 4 ― registry 組成の共有（0.25 人週、独立）
`fn core_cleaners(home, config)`（standard ∪ special）を 1 つ作り all_cleaners / clean_by_name / filter_support が使う。filter_support は plugins を漏らす（:633-647 は standard ∪ special のみ → plugin 名の [cleaner.*] が「unknown cleaner name」誤警告）が、plugin を警告に足すかは意味論判断が要るため本 PBI では doc コメントで意図的に除外と明記のみ（別 PBI 化推奨）。registry 構築サイトは 6 → 3（core_cleaners / all_cleaners / caches group）

### 設計却下（design-it-twice の記録）
- SpecialTarget トレイトオブジェクト（enum + trait impl の二重 identity）は却下
- 完全なマクロ DSL は却下（ApfsSnapshot の排他 flag → exit 1 のような手続きはデータ行に入らない）
- GenericCacheGroupCleaner 登録は却下（registry pin「1 cleaner = 1 name」違反 + 死に interface、PBI 76 が同一理由で apfs の 2-Cleaner 分割を却下済み）
- per_cleaner_filters の表列化は却下（cleaner.rs:253-257「cleaner の override が唯一の所有者」を窃取）
- correspondence pin 完全削除は却下（coverage.rs E2E だけでは速いフィードバックを失う）

## pin（壊してはいけない契約）
1. correspondence pin（main.rs:1650-1759）― 縮小対象。消すのではなく exhaustive match 化でコンパイル時へ移す
2. tests/coverage.rs:10-33 ― targets 出力全名の CLI subcommand 有効性（--permanent clean <t> --help）+ 重複なし。Phase 3 の clap attr 移動が help を壊していないことの自動検証網
3. registry pin「1 cleaner = 1 name」― main.rs:1793-1804 unique / :1936-1944 表順序 / :1946-1955 non-overlap / :1918-1934 翻訳不要 / :1900-1916 登録網羅 + coverage.rs no_duplicate_targets
4. CLI help / clap attr ― 特殊変種の #[command(name)]（library-logs / device-support / ios-backup / apfs-snapshot / vm-swap）、xcode --sub value_delimiter、device-support --keep default_value、library-logs --all short 'a'。E2E（apfs_snapshot 7 件 / xcode / dry_run）が pin
5. PBI 68 pin（tests/app_support.rs、進捗総計 = items.len()）― cleaner 側。非接触だが回帰禁止
6. PBI 73 ソース pin（main.rs:2284-2308）― Phase 0/2 の caches 腕編集は report_batch_failures + report_total_freed 呼び出しを保持
7. PBI 74 pin（src/unattended.rs:278-370 selection_matrix_pins_the_three_policies + auto/all 腕の文言位置）― All 腕非接触で維持
8. PBI 76 pin（tests/apfs_snapshot.rs E2E 7 件、--all/--thin 排他エラー）― ApfsSnapshot 腕は Phase 3 で「表に名前が出る」だけでロジック非接触
9. PBI 44/34 trash-mode pins（generic/mise/library_caches/orphan/app_support/project_artifacts の clean_with_opts pin 群）― Phase 0 はこの pin 群の呼び出し側を閉じる修正（整合）
10. PBI 59 identity/seam pins（main.rs:2317-2467、typed error 分類、xcode sub 文言 2 系統）― Phase 1 で文言統一なら 2 件同時更新

## scope fence
1. run 腕本体の統合なし（All/Auto/External/Trash/ApfsSnapshot の validation・exit パスは genuinely heterogeneous）
2. risk 分類の表移動なし（main.rs:488-490「There is no central list」設計 + RISK_EXPECTED 意味 pin を維持）
3. filter_support capability の表列化なし（trait 所有維持）
4. plugin の per-cleaner 警告への追加なし（doc コメントでの意図明記のみ）
5. NOT_SCANNED / LEGACY_SCAN_TARGETS / RISK_EXPECTED の生成化なし（identity でなく意味 pin）
6. clap help・UX・出力文言の変更なし（Phase 1 のエラー文言統一は pin 更新を伴う唯一の visible change として明示。文言を触りたくない場合は引数化する選択肢）
7. Cleaner trait / Cleaner::name() の変更なし（registry pin 群が依存）
8. マクロ DSL の完全化、Linux/Windows 対応なし
9. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] docs/HOWTO-ADD-target.md と conventions spec が更新されている
