# PBI: フルディスクアクセス事前診断(doctor)

## ユーザーストーリー
macOS開発者として、フルディスクアクセス(FDA)が足りないせいで計測や削除が黙って失敗するのを事前に知りたい、なぜなら `Operation not permitted` でスキップされた結果、scan の合計が実際より小さく見えて判断を誤るから

## ビジネス価値
TCC保護領域(`~/Library/Mail`、`MobileSync/Backup`、`~/.Trash` 等)の取りこぼしが可視化され、「消したのに減らない」混乱が減る。測定指標は、権限不足が原因で未計測になったパスをユーザーが scan 1回で把握できること。

## 優先度
- 順位: 01 / 5
- RICEスコア: 427(Reach=400 / Impact=2 / Confidence=80% / Effort=1.5人週)
- 根拠: 小さく独立しており、scan の注記として他PBIにも使い回せる。

## BDD受け入れシナリオ

```gherkin
Scenario: FDAがない場合に警告する
  Given 実行元ターミナルにFDAが付与されていない
  When  `sasurahime doctor` を実行する
  Then  読めなかった保護パスの一覧が表示される
  And   システム設定での付与手順が表示される

Scenario: scan で計測不能の件数を注記する
  Given 保護パスの読み取りが拒否される
  When  `sasurahime scan` を実行する
  Then  合計行に「N 件は権限不足で計測不能」と表示される

Scenario: FDAがある場合は静かに通る
  Given 全ての保護パスが読める
  When  `sasurahime doctor` を実行する
  Then  問題なしと表示され終了コードは 0 になる

Scenario: 存在しないパスは問題にしない
  Given Mail を使っておらず ~/Library/Mail が存在しない
  When  `sasurahime doctor` を実行する
  Then  そのパスは「未使用」として扱われ警告されない
```

## 受け入れ基準
- [ ] `doctor` サブコマンドが保護パス群を `read_dir` で試行し、`PermissionDenied` を集約する
- [ ] scan 末尾に権限不足の件数を注記する(`--json` 時はフィールドで出力)
- [ ] 実在しないパスは問題扱いしない
- [ ] 設定変更は自動で行わず案内のみ

## テスト戦略(t_wadaスタイル)

### E2Eテスト
- tempdir を HOME にし、`chmod 000` のディレクトリを含めて `doctor` の出力と終了コードを検証(ハッピーパスと権限不足の2本)

### 統合テスト
- 保護パス判定関数に偽ルートを渡して集約結果を検証
- scan のスキップ集計が doctor と同じ結果になること

### 単体テスト
- `io::ErrorKind` から診断結果への変換
- 存在しないパスの分類
- 注記文言の件数整形(0件・1件・複数件)

## 実装アプローチ
- **Outside-In**: E2E から開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 各レイヤーで適用
- **リファクタリング**: グリーンのたびに scan とのスキップ集計の重複を除去

## 見積もり
3 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: `pbi/2026-10-22-01-fix-scan-unavailable.md` とスキップ集計が重なる
- テスタビリティ: 保護パス表を引数化し tempdir で差し替える
- 非機能要件: `read_dir` の成否のみで判定し、中身は読まない

## 実装者向け注記

### 現状コードの確認
確認済み: `src/main.rs` の `Commands` に `doctor` はない(2026-10-23 時点)。
```bash
grep -rn "doctor" src/
grep -rn "Operation not permitted" src/
grep -n "is_skippable_error" src/cleaner.rs
```

### 実装手順
1. E2E: `doctor` の出力と終了コードのテストを書く(Red)
2. 統合: 偽ルートでの保護パス集約テストを書く(Red)
3. 単体: エラー種別の変換テストを書く(Red)
4. `src/doctor.rs` に保護パス表と `probe()` を実装(Green)
5. `main.rs` に `Doctor` を登録し、scan の注記を接続(Green)
6. スキップ集計の共通化(Refactor)

### 落とし穴
- FDA は親プロセス(ターミナルアプリ)単位で付与される。案内文に明記する
- 権限不足と「存在しない」を混同すると、未使用ユーザーに警告が出続ける

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] E2E/統合/単体のすべてが揃っている
- [ ] コードレビュー完了
- [ ] リファクタリング完了(グリーン後)
- [ ] README の Data access を更新済み
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る
