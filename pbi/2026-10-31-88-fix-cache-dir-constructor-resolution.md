出典: .plan/whywhy-round4-hf-flake.md（why-why round 4「huggingface 未 guard テスト群の flake（根本解決の設計判断）」、2026-10-31 読み取り専用分析。収束した解が本実装計画の素）

# PBI-88: cache-dir resolution を constructor 時の 1 回に移す（依存注入）

## ユーザーストーリー

sasurahime の開発者として、`cargo test` を 8 並列で走らせても huggingface / pre-commit のテストが兄弟テストの env 値を拾わず安定することがほしい、なぜなら cache-dir 解決 closure が detect/clean のたびに process env を再読する現状では、guard を足し忘れたテストが兄弟の値を拾って構造的に flake し、開発者の ambient HF_HOME が実在キャッシュを指しているときはテストの clean が実 HF モデルキャッシュを削除し得るから。

sasurahime の保守担当者として、env 依存の cache-dir 解決が flutter / terraform / act と同じ「構築時 1 回」の方式 1 つに統一され、テストから process env を触る運用が不要になることがほしい、なぜなら closure 形（call 時読み）が 2 cleaner だけに残っていると、薄いラッパーの実例（huggingface.rs）から closure 形ごと写すのが最短になり、新規 cleaner を足すたびに同じ flake と guard 追加が復活するから。

## 優先度

RICE: Reach 3 × Impact 3 × Confidence 95% / Effort 1.5 人週 = スコア 5.7

- Reach 3: cargo test の全実行（8 並列が常態）+ 新規 cli-fallback 系 cleaner を足すたびのリスク。
- Impact 3: (1) 8 並列 flake の構造的解消、(2) 空振りパス（fixture を観測しない緑）の消滅、(3) 開発者の実 HF モデルキャッシュを実削除し得る data-loss ハザードの封印。flake ノイズだけでなく安全面を含む。
- Confidence 95%: flake 機構が実コードで確認済み（closure: huggingface.rs:22-32、pre_commit.rs:23-40、実体: safety.rs:105）。production 不変の検証（main が cleaner 構築後に env を変更するコードが存在しない。env::set_var はテストのみ）が grep で可能。cache-dir の決定タイミングが構築時 1 回に寄るだけなので、production の返すパスに観測可能な差はない。
- guard 6 本追加の対案（Effort 0.5）について: 素の RICE では対案が 3 × 3 × 0.95 / 0.5 = 17.1 と高く出る。ただし Impact 3 は「ハザードを閉じる」場合の値で、flake を一時鎮火するだけの対案は空振りパスと実データ削除ハザードを残すため実質 Impact は 1 程度（3 × 1 × 0.95 / 0.5 = 5.7 で構造的解と同点）。さらに guard は「落ちたと観測されたテストに足す」パッチで、欠落が新規テストのたびに再発する完了しない作業であり、鎮火のたびに次の 6 本が残る（whywhy round 4 裁定）。時間を通じた累積効果で構造的解を上位に置く。

## 背景（問題の 3 種）

**実コードの確認**（main @ d437647）:

- `src/cli_fallback.rs`: `CliFallbackCleaner` は cache-dir を `resolve_cache_dir: Box<dyn Fn(&Path) -> PathBuf + Send + Sync>`（:35）で保持し、`with_parts`（:40-54）が closure を受け取る。`cache_dir()`（:56-58）は closure を呼ぶだけ。`detect()`（:71）と `clean()`（:98）が内部で `self.cache_dir()` を呼ぶため、detect/clean のたびに closure が実行される。
- `src/cleaners/huggingface.rs`: `HuggingFaceCleaner::new` が `with_parts` に渡す closure（:22-32）は呼び出しごとに `crate::safety::resolve_env_dir("HF_HOME", "hub", h.join(".cache/huggingface/hub"), "huggingface")` を実行する。
- `src/cleaners/pre_commit.rs`: 同型の closure（:23-40）が `$PRE_COMMIT_HOME`（set-but-empty は `is_ok()` 判定）→ `$XDG_CACHE_HOME` → `~/.cache/pre-commit` の優先度で、呼び出しごとに resolve する。
- `src/safety.rs`: `resolve_env_dir` の doc（:83-98、Side effect セクション :85-91）は「call 時に読む、注入は behavior change として範囲外（PBI 57）」を pin する。実体（:99-129）は呼び出しごとに `std::env::var(var)`（:105）を読み、unsafe 値は warn して default に落ちる（join 前検証 + join 結果検証の fail-closed）。
- テスト現状: huggingface.rs は 11 本（guard 付き 5 本 = override pin 4 本 :239-337 + `clean_dry_run_uses_cli_when_tool_available` :216、未 guard 6 本 = :141 / :149 / :161 / :180 / :201 / :339）。pre_commit.rs は 10 本すべて guard 付き（:146 / :159 / :178 / :202 / :226 / :246 / :270 / :294 / :317 / :353）。guard は `TEST_LOCK` + `EnvGuard`（set_var / remove_var の process-global 操作、ロックを取らない）で、累積 38 サイト（whywhy round 4 計上。全 src での TEST_LOCK 文字列出現は 57 行）。
- flutter / terraform / act（`src/cleaners/generic.rs`）は構築時 1 回 resolve で値が引数で流れる。closure 形（call 時読み）は CliFallback の 2 ラッパーだけ。
- `tests/` 配下の E2E（tests/huggingface.rs:65、tests/pre_commit.rs:71）は spawn 時の `.env()` 注入で子プロセス限界であり、process-global な set_var には該当しない。

**問題の 3 種**:

1. **flake の構造的再発**: closure が detect/clean のたびに process env を読むため、guard 生存中の兄弟テストが設定した値を lock を取らない未 guard テストが拾う。guard-per-test は歴史的に「落ちたと観測されたテスト」にしか適用されてこず、未 guard 6 本が残っている。guard を足しても新規テストのたびに欠落が再発する（累積 38 サイト、1 テストあたり 3〜5 行の定型的 preamble）。
2. **data-loss ハザード（最重要）**: 未 guard テストは開発者の ambient `HF_HOME` も読む。`clean_fallback_deletes_dir` は ambient 下で resolver が実在する `$HF_HOME/hub` を返すと、`NoToolRunner`（chflags 実プロセス許可）経由の `delete_entries` が実 HF モデルキャッシュを実削除する。fixture への assertion は落ちるが、実データは既に消えている。fixture 外パスを区別する tripwire は `is_safe_delete_target` にはなく（system path のみ拒否）、代わりの隔離手段（解決値の注入）が採用されていない。
3. **PBI 71 契約の残存アンチパターン**: PBI 71（trashmode-concrete）は「値は引数で流す、process-global を再読しない」を TrashMode で確立済み。cache-dir closure（detect/clean 時に global env を再読）は同一アンチパターンの残存物で、契約の横断適用が行われていない。

**なぜなぜ分析の根本原因**（whywhy round 4、3 立場）:

- (A) 攻撃者視点: cache-dir 解決が call 時 process env に置かれ、テストの隔離が「env を替える側の convention（guard）」に委ねられた。忘れ・空振りパス・ambient 経由の実データ削除ハザードの 3 種が構造に残る。落ちない空振りケース（fixture を一切観測せず緑で完走する `detect_returns_not_found_when_dir_missing` 等）が検出不能性の核。
- (B) 保守担当者視点: guard-per-test はテスト追加のたびの線形コストと必ず再発する欠落を生む。正の雛形（書くときに足す規約）、env 名の single source、方式差の明示、判断理由の保存がなく、半年後の担当者は同じ穴を掘り続ける。
- (C) テストインフラ視点: 並列隔離モデルとして「shared global の直列化（guard + lock）」を採用したこと自体が、依存注入（値は引数で流す）モデルの未適用による。guard は動くが保証が convention 遵守に依存し、欠落・空振り・実データ衝突が構造に含まれる。

## BDD 受け入れシナリオ

```gherkin
Scenario: 高並列 cargo test でも huggingface / pre-commit のテストは自分の fixture を観測する
  Given huggingface と pre-commit のテスト群が 8 並列で走る
  And あるテストが cache-dir を制御する env 値を保持している
  When 各テストが自分の fixture パスで detect / clean を実行する
  Then 各テストは自分の fixture パスだけを観測対象とし、兄弟テストの env 値を拾わない
  And fixture を一切観測しないまま緑になる空振りテストは存在しない

Scenario: 開発者の実 HF キャッシュは cargo test で消えない
  Given 開発者の環境に ambient HF_HOME が設定され実在キャッシュ（モデルファイル入り）を指している
  And cargo test が huggingface の clean 系テストを実行する
  When テストが fixture の cache-dir を明示指定してクリーナーを構築し clean を呼ぶ
  Then クリーナーは fixture パスだけを削除する
  And ambient HF_HOME の実在キャッシュは 1 ファイルも変わらない

Scenario: env で cache 先を変える本番契約は現状維持
  Given ユーザーが HF_HOME / PRE_COMMIT_HOME / XDG_CACHE_HOME で cache 先を設定している
  And 値が安全なパスである
  When sasurahime が huggingface または pre-commit クリーナーを構築する
  Then 構築時に env が 1 回だけ読まれ解決済みの cache-dir が確定する
  And detect / clean は確定値を使い process env を再読しない
  And 未設定時の既定パス（~/.cache/huggingface/hub、~/.cache/pre-commit）は今日と同一

Scenario: 不安全な env 値は構築時に fail-closed で既定に落ちる
  Given ユーザーが HF_HOME=/ など不安全な値を設定している
  When クリーナーが構築される
  Then 構築時に警告が出て既定の cache-dir に落ちる
  And join 前の値検証と join 結果検証は resolve_env_dir 内に残る

Scenario: env 依存 cleaner の解決方式が 1 つに統一される
  Given env で cache-dir を解決する cleaner が 5 つある（huggingface / pre-commit / flutter / terraform / act）
  When cache-dir 解決の方式を確認する
  Then 5 cleaner すべてが構築時 1 回の解決で方式が 1 つになる
  And call 時に env を読む closure 形は消える
```

## 受け入れ基準

- [ ] `cli_fallback.rs` の `resolve_cache_dir` field が `Box<dyn Fn(&Path) -> PathBuf + Send + Sync>` から解決済み `PathBuf` に変わり、`with_parts` の第 4 引数が closure から解決済み値に変わる。`cache_dir()` は field を返すだけになる
- [ ] `huggingface.rs` / `pre_commit.rs` の closure body がそれぞれ `new()` 本体に移り、構築時に 1 回だけ `resolve_env_dir` が呼ばれる。detect / clean は process env を再読しない
- [ ] 本番契約「env で cache 先を変える」は保持: HF_HOME / PRE_COMMIT_HOME / XDG_CACHE_HOME の override は構築時に解決に効き、未設定時の既定パスは不変
- [ ] `safety.rs` の `resolve_env_dir` doc が「constructor で 1 度読む、fail-closed 検証は維持」に改訂され、PBI 57 pin（「call 時副作用」「注入は範囲外」）の置換が doc 内に記録される。`resolve_env_dir` のシグネチャ・fail-closed 検証コード・warn 挙動は無修正
- [ ] `huggingface.rs` に `#[cfg(test)]` のテスト用 factory（home / cache_dir / runner を渡す）が追加され、override pin 4 本が EnvGuard + TEST_LOCK なしで解決値を明示指定する形に書き換わる。resolver 単体の env→解結果 pin は safety.rs 既存 6 本 + huggingface の `resolve_env_dir` 直呼び pin で維持
- [ ] 未 guard 6 本 + pre_commit 10 本が factory で cache_dir を明示し、guard（TEST_LOCK / EnvGuard）が不要になる（guard を足すのではなく不要にする）。env 目的の TEST_LOCK hold と EnvGuard は huggingface.rs / pre_commit.rs から消える
- [ ] data-loss ハザード pin テスト 1 本: ambient HF_HOME（モデルファイル入りの実在 tempdir）下でも、factory 注入パス以外は削除されず実在キャッシュが 1 ファイルも変わらないことを pin
- [ ] `cli_fallback.rs` 自身のテストが解決済み PathBuf を渡す形に簡素化される（むしろ簡潔になる）
- [ ] `tests/AGENTS.md` に正の雛形規約が追記される: 「cache-dir は constructor 解決。テストで process env を set_var しない（resolve_env_dir 単体 pin を除く）。env 依存の新規 cleaner は generic 系の構築時解決に倣う」
- [ ] TEST_LOCK 自体は verbose 目的の用途（src/context.rs、generic.rs の VerboseGuard、trash.rs 等）で残る。env 目的の hold のみ全廃
- [ ] `cargo test -- --test-threads 8` の高並列で huggingface / pre_commit 全テストが複数回安定する
- [ ] `cargo clippy --tests -- -D warnings` が 0、`cargo fmt --check` が通る

## テスト戦略（Outside-In TDD）

Outside-In。最も外側（ハザードの実在と flake の観測）で Red を確認してから、内側へ向かって Green にする。Red → Green の順序を守る。

- **Red 1（ハザードの実証、最外側）**: 現行コードで、モデルファイル入りの実在 tempdir キャッシュを `EnvGuard::set("HF_HOME", ...)` で設定し（実開発者キャッシュは使わない立替）、`HuggingFaceCleaner::new(tmp2)` → `clean(false)` を呼ぶと resolver が `$HF_HOME/hub` を返して実在キャッシュが消える（fixture 外パスの実削除）。この「fixture 外が消える向きの Red」をコミット前に観測して記録する。
- **Red 2（高並列 flake の観測、ゲート）**: `cargo test -- --test-threads 8` を huggingface / pre_commit に限定して複数回実行し、未 guard 6 本の flake と空振りパス（fixture を観測しない緑）を観測する。これは新規テストではなく検証ゲート。
- **Green 1（adapter 型変更）**: `cli_fallback.rs` の field 型と `with_parts` 署名を解決済み `PathBuf` に変更。自身のテスト 2 本を簡素化。
- **Green 2（2 constructor で構築時解決）**: `huggingface.rs` / `pre_commit.rs` の closure body を各 `new()` 本体に移す（`let cache_dir = resolve_env_dir(...); with_parts(name, config, home, cache_dir, runner)`）。
- **Green 3（safety.rs doc 改訂）**: `resolve_env_dir` の Side effect doc を「constructor で 1 度読む、fail-closed 検証はここで行う」に改訂し、PBI 57 pin の置換を doc 内に明記する。
- **Green 4（テスト 21 本の factory 書き換え）**: huggingface.rs（11 本すべて）+ pre_commit.rs（10 本すべて）を factory 経由にし、guard を不要にする。
- **Green 5（ハザード pin）**: Red 1 の観測を factory 経由の恒久 pin に書き換える（ambient 下でも factory 注入パス以外を削除しない）。
- **規約追記**: tests/AGENTS.md に正の雛形を追記する。
- **検証ゲート**: DoD のコマンド群。

- **E2E / ゲート**: ambient 下の Red 反転。HF_HOME を実在 tempdir に設定した状態で `cargo test clean_fallback_deletes_dir` が、factory 書き換え後は green に反転し複数回安定する。TCC 依存の既知失敗との混同に注意（DoD 参照）。tests/huggingface.rs:65（HF_HOME=/etc spawn pin）と tests/pre_commit.rs:71（PRE_COMMIT_HOME=/ spawn pin）は子プロセス限界の `.env()` で無修正で green。
- **integration（huggingface.rs / pre_commit.rs の単体テスト群）**: factory 経由への書き換え。override pin 4 本は「解決値を明示指定したクリーナーの cache_dir() pin」に、env→解結果 pin は `resolve_env_dir` 直呼び assertion に分離する。pre_commit の set-but-empty 分岐の pin（`cache_dir_uses_safe_default_when_xdg_cache_is_unsafe`、`detect_uses_xdg_cache_home_unconditionally`）は期待値無修正で維持。
- **unit**: safety.rs `resolve_env_dir` 単体 pin 群（6 本）は無修正で green 維持（fail-closed 検証は resolve_env_dir 内に残る）。cli_fallback.rs 自身のテスト（RecordingReporter 経由の ledger pin、unsafe-dir skip pin）は解決済み値を渡す形に簡素化し green 維持。
- **モック**: 外部コマンドは CommandRunner 注入（tests/AGENTS.md の方針どおり）。NoToolRunner / CliToolRunner / CliToolRunnerFailing は現状維持。

## 見積もり

1.5 人週（1.0 から 1.5）

内訳: adapter 型変更 + 2 constructor 移設 0.4 / safety.rs doc 改訂 0.1 / テスト 21 本の factory 書き換え 0.6 / ハザード pin テスト + Red 反転確認 0.2 / 検証ゲート（高並列複数回含む）0.2

## 実装計画

### 実装順（Red → Green の順序）

1. **Red（ハザード実証）**: 現行コードで ambient HF_HOME（実在 tempdir キャッシュ）下の `clean` が fixture 外パスを実削除することを観測する（実在 tempdir の立替。実データは使わない）。
2. **Red（高並列 flake 観測）**: `cargo test -- --test-threads 8` を huggingface / pre_commit に限定して複数回実行し、flake と空振りパスを観測する。
3. **Green 1**: `cli_fallback.rs` の `resolve_cache_dir` field を `Box<dyn Fn(&Path) -> PathBuf + Send + Sync>` から `PathBuf` に、`with_parts` の第 4 引数を closure から解決済み `PathBuf` に変更。`cache_dir()` は field を返すだけに。自身のテスト 2 本は解決済み値を渡す形に簡素化する。
4. **Green 2**: `huggingface.rs` / `pre_commit.rs` の closure body を各 `new()` 本体に移す。本番契約「env で cache 先を変える」は保持され、読みは構築時 1 回になる。flutter / terraform / act と方式が統一される。
5. **Green 3**: `safety.rs` の `resolve_env_dir` doc を改訂する（「call 時に読む、注入は範囲外（PBI 57）」→「constructor で 1 度読む。fail-closed 検証（override 値の事前検証、join 結果検証）はここで行う」）。PBI 57 pin の置換を doc 内に明記する。
6. **Green 4**: `huggingface.rs` / `pre_commit.rs` に `#[cfg(test)]` のテスト用 factory（home と cache_dir と runner を渡す、cache_dir 明示）を追加。override pin 4 本は EnvGuard + TEST_LOCK を外して解決値（`/Users/u/hf/hub` 等の解決後値）を明示指定する形に。未 guard 6 本 + pre_commit 10 本も factory で cache_dir を明示し、guard を不要にする（足すのではなく不要にする）。
7. **Green 5**: Red 1 の観測を factory 経由の恒久 pin に書き換える: ambient HF_HOME（実在 tempdir キャッシュ）下でも、factory 注入パス以外は削除されず実在キャッシュが 1 ファイルも変わらないことを pin する。
8. **規約追記**: `tests/AGENTS.md` に「cache-dir は constructor 解決。テストで process env を set_var しない（resolve_env_dir 単体 pin を除く）。env 依存の新規 cleaner は generic 系の構築時解決に倣う」を追記し、次の新規 cleaner で欠落が構造的に起きないようにする。
9. **検証ゲート**: DoD のコマンド群（高並列複数回 + ambient 下 Red 反転確認を含む）。

### 対象ファイル

`src/cli_fallback.rs`（field 型 + `with_parts` 署名 + `cache_dir()` + 自身のテスト簡素化）、`src/cleaners/huggingface.rs`（new 本体 + factory + テスト 11 本）、`src/cleaners/pre_commit.rs`（new 本体 + factory + テスト 10 本）、`src/safety.rs`（doc コメントのみ）、`tests/AGENTS.md`（規約追記）。CHANGELOG.md は触らない。

### 期待する Red → Green

Red 1（ハザード観測）と Red 2（高並列 flake）が先に現れる。Green 1〜5 の後、全テストが green で、ambient 下の Red が反転（green 化）し、8 並列複数回で flake 0 になる。既存 pin（safety.rs 6 本、tests/ 配下の spawn pin 2 本、pre_commit の XDG pin）は無修正で green が回帰検知。

### 現状確認コマンド（着手前に実行）

```bash
grep -n "resolve_env_dir" src/cleaners/huggingface.rs src/cleaners/pre_commit.rs
grep -n "resolve_cache_dir\|with_parts" src/cli_fallback.rs
grep -rn "EnvGuard\|TEST_LOCK" src/cleaners/huggingface.rs src/cleaners/pre_commit.rs | wc -l
grep -rn "env::set_var" src/   # production に env 変更コードが無いことの確認
```

### design-it-twice

- **案 A（採用）**: `with_parts` の resolver closure を解決済み `PathBuf` に変え、2 constructor で構築時 1 回 resolve。テスト用 factory で解決値を明示。PBI 71 契約と整合し、generic 系 3 cleaner（flutter / terraform / act）と方式が統一され、guard が不要になる（convention が不要になるので欠落不可能）。波及は有限（2 constructor + `cli_fallback.rs` 自身のテスト）。
- **案 B（却下）**: guard 6 本追加（flake 鎮火のみ、Effort 0.5）。3〜5 行 × 6 本で即効性はあるが、欠落は新規テストのたびに再発し、空振りパスと ambient 経由の実データ削除ハザードは防げない。素の RICE は対案が高く出る（3 × 3 × 0.95 / 0.5 = 17.1）が、Impact 3 は「ハザードを閉じる」場合の値で、flake を一時鎮火するだけの対案の実質 Impact は 1 程度（3 × 1 × 0.95 / 0.5 = 5.7 で構造的解と同点）かつ鎮火のたびに次の 6 本が残る完了しないパッチ（whywhy round 4 裁定）。累積効果で構造的解を上位に置く。
- **案 C（却下）**: resolve 結果の OnceLock キャッシュ（初回のみ env を読む）。並列下で凍結タイミングが非決定的になり、一時 flake が永続汚染に悪化する。本番は 1 run 1 構築で差が観測不能だが、テストは 1 プロセスで多数の cleaner 構築と複数 env 状態を必要とし、once の意味が崩れる。
- **案 D（却下）**: global env freeze（テスト全体の env snapshot、書き換え禁止）。「override が解決に効く」という resolver pin 群（safety.rs 6 本 + huggingface の直呼び pin）と矛盾し、pin 用の例外口が新たな共有資源になって問題が形を変えて再発する。

### 落とし穴

- Red 1 のハザード実証は実開発者キャッシュで行わない（実在 tempdir を立替に使い、EnvGuard で設定して Drop で復元）。実データを消す検証はしない。
- pre_commit の closure は `PRE_COMMIT_HOME` の set-but-empty を `is_ok()` で分岐する。factory 移設時にこの分岐を壊すと XDG 経路の pin 2 本（`cache_dir_uses_safe_default_when_xdg_cache_is_unsafe`、`detect_uses_xdg_cache_home_unconditionally`）が落ちる。
- huggingface override pin 4 本の中に `resolve_env_dir` 直呼び assertion が混在している（:252-256、:278-282、:300-309）。factory 書き換え時にこの assertion を消さない。resolver 単体の env→解結果 pin は safety.rs 6 本 + この直呼び pin で維持する。
- `tempfile::TempDir` は /var/folders/... 配下で `is_safe_delete_target` を通るが、`TempDir::new_in("/tmp")` は /private/tmp に正規化され拒否される。factory の cache_dir には `tempfile::TempDir`（デフォルト）のパスを渡す。
- `cache_dir()` の `#[cfg(test)]` accessor（huggingface.rs:37-40、pre_commit.rs:45-48）は adapter 側の `pub(crate) cache_dir()` 依存。field 返しに変わっても accessor はそのまま使える。
- `clean_fallback_deletes_dir`（huggingface / pre_commit）は TCC（macOS プライバシー設定）依存の既知失敗。ambient 下 Red 反転確認は「TCC を除く環境での green 化」と「TCC 自体の解消」を混同しない（この PBI では TCC は解消しない）。
- EnvGuard はロックを取らない set_var / remove_var なので、ハザード pin テストでも guard 生存窓を最小に保ち、他テストとの重なりは factory 経由の構造で無害化する。

## pin（壊してはいけない契約）

- 本番契約「env で cache 先を変える」: HF_HOME / PRE_COMMIT_HOME / XDG_CACHE_HOME の override は production で引き続き解決に効く（読みが構築時 1 回になるだけ）。未設定時の既定パス（~/.cache/huggingface/hub、~/.cache/pre-commit）は不変。
- PBI 57 の fail-closed 検証: `resolve_env_dir` の override 値の join 前検証 + join 結果検証、unsafe 値の warn + default fallback（safety.rs:108-125）、シグネチャ（var, subpath, default, label）は不変。safety.rs 単体 pin 群（6 本）は無修正で green。
- PBI 57 doc pin の置換は記録付きで行う: 「Injecting the value instead is a behavior change and is deliberately out of scope (PBI 57)」（safety.rs:90-91）が本 PBI の doc 改訂ステップで「constructor で 1 度読む」契約に置き換わることを、置換後の doc 内に明記する（無記録置換しない）。
- PBI 71 契約: 値は引数で流す、process-global を再読しない（TrashMode で確立済み）。本 PBI はその cache-dir への横断適用。
- PBI 77 ledger 移行: fallback delete は `crate::trash::delete_entries` 経由（進捗・skip 記録・freed 合算・CleanResult 組立は ledger 側所有）。`cli_fallback.rs:179-183` の no-caller-side-guard コメント契約を含め、削除経路は無変更。
- `clean_cli_or_fallback` の契約: dir 非存在で `Ok(CleanResult::empty)`、CLI 失敗は bail（freed を偽らない）、dry-run は削除しない、recreate は ledger 後処理（huggingface recreate=true / pre-commit recreate=false の `CliFallbackConfig` は無変更）。
- detect 契約: dir 非存在 NotFound、bytes > 0 Pruneable、verbose 時の target 表示は現状維持。
- PBI 81: CLI 失敗の warn 文言は `warn_tool_failure` 共有ヘルパ経由。
- tests/ 配下の E2E spawn pin: tests/huggingface.rs:65（HF_HOME=/etc → default fallback）、tests/pre_commit.rs:71（PRE_COMMIT_HOME=/ → default fallback）は子プロセス限界の `.env()` で無修正で green。
- pre_commit の env 優先度: `$PRE_COMMIT_HOME` > `$XDG_CACHE_HOME/pre-commit` > `~/.cache/pre-commit`、set-but-empty PRE_COMMIT_HOME は unsafe として default に落ちる現行分岐。
- TEST_LOCK の verbose 目的用途（src/context.rs の定義、generic.rs VerboseGuard、trash.rs / batch.rs 等）は残す。env 目的の hold のみ全廃。
- 既知 TCC 失敗 4 本（`cleaners::huggingface::tests::clean_fallback_deletes_dir`、`cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries`、`cleaners::pre_commit::tests::clean_fallback_deletes_dir`、`trash::tests::delete_ctx_trash_mode_moves_to_trash`）は本 PBI では解消しない（TCC 依存は範囲外）。
- tests/AGENTS.md の既存指針（Outside-In TDD、CommandRunner モック、tempdir fixture）は維持。追記は additive。
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用。

## scope fence

1. 本番契約「env で cache 先を変える」の削除はしない（env 読みは残る、constructor 時に 1 回）。env を読まない cleaner への全廃は production 機能を壊すので実装しない。
2. ledger / `trash.rs` の削除経路（delete_entries / safe_delete / seam_delete / TrashBackendOverrideGuard）には触れない。
3. flutter / terraform / act 等の既に constructor 解決の cleaner（`src/cleaners/generic.rs`）には触れない。
4. `resolve_env_dir` の fail-closed 検証の弱体化はしない（検証コード・warn 挙動・シグネチャは不変、doc コメントの改訂のみ）。
5. PBI 57 doc pin の無記録置換はしない（doc 改訂ステップで置換を明記する）。
6. CHANGELOG.md には触れない（統合側で一括記載する）。
7. Cleaner trait / clean 契約（detect / clean シグネチャ、opts honour）の変更なし。Cleaner 登録（cleaners/mod.rs）と main.rs の構築箇所は変更なし。
8. `resolve_env_dir` のシグネチャ（var, subpath, default, label）は変更しない。
9. tests/ 配下の既存 E2E は無修正（spawn pin 2 本を含む）。テスト改訂は src 配下の `#[cfg(test)]` 単体テスト 21 本と tests/AGENTS.md 追記のみ。
10. Linux / Windows 対応なし。

## DoD

- [ ] `cargo test --no-fail-fast` が green（既知の TCC 依存 4 失敗を除く: `cleaners::huggingface::tests::clean_fallback_deletes_dir`、`cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries`、`cleaners::pre_commit::tests::clean_fallback_deletes_dir`、`trash::tests::delete_ctx_trash_mode_moves_to_trash`。`clean_fallback_deletes_dir` は TCC 依存の既知失敗で、この PBI では TCC 自体は解消しない）
- [ ] `cargo clippy --tests -- -D warnings` が 0
- [ ] `cargo fmt --check` が通る
- [ ] `cargo test -- --test-threads 8` の高並列で huggingface / pre_commit 全テストが複数回安定（flake 0）
- [ ] ambient 下 Red 再現の反転確認: HF_HOME を実在 tempdir に設定した `cargo test clean_fallback_deletes_dir` が、factory 書き換え後は green に反転する
- [ ] data-loss ハザード pin テスト 1 本が実装され green
- [ ] BDD 受け入れシナリオ 5 本が自動テストとして実装され、パスする（高並列安定性と Red 反転はゲート、ハザード pin と env 契約と方式統一は単体 pin で担保）
- [ ] tests/AGENTS.md に正の雛形規約が追記されている
- [ ] pin の列挙がすべて無修正で green（既知 TCC 失敗 4 本を除く）

## 備考

- 実装は worktree で subagent-driven（AGENTS.md 契約どおり。orchestrating session は実装コードを書かず、1 タスク 1 サブエージェントで実施する）。
- 構造 + doc（実装順 3〜5）とテスト改訂（6〜7）を同一 PBI 内の順次ステップで実施する。途中状態で closure 形と factory 形が混在しないよう、guard-per-test の温存期間を設けない。
- 実データ削除ハザードの封印を含むため、実装完了時は安全性主張（pin テストが実在し壊すと失敗すること）の裏取りをコミット前に確認する。
