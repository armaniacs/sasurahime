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

- [ ] main.rs:930-933: `scan --json` が `scan_cache::flush` をバイパスしない（flush 後に return）
- [ ] main.rs:469-473: home() が空の HOME を filter（トリップワイヤの empty filter と同じ挙動）
- [ ] batch.rs: BatchSummary に skipped フィールドを追加し、全バッチ入口（auto/Caches/All/run_auto/run_interactive）で報告。single-target の "N file(s) skipped" 形状に倣う
- [ ] batch.rs:23-46 + progress.rs:156: 非アニメーション分岐が steady tick を有効化しない（no-tick-while-prompting 原則の実装）。reporter dead param を解消または記録
- [ ] generic.rs:882-894: resolve_env_dir が join 結果を検証（`..` 付き subpath を fail-closed に）
- [ ] main.rs:983-998: 3つの手 maintenance バリアントリストの対応を pin するテスト（External を含む全 variant で command_name/dry_run が panic しない）
- [ ] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- E2E/統合: scan --json の flush、空 HOME、バッチ skipped 報告。TDD red → green

## 実装者向け注記

BatchSummary への skipped 追加は呼び出し側（main.rs のバッチ入口）の変更を伴う。手 maintenance リストの pin テストは main.rs のテストモジュールに追加（extern crate として cleaner.rs の CleanTarget を参照）。
