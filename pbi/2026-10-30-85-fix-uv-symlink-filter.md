# PBI-85: uv の simple-vN 候補フィルタに通常ファイル拒否を追加し keep-highest 判定を共通ヘルパに統合する

出典: PBI 77 scope fence 項 2 で「別フォローアップ PBI」と記録された持ち越し（uv.rs:39 の file-as-candidate latent bug と keep-highest 統合、深掘り 2026-10-30）

## ユーザーストーリー
uv 利用者として、キャッシュ配下の通常ファイルを誤って削除候補にしないことがほしい。なぜなら simple-vN という名前の通常ファイルが最高世代として計上され、実際の index ディレクトリまで削除されるデータ損失の可能性があるから。

sasurahime の保守担当者として、uv の keep-highest（最新世代を残す）判定が gradle / browsers / jetbrains と同一のヘルパ 1 つにまとまることがほしい。なぜなら世代保持の契約が 2 実装に分裂したままでは、片側だけ挙動が変わるドリフトを pin で検知しにくいから。

## 優先度
RICE: Reach 2 × Impact 3 × Confidence 80% / Effort 0.5 人週 = スコア 9.6（本ラウンド 2 位 / 全 4 件）
- 根拠: 実害は潜在形だが向きがデータ損失で Impact 3。~/.cache/uv 配下に simple-vN という名前の通常ファイルが現れる頻度は実測できていないため Reach 2、修正の有効性は単体テストで確実に検証できる一方、現場でそのファイルに遭遇する確率が不確実なため Confidence 80%。修正は候補フィルタ 1 行の強化と既存 pub(crate) ヘルパの借用（browsers.rs:2 に同一前例あり）で、削除経路に触れないため Effort 0.5 人週。

## 発見の詳細（深掘り結果、main の実コード確認済み）
- **フィルタの現状**: src/cleaners/uv.rs:38-39 の GAP-006 ガードは symlink だけを除外する（`!file_type().map(|t| t.is_symlink()).unwrap_or(true)`）。通常ファイルは除外されず、parse_simple_version（uv.rs:25-27）は名前だけを見るため、通常ファイル simple-vN がそのまま候補集合 versions に入る。file_type の読み取り失敗は unwrap_or(true) でスキップされ安全側。
- **バグの二つの顔**:
  1. ファイル自身が削除候補になる: simple-v16（dir）+ simple-v17（file）+ simple-v21（dir）では max=21 に対し simple-v16（dir）と simple-v17（file）の両方が候補として返る。clean は候補をそのまま crate::trash::delete_entries に渡す（uv.rs:107-128）ため、rm -rf / chflags -R / Trash move の各削除経路はファイルでも機能し、通常ファイルが削除される。dir_size（format.rs:23-31）もファイル root をそのまま合算する。
  2. ファイルが最高世代を装い keep-highest を壊す: simple-v21（dir）+ simple-v99（file）では max=99 となり、実際の最新 index である simple-v21 が「古い」と判定されて削除候補になる。最新 index のデータ損失。
- **単一エントリ短絡が pin を無効化**: uv.rs:47-49 の versions.len() <= 1 短絡のため、通常ファイルが単独で存在するだけでは空が返る。既存テスト detect_old_indexes_skips_regular_files（現行 uv.rs:338-349。PBI 77 記録時点の :226-236 は同実装後の行ずれ）はファイルを 1 つ置くだけなので短絡で pass し、フィルタの有無を pin していない。テスト内のコメント（filter_map で除外される旨）も実態と合っていない。バグ再現には実ディレクトリとの併存が必要。
- **keep-highest 重複の実態**: uv.rs:47-53（単一 u32 キーの max + retain(n < max)）と gradle.rs:31-45 の keep_highest（Vec<u32> 辞書式キー + filter(k != max) + 空キー排除）が「最高キー以外を候補として返す」同一判定の別実装。keep_highest の消費者は gradle 自身（gradle.rs:77、:200 の jetbrains by_ide ループ）と browsers（browsers.rs:2 で import、:72 で使用）。単一 u32 キーでは retain(n < max) と filter(k != max) は数値的に同値で、uv の len() <= 1 短絡は keep_highest の「単一候補なら空」契約（keep_highest_single_candidate_returns_empty が pin）に吸収される。したがって uv 側の置換は挙動無変更で機械的に行える。

## BDD 受け入れシナリオ
Scenario: キャッシュ配下の通常ファイルが削除候補にならない
  Given ~/.cache/uv に通常ファイル simple-v17 が置かれている
  When  scan（uv cleaner の候補検出）を実行する
  Then  simple-v17 は削除候補に含まれない
  And   simple-v17 はファイルシステムに残る

Scenario: 通常ファイルが最高世代を装っても実 index は候補にならない
  Given ~/.cache/uv に実ディレクトリ simple-v21 と通常ファイル simple-v99 がある
  When  scan を実行する
  Then  simple-v21 は削除候補にならない（keep-highest の最新世代が保たれる）
  And   候補は空になる

Scenario: keep-highest 判定が共通ヘルパ経由になる
  Given uv / gradle / browsers / jetbrains が世代保持判定を必要とする
  When  uv の detect_old_indexes を実行する
  Then  gradle.rs の keep_highest と同一のヘルパ経由で候補が返り uv 固有の retain 実装が消える
  And   browsers / gradle / jetbrains の既存 pin は無変更で green

Scenario: uv E2E で通常ファイルが誤削除されない
  Given fake uv を PATH に置き ~/.cache/uv に実ディレクトリ simple-v16 と simple-v21、通常ファイル simple-v17 がある
  When  --permanent clean uv を実行する
  Then  simple-v16 のみが削除され simple-v21 と simple-v17（ファイル）が残る
  And   exit 0 で終わる

## 受け入れ基準
- [ ] src/cleaners/uv.rs:39 の候補フィルタが「実ディレクトリのみ許可」に強化され、symlink に加えて通常ファイルと file_type 読み取り失敗が候補集合から除外される
- [ ] 通常ファイルが最高世代を装っても実 index の keep-highest 判定が壊れない（単体テスト 2 系統で固定: 実ディレクトリとの併存、通常ファイル単独）
- [ ] uv.rs 固有の keep-highest 判定（len() <= 1 短絡 + max + retain）が共通ヘルパ呼び出しに置換され、gradle.rs の keep_highest との重複が消える（design-it-twice の判断をコメントに記録）
- [ ] 既存の実質空テスト detect_old_indexes_skips_regular_files（uv.rs:338-349）が、単一エントリ短絡に頼らない歯のある pin に書き換えられる（通常ファイル + 実ディレクトリ 2 世代の併存で、ファイルが候補に入らず max を膨らませないことを pin）
- [ ] uv E2E（tests/uv.rs）に通常ファイル併存ケース 1 本が増え、既存 3 本（dry-run 無削除、old 削除 + newest 残存、not found exit 0）は無修正で green
- [ ] gradle.rs の keep_highest / is_version_dir は契約・可視性（pub(crate)）とも無変更で、browsers.rs:2 の依存が無傷
- [ ] detect_old_indexes のシグネチャと戻り値の意味（最高世代より古い simple-vN パス集合、単一世代なら空）が不変
- [ ] cargo clippy --tests -- -D warnings が 0、cargo fmt --check が通る

## テスト戦略（Outside-In TDD）
- E2E: tests/uv.rs に通常ファイル併存の 1 本を追加（fake uv + `--permanent clean uv`。simple-v16 が消え、simple-v21 と通常ファイル simple-v17 が残る）。既存 tests/uv.rs 3 本と tests/trash.rs の uv キャッシュ系 8 本（dry-run / trash / permanent / config trash_mode=false）は無修正で green
- integration: uv.rs 単体テスト群でフィルタ境界値を pin（symlink / 通常ファイル / 実ディレクトリの 3 系統 + 通常ファイル単独の空返却）。keep-highest 統合後に detect_old_indexes 群全体が green で、返り値の意味が変わっていないことを pin
- unit: gradle.rs の keep_highest / is_version_dir は非接触で、既存 4 pin（keep_highest_returns_all_but_highest_key、keep_highest_skips_candidates_without_a_numeric_key、keep_highest_single_candidate_returns_empty、is_version_dir_rejects_symlinks）が維持されること

### 実質空テストの書き換え指示
detect_old_indexes_skips_regular_files（現行 uv.rs:338-349）は「通常ファイル 1 つだけ」では短絡で pass するため pin になっていない。simple-v17（file）+ simple-v16（dir）+ simple-v21（dir）の 3 併存に書き換え、(1) simple-v17 が候補に含まれない (2) simple-v16 が候補に含まれる (3) simple-v21 が候補に含まれない、を pin する。この状態が現行コードで Red（simple-v17 が max 計算に入り候補に混入する）になることをコミット前に確認する。通常ファイル単独のケースは検出境界（空返却）として独立テストに残す。

## 見積もり
0.5 人週（0.4 から 0.6）
内訳: フィルタ強化 + 単体テスト書き換え 0.2 / keep-highest 統合 0.15 / E2E 追加 0.1 / 検証ゲート 0.05

## 実装計画
### 問題
src/cleaners/uv.rs の detect_old_indexes は、~/.cache/uv 配下のエントリから symlink だけを除外して simple-vN という名前のエントリを候補に採用する（uv.rs:36-45）。通常ファイルは除外されないため、(1) ファイル自身が削除候補になり delete_entries 経由で削除される、(2) ファイルが max を膨らませて実 index を候補に落とす、の二つのデータ損失経路がある。単一エントリ短絡（uv.rs:47-49）と実質空のテスト（uv.rs:338-349）がこの潜在バグを隠している。また keep-highest 判定は gradle.rs:31-45 の keep_highest と重複している。

### 期待する Red → Green
- Red: 書き換えた detect_old_indexes_skips_regular_files と新規 E2E が現行コードで fail する（simple-v17 ファイルが候補に入る / E2E でファイルが削除される）
- Green: フィルタ強化の後、両テストが pass し、既存 detect_old_indexes 群・keep_highest 群・uv E2E が無変更で green のまま
- keep-highest 統合は Green 上のリファクタリングとして行い、既存 detect_old_indexes 群の green を維持して挙動無変更を確認する

### 解
Step 1 単体テスト書き換え（Red 確認）→ Step 2 フィルタ強化（Green）→ Step 3 keep-highest 統合（リファクタリング、green 維持）→ Step 4 E2E 追加 → Step 5 検証ゲート（cargo test / cargo clippy --tests -- -D warnings / cargo fmt --check）

- Step 2: uv.rs の symlink フィルタを gradle.rs の is_version_dir（gradle.rs:19-24、実ディレクトリのみ許可、file_type 読み取り失敗は拒否）に置換する。借用方向は browsers.rs:2 と同一
- Step 3: uv.rs の候補集計を keep_highest（gradle.rs:31-45）に置換する。単一 u32 キーは vec![n] に包んで渡す（1 要素の辞書式比較は数値比較と同値）。len() <= 1 短絡と手書き max + retain は削除する（keep_highest の単一候補契約に吸収される）。統合の判断はコメントに design-it-twice 記録として残す

### 設計判断（design-it-twice の記録）
- 案 A（採用）: gradle.rs の pub(crate) keep_highest / is_version_dir を uv.rs から借用する。前例が browsers.rs:2 に存在し、借用方向・可視性・契約が実証済み。gradle 側は 1 行も変更しないため browsers / jetbrains の回帰面がゼロ
- 案 B（却下）: keep_highest / is_version_dir を新しい共通モジュール（src/cleaners/retention.rs など）に移動し gradle / browsers / uv を再配線する。恒久的な置き場所としては正しいが、3 cleaner の import 移動と gradle.rs テスト群の移動が伴い、バグ修正 PBI にしては接触面が広い。消費者が 3 cleaner に揃った時点で移動を別 PBI として行う（本 PBI では案 A で借用し、共通モジュール化の前提を整える）

## pin（壊してはいけない契約）
- PBI 77 の ledger 移行: simple-vN 削除は crate::trash::delete_entries 経由（uv.rs:100-128。進捗・skip 記録・freed 合算・CleanResult 組立は ledger 側所有、呼び出し側に unsafe-path ガードを置かない）、fatal per-item error で clean 全体を中断しない（skip 記録 + 継続、exit 1 は skip 記録経由のみ）
- freed の二相会計: phase 1 = simple-vN の ledger sum、phase 2 = prune 前後の dir_size delta、before 計測は ledger 後（uv.rs:130-146、clean_freed_is_the_ledger_sum_plus_the_prune_delta が pin。二重計上なし）
- PBI 53: uv cache prune --force の失敗は freed に 0 を足し Err にしない（log::warn で可視化、clean_reports_zero_freed_when_prune_exits_nonzero が pin）
- GAP-006: symlink は削除候補から除外され続ける（detect_old_indexes_skips_symlinks が green 維持。本 PBI のフィルタ強化は拒否対象を広げる向きのみ）
- detect_old_indexes のシグネチャ pub fn detect_old_indexes(&self) -> Vec<PathBuf> と戻り値の意味（最高世代より古い simple-vN パス集合、単一世代なら空）は不変。変えるのは候補フィルタと判定の内部のみ
- uv E2E（tests/uv.rs 3 本、tests/trash.rs の uv キャッシュ系 8 本）は無修正で green（本 PBI の E2E 追加 1 本を除く）
- gradle.rs の keep_highest / is_version_dir の契約・可視性（pub(crate)）、browsers.rs:2 の依存は無傷。借用方向は uv から gradle への 1 向きのみ
- seam tripwire（is_safe_delete_target。tempdir が /private/tmp になるため拒否される挙動、clean_fatal_index_error_does_not_abort_the_prune_phase が pin）には触れない

## scope fence
1. trash.rs の ledger interface（delete_entries / delete_ledger / seam_delete / DeletionOutcome）と削除経路の変更なし（本 PBI は候補フィルタと keep-highest 判定の内部のみ）
2. Cleaner trait / clean 契約の変更なし
3. gradle.rs / browsers.rs / jetbrains.rs の keep-highest 挙動変更なし（gradle 側は 1 行も変更しない。共通モジュールへの移動は別 PBI）
4. detect() のサイズ計測（cache 全体の dir_size 計測、detect_measures_full_cache_dir が pin）の変更なし
5. uv cache prune --force の失敗意味と warn 挙動の変更なし
6. parse_simple_version のシグネチャと挙動の変更なし
7. README / docs/SUPPORTED.md の更新不要（フィルタ内部の修正で利用者可視の挙動変化なし）
8. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 書き換えたテストと E2E が Red → Green を経ている（現行コードで fail することをコミット前に確認）
- [ ] pin テストがすべて green
