# PBI-39: [custom] ターゲットを clean <name> で到達可能に（catch-all の影を解消）

## ユーザーストーリー

[custom] ターゲットを設定したユーザーとして、doc（docs/HOWTO-USE.md:414 "sasurahime clean my-project で個別に清掃可能"）どおり CLI で個別清掃できるようにしてほしい。なぜなら、external_subcommand の catch-all が [custom] 名を影に落とし、"no plugin named" で失敗するから。

## 発見元

1023a 全体レビュー Major #3 + #12 + Minor（main.rs:319-320, 1254-1273, 594-600）。実証済み（exit 1）。

## BDD受け入れシナリオ

```gherkin
Scenario: [custom] ターゲットは clean <name> で清掃できる
  Given config.toml に [[custom]] name = "my-workspace" が定義されている
  When  ユーザーが sasurahime clean my-workspace を実行する
  Then  そのターゲットの清掃が実行される
  And   パスがブロックリスト内の場合は拒否される

Scenario: [custom] 名がビルトイン名と衝突した場合は警告される
  Given config.toml に [[custom]] name = "uv" が定義されている
  When  sasurahime が起動する
  Then  ビルトイン名との衝突が警告される（または拒否される）
```

## 受け入れ基準

- [ ] clean <name> の解決順で [custom] 名が external_subcommand の catch-all より先に解決される（既知プラグイン名は現行どおり優先）
- [ ] [[custom]] のビルトイン名衝突を起動時に検証（重複クリーナー行を避ける。警告またはエラー）
- [ ] E2E テスト: [[custom]] config を書いて clean <custom-name> の削除経路を assert、[[custom]] の unsafe 拒否（/etc など）をバイナリ経由で assert
- [ ] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- E2E: tempdir HOME + [[custom]] config で clean <name> の削除/拒否。TDD red（現行が "no plugin named" で失敗）→ green

## 実装者向け注記

CustomPathCleaner の is_safe_delete_target 再検証（custom.rs:50-57）は既存。main.rs の Clean 解決順（custom 登録名 → external_subcommand）の変更が本体。docs/HOWTO-USE.md:414 との整合を確認。
