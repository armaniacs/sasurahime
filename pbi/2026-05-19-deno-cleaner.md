# PBI: Deno Cache Cleaner

## ユーザーストーリー
Denoユーザーとして、Deno のモジュールキャッシュを掃除したい、なぜなら `deno cache` でダウンロードした依存モジュールが徐々に蓄積するから。

## ビジネス価値
Deno ユーザーは今は少ないが増加傾向。実装コストが極めて低い（`deno cache -r` の1行）。

## BDD受け入れシナリオ

```gherkin
Scenario: Deno キャッシュを削除する
  Given deno コマンドが PATH に存在する
  When  sasurahime clean deno を実行する
  Then  deno cache -r が実行される

Scenario: Deno が未インストールならスキップ
  Given deno コマンドが PATH に存在しない
  When  sasurahime clean deno を実行する
  Then  "deno: not found" と表示される
```

## テスト戦略

### E2Eテスト
- ダミーの `deno` スクリプトを作成して実行確認
- deno 未インストール時のスキップ確認

### 単体テスト
- なし（外部CLI委譲のみ）

## 実装アプローチ
- `deno cache -r`（bun / go / pip と同じ外部CLI委譲パターン）
- detect: `deno` が PATH にあれば pruneable と報告
- `src/cleaners/generic.rs` に追加（既存の `GenericCleaner` パターン）
