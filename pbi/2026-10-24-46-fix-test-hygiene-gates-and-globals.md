# PBI-46: テスト衛生（mise ゲート実体化 + 未リセット グローバル + 空弁論除去）

## ユーザーストーリー

リグレッションを防ぎたい開発者として、安全ゲートが実際に主張し、テストのグローバル pin がリークしないようにしてほしい。なぜなら、mise の名前付き安全ゲートがアクティブバージョンの生存を一度も assert せず、generic/project_artifacts/hint のテストが set_trash_mode の restore を欠き（デフォルト trash に戻らず PERMANENT を残す）、フレーク源だから。

## 発見元

1023a 全体レビュー Major #10 + #11 + Minor（tests/mise.rs:112-146, generic.rs:1061/1081, project_artifacts.rs:448/522, hint/mod.rs:412/444, batch.rs:203, tests/trash.rs:223/256/329, tests/act.rs:60-111）。

## BDD受け入れシナリオ

```gherkin
Scenario: mise の安全ゲートがアクティブバージョンの生存を主張する
  Given clean_mise_active_version_is_never_deleted が実在する
  When  アクティブバージョンを消すリグレッションを意図的に入れる
  Then  このテストが失敗する（生存 assert の実在により）

Scenario: テストの trash-mode pin はリークしない
  Given generic/project_artifacts/hint のテストが set_trash_mode を使う
  When  テストが終了する（assert 失敗を含む）
  Then  グローバルはデフォルト（trash）に戻る（panic-safe TrashModeGuard）
```

## 受け入れ基準

- [ ] tests/mise.rs:112-146 にアクティブバージョン生存の assert を追加（重複 success() assert と eprintln ダンプを実主張に置換）
- [ ] generic.rs:1061/1081（restore がデフォルトと逆）、project_artifacts.rs:448/522（restore なし）、hint/mod.rs:412/444（restore が最終文のみ）を panic-safe TrashModeGuard に置換
- [ ] batch.rs:203 の HOME スワップに TEST_LOCK を付与（履歴プローブとのレース解消）
- [ ] tests/trash.rs:223/256/329 の空弁論 `!contains("moved to Trash")` を除去（本番が決して出力しないリテラル）
- [ ] tests/act.rs と tests/generic.rs:297-316 のトリップワイヤ拒否テストに拒否の出力/結果の実主張を追加
- [ ] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 単体+統合: テストコードの修正のみ。red 証拠として意図的 break（mise 生存 assert の欠如を証明）を一度入れて revert する

## 実装者向け注記

2ba7aad は4クリーナーのテストモジュールのみを対象としたため generic/project_artifacts/hint が残った。TrashModeGuard は test_helpers.rs:121-137（panic-safe）。generic.rs:1041-1083 の pin テスト自体の主張は正しいので restore 方法のみ修正。既知の TCC 環境依存4件は回帰ではない。
