# PBI: プライバシー関連ドキュメントの整備

## ユーザーストーリー
ユーザーとして、`sasurahime` が自分のデータにどのようにアクセスするかを知りたい。なぜなら、`sasurahime` は `~/Library/Application Support/MobileSync/Backup`（iOS バックアップ）など個人データが含まれうるディレクトリにアクセスするため、プライバシーに関する透明性が不足しているからである。

## ビジネス価値
- ユーザーの情報に基づいた同意（informed consent）の確保
- GDPR / CCPA の透明性要件への準拠
- iOS バックアップ削除時のリスク認識向上

## BDD受け入れシナリオ

```gherkin
Scenario: README に Privacy セクションが存在する
  Given ユーザーが README.md を開いたとき
  When Privacy セクションを読む
  Then 以下の情報が記載されている:
    - アクセスするデータの種類（キャッシュ、ログ、iOS バックアップ等）
    - データ処理はすべてローカルで行われ外部送信しないこと
    - 削除履歴は local history.json に保存されること
    - max_entries 設定で履歴保存件数を制御できること

Scenario: iOS バックアップ削除前に個人データに関する警告が表示される
  Given iOS バックアップが存在する
  When sasurahime clean ios-backup を実行する
  Then 復元不可性の警告に加えて
  And 「iOS バックアップには連絡先・メッセージ・写真等の個人データが含まれています」という説明が表示される

Scenario: 削除履歴の保存が明示される
  Given ユーザーが初めて sasurahime を使用するとき
  When 任意の clean 操作を実行する
  Then 削除履歴が ~/.local/share/sasurahime/history.json に保存されること
  And その事実が README の Privacy セクションに記載されている
```

## 受け入れ基準
- [ ] README.md に「Privacy」セクションが追加されている（EN + JA）
- [ ] iOS バックアップ削除時の警告文が強化されている
- [ ] `history.json` の保存と設定方法がドキュメントに記載されている
- [ ] `sasurahime clean ios-backup --dry-run` でも警告が表示される
- [ ] 全既存テストがパスする（コード変更は警告文のみ）

## テスト戦略（t_wadaスタイル）

### E2Eテスト（2）
- `sasurahime clean ios-backup --dry-run` で強化された警告が表示される
- 警告文に「連絡先」「メッセージ」などのキーワードが含まれる

### 統合テスト（1）
- iOS バックアップの警告メッセージが期待通りにフォーマットされる

### 単体テスト（2）
- 警告メッセージの文言がプロダクションコードと一致する
- 非 TTY 環境（--yes）では iOS バックアップがスキップされる（既存）

## 実装アプローチ
- **ドキュメント優先**: まず README の Privacy セクションを書き、コード修正は警告文の強化のみ
- テストは E2E で警告文の表示を確認

## 見積もり
1 SP（ドキュメントと警告文の軽微な修正、1日未満）

## 技術的考慮事項
- 依存関係: なし
- コード変更: `src/cleaners/ios_backup.rs` の警告文のみ
- ドキュメントは EN / JA の両方を更新

## 実装者向け注記

### 現状コードの確認
```bash
# iOS バックアップの警告文
grep -n "cannot be restored\|irreversible\|backup" src/cleaners/ios_backup.rs src/main.rs

# README の現状
grep -n "Privacy\|privacy\|personal\|personal data\|telemetry" README.md
```

### 修正箇所
#### README.md に追加する Privacy セクション（案）
```markdown
## Privacy

sasurahime operates entirely locally on your machine. It does not:
- Send any data over the network
- Collect telemetry or usage statistics
- Store or transmit personal information

### Data access

sasurahime reads the following directories to identify cleanable cache data:
- `~/.cache/`, `~/Library/Caches/`, `~/.local/share/`, `~/Library/Application Support/`
- `~/Library/Application Support/MobileSync/Backup/` (iOS backups — only with `sasurahime clean ios-backup`)
- ...

### Deletion history

Every successful clean operation appends a record to `~/.local/share/sasurahime/history.json`
with the cleaner name, freed bytes, and timestamp. This data stays on your machine.
You can control the maximum number of entries via `[history].max_entries` in `config.toml`
(default: 1000).
```

#### iOS バックアップ警告文の強化
`src/cleaners/ios_backup.rs` の既存警告文:
```
"Warning: iOS backups cannot be restored once deleted"
```
→ 以下に変更:
```
"Warning: iOS backups contain personal data (contacts, messages, photos, etc.) and cannot be restored once deleted"
```

### 落とし穴
- 日本語版 README（`docs/README.ja.md` など）があればそちらも更新
- 警告文のマッチ（E2Eテストでアサートしている可能性）を確認し、テストも更新する
- コードにプライバシーポリシー文書を同梱する場合は、`PRIVACY.md` を新規作成してもよい

## Definition of Done
- [ ] README.md に Privacy セクションが追加されている（EN + JA）
- [ ] iOS バックアップ削除時の警告文が強化されている
- [ ] プライバシー関連の E2E テストがパスする
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
