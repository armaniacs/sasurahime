出典: TTY 対話ウォークスルーgap の実測（2026-11-01、main @ ddc6395、39/41 integration ファイルが実バイナリ spawn の Outside-In E2E を持ち、対話ループ本体だけが歩いていない実測）+ なぜなぜ分析 round 8（.plan/whywhy-round8-freed-drift.md）の「select 経路の pin は不可、ヘルパー直呼びが唯一」記録。/pbi-backlog-prioritizer の 2 候補のうち順位 1。

# PBI-102: 選択 port のテスト注入 seam を新設し TUI 経路の E2E を書けるようにする

## ユーザーストーリー
sasurahime の保守担当者として、テストから `PromptPort` の選択解決を差し替えられる seam がほしい、なぜなら TUI で選択する→削除される→freed が出る一連の対話経路を E2E で pin できるようになり、対話経路の freed 計測（PBI 97、PBI 101 で発見された drift の実在した筋）の回帰を検出したいから

## 優先度
- 順位: 1 / 全 2 候補
- RICEスコア: ≒ 8.5（Reach=2 / Impact=1.5 / Confidence=85% / Effort=0.3 人週）
- 根拠: 疑似 TTY ハーネス（PBI-103、スコア 1.0）と同じ価値帯（対話フローの E2E 化）だが、port インフラが全て実在（`PromptPort` trait、TtyPrompt / NonInteractive アダプタ、7 つの `multi_select_or_skip` 呼び出し元）で欠けているのは注入 seam のみ。依存追加なし・flaky なし・既存 `confirm_on_tty`（PBI 81）と同型の port 設計。対話ループのメニュー描画（stdout）まで assert 可能で、dialoguer 自体のキー操作（raw mode / ANSI）だけが残る。PBI-103 とは**代替関係**にあり、本 PBI 着地時の PBI-103 残存価値は dialoguer 自体の描画（第三者 crate の契約）のみに縮小するため、順位は依存関係により確定

## 背景/問題

39/41 の integration ファイルが実バイナリを spawn する Outside-In E2E を持ち（tests/AGENTS.md の規約どおり）、非対話コマンドの体験はほぼ全域でカバーされている。**対話ループ本体（TUI 起動 → detect 表示 → multi-select → confirm → clean → freed 表示 → summary）を端末から操作する体験だけがどのテストも歩いていない。**

実コードの実測（main @ ddc6395）:

1. `PromptPort` trait は実在（src/prompt.rs:106、PBI 75）: `confirm` と `multi_select` を持ち、TtyPrompt（:136、dialoguer 経路）と NonInteractive（:183、標準 remedy 経路）の 2 アダプタが実装済み
2. `CleanPrompt::for_current_run()`（:64-70）は `is_interactive()`（stdin is_terminal）で 2 アダプタを解決する ambient 設計。**テストプロセスは TTY を持たないため、どのテストも常に NonInteractive が解決され、TtyPrompt の `multi_select`（dialoguer 経路）を通るテストが 1 本も存在しない**
3. 選択経路の代替 pin: drift pin 5 本（library_logs / orphan_containers / library_caches / app_support / ios_backup はヘルパー直呼び + `TrashMode::Permanent`）と TUI 見積もりの unit pin（sibling_counts_owned）で部分担保だが、**「対話で選ぶ→削除される→freed が出る」一連の流れは E2E 未テスト**。round 8 の C2（ios_backup freed drift、select 待ちの前に collect した stale size を渡すバグ）はまさにこの筋で実在した
4. `confirm_on_tty`（:89）は port を使わない第 3 の confirm semantics（PBI 81）で、モック注入筋を持たない

## 根本原因

- 攻撃者/ハッカー視点: 対話経路の freed 計測が「ヘルパー直呼び」でのみ pin され、TUI が解決した port から削除までの流れに契約 pin が無い。select 待ちの前後で drift する数値（C2 のクラス）は、対話経路を歩くテストが無い限り検出できない
- 半年後の保守担当者視点: `for_current_run` の ambient 解決は production の呼び出し元に port を渡さない設計（PBI 75 の裁定: CleanOptions スレッディング不要）で正しいが、テストが解決を差し替える筋が doc にも実装にも無い。新規の選択経路実装者は「対話経路は E2E で守られていない」ことを毎回再発見する
- エンドユーザー視点: TUI で選択して confirm した後に消えるものと出る数字の整合は、対話フローの回帰テストが無いためリファクタのたびに手動確認に戻る

## 発見の詳細（実コードの確認、main @ ddc6395）

- port インフラ: `PromptPort`（prompt.rs:106）= confirm + multi_select。アダプタ 2 個（TtyPrompt :136 / NonInteractive :183）、どちらも stateless unit struct で解決結果は `&'static` trait object
- 解決: `CleanPrompt::for_current_run()`（:64-70）= `is_interactive()` で分岐。テストプロセス（stdin 非 TTY）では常に NonInteractive
- 呼び出し元: `multi_select_or_skip` 7 箇所（library_logs / orphan_containers / library_caches / app_support / ios_backup / ollama / apfs_snapshot）、`confirm_on_tty` 1 箇所（colima）
- 既存 pin との整合: round 8 の type-name pin（本番領域に SuppressReporter 参照なし）、集約 pin（`multi_select_or_skip` 存在 + 空選択行不在）、non-TTY pin（`[<target>] not a terminal`）は port 解決の形状に触れない
- 既実装確認（フェーズ 0）: PromptPort と 2 アダプタは src/prompt.rs に実在（grep + 実コード読み確認済み）。**本 PBI は「port の新設」ではなく「注入 seam の新設」**。疑似 TTY ハーネスは src/ にも tests/ にも存在しない（grep で確認済み）
- テスト規約: tests/AGENTS.md は「cache-dir 解決を env var で操らずテストファクトリで作る」ことを要求（`HuggingFaceCleaner::with_cache_dir` 前例）。選択 port の注入も同型（テストファクトリ or test-only override）が妥当

## BDD 受け入れシナリオ

```gherkin
Scenario: モック選択を注入して対話経路の E2E が歩ける
  Given テストが注入 seam でモック選択 port（multi-select で [0, 1] を返す）を差し込んだ
  When  対話経路（TUI ループと同じ選択筋）で clean を実行する
  Then  選択された項目が削除され、freed が削除時の実測で報告される
  And   実 TTY は要求されない

Scenario: 注入しない production の解決は今日どおり
  Given 注入 seam を使わない通常のランタイム
  When  TTY 無しで対話経路を起動する
  Then  「not a terminal」の標準 remedy が出て interactive path を skip する（既存 non-TTY pin と同一挙動）

Scenario: 空選択の対話経路が共有 routing を通る
  Given モック選択 port が空の選択（Some(vec![])）を返した
  When  対話経路で clean を実行する
  Then  空選択の行動（`[{name}] nothing selected` + Ok(None) routing）が共有 port から出る
```

## 受け入れ基準
- [ ] テストから選択解決を差し替えられる seam が新設される（テストファクトリ `CleanPrompt::with_port` 相当、または test-only override。production 呼び出し元のシグネチャと ambient 解決は不変）
- [ ] 対話経路（select → 削除 → freed 表示）の E2E が 1 本以上書ける（モック選択注入、実バイナリまたは interactive.rs のループ経由）
- [ ] production の TTY 解決（dialoguer 経路）は今日どおり、公開 API 変更なし
- [ ] drift pin 5 本（ヘルパー直呼び・Permanent）と round 8 の pin 群（type-name pin、集約 pin、non-TTY pin）は green 維持
- [ ] RED 確認の証拠: seam を壊す（モック選択が port に届かない形）→ E2E 失敗 → 戻す、を記録

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- モック選択注入による対話経路の clean（freed が削除時実測で出ることの主張）

### 統合テスト
- モック port × interactive.rs の選択ループ（multi-select → 削除筋）

### 単体テスト
- 注入 seam 自体の解決マトリクス（注入あり/なし、空選択、confirm 経路）

## 実装アプローチ
- **Outside-In**: E2E（失敗確認）→ seam 新設 → 対話ループ経由の統合 → 単体
- **Red-Green-Refactor**: seam が無い状態で E2E が書けないことを RED として記録してから新設
- gate を触る場合は src/batch.rs の行家族所有者表を同じ PR で更新

## 見積もり
3 ストーリーポイント（要チームでの見積もり）

## 技術的考慮事項
- 依存関係: なし（新規 crate 不要）
- テスタビリティ: モック port は stateless unit struct（既存アダプタと同型）。選択結果（Vec&lt;usize&gt; / bool）のみを port 契約にし、dialoguer の型を port 境界から漏らさない
- 非機能要件: production の ambient 解決（PBI 75 裁定）は不変。`is_interactive()` の呼び出し回数・タイミングを変えない

## 実装者向け注記

### 現状コードの確認
（着手前に必ず実行すること）
```bash
grep -n "trait PromptPort" src/prompt.rs
grep -n "for_current_run" src/prompt.rs
grep -rn "multi_select_or_skip(" src/ --include="*.rs" | grep -v "src/prompt.rs"
```
PromptPort と 2 アダプタは実在（本 PBI は seam の新設）。疑似 TTY は導入しない（PBI-103 の領域）。

### 実装手順
1. E2E（RED）: モック選択注入の対話経路 E2E を書き、seam が無いため書けないことを確認（または seam 前提のテストを RED で記録）
2. seam 新設: `CleanPrompt::with_port`（テストファクトリ、`with_cache_dir` 前例と同型）または test-only override。production の `for_current_run` は今日どおり
3. 対話ループ経由の E2E を green にする（freed が削除時実測である主張を含む）
4. 既存 pin 群（drift pin 5 本、round 8 の pin 群）の green 維持確認
5. RED 確認（seam を壊す → E2E 失敗 → 戻す、証拠を記録）

### 落とし穴
- production 呼び出し元に port パラメータを足す書き方（シグネチャ変更）は PBI 75 の ambient 裁定を否定するため不採用。seam はテスト側のみの差し替え
- モック port が dialoguer の型（MultiSelect 等）を返すとテストが dialoguer に依存する。port 契約は選択結果のみ
- prompt.rs の集約 pin（本番領域に `multi_select_or_skip` が存在すること）は seam 新設後も green を維持する形状にする（needle は組立リテラル）
- `confirm_on_tty`（PBI 81）は port を使わない第 3 semantics。本 PBI では触らない（選択 port のみ）。触る場合は PBI-process の gate 表更新を伴う

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 対話経路の E2E が 1 本以上で RED 確認の証拠が記録されている
- [ ] production の挙動変更なし（既存 pin 群 green 維持）
- [ ] コードレビュー完了
- [ ] tests/AGENTS.md の port 注入規約に 1 行追記（必要なら）
