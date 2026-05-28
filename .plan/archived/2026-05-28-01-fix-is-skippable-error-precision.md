# PBI: is_skippable_error の文字列マッチング精度向上

## ユーザーストーリー
デベロッパーとして、`sasurahime clean` が通常のエラーメッセージを誤ってスキップしないでほしい。なぜなら、部分文字列マッチングにより真面目なエラー（ファイルシステム破損など）が抑制され、問題の切り分けが遅れるからである。

## ビジネス価値
- エラーの誤抑制によるデータ損失リスクの低減
- `is_skippable_error` の信頼性向上（Red Team / Blue Team 2名重複指摘）
- スコア影響: Medium × 2 → スコア +10 相当

## BDD受け入れシナリオ

```gherkin
Scenario: io::ErrorKind に合致するエラーはスキップされる
  Given PermissionDenied / WouldBlock / AlreadyExists の io::ErrorKind エラーが発生したとき
  When is_skippable_error() を呼び出す
  Then true を返す

Scenario: 通常のエラーメッセージに "Permission denied" という単語が偶然含まれている場合
  Given "The database says: Operation not permitted in current mode" というエラーメッセージ
  When is_skippable_error() を呼び出す
  Then false を返す（io::ErrorKind が PermissionDenied でない場合のみ）

Scenario: ファイル名に "trash failed" が含まれるパス起因のエラー
  Given "/Users/foo/.trash failed to remove" のようなパスを含む io::Error
  When is_skippable_error() を呼び出す
  Then io::ErrorKind が PermissionDenied/WouldBlock/AlreadyExists でなければ false

Scenario: trash crate 由来のラップされたエラーはスキップされる
  Given trash crate が "trash failed: Resource busy" を返したとき
  When is_skippable_error() を呼び出す
  Then true を返す
```

## 受け入れ基準
- [ ] 既存の全テストが修正後もパスする（後方互換性）
- [ ] 6件のエッジケーステスト（Test Experts 作成済み）がパスする
- [ ] `is_skippable_error` の返り値が予期せず true になるケースが既存テストでカバーされている

## テスト戦略（t_wadaスタイル）

### E2Eテスト（1）
- 各 cleaner の clean 実行時にスキップすべきでないエラーが正しく伝播することを確認（既存）

### 統合テスト（3）
- `is_skippable_error` が `io::ErrorKind` 由来のエラーを正しく分類する
- `trash` クレートのラップエラーが正しく検出される
- ファイル名にキーワードを含む io::Error が誤検出されない

### 単体テスト（14 = 既存8 + 新規6）
- 既存: PermissionDenied, WouldBlock, AlreadyExists, NotFoundは非スキップ, ConnectionRefusedは非スキップ, CleanCancelledは非スキップ, 文字列マッチ, arbitrary errorは非スキップ（8件）
- Test Experts 追加済み: 6件のエッジケース（false positive, trash failed, Resource busy, filesystem corruption etc.）

## 実装アプローチ
- **Outside-In**: エッジケーステスト（既存）→ 実装 → 全テスト通過確認
- **Red-Green-Refactor**: テストが既に存在するので、実装を変更して全テスト通過を確認

## 見積もり
1 SP（1人日未満の小規模変更）

## 技術的考慮事項
- 依存関係: なし（`src/cleaner.rs` の単一関数変更）
- テスタビリティ: テスト済み（Test Experts が6件のエッジケーステストを追加済み）
- 非機能要件: パフォーマンス影響なし

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "is_skippable_error" src/
```

### 現状の課題
`src/cleaner.rs:119-133` の `is_skippable_error` 関数:
1. `io::ErrorKind` downcast（L120-127）: 正しい。`PermissionDenied`, `WouldBlock`, `AlreadyExists` — これらは維持
2. 文字列マッチング（L128-132）: 問題。`format!("{e:#}")` でエラーチェーン全体を文字列化し、部分文字列 `contains()` でマッチ。ファイル名やエラーメッセージに偶然含まれるキーワードも誤マッチする

### 修正方針
`trash` crate のエラー型を特定する方法が現状ないため、以下の段階的アプローチ:
1. `io::ErrorKind` downcast が優先（現状維持）
2. 文字列マッチを `io::ErrorKind` の downcast に失敗した場合のみ実行するよう順序変更（現状と同じだが、io::ErrorKind マッチが優先されることを明確化）
3. 文字列マッチにアンカーを追加（前方一致または正規表現で "trash failed:" のようなプレフィックスを要求）
4. または `trash` クレートのエラー型をラップする専用マッチャーを実装

### 落とし穴
- `trash` クレートは独自のエラー型を持ち、`io::ErrorKind` を保持しない場合がある。これを文字列マッチ以外で検出する方法を調べること
- 既存のテストがパスし続けることを確認すること。Test Experts が6件のエッジケーステストを追加済み

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする（6件のエッジケーステスト含む）
- [ ] `cargo test` 全511テストがパスする
- [ ] コードレビュー完了
- [ ] リファクタリング完了（グリーン後）
