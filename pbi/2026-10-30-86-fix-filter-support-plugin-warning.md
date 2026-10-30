# PBI-86: filter_support に plugin 名を載せて [cleaner.<plugin>] の unknown 誤警告をなくす

出典: /improve-codebase-architecture 前ラウンドの深掘りで記録された持ち越し「filter_support の plugin 漏れ」（PBI-82 が widening を別 PBI とする判断で残した分。深掘り 2026-10-30）

## ユーザーストーリー
plugin を定義したユーザー（`sasurahime-*` を PATH に置くユーザー）として、自分の plugin を指す `[cleaner.foo]` が unknown cleaner name と誤警告されないことがほしい、なぜなら plugin は sasurahime が発見して scan と clean で正しく扱っている対象なのに、警告だけが「未知の名前」として出て `[config]` 警告の信頼性を損なうから。

## 優先度
RICE: Reach 2 × Impact 1 × Confidence 80% / Effort 0.3 人週 = スコア 5.3（本ラウンド 3 位）
- Reach 2: plugin（`sasurahime-*` を PATH に置く）を定義したユーザーの [cleaner.*] 設定が影響を受ける
- Impact 1: 誤警告のノイズ。clean 自体は正しく動くため、警告の信頼性低下だけがコスト
- Confidence 80%: 警告の出どころ（main.rs:811-813）と plugin 名集合の入手経路（plugin.rs:208-240）が実コードで確認済み。plugin に capability None を載せる意味論判断（フィールド付きセクションの警告が unknown から無効設定系に変わる点）に判断が残るため 95% ではない

## BDD 受け入れシナリオ
Scenario: plugin を定義したユーザーの [cleaner.foo] が unknown cleaner name と誤警告されない
  Given PATH 上に sasurahime-foo という実行可能な plugin が存在する
  And   config に plugin 名の [cleaner.foo] セクションがある
  When  `sasurahime auto`（または `clean all`）を走らせる
  Then  foo は `[foo] unknown cleaner name` として警告されない
  And   scan / targets / clean / explore の各エントリポイントでも同様に誤警告は出ない

Scenario: 本当に未知の名前は警告され続ける
  Given PATH 上に sasurahime-bar という plugin は存在しない
  And   config に [cleaner.no-such] セクションがある
  When  `sasurahime auto` を走らせる
  Then  no-such は `[no-such] unknown cleaner name` として警告される

Scenario: plugin 名にフィールドを書いたときは unknown ではなく capability に従う警告になる
  Given PATH 上に sasurahime-foo という plugin が存在する
  And   config に `[cleaner.foo] older_than_days = 30` がある
  When  `sasurahime scan` を走らせる
  Then  foo は unknown cleaner name として警告されない
  And   foo のフィールドに対する警告は、plugin が per-cleaner filter を保持しない旨の既存文言（this cleaner has no per-cleaner filters 系）に従う

## 受け入れ基準
- [ ] `filter_support`（src/main.rs:786-798）が plugin 名を support に載せる。plugin 集合は引数（`&[cleaners::plugin::PluginCleaner]`）で受け、core_cleaners の map（:790-793）に続けて `(Cleaner::name(), Cleaner::per_cleaner_filters())` を chain する（all_cleaners の plugin chain、main.rs:736-740 と同型。capability は trait 所有のまま、PBI-82 scope fence 3 の維持）
- [ ] `warn_ineffective_per_cleaner_settings`（src/main.rs:835-844）に同じ引数が付き、唯一の呼び出し元 main.rs:1255-1257 が `PluginCleaner::discover_excluding(&builtin_target_names())`（plugin.rs:208-240、all_cleaners と同じ reserved 除外）を渡す。`config.per_cleaner` が空なら発見自体が起きない（現行 guard main.rs:1255 維持）
- [ ] main.rs:778-785 の PBI-82 doc コメントを「plugin を含む現在の契約の記述」に書き換える（除外の記述は本 PBI で不正確になるため、PBI-78 と同じ「整列済み契約の記述」への書き換えパターン）
- [ ] E2E 1 本（tests/config_warnings.rs）: tempdir bin dir に sasurahime-foo を install_plugin（tests/plugin.rs:14-18 パターン）で置き、config に [cleaner.foo] を書いて auto を走らせ、`[foo] unknown cleaner name` が出ないことを pin する。修正前 Red / 修正後 Green
- [ ] 単体 3 本（src/main.rs tests モジュール、ineffective_settings_* 群 main.rs:2474-2562 の兄弟）: (1) filter_support が plugin 名を trait capability で載せる (2) plugin 名のキー（フィールドなし）は無音 (3) plugin 名のキー（フィールドあり）は「no per-cleaner filters」系になり unknown 誤警告ではない
- [ ] 本当に未知の名前への警告は維持される: main.rs:2511-2519 / :2521-2531 と tests/config_warnings.rs:87-98 が無修正で green
- [ ] `[cleaner.<builtin>]` と [[custom]] の警告動作は無変更: builtin の capability 警告（main.rs:2481-2490）、[[custom]] 名の無音（main.rs:794-796）、[[custom]] 衝突警告（main.rs:848-856）、builtin 名は discover_excluding の reserved で二重掲載されない（plugin.rs:227-229）
- [ ] 警告のための plugin 発見は PATH の read_dir と exec bit 確認までで、plugin プロトコルコマンド（scan --json / clean）を実行しない（plugin.rs:208-240 の現行契約）
- [ ] cargo clippy --tests -- -D warnings が 0 / cargo fmt --check が通る

## テスト戦略
Outside-In TDD。外側の既存 E2E が pin となり、内側へ向かって新規テストを Red → Green → Refactor で書く。
- E2E（外側、修正前 Red / 修正後 Green）: tests/config_warnings.rs に plugin ケースを 1 本。fixture は tests/plugin.rs:6-18 のパターン（tempdir bin dir + install_plugin + PATH env）で sasurahime-foo を PATH に置き、tests/config_warnings.rs:12-16 の write_config パターンで [cleaner.foo] を書く。`auto` を走らせる（tempdir HOME なので安全）。警告の発火位置は起動時共通（main.rs:1255-1257）のため every_entry_point_warns_exactly_once（tests/config_warnings.rs:53-72）が全エントリポイントの正確に 1 回を引き続き pin する。[cleaner.*] 警告系の既存 5 本（:24, :45, :53, :74, :87）は PATH=/usr/bin:/bin のまま無修正で green
- Unit（内側）: src/main.rs tests モジュールに 3 本。filter_support に plugin 名を渡したとき / 渡さないときの警告分岐を、plugin 集合を引数として直接与えて検証する（実 PATH に依存しない）。既存 9 本（ineffective_settings_*、main.rs:2474-2562）は plugin 引数に空集合を渡すだけで無修正 green（filter_support 直呼びの :2550-2562 も含む）
- Integration: src/cleaners/plugin.rs は非接触のため Cleaner 単体の新規 pin なし。plugin の detect / clean 契約は plugin.rs の既存テスト群（protocol roundtrip、cap、reservation、exec bit）が引き続き pin する
- モック: 不要。警告は plugin プロトコルを実行しない設計のため CommandRunner 注入も不要

## 見積もり
0.3 人週（filter_support と warn_ineffective_per_cleaner_settings のシグネチャ変更 2 箇所 + plugin chain 1 箇所 + doc コメント書き換え 1 箇所 + E2E 1 本 + 単体 3 本。PBI-82 Phase 4 が 0.25 と概算した領域に doc 更新と E2E を足した値）

## 実装計画

### 問題（深掘り結果）
- `filter_support`（src/main.rs:786-798）は `core_cleaners`（:723-731、standard ∪ special）と `config.custom`（:794-796、[[custom]]）だけを列挙し、plugin を含まない
- `[cleaner.<name>]` セクションは `PerCleanerConfig`（src/config.rs:6-11）で読まれ、`ineffective_per_cleaner_settings`（main.rs:802-833）が各キーを support から探し、見つからなければ :811-813 で `[{key}] unknown cleaner name` を出す。この分岐が plugin 名でも発火する（セクションにフィールドがなくても先に unknown 判定される）
- 警告は main() 起動時 1 箇所（main.rs:1255-1257）から `warn_ineffective_per_cleaner_settings`（:835-844）経由で stderr に出る。サブコマンドに依存せず全エントリポイントで共通（tests/config_warnings.rs:53 の pin）
- plugin 名集合は `PluginCleaner::discover_excluding(&builtin_target_names())`（plugin.rs:208-240、PATH を `:` で走査し `sasurahime-<name>` 形式 + exec bit、`plugin_name_from` :19-25）の戻り値から入手可能。all_cleaners（main.rs:737）/ targets 出力（:1398-1399）が既に同じ形で発見しており、`clean <name>` の plugin arm は reserved なしで発見する（:1779-1780）
- `PluginCleaner` は `Cleaner::per_cleaner_filters()` をオーバーライドしていないため capability は None（cleaner.rs:344-348）。plugin プロトコルに per-cleaner filter の仕組みは存在しない（`PluginCleaner::clean` は opts を捨てる、plugin.rs:408-413）
- PBI-82 はこの除外を doc コメント（main.rs:778-785）で意図明記したのみで widening を別 PBI とした。その doc は本 PBI で契約の記述に書き換わる

### 解（案 A: plugin 集合の引数追加。案 B: 登録済み集合の共有は design-it-twice で却下）
1. `filter_support` に plugin 集合の引数を追加: `plugins: &[cleaners::plugin::PluginCleaner]`。core_cleaners からの map（main.rs:790-793）に続けて `plugins.iter().map(|p| (p.name().to_string(), p.per_cleaner_filters()))` を chain する。all_cleaners の plugin chain（main.rs:736-740）と同型で、capability は trait 所有のまま
2. `warn_ineffective_per_cleaner_settings`（main.rs:835-844）にも同じ引数を追加し、唯一の呼び出し元 main.rs:1255-1257 が `PluginCleaner::discover_excluding(&builtin_target_names())` を渡す。per_cleaner が空なら発見自体が起きない（現行 guard :1255 維持、PATH スキャンの追加は [cleaner.*] 利用時の 1 回だけ）
3. main.rs:778-785 の doc コメントを契約の記述に書き換える
4. テスト: E2E 1 本（tests/config_warnings.rs）+ 単体 3 本（src/main.rs tests モジュール）

### design-it-twice の記録（案 B: plugin 登録済み集合の共有）
- `filter_support` が内部で `discover_excluding` を呼ぶ形（all_cleaners と同じ形に寄せる）: シグネチャ変更不要だが main.rs の単体テストが実 PATH を走査するようになり、開発環境の `sasurahime-*` バイナリがテスト結果に漏出する（flaky）
- 発見結果を process-global（context.rs や scan_cache のような共有）に載せる: PBI 71 の「resolved value flows down as an argument and nothing re-reads a process-global mode」（main.rs:1265-1269）と逆方向の設計で、値は引数で流す記録済み原理を窃取する
- 案 A は既存単体 9 本を空集合を渡すだけで green に保ち、警告経路に実 PATH 依存を持ち込まないため採用

### 対象ファイル
- src/main.rs: filter_support（:786-798）/ warn_ineffective_per_cleaner_settings（:835-844）/ 呼び出し元（:1255-1257）/ doc コメント（:778-785）/ tests モジュール（:2450-2562 の兄弟に 3 本）
- tests/config_warnings.rs: E2E 1 本追加
- 非接触: src/cleaners/plugin.rs、src/config.rs、src/cleaner.rs

### 実装順
(1) E2E Red（tests/config_warnings.rs に plugin ケースを 1 本、修正前の `[foo] unknown cleaner name` を確認）→ (2) src/main.rs の filter_support + warn_ineffective_per_cleaner_settings の引数追加と plugin chain + 単体 3 本 → (3) main.rs:778-785 の doc コメント書き換え → (4) 検証ゲート（DoD）

### 裁定済み事項（深掘りの未解決疑問への回答）
- capability は trait default の None（plugin プロトコルに per-cleaner filter が存在しない以上、正直な値。新 enum variant や文言追加はしない）。フィールド付き `[cleaner.foo]` は「no per-cleaner filters」系の警告に変わるが、unknown 誤警告より正確なのは現行実態どおり
- 発見の reserved は `builtin_target_names()`（all_cleaners と同じ、main.rs:737）。builtin 名を `[cleaner.<builtin>]` で書いた場合は built-in cleaner の capability がそのまま当たり、二重掲載は起きない
- `core_cleaners` 自体は無変更（plugin は core registry に入れない。all_cleaners と同様に filter_support の chain で足す）

## pin（壊してはいけない契約）
- 本当に未知の名前への警告: main.rs:811-813 の unknown 分岐と文言。単体 pin `ineffective_settings_reports_unknown_cleaner_names`（main.rs:2511-2519）/ `ineffective_settings_reject_the_old_cocoa_pods_alias`（:2521-2531）/ E2E pin `unknown_cleaner_name_gets_its_own_line`（tests/config_warnings.rs:87-98）が無修正で green
- PBI-82 の registry pin 群: `core_cleaners` 組成（main.rs:723-731、standard ∪ special のみ。plugin を入れない）、special 行形式と ALL_TARGETS（`builtin_target_names` main.rs:715-717 が ALL_TARGETS から生成される）、registry 構築サイトの 3 つ構成（core_cleaners / all_cleaners / caches group）を増やさない
- plugin プロトコルの契約: scan --json の `{"status", "bytes"}` と clean の trailing JSON は additive 拡張（plugin.rs:35-56、fallback :418-426、`--json` を clean に渡さない :423-425）、PBI 66 の claimed bytes / freed cap（plugin.rs:401-403、:428-438、cap_claimed_at_verified :168-172）、run_protocol の timeout + 1 MiB drain（:242-249）と kill_tree（:334-354）、Risk::Moderate で無人実行しない（:461-463）、発見の reserved 除外と PATH 先頭勝ち（:204-240）。本 PBI は src/cleaners/plugin.rs に接触しない
- filter_support の呼び出し元と警告のタイミング: 発火は main() 起動時 1 箇所（main.rs:1255-1257）、filter_support の消費者は `warn_ineffective_per_cleaner_settings`（main.rs:836）と main.rs テスト（:2558）の 2 箇所。全エントリポイントで正確に 1 回の pin（tests/config_warnings.rs:53-72 every_entry_point_warns_exactly_once、:45-51 no_config_means_no_warning、:74-85 accepted_filters_are_silent）
- clean 実行の選別は無関係: filter_support は警告専用の capability 列挙であり、auto / clean all / --yes の実行対象選別（all_cleaners 経由、main.rs:733-760、Risk と auto_clean_allowed と ManualOnly ラップ :741-747）に影響しない。本 PBI で all_cleaners と選別ロジックは無変更
- 警告の既存文言: ヘッダ `[config] some [cleaner.*] settings have no effect:`（main.rs:840）と capability 別文言（:826-829）は無変更。plugin への unknown 誤警告を出なくするだけ
- capability の trait 所有: `PerCleanerFilters` の override が唯一の所有者（cleaner.rs:252-263）、plugin は trait default None（:344-348）。表列化しない（PBI-82 scope fence 3 の維持）
- built-in と [[custom]] の警告動作: `[cleaner.<builtin>]` の capability 警告（main.rs:2481-2490）、[[custom]] 名の無音（:794-796）、[[custom]] 衝突警告（colliding_custom_names :848-856）は無変更
- 警告のための plugin 発見は read_dir + exec bit のみで plugin プロトコルを実行しない（plugin.rs:208-240 の現行契約）

## scope fence
1. plugin プロトコル本体・登録形式（`sasurahime-<name>` 命名、exec bit、timeout、JSON 形式）の変更なし。src/cleaners/plugin.rs は非接触
2. registry の表形式変更なし（macro 表、`PerCleanerFilters` enum、Cleaner trait の変更なし）
3. warning 文言の変更なし（plugin への unknown 誤警告を出ないようにするだけ。文言の追加・変更はしない）
4. all_cleaners / clean_by_name / scan / targets / doctor / interactive の registry 構築は無変更（plugin 集合の共有化や発見の 1 回化はしない）
5. PBI-83 の hints cleaner（`KnownAppHintsCleaner`、main.rs:702-708）への接触なし
6. [[custom]] と exclude の扱い、src/config.rs のパースは無変更
7. plugin 側の capability 向上（per-cleaner filter 対応を plugin プロトコルに求める拡張）は別 PBI
8. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] E2E 1 本 + 単体 3 本が追加され、main.rs:778-785 の doc コメントが契約の記述に更新されている
