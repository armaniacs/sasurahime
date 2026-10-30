# checking-team レビュー報告書 — sasurahime

- 実行日時: 2026-10-30 21:05 (JST) / 実行時間 約5時間20分 (15:46〜21:05)
- スキル: checking-team(22専門家 多角的レビュー、並列実行)
- レビュー対象: `review/checking-team` ブランチ全体(HEAD = main `f3366a4` と同一コミットのため差分ゼロ → **現状ツリー全体(306ファイル、src 31,113行)を対象とする現状レビュー**)
- 比較ブランチ: main
- グラフ: graphify 利用可能(部分的 — 詳細は「備考」)

---

## 総合スコア: 89/100(22観点の平均)

**High 6件 / Medium 29件 / Low 19件**

## 観点別スコア

| 観点 | スコア | High | Medium | Low |
|---|---:|---:|---:|---:|
| Red Team(攻撃) | 75 | 1 | 1 | 1 |
| Blue Team(防御) | 75 | 1 | 1 | 1 |
| System Architect | 95 | 0 | 1 | 2 |
| Maintainability | 90 | 0 | 2 | 0 |
| Legacy Bridge | 90 | 0 | 2 | 0 |
| UI | 90 | 0 | 2 | 1 |
| アクセシビリティ | 95 | 0 | 1 | 1 |
| i18n | 95 | 0 | 1 | 1 |
| ドキュメント | 90 | 0 | 2 | 1 |
| チューニング | 90 | 0 | 2 | 1 |
| SRE / Ops | 75 | 1 | 1 | 1 |
| FinOps | 90 | 0 | 2 | 0 |
| エッジ・モバイル | 100 | 0 | 0 | 2 |
| コンプライアンス&プライバシー | 95 | 0 | 1 | 0 |
| 倫理&バイアス | 95 | 0 | 1 | 1 |
| サプライチェーン | 90 | 1 | 1 | 1 |
| APIコントラクト | 95 | 0 | 1 | 2 |
| ドメインロジック | 90 | 0 | 2 | 0 |
| データ整合性 | 95 | 0 | 1 | 0 |
| リファクタリング | 90 | 0 | 2 | 1 |
| DX | 95 | 0 | 1 | 1 |
| テスト(Wave 3) | 55 | 2 | 1 | 0 |

---

## High 指摘(6件)

### H1. 🎯 HOME 配下の機密サブディレクトリ(~/.ssh 等)が env-var リダイレクト 1 つで削除対象になる
- 場所: `src/safety.rs:23-40`(is_safe_delete_target)、`src/safety.rs:105-125`(resolve_env_dir)
- 影響: `is_safe_delete_target` は HOME 自身と祖先のみをブロックし、HOME 配下サブディレクトリは一律許可。一方 `resolve_env_dir` が環境変数でキャッシュディレクトリを上書きできるため、`export ACT_CACHE_DIR=$HOME/.ssh`(または TF_PLUGIN_CACHE_DIR=~/.gnupg)が仕込まれた状態で `--yes`/auto が実行されると、Risk::Safe クリーナーが無確認で ~/.ssh 配下を削除する。悪意あるセットアップスクリプト・dotfiles が 1 行を永続化するだけで成立する攻撃ベクトル。PBI 57/19 の「値自体の検証」では防げない(値は legitimate な HOME サブディレクトリとして pass する)。
- 対処: `~/.ssh`、`~/.gnupg`、`~/.config`、`~/.aws`、`~/.kube` 等の機密サブディレクトリを `is_safe_delete_target` のブロックリストに追加。加えて env override 由来ターゲットの初回削除確認、または env override 対応クリーナーの Risk::Safe 外し。
- 発見: Red Team / テスト専門家(テストゼロ確認)

### H2. 🎯 mise ピン交差チェックが完全一致照合のため、プレフィックス/エイリアス指定のピンが保護されない
- 場所: `src/cleaners/mise.rs:128`(unused_versions)、`src/cleaners/mise.rs:180-196`(spec_versions)
- 影響: ピン集合には `.mise.toml` の生の spec 文字列(`"20"`、`"lts"`、`latest` 等)が入り、installs 側は解決済みバージョン名(`20.11.0`)。完全一致のため `node = "20"` を要求するプロジェクトのランタイムが保護されず、`mise ls --current` は実行時 CWD のアクティブバージョンしか返さないため、プロジェクトでのみ使われるランタイムが「未使用」判定で削除され得る。AGENTS.md の mise クロスチェック規則の意図が、最も一般的な書き方で静かに破れる。
- 対処: (a) mise 自身で spec を解決してから照合、または (b) フェイルクローズとして spec がインストール済みバージョン名と完全一致しない場合は当該ツールのバージョンを保護(前方一致分を含む)。`v` プレフィックス正規化も併せて E2E で固定。
- 発見: Blue Team / テスト専門家(テストゼロ確認)

### H3. 🎯 release ビルド(panic="abort")では panic isolation が機能せず、1 つの cleaner の panic が scan/`--yes`/TUI 全体を落とす
- 場所: `Cargo.toml:59`、`src/scanner.rs:113-126`(自己言及のコメントあり)
- 影響: PBI 33「panic isolation + one-pass is_available」の目的は「one bad cleaner must not take down scan/--yes/TUI」だが、release プロファイルは `panic = "abort"` のため `catch_unwind` が unwind せず、detect 内の panic は即プロセス abort。コードコメント自身が限界を認めており、shipped binary では PBI 33 の受入基準が実質未達。テストは debug(unwind)でしか通らず CI で検出されない。
- 対処: (1) release も `panic = "unwind"` に戻す(サイズ増は lto+opt-z で大部分相殺可能)、または (2) abort を維持するなら PBI 33 の適用範囲を「debug/test のみ」と明記し代替分離を backlog 起票。最低限 README/docs に既知の限界として記載。
- 発見: SRE / Ops

### H4. 依存関係の脆弱性・ライセンス自動監視が一切ない(cargo-audit / cargo-deny 不在)
- 場所: `.github/workflows/ci.yml`(監査ステップなし)/ Cargo.lock(170 クレート)
- 影響: 削除系 CLI という高い信頼前提のツールでありながら、推移的依存 170 個の既知 CVE・ライセンス混入を検知する仕組みが CI にない。具体例: `fs2 = "0.4"` は 2021 年の 0.4.3 から更新停止で RustSec で unmaintained 扱いとされる(fs4 が後継)。
- 対処: CI に `cargo deny check`(advisories + licenses + sources)または `cargo audit` を 1 ステップ追加。`fs2` → `fs4` への置換を backlog 化。
- 発見: サプライチェーン・センチネル

### H5. 🎯 mise ピン保護を固定するテストが存在しない(Wave 1 指摘のまま未修復)
- 場所: `tests/mise.rs:152-248`(既存ピンテストは完全一致のみ)
- 影響: `node = "20"` / `"lts"` / テーブル形式のピンで「20.11.0 が生き残る」ことを主張するテストが unit・E2E ともにゼロ。推奨テストは現状コードでは RED になるはず = 実バグの裏取りになる。
- 対処: Outside-In 手順通り tests/mise.rs に 4 ケースの E2E を追加(RED 確認)→ spec 解決/前方一致保護を実装して GREEN。単体に `spec_versions` 正規化テストを併設。
- 発見: テスト専門家

### H6. 🎯 env-var リダイレクトで HOME 配下の機密サブディレクトリが削除対象になる分岐のテストがゼロ(Wave 1 指摘のまま未修復)
- 場所: `tests/act.rs:63-97`(root・/usr/local の拒否方向のみ)
- 影響: `ACT_CACHE_DIR=$HOME/.ssh` のような機密サブディレクトリを指すオーバーライドの分岐が unit・E2E ともにカバレッジゼロ。Risk::Safe + `--yes` で無確認削除につながる safety-critical 分岐。
- 対処: src/safety.rs tests に `EnvGuard::set("ACT_CACHE_DIR", &home.join(".ssh"))` の unit テスト、tests/act.rs に同条件 E2E を追加し「機密サブディレクトリを指すオーバーライドは default にフォールバックする」ことを主張(現状 RED)→ H1 の blocklist 実装とセットで GREEN。
- 発見: テスト専門家

---

## Medium 指摘(29件)

### 安全性・削除系
1. **🔍 scope チェックと削除間の TOCTOU レース**(Red)— `src/trash.rs:172-194` — 判定後の削除が raw path のまま。canonicalize パスを削除にも使用、または削除直前の再 canonicalize と一致確認(fail-closed)。
2. **`clean mise` が `ls --current` 失敗を「nothing to clean」として正常終了扱い**(Blue)— `src/cleaners/mise.rs:228-234` — 失敗時はメッセージ変更+`CleanResult` に skip 記録+exit 非ゼロ(all-skipped → exit 1 契約に合わせる)。
3. **🎯 `--older-than`/`--larger-than` が `--yes` でサイレント無視**(Legacy)— `src/main.rs:551-556`、`1642-1646` — `global = true` で受けるが消費は `clean all` のみ。global を外す/フィルタを流す/無視警告のいずれか。
4. **🎯 外部 CLI 委譲クリーナー(huggingface/pre-commit)が opts.trash_mode を無視して恒久削除**(Legacy)— `src/cli_fallback.rs:131-167` — `uses_trash=false` 報告は正直だが「デフォルトはゴミ箱」契約との齟齬がコード doc 止まり。docs/SUPPORTED.md に明記。
5. **🎯 uchg 保護フラグ解除に失敗しても削除を続行しサマリーに現れない**(倫理&バイアス)— `src/trash.rs:187-192` — chflags 失敗を SkippedEntry 相当かサマリー警告に載せる。
6. **🎯 `..` 判定が文字列部分一致で正当なディレクトリ名を拒否**(ドメインロジック)— `src/safety.rs:20-22` — `path.components().any(|c| c == Component::ParentDir)` に置換。
7. **🎯 全外部コマンドの 30 秒固定タイムアウトで遅い cleanup が途中 kill**(ドメインロジック)— `src/command.rs:11` — brew cleanup -s --prune=all / uv cache prune --force / ollama rm が 30 秒を容易に超える。run_with_timeout を追加し呼び出し側が指定可能に。

### アーキテクチャ・保守性
8. **main.rs が 4 責務を持つ 2,889 行 god file**(System Architect + Maintainability、重複発見)— マクロ定義+クリーナーテーブル、CLI 解析、全ディスパッチ、bin テスト約 1,050 行が同居。bin テスト分離 → dispatch 抽出 → `all_cleaners()` 構築の main 冒頭集約。
9. **🎯 generic.rs(2,062 行)が PathCleaner 実装と共有ヘルパー提供の二役**(Maintainability)— `src/cleaners/generic.rs:758-773`(detect は cached_dir_size)/`824-855`(clean_filtered は dir_size)でサイズ判定が二重実装・不整合の余地。meets_* ヘルパーを filter モジュールへ抽出し detect/clean で共有。
10. **🎯 delete_ledger が生 `println!`/`eprintln!` で出力**(リファクタリング)— `src/trash.rs:300-333` — writer-seam パターン(command.rs warn_tool_failure)に反し、テストからモック不能。出力をローカル関数に集約、可能なら ProgressReporter へ per-entry メソッド追加。
11. **🎯 include_str! によるソース文字列 pin テストが脆い**(リファクタリング)— `src/trash.rs:622-646` 等 — 動作変更なしのリファクタで壊れる。コンパイル時強制できる契約は型・可視性へ寄せ、pin は文脈付き 1 本に絞る。
12. **🎯 tests/ 39 ファイルが個別テストバイナリとして compile/link**(DX)— フィードバックループが伸びる。共通フィクスチャ + tests/common mod 構成に統合してバイナリ数を圧縮。

### キャッシュ・パフォーマンス運用
13. **🎯 スキャンキャッシュの無効化が「ディレクトリ直下の変更」しか検出しない**(チューニング)— `src/scan_cache.rs:59-63` — 孫階層の変更が最大 1 時間古いサイズを serve。detect 経路の size==0 スキップ・larger_than_mb 判定に波及。MAX_AGE_SECS 短縮(例 5 分)または doc の正直化+`--no-cache` 案内。
14. **🎯 グローバル Mutex 内で 3 度目の冗長 fs::metadata**(チューニング)— `src/scan_cache.rs:154-160` — `record_if_unchanged` に既知の after を渡すシグネチャに変更し、ロック内は純粋な HashMap 操作のみに。
15. **scan-cache.json の保存が非アトミックで破損再構築がサイレント**(SRE)— `src/scan_cache.rs:42-50` — 一時ファイル+rename でアトミック保存、破損・保存失敗時に log::warn。
16. **🎯 scan-cache.json エントリが期限切れ後も永久残存で単調増成**(FinOps)— `src/scan_cache.rs:53-64` — save 時に期限切れ一括除去+compact JSON。

### CI・サプライチェーン
17. **CI が push + pull_request 二重起動で macOS ランナー分数を無駄消費**(FinOps)— `ci.yml:3-9` — concurrency group 追加+PR では pull_request トリガのみに絞る。
18. **第三者 GitHub Actions がタグピン留め(SHA 固定なし)**(サプライチェーン)— `ci.yml:11,31` — actions/checkout・actions/cache をコミット SHA でピン留め。
19. **🎯 release panic="abort"**(→ High H3 と同根、SRE スコア内で計上)

### UX・i18n・ドキュメント
20. **🎯 ask_yn の回答規則が「y/Y のみ有効」で "yes" 入力が abort**(UI)— `src/interactive.rs:105-107` — テストで意図的に固定された仕様。"yes" 先頭一致を許容するか decline 時のフィードバック追加。
21. **`sasurahime targets` の `{:<12}` 固定幅で長いターゲット名がカラムを崩す**(UI)— `src/main.rs:1394` — orphan-containers 等の 17 文字名で桁が崩れる。最大名長から幅を計算。
22. **docs サイトの言語スイッチで「現在の言語」が視覚表現のみ**(アクセシビリティ)— `docs/index.html:27`、`docs/ja/index.html:27` — `aria-current="true"` を付与。
23. **🎯 「Trash から復元できる」表記が言語版・媒体間で矛盾し日本語版が過大約束**(i18n)— `README.md:248`、`docs/index.html:74`、`docs/ja/index.html:71` vs `README.md:8` — 英語版の注意書き(Put Back が出ないことがある)に揃える。
24. **🎯 対応ターゲット数の表記が 40+/50+/55 と 3 通りに不整合**(ドキュメント)— README 50+、SUPPORTED.md 55、サイト 40+ — 55 を基準に統一(ターゲット追加のたびに 5 ファイル直す運用の是正)。
25. **🎯 docs サイトの `cargo install sasurahime` が README の `cargo install --path .` と矛盾**(ドキュメント)— `docs/index.html:167・204` — crates.io 公開有無を確認の上、手順を統一。
26. **🎯 環境変数オーバーライドの値が stderr 警告にそのまま出力**(コンプライアンス&プライバシー)— `src/safety.rs:109-111,118-120` — CI ログ等経由の意図しない流出面。HOME 基準の短縮表示へ。
27. **🎯 `scan --json` 出力スキーマにバージョン識別がなく機械可読契約が暗黙的**(APIコントラクト)— `src/scanner.rs:48-87` — フィールド一覧の文書化+将来の schema_version 追加。
28. **🎯 Trash モードで `bytes_freed` が実解放量を過大報告**(データ整合性)— `src/trash.rs:312-318` — Trash 残存中のディスクは未解放。「moved to Trash (empty Trash to reclaim)」の明示または bytes_freed/bytes_trashed 分離。
29. **🔍 実 Trash バックエンドにハード依存する 4 テストが TCC/サンドボックス環境で常時失敗**(テスト専門家、実測 829 passed / 4 failed)— `src/trash.rs:375`、huggingface.rs、library_logs.rs、pre_commit.rs — mise.rs:840-853 の seam-truth パターン/TrashBackendOverrideGuard に統一。

---

## Low 指摘(19件、要点のみ)

- ANSI エスケープ無害化なし(command.rs:50-63, safety.rs:109-121)— Red
- `--yes` 時 Xcode 起動中でも DerivedData 削除続行(main.rs:1480-1488)— Blue
- is_sys が文字列 starts_with でコメント「component-wise」と不一致(safety.rs:53)— SA(誤る方向は fail-closed)
- CleanOptions の Default derive が resolve-once 契約を型保証しない(cleaner.rs:214-230)— SA
- dialoguer バージョンコメント陳腐化(interactive.rs:388)— UI
- docs サイトに `<main>` ランドマーク・aria-hidden なし(ux/a11y)
- CLI メッセージ英語のみ(設計通り、README での明記で緩和可)— i18n / 倫理
- README scan サンプル語彙が status_label 実装と不一致 — ドキュメント
- dir_size がディレクトリ自身のブロックを数えない(format.rs:23-31)— チューニング
- CI 診断ステップの常時実行(失敗時限定化可)— SRE
- rayon 既定グローバルプールの全コア占有 / steady tick 常時スレッド起動 — エッジ
- Cargo.toml 緩いレンジ指定(lock コミット済みで実害小)— サプライチェーン
- 0.4.x 終了コード契約変更(0→1)が changelog 通知のみ(README に終了コード表なし)— APIコントラクト
- ios-backup ヘルプ文言「irreversible — backed up to Trash」自己矛盾(main.rs:426)— APIコントラクト
- is_safe_delete_target の 27 条件 && チェーン(BLOCKED_PREFIXES 形式へ整理可)— リファクタリング
- AGENTS.md の clippy コマンドが Makefile より弱い(--tests なし)— DX

---

## 確認済みの良好点(抜粋)

- **削除安全シームの一元化**: 全削除が `safe_delete`/`seam_delete` 経由で PBI 77 のバイパス不在。scope tripwire → chflags → 型付き DeleteError による skip/fatal 分類(PBI 59 の variant 照合、文言照合なし)が trash.rs/safety.rs に集約。
- **同意設計**: `--yes` は Risk::Safe のみ(unattended.rs、NeverUnattended で Risky/Manual を構造的に排除)、Risk 既定 Moderate、auto は [policy] セクション必須(PBI-41)。
- **外部送信なし**: Cargo.lock 170 クレートにネットワーク系なし、src にネットワークコードなし。README の Privacy 記述は実装と一致。
- **契約の pin テスト**: define_cleaners! 単一ソース+ドリフトテスト、`scan --json` スキーマ、auto_clean_allowed == (risk == Safe)、終了コード単一オーナー、config serde default/alias。
- **テスト基盤**: TEST_LOCK 直列化、CommandRunner モック注入の統一、TrashBackendOverrideGuard、Outside-In のモック戦略一貫性。
- **TODO/FIXME 0件**、非テストコードの unwrap は plugin.rs の 1 件のみ、`#[allow(clippy::...)]` 0 件。

## 自動修正(Phase 5 — 未コミット、現行 main チェックアウト上に適用)

1. `AGENTS.md`: clippy ゲート例を `cargo clippy --tests -- -D warnings` に更新(Makefile と統一)
2. `README.md` / `docs/index.html` / `docs/ja/index.html`: Trash 復元主張を英語版の注意書きに揃え、ターゲット数表記を 55 に統一
3. `src/interactive.rs`: dialoguer バージョンコメントを Cargo.toml に合わせ修正
4. `.github/workflows/ci.yml`: concurrency group を追加(二重起動の cancel-in-progress)

※ 挙動変更を伴う修正(機密サブディレクトリ blocklist、mise spec 解決、panic profile、`..` 判定、ask_yn 規則、テスト書き換え)は自動適用せず「実装提言」として以下に記載。

## 実装提言(PBI 候補、優先度順)

1. **H1+H6**: 機密サブディレクトリ blocklist(~/.ssh 等)の `is_safe_delete_target` 追加+RED テスト(E2E+unit)
2. **H2+H5**: mise ピン spec の解決/フェイルクローズ保護+RED テスト(E2E 4 ケース+単体正規化)
3. **H3**: release panic profile の再検討(panic="unwind" への変更または限界明記)
4. **H4**: CI へ cargo-deny 追加+fs2→fs4 置換
5. **M(テスト)**: 4 テストの seam-truth パターン統一(常時失敗の解消)
6. **M(安全系)**: `--older-than` の `--yes` 無視解消、TOCTOU 軽減、`..` 判定の components 化、30 秒タイムアウトの緩和
7. **M(構造系)**: main.rs 分割(bin テスト分離→dispatch 抽出)、generic.rs フィルタ述語の共有化

## 備考

- **graphify は部分的**: AST 2,646 ノード/6,739 エッジ+セマンティック 3/10 チャンク(57 ファイル分)で構築。chunk 1/2/3/5/7/8/9 はサブエージェントのタイムアウト(長時間実行の打ち切り)で未取得 — ドキュメント側のノード欠落あり。`graphify-out/graph.json`(ラベル付き)+`GRAPH_REPORT.md` 生成済み。レビュー観点の判断には影響なし(全観点がコード実読で検証済み)。
- **cargo test 実測**: 829 passed / 4 failed(失敗はすべて実 Trash バックエンドの TCC/サンドボックス依存、テストロジックのバグではない)。
- レビュー対象ブランチは main と同一コミットのため差分ゼロ — 本レビューは現状コード全体に対する評価として読むこと。
- レビュー実行環境: DeepSeek Harness、サブエージェント駆動(Wave 1: 5 観点並列 → Wave 2: 4 クラスタ並列 → Wave 3: テスト専門家)。
