# PBI: プロンプト suspend 契約の統一（live reporter を 6+ 箇所に正しく渡す）

## ユーザーストーリー
TUI でクリーニングするユーザーとして、選択プロンプトの表示中にスピナーが再描画されない状態がほしい。なぜなら、プロンプトポート（PBI 55）の suspend 契約は live reporter を渡す call site でのみ有効だが、6+ 箇所が `&DeepSuppressReporter`（`suspend_live_output` を上書きしない no-op）をハードコードしており、TTY では dialoguer の下でスピナーが再描画されうるから。

## 優先度
- 順位: 64 / 全 9 候補
- RICE スコア: 3.2（Reach=2 / Impact=2 / Confidence=80% / Effort=1.0 人週）
- 根拠: `TtyPrompt` は渡された reporter の `suspend_live_output` で dialoguer を包む（src/prompt.rs:80-118）が、library_caches.rs:252、library_logs.rs:299、app_support.rs:325/335/414/434、ios_backup.rs:148、apfs_snapshot.rs:364/407 が `&DeepSuppressReporter` をハードコードする。`DeepSuppressReporter` は `suspend_live_output` を上書きせず（src/progress.rs:320-327）、既定の no-op（progress.rs:17-19）を継承する。live reporter を渡しているのは orphan_containers.rs:436、ollama.rs:164、generic.rs:567 のみ。確認プロンプトが hit した dialoguer 再描画バグが 5 クリーナーで再現しうる。どの reporter を渡すかを固定するテストが無い

## ビジネス価値
- dialoguer 再描画バグ（確認プロンプトの二重表示事故と同型）が 5 クリーナーで再現するバグのクラスが消える
- ポートの docstring の「ここで一度だけ直る」が実際に真になる

## BDD 受け入れシナリオ
Scenario: 選択プロンプト中にスピナーが再描画されない
  Given live スピナーが動いている状態で選択プロンプトが表示される
  When  dialoguer が描画する
  Then  live 出力は suspend され、再描画が競合しない

Scenario: どの reporter を渡すかがテストで固定される
  Given 6+ の call site
  When  call site が TtyPrompt に reporter を渡す
  Then  live reporter を渡していることがソースピンテストで固定される

## 受け入れ基準
- [x] TtyPrompt を呼ぶ全 call site が live reporter を渡す（no-op suspend のハードコードが消える）
- [x] call site の reporter を固定するソースピンテストが追加されている
- [x] dialoguer 呼び出しは prompt.rs 1 か所に集約されたまま
- [x] 全テストが緑

## テスト戦略
- 統合: prompt::tests の suspend 契約テスト（RecordingReporter で suspend 回数を数える）
- 検索: `grep -rn "DeepSuppressReporter" src/cleaners/ --include="*.rs" | grep -v tests` が TtyPrompt 呼び出し元で 0 件

## 実装アプローチ
- 実装順: (1) 各 cleaner の clean(dry_run, reporter) が reporter を保持しているか確認、(2) TtyPrompt 呼び出しに live reporter を渡す、(3) ソースピンテスト
- 判断の根拠: ポートがグローバルな live reporter を持つ選択肢は採らない（グローバル state の再導入になる）。call site が渡す方向で統一する

## 見積もり
1.0 人週

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
- [x] CHANGELOG の [Unreleased] に追記
- [x] コードレビュー完了
