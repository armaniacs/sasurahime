出所: テスト手法レビュー(2026-11-21、アンカー main @ 0481291)。フェーズ 0 の既実装確認済み(2026-11-21 時点で cargo-nextest は未導入、Makefile の test は素の cargo test のみ)。

# PBI 2026-11-21-08: nextest による per-test 計測基盤を整える

## ユーザーストーリー

保守担当者として、テストごとの実行時間とリトライ状況が見える化される仕組みがほしい、なぜなら「1 回だけ 30 秒かかったテストスイート」のような外れ値や、稀なフレーキーを「いつ・どのテストで」特定できるようにするから。

## ビジネス価値

- **価値**: テストスイートの観測可能性。実測 60 秒の `make test` の内訳が per-test で見え、外れ値(2026-11-21 実測: tests/plugin.rs が 3 回中 1 回だけ 30.06 秒)の属性付けと、リトライでのフレーキー検出が可能になる
- **測定方法**: `make test-nextest` が per-test timing を表示する。`--retries 2` で retry が発生したテストがレポートに現れる。既存の `make test`(cargo test)は無変更で green

## 実測の事実(2026-11-21 レビュー)

- `make test` = `cargo test` のみ。per-test の実行時間は表示されず、外れ値(下記)の属性付けができない
- tests/plugin.rs が 3 回中 1 回だけ 30.06 秒(他は 1.0〜1.3 秒)。30 秒は `grandchild_holding_pipe_does_not_hang_the_scan` の孫プロセス `sleep 30` と一致するが、再現性がなく推論だけでは断定できない
- cargo-nextest は未導入。Makefile には cargo-outdated / cargo-audit / cargo-msrv の「あれば使う、なければ案内する」パターンが既存(Makefile:58-83)
- cargo test 全体: wall 約 60 秒、CPU 使用率 46%(プロセス spawn と待ち時間が支配的)

## BDD 受け入れシナリオ

```gherkin
Scenario: 外れ値テストを特定する(ハッピーパス)
  Given per-test 計測が有効なテストランナーがある
  When  保守担当者が make test-nextest を実行する
  Then  テストごとの実行時間が一覧され、最も遅いテストが即座に分かる

Scenario: 稀なフレーキーを記録に残す(エラー/境界ケース)
  Given あるテストが実行タイミングによって偶発 red する
  When  リトライ付きのテストランナーでスイートを回す
  Then  リトライで green になったことがレポートに記録され、フレーキーの存在が可視化される
```

## 受け入れ基準

- [ ] Makefile に `test-nextest` ターゲットを追加する。cargo-outdated / audit と同じ「あれば使う、なければ導入案内を出す」パターンに従う
- [ ] `cargo nextest run` に `--retries 2` を設定し、retry の発生がレポートで分かる
- [ ] per-test timing が確認できる実行方法を tests/AGENTS.md または AGENTS.md のコマンド節に 1 行追記する
- [ ] 既存の `make test` / `make release`(lint test build …)は無変更で動く(release の test は cargo test のまま)
- [ ] nextest 未導入環境で `make test-nextest` が導入案内を出して失敗しない(exit 0 の案内 or 明示的な案内メッセージ。outdated ターゲットの現行挙動に合わせる)
- [ ] `cargo clippy --tests -- -D warnings` と `cargo fmt --check` がクリーン(Makefile 変更のみでも実行して確認)

## テスト戦略(t_wada スタイル)

### E2Eテスト
- `make test-nextest` が nextest 導入環境で per-test timing を表示し、未導入環境で案内を出す(shell レベルの確認。Rust テストは不要)

### 統合テスト
- なし(計測基盤の導入であり、プロダクトコードの変更はない)

### 単体テスト
- なし。本 PBI 自体は新規テストを追加しない(既存スイートの観測手段を増やすのが目的)

## 実装アプローチ
- **Outside-In**: まず Makefile ターゲット(ユーザーが触る面)→ 動作確認 → ドキュメント 1 行
- **Red-Green-Refactor**: 対象がシェルのため厳密な TDD サイクルではなく、確認手順を先に定義してから実装する

## 見積もり
1 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: なし(PBI 06 / 07 と並列可。Makefile と docs を触るのは本 PBI のみ)
- テスタビリティ: 本 PBI が計測基盤そのものを提供する
- 非機能要件: 既存の cargo test パスと release フローを壊さない

## 実装者向け注記

### 現状コードの確認
(2026-11-21 時点で実施済み。着手時に再確認すること)
```bash
cargo nextest --version
rg -n "outdated|audit|msrv" Makefile
```
nextest 未導入。Makefile:58-83 に「あれば使う」パターンの先例がある。

### 実装手順
1. `cargo install cargo-nextest --locked`(ローカル検証用)
2. Makefile の依存チェック節(Makefile:53 以降)に `test-nextest` ターゲットを追加する:
   ```make
   test-nextest:
   	@if command -v cargo-nextest >/dev/null 2>&1; then \
   		cargo nextest run --retries 2; \
   	else \
   		echo "⚠️   cargo-nextest not installed."; \
   		echo "    Install: cargo install cargo-nextest --locked"; \
   	fi
   ```
3. `.PHONY` 行に `test-nextest` を追記する
4. AGENTS.md の Commands 節(or tests/AGENTS.md)に `make test-nextest` の 1 行説明を追記する
5. `make test` と `make test-nextest` を実行して両方が期待どおりであることを確認する

### 落とし穴
- **nextest は doc-tests を実行しない**。現行 `cargo test` は doc-tests 4 本を含む。`make test-nextest` を `make test` の代わりに使う場合は `cargo test --doc` を併走させるか、ターゲット内で `cargo nextest run ... && cargo test --doc` とする(どちらにするか PR で明記)
- nextest は各テストを**独立プロセス**で実行する。プロセス内グローバル(TEST_LOCK、verbose フラグ)由来のフレーキーは構造的に消えるが、PBI 06 の cargo test パス修正の代わりにはならない(release フローと Makefile test は cargo test を使い続けるため)
- `--retries 2` はフレーキーを「隠す」のではなく「記録に残す」ための設定。retry で green したテストはレポートに flaky として現れる。これを黙視しないこと
- Makefile のレシピ行はタブで書く(スペースだと make が落ちる)

## Definition of Done
- [ ] 全 BDD シナリオが手順として確認されパスする(per-test timing の表示、retry の記録)
- [ ] 既存の `make test` / `make release` が無変更で動く
- [ ] コードレビュー完了(GitHub PR での approve を必須)
- [ ] リファクタリング完了(確認後)
- [ ] ロールバック手段: Makefile 追記と docs 1 行のみのため revert で完全に戻る
- [ ] ドキュメント更新済み(AGENTS.md または tests/AGENTS.md の 1 行)
