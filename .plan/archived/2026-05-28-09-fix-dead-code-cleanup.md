# PBI: デッドコードのクリーンアップ（`#[allow(dead_code)]`）

## ユーザーストーリー
デベロッパーとして、`#[allow(dead_code)]` で黙殺されたデッドコードの意図を明確にしたい。なぜなら、`pub` で定義されながらプロダクションコードから参照されないアイテムが複数存在し、後から見た開発者が「使われているのか不要なのか」判断できないからである。

## ビジネス価値
- コードの意図明確化（Maintainability Guardian Medium 指摘）
- コンパイラ警告の本来の価値（本当のデッドコード検出）を回復
- `#[allow(dead_code)]` にコメントを追加して将来の削除判断を容易に

## BDD受け入れシナリオ

```gherkin
Scenario: 本当に不要なデッドコードが削除される
  Given ScanStatus::PermissionDenied バリアントが未使用である
  When 削除する
  Then コンパイルが通り、既存テストがパスする

Scenario: 将来のために残すコードに意図が明記される
  Given CleanResult.name, uses_trash が #[allow(dead_code)] で覆われている
  When 維持判断をする
  Then コメントで将来の使用予定（例: TUI 表示用）が明記される
  And #[allow(dead_code)] が維持される

Scenario: 使用予定のない dead code が削除される
  Given LARGE_TRASH_THRESHOLD_BYTES と format_trash_* 関数が未使用
  When 使用予定を確認する
  Then 不要なら削除、必要な場合は使用箇所を追加する
```

## 受け入れ基準
- [ ] `ScanStatus::PermissionDenied` バリアントが削除されている
- [ ] `#[allow(dead_code)]` の全箇所に意図を示すコメントが追加または削除されている
- [ ] `cargo build` で新しい dead_code 警告が発生しない
- [ ] 全既存テストがパスする（ロジック変更なし）

## テスト戦略（t_wadaスタイル）

### 単体テスト（0 — ロジック変更なし）
- コンパイルが通ることのみ確認（コンパイラが保証）

## 実装アプローチ
- コンパイル駆動: `#[allow(dead_code)]` を1つずつ剥がし、警告が出るか確認してから判断

## 見積もり
1 SP（1日未満、純粋なコードクリーンアップ）

## 技術的考慮事項
- 依存関係: なし
- リスク: 極めて低い（ロジック変更なし、コンパイルが全てを検証）

## 実装者向け注記

### 現状確認
```bash
grep -rn "#\[allow(dead_code)\|#\[expect(dead_code)\]" src/ --include="*.rs" | grep -v "test\|#\[cfg(test)"
```

### 各アイテムの判断基準
| アイテム | 場所 | 判断 | 理由 |
|---------|------|:----:|------|
| `ScanStatus::PermissionDenied` | `src/cleaner.rs:12` | **削除** | 未使用。スキャン結果で権限エラーを通知する将来のユースケースは `ScanStatus::NotFound` で代替可能 |
| `CleanResult.name` | `src/cleaner.rs:60-61` | **維持 + コメント** | `pub field`。将来的に TUI 表示やログ出力で cleaner 名を取得するために必要 |
| `CleanResult.uses_trash` | `src/cleaner.rs:63-64` | **維持 + コメント** | `pub field`。将来的に TUI で「Trash / Permanent」表示に使用予定 |
| `LARGE_TRASH_THRESHOLD_BYTES` | `src/cleaner.rs:78-79` | **削除または使用** | 現在未使用。Trash 警告の閾値としての役割は `format_large_trash_warning` の中で直接 1GB と比較されている。確認後削除 |
| `format_trash_warning()` | `src/cleaner.rs:82-105` | **削除または使用** | PBI-C で追加されたが現在未使用。使用予定がなければ削除 |
| `format_large_trash_warning()` | `src/cleaner.rs:107-125` | **削除または使用** | 同上 |

### 修正手順
1. `#[allow(dead_code)]` を1つずつ剥がす
2. `cargo build` で警告が出るか確認
3. 警告が出たら、上記判断基準に従って削除またはコメント追加
4. 全 `#[allow(dead_code)]` を処理したら `cargo test` と `cargo clippy` を実行

## Definition of Done
- [ ] 全 `#[allow(dead_code)]` が処理済み（削除 or コメント付き維持）
- [ ] `cargo build` で新しい警告が出ない
- [ ] `cargo clippy -- -D warnings` が通る
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
