# PBI-007: ログファイル自動整理

> **2026-05-17 更新**: kilo 専用から汎化。初期ターゲットは kilo / opencode / claude-code の 3 つ。PBI-011 の設定ファイルでユーザーが追加できる設計にする。

## ユーザーストーリー
**AI コーディングツール（kilo 等）を日常的に使う開発者**として、一定日数より古いログファイルを自動削除したい、なぜなら各50MB のログが数十ファイル溜まって 1GB を超えることがあるから。

## ビジネス価値
- 今回のセッションで kilo ログ 43ファイル・1.7GB のうち 7日以内の分を残して約 1.1GB 回収

## BDD 受け入れシナリオ

```gherkin
Scenario: 7日より古い kilo ログを削除する
  Given ~/.local/share/kilo/log に 43 個のログファイルが存在する
  And そのうち 22 個が 7 日以上前のタイムスタンプを持つ
  When `sasurahime clean logs` を実行する
  Then 7 日より古い 22 ファイルが削除される
  And 最新 7 日分のファイルは保持される
  And 削除ファイル数と回収サイズが表示される

Scenario: 保持日数を変更する
  When `sasurahime clean logs --keep-days 30` を実行する
  Then 30 日より古いファイルのみ削除される

Scenario: ログディレクトリが存在しない
  Given ~/.local/share/kilo/log が存在しない
  When `sasurahime clean logs` を実行する
  Then "kilo logs: not found" と表示して正常終了する

Scenario: --dry-run で削除対象を確認する
  When `sasurahime clean logs --dry-run` を実行する
  Then 削除対象ファイル一覧とサイズが表示される
  And 実際には削除されない
```

## 対象ログディレクトリ（初期ハードコード）

| ツール | パス | 除外 |
|--------|------|------|
| kilo | `~/.local/share/kilo/log/` | `dev.log` |
| opencode | `~/.local/share/opencode/logs/` | — |
| claude-code | `~/.local/share/claude/logs/` | — |

PBI-011 の設定ファイルでユーザーが追加ターゲットを定義できる。

## 受け入れ基準
- [ ] 上記 3 ツールのログディレクトリを対象とする
- [ ] デフォルトは 7 日以上前のファイルを削除
- [ ] `--keep-days <N>` で保持日数を変更できる
- [ ] `--dry-run` で削除せず一覧表示のみ
- [ ] ログディレクトリ不在時はスキップ
- [ ] `LogTarget { name, path, pattern, exclude }` 構造体で定義し、設定ファイルから拡張可能にする

## t_wada スタイル テスト戦略
```
E2Eテスト:
- tmpdir に 10日前・3日前・1日前のログを作成し、--keep-days 7 で 10日前のみ削除されることを確認

統合テスト:
- LogCleaner::find_old_logs(dir, days=7) が正しいファイル一覧を返すことを確認

単体テスト:
- is_older_than(mtime, days=7) の境界値テスト（6日23時間 => false、7日1秒 => true）
- "dev.log" が除外対象になること
```

## 実装アプローチ
- `std::fs::metadata().modified()` でタイムスタンプ取得
- 対象パターンは設定ファイルで拡張できるよう `LogTarget { path, pattern, exclude }` で定義

## 見積もり
2 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] 境界値（ちょうど N 日）の動作を確認
- [ ] `cargo clippy` 警告ゼロ
