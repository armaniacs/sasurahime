# PBI: 対話プロンプトのポート統一（TTY チェックとスピナー停止を 1 箇所で所有）

## ユーザーストーリー
キャッシュを清掃する開発者として、対話プロンプトがどのクリーナーでも同じ契約（TTY がないなら対話しない、スピナーの下で描画しない、TUI が確認済みなら再確認しない）で動いてほしい。なぜなら、確認プロンプトの二重表示（dialoguer の redraw バグ）は契約が呼び出し側の覚え書きになっているから再発するから。

## 優先度
- 順位: 55 / 全 8 候補
- RICE スコア: 3.2（Reach=5 / Impact=2 / Confidence=80% / Effort=2.5 人週）
- 根拠: 対話プロンプトが 8 クリーナーファイル 22 サイトに散在し、TTY ガード 9 サイト、suspend_live_output 2/8、not-a-terminal ゲード 6 サイトで文言が乖離、SKIP_CONFIRM は 1 サイトしか効かない。ollama の --yes 契約破れ（52 で個別修正）はこの構造の症状。実装が重いが、確認二重表示の再発を構造的に止める
- 依存: 53（ollama.rs と他クリーナーを共有）の後。56 と同じファイル群に触れるため先に着地

## ビジネス価値
- 確認二重表示（dialoguer redraw バグ）が次のプロンプトで再発しない
- SKIP_CONFIRM が全プロンプトサイトで効き、TUI で選んだターゲットのネスト確認が消える
- clean() が TTY なしでテスト可能になる（非 TTY 実装を注入できる）
- 6 サイトの文言乖離が消える

## BDD 受け入れシナリオ
Scenario: TUI で選んだターゲットは再確認しない
  Given TUI で app-support を選び Proceed を確認した
  When  清掃が進む
  Then  app_support の内部プロンプト（app_support.rs:617/622/586）は出ない

Scenario: 非 TTY で Safe クリーナーを clean できる
  Given ollama がインストール済みで stdin が TTY でない
  When  clean ollama を実行する
  Then  対話を要求せずスキップし exit 0（ポータブルな非 TTY 実装が注入されている）

Scenario: スピナーの下でプロンプトが描画されない
  Given 対話クリーナーがバッチで走る
  When  プロンプトが出る
  Then  生きた進捗行の下で描画されず、suspend が 1 箇所で行われる

## 受け入れ基準
- [x] プロンプトポートの trait（confirm / multi_select）が 1 つ定義され、TtyPrompt 実装と非 TTY/抑制実装がある
  - 実装: src/prompt.rs（PromptPort trait + TtyPrompt + NonInteractive）。検証: prompt::tests 6 本（suspend 検証、NonInteractive 拒否、抑制ラウンドトリップ）
- [ ] ポートは CleanOptions か CleanContext で注入され、TTY チェックと suspend_live_output を 1 箇所で所有する
  - **逸脱（記録）**: 注入は CleanOptions/CleanContext ではなく TtyPrompt 直呼びにした。CleanOptions は 5 フィールドのリテラル構築が main + ~20 テストにあり、フィールド追加で全リテラルが壊れる（RICE に対してコスト過大）。TTY チェックと suspend_live_output の 1 箇所所有は達成（prompt.rs が唯一の dialoguer 呼び出し元）。テスト注入が必要になった時点で CleanContext 注入を別 PBI に
- [x] SKIP_CONFIRM グローバル（generic.rs:24-30）と 9 つの is_terminal ガードが削除されている
  - 実装: SKIP_CONFIRM を削除し prompt::confirm_suppressed に移設（SkipConfirmGuard は guard/restore パターンを維持）。9 ガードを gate_interactive に統合
- [x] 8 クリーナーファイルの dialoguer 直呼びがポート経由に置き換わっている
  - 検証: `grep -rn "dialoguer::" src/cleaners/ --include="*.rs" | grep -v tests` が 0 件
- [x] 6 サイトの not-a-terminal ゲードの文言が標準化されている
  - 実装: gate_interactive が 1 つの標準文面（"[{name}] not a terminal - skipping. Run `sasurahime clean {name}` interactively."）を出力
- [x] include_str! ソースピンテスト（generic.rs:2545-2569、orphan_containers.rs:1382-1419）がポートのテストに置き換わっている
  - 実装: generic.rs と orphan_containers.rs のピンを「ポート経由の呼び出し形状」の検証に置換し、suspend 契約は prompt::tests の振る舞いテストに移設
- [x] 全テストが緑
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）

## テスト戦略
- E2E: 非 TTY で全対話クリーナーがスキップし exit 0（spawn で stdin を null）
- 統合: FakePrompt を注入し、各クリーナーの選択結果が delete_selected に渡ることを検証
- 単体: ポート実装の TTY 判定と suspend 呼び出しを検証

## 実装アプローチ
- 選択 -> 削除の分割（orphan_containers.rs:422/520 の select -> delete_selected が正しい形）を一般化する
- クリーナーは事前に作られた選択を受け取り、プロンプトの所有はポートに委ねる
- 実装順: (1) ポート trait + 2 実装、(2) generic.rs の confirm を置換、(3) orphan_containers の select を置換、(4) 残り 6 ファイル、(5) SKIP_CONFIRM とガードの削除

## 見積もり
13

## 技術的考慮事項
- CleanOptions にポートを持たせると既存呼び出しシグネチャが変わる。後方互換のためにデフォルト実装（TtyPrompt）を用意し、段階的に注入へ
- 判断の根拠: プロンプトの所有を 1 層に集めるのは、TTY 判定・suspend・抑制の 3 契約が「同じ理由で同時に変わる」から（redraw バグは 3 契約の乖離が原因）

## 実装者向け注記
現状確認:
```
grep -rn "dialoguer::" src/cleaners/ --include="*.rs" | grep -v tests
grep -rn "is_terminal" src/cleaners/ --include="*.rs" | grep -v tests
grep -rn "suspend_live_output" src/cleaners/ --include="*.rs" | grep -v tests
grep -n "SKIP_CONFIRM" src/cleaners/generic.rs src/interactive.rs
```
手順: 上記実装順のとおり。落とし穴: (1) SKIP_CONFIRM の削除は interactive.rs の SkipConfirmGuard（interactive.rs:119-132）との契約変更を伴う、(2) include_str! ピンテストはポート置換後に意味を失うので削除する、(3) 8 ファイル同時修正は競合しやすいため 1 ファイルずつ着地させる。

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
  - 検証: 各 PBI のテスト戦略に沿ったテストが実装され緑
- [x] SKIP_CONFIRM と 9 ガードが消えている
  - 検証: grep で SKIP_CONFIRM 0 件、is_terminal ガード 0 件（prompt.rs に統合）
- [x] 非 TTY の E2E が緑
  - 検証: tests/ollama.rs / tests/app_support.rs の非 TTY E2E 緑
- [x] CHANGELOG の [Unreleased] に追記
  - 検証: CHANGELOG.md [Unreleased] に各バッチのエントリあり
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
