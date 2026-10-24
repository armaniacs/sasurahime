# PBI-47: Minor バンドル（scan --json flush、home() 空 HOME、BatchSummary skipped、steady tick）

## ユーザーストーリー

CLI/バッチ経路を利用するユーザーとして、`scan --json` がキャッシュを温め、空の HOME で沈黙裏に CWD 相対へ書き込まず、バッチの per-entry 失敗が見え、プロンプト競合の steady tick がないようにしてほしい。

## 発見元

1023a 全体レビュー Minor（main.rs:930-933, main.rs:469-473, batch.rs:12-17/23-46, progress.rs:156, generic.rs:882-894, main.rs:983-998）。

## BDD受け入れシナリオ

```gherkin
Scenario: scan --json はキャッシュを温める
  Given キャッシュが初期化されている
  When  ユーザーが scan --json を実行する
  Then  detect 結果が scan_cache に保存される

Scenario: 空の HOME は CWD 相対へ沈黙裏に書かない
  Given HOME 環境変数が空文字列である
  When  sasurahime が起動する
  Then  home() は empty filter で dirs::home_dir() にフォールバックする
  And   キャッシュ/履歴が CWD に書かれない

Scenario: バッチの per-entry 失敗はスキップ数として見える
  Given バッチ実行中に per-entry の skippable 失敗がある
  When  run_batch が完了する
  Then  BatchSummary が skipped 数を運び、CLI が報告する
```

## 受け入れ基準

- [x] main.rs:930-933: `scan --json` が `scan_cache::flush` をバイパスしない（flush 後に return）
- [x] main.rs:469-473: home() が空の HOME を filter（トリップワイヤの empty filter と同じ挙動）
- [x] batch.rs: BatchSummary に skipped フィールドを追加し、全バッチ入口（auto/Caches/All/run_auto/run_interactive）で報告。single-target の "N file(s) skipped" 形状に倣う
- [x] batch.rs:23-46 + progress.rs:156: 非アニメーション分岐が steady tick を有効化しない（no-tick-while-prompting 原則の実装）。reporter dead param を解消または記録
- [x] generic.rs:882-894: resolve_env_dir が join 結果を検証（`..` 付き subpath を fail-closed に）
- [x] main.rs:983-998: 3つの手 maintenance バリアントリストの対応を pin するテスト（External を含む全 variant で command_name/dry_run が panic しない）
- [x] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- E2E/統合: scan --json の flush、空 HOME、バッチ skipped 報告。TDD red → green

## 実装者向け注記

BatchSummary への skipped 追加は呼び出し側（main.rs のバッチ入口）の変更を伴う。手 maintenance リストの pin テストは main.rs のテストモジュールに追加（extern crate として cleaner.rs の CleanTarget を参照）。

## 検証記録 (2026-10-24)

- 実装コミット: 13137f6 / 6d403fa / 48d5061 / 5303a34 / 4354890 / 6302dce / ec378b8 (wt-minor47、1023a にマージ済み、+549/−6)
- 内容: (1) scan --json が early return 前に scan_cache::flush (--no-cache 尊重の pin 付き)。(2) home() が空の HOME を filter し dirs::home_dir() にフォールバック (tripwire の empty filter 規約と同一)。(3) BatchSummary に skipped フィールド (`Vec<(String, String)>`)、run_batch が収集し、全 5 バッチ入口 (Commands::Auto / CleanTarget::Caches / CleanTarget::All / run_auto / run_interactive) で single-target 形状の報告。(4) 非アニメーション分岐から enable_steady_tick を除去 (animated 分岐は tick 保持、no-tick-while-prompting 原則)。reporter dead param は API 変更ではなく batch.rs の doc に記録。(5) resolve_env_dir が join 結果を検証 (contains ".." または is_safe_delete_target 失敗で fail-closed、PBI-46 と同系の eprintln + default、既存の stderr pin は green 維持)。(6) command_name/dry_run に panic-free の External arm + 全 variant を列挙する対応 pin テスト。
- 備考: サブ項目 5 の新規ケースは CLI 呼び出し側が今日リテラル subpath のみのため resolver レベル (統合) で pin。サブ項目 2 の bin テストは lib test_helpers が cfg(test) ゲートのため手動 HOME スワップ。why-why 連鎖は 6 サブ項目とも親に報告済み。
- 検証: 1023a HEAD 8a466d4 で lib 701 passed / 4 failed (失敗 ⊆ 既知 TCC セット) / bin 32/32 / 統合 38 ターゲット 275 テスト green / clippy 0 warnings / fmt clean。
