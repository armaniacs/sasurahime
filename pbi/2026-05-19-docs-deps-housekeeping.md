# PBI: Documentation & Dependency Housekeeping

## ユーザーストーリー
プロジェクトメンテナーとして、README が最新の出力と一致し、依存関係のバージョンが整理されていることがほしい、なぜなら現在 README の scan 出力例にバージョンバナーが反映されておらず、`windows-sys` が3バージョン混在しているから。

## ビジネス価値
プロジェクトを初めて見た開発者が正確な情報を得られる。SBOM 生成ツールが正しく依存関係を解析できる。推移的依存関係のバージョン混在による潜在的なコンフリクトリスクを低減する。

## BDD受け入れシナリオ

```gherkin
Scenario: README の scan 出力例が実際の出力と一致する
  Given README に scan コマンドの出力例が記載されている
  When  実際に sasurahime scan を実行する
  Then  出力例のフォーマットが実際の出力と一致する
  And   バージョンバナーが stderr に出力されることが記載されている

Scenario: cargo update 後に windows-sys の重複が解消される
  Given Cargo.lock に windows-sys が3バージョン存在する
  When  cargo update を実行する
  Then  重複バージョンが可能な限り統合される
```

## 受け入れ基準
- [ ] README の scan 出力例がバージョンバナー（stderr）を含め実際の出力と一致している
- [ ] README に存在しないフラグ（`scan --dry-run` 等）が削除または修正されている
- [ ] `cargo update` が正常に完了する
- [ ] `windows-sys` のバージョン数が削減されている（完全解消できなくても改善）
- [ ] `cargo test` 全パス

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- README のコマンド例が実際にエラーにならないことの確認

### 統合テスト
- なし（ドキュメント + 依存関係整理のため）

### 単体テスト
- なし

## 実装アプローチ

### 1. README 更新
`README.md` の変更差分チェック：

- `scan` 出力例：先頭行に `sasurahime v0.1.2` が stderr として追加されたことを反映（stdout の例はそのまま）
- `scan --dry-run` 行の削除（存在しないフラグ）または実際の動作に合わせた修正
- `brew` ターゲット名を README Usage に追加（現状の一覧と実際のターゲットが一致しているか確認）

### 2. cargo update 実行

```bash
cargo update
```

これにより Cargo.lock 内の依存関係が最新の互換バージョンに更新される。`windows-sys` の重複が解消されることを確認。

- **分割戦略**: README 更新と `cargo update` は独立して実施可能なので、どちらからでも良い

## 見積もり
1ストーリーポイント未満

## 技術的考慮事項
- 依存関係: `cargo update` は既存の Cargo.toml のバージョン制約内で実行される。予期せぬ breaking change が入らないことを確認
- テスタビリティ: README の正確性は目視確認が主
- リスク: `cargo update` によって少数のマイナーバージョンアップが入る可能性があるが、ロックファイルの更新は安全

## Definition of Done
- [ ] README の scan 出力例が実出力と一致
- [ ] README の Usage セクションが正確
- [ ] `cargo update` が正常実行され、`windows-sys` のバージョン数が削減された
- [ ] `cargo test` 全パス
- [ ] コミット完了
