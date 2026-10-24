# PBI-38: プラグインターゲット（External）の dry-run/help 処理修正

## ユーザーストーリー

プラグインを利用するユーザーとして、`--dry-run` がプラグインターゲットでも効き、`--help` が決して削除を実行しないようにしてほしい。なぜなら、CleanTarget::External が raw 引数からのみ --dry-run を再パースし（グローバルフラグを無視）、--help が生引数として捕捉されてプラグインの実削除が走るから。

## 発見元

1023a 全体レビュー Major #1 + #2 + #12（main.rs:1266, tests/dry_run.rs 未カバー）。実証済み（デバッグバイナリで `--dry-run clean <plugin>` が実削除、`clean <plugin> --help` が実削除）。

## BDD受け入れシナリオ

```gherkin
Scenario: グローバル --dry-run がプラグインターゲットに到達する
  Given sasurahime-foo プラグインが PATH にある
  When  ユーザーが sasurahime --dry-run clean sasurahime-foo を実行する
  Then  プラグインは --dry-run 引数付きで起動される
  And   削除は行われない

Scenario: プラグインターゲットの後ろの --help は削除を実行しない
  Given sasurahime-foo プラグインが PATH にある
  When  ユーザーが sasurahime clean sasurahime-foo --help を実行する
  Then  プラグインの clean は実行されない
  And   ヘルプ（または無操作）で終了する
```

## 受け入れ基準

- [x] main.rs:1266 の External arm が `cli.dry_run` を OR する（グローバルフラグの到達）
- [x] `--help`/`-h` が external 引数に含まれる場合、プラグインの clean を実行しない（ヘルプ扱いにする）
- [x] tests/dry_run.rs に External のグローバル/ローカル --dry-run テストを追加（--permanent コントロール付きの非空虚な主張）
- [x] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- E2E: tests/dry_run.rs に External ケース（tests/plugin.rs の fake プラグインを流用）。TDD red（現行がグローバルフラグを無視）→ green

## 実装者向け注記

--dry-run のローカル位置（`clean <plugin> --dry-run`）は現行動作で機能している。グローバル位置の到達が修正対象。--help の捕捉は external_subcommand の catch-all との兼ね合いに注意（PBI 39 と同じ領域だがファイルは main.rs の External arm 内で完結）。

## 検証記録 (2026-10-24)

- 実装コミット: faf4183 (wt-ext、1023a にマージ済み)
- 内容: External arm が `cli.dry_run || args contain --dry-run` を OR、`--help`/`-h` が external 引数に含まれる場合はアドバイザリ表示 + exit 0 でプラグインを一切実行しない。tests/dry_run.rs にグローバル/ローカル --dry-run テストと --permanent コントロールを追加。
- 検証: 1023a HEAD 64c44a1 で lib 失敗 ⊆ 既知 TCC セット / bin 30/30 / 統合 38 ターゲット green / clippy 0 warnings / fmt clean。
