# PBI: パージ可能領域の表示とスナップショット間引き

## ユーザーストーリー
macOSユーザーとして、大きなファイルを消しても空きが増えないとき、必要な分だけローカルスナップショットを間引きたい、なぜなら現状の apfs-snapshot は個別削除しかなく、目標容量を指定した解放ができないから

## ビジネス価値
「消したのに減らない」の原因切り分けと対処が1コマンドで済む。OS自身の間引きロジックに任せるため、個別削除より安全。測定指標は、指定した目標に対する解放量が前後の空き差で報告されること。

## 優先度
- 順位: 05 / 5
- RICEスコア: 192(Reach=400 / Impact=2 / Confidence=60% / Effort=2.5人週)
- 根拠: 有用だが root 権限が必要で検証環境が限られるため、信頼度を下げた。

## BDD受け入れシナリオ

```gherkin
Scenario: 目標容量を指定して間引く
  Given ローカルスナップショットが存在する
  When  `sasurahime clean apfs-snapshot --thin 10GB` を実行し確認に同意する
  Then  `tmutil thinlocalsnapshots / 10000000000 2` が実行される
  And   前後の空き容量の差が解放量として表示される

Scenario: 緊急度4は明示指定と追加確認が必要
  When  `--thin 10GB --urgency 4` を指定する
  Then  進行中のバックアップが中断される旨の警告が表示される
  And   追加の確認が求められる

Scenario: dry-run では実行しない
  When  `--thin 10GB --dry-run` を指定する
  Then  tmutil は呼ばれず予定のみ表示される

Scenario: tmutil が使えない場合はスキップする
  Given tmutil が PATH にない、または権限が足りない
  When  `--thin 10GB` を指定する
  Then  エラー終了せず NotFound または Skipped として表示される
```

## 受け入れ基準
- [ ] 緊急度の既定は 2。4 は `--urgency 4` と追加確認が必須
- [ ] 実行は `CommandRunner` 経由で、内部で sudo を呼ばず再実行手順を案内する
- [ ] 解放量は前後の空き差で報告し、パージ可能領域は遅延更新される旨を既存の注記と整合させて表示する
- [ ] `dd` や `hdiutil` による擬似ストレージ圧迫での強制パージは実装しない

## テスト戦略(t_wadaスタイル)

### E2Eテスト
- `CommandRunner` モックで tmutil の引数列(bytes, urgency)と出力を検証

### 統合テスト
- dry-run で呼び出しが 0 回であること
- tmutil 不在・権限不足時の状態遷移

### 単体テスト
- `10GB` 等のサイズ文字列のパース
- 緊急度の範囲検証(1から4以外は拒否)
- 解放量の計算(増減が負にならない)

## 実装アプローチ
- **Outside-In**: E2E から開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 各レイヤーで適用
- **リファクタリング**: 既存の `freed_bytes` と前後比較の処理を共通化

## 見積もり
5 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: なし
- テスタビリティ: tmutil と空き容量取得を注入可能にする(`with_free_space` が既にある)
- 非機能要件: パージ可能領域の概算は OS バージョン差があるため、取得できない場合は表示を省略する

## 実装者向け注記

### 現状コードの確認
確認済み: `thinlocalsnapshots` は `src/` に存在しない。`apfs_snapshot.rs` に purgeable の言及はコメントのみ(2026-10-23 時点)。
```bash
grep -rn "thinlocal" src/
grep -n "purgeable\|tmutil" src/cleaners/apfs_snapshot.rs
grep -n "ApfsSnapshot" -A 6 src/main.rs
```

### 実装手順
1. E2E: tmutil 引数列のテストを書く(Red)
2. 統合: dry-run と tmutil 不在のテストを書く(Red)
3. 単体: サイズ文字列と緊急度のテストを書く(Red)
4. `ApfsSnapshotCleaner` に thin オプションを追加(Green)
5. `main.rs` の ApfsSnapshot に `--thin` `--urgency` を追加(Green)
6. 前後比較の共通化(Refactor)

### 落とし穴
- 緊急度 4 はバックアップを強制中断する。既定にしない
- 解放量はパージ可能領域の遅延更新で過小に見えることがある。注記で補う

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] E2E/統合/単体のすべてが揃っている
- [ ] コードレビュー完了
- [ ] リファクタリング完了(グリーン後)
- [ ] README を更新済み
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る
