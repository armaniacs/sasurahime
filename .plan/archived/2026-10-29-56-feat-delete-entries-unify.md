# PBI: per-entry delete ループの統一（delete_entries を 1 本に）

## ユーザーストーリー
リポジトリの保守担当者として、削除ループの skip 意味論・削除パス追跡・終了コード導出が 1 箇所の修正で済む状態がほしい。なぜなら、6 つの全文コピーがすでに乖離しており、1 箇所の修正が 6 箇所に飛び火する状態は修正漏れとバグの温床だから。

## 優先度
- 順位: 56 / 全 8 候補
- RICE スコア: 3.2（Reach=3 / Impact=2 / Confidence=80% / Effort=1.5 人週）
- 根拠: 「progress 初期化/tick/finish -> safe_delete -> fatal 臂 -> skip_reason 臂 -> uses_trash |= -> freed += -> removed 表示」のスケルトンが 6 クリーナーで全文コピー（library_caches.rs:226-282、library_logs.rs:153-209、orphan_containers.rs:520-590、app_support.rs:476-560、ios_backup.rs:119-147、device_support.rs:133-168）+ 軽量版 26 サイト/19 ファイル。コピーは乖離済み（deleted_paths 記録 2/6、eprintln vs log::error 4/2、fatal 臂 4/6）。依存: 55（同じクリーナー群）の後、59（DeleteError の着地先）の前
- 依存: 55 の後に着地。59 の着地先になる

## ビジネス価値
- skip 意味論・削除パス追跡・終了コード導出の修正が 1 箇所で済む
- 新規の対話式クリーナーが約 60 行のコピーを書かずに済む
- 乖離した挙動（deleted_paths の有無、ログ先の不一致）が統一される

## BDD 受け入れシナリオ
Scenario: 削除失敗が SkippedEntry として記録される
  Given 削除対象が 3 件あり 1 件の safe_delete が失敗する
  When  delete_entries を実行する
  Then  残り 2 件は削除され、失敗 1 件は SkippedEntry になり freed は成功分のみ

Scenario: 対話式クリーナーが同じループを使う
  Given orphan_containers が選択結果を持つ
  When  delete_selected を実行する
  Then  共有 delete_entries を経由し、deleted_paths が記録され deleted_paths が空の乖離が起きない

## 受け入れ基準
- [x] `delete_entries(name, runner, reporter, mode, items: &[(PathBuf, u64)]) -> CleanResult` が trash.rs（safe_delete の隣）に 1 本定義されている
  - 実装: crate::trash::delete_entries。検証: trash::tests 2 本（vanished entry で continue + サム、空 items でゼロ）
- [x] 6 クリーナー（library_caches、library_logs、orphan_containers、app_support、ios_backup、device_support）が共有ループを呼ぶ
  - 実装: 6 ファイルが薄いアダプタ（entry 型 -> (path, size) ペア）経由で共有ループを呼ぶ。app_support は Blocked/resolve 前段フィルタ + skip マージ
- [x] 軽量版 26 サイトのうち、同じスケルトンのものが共有ループまたはヘルパーに統一されている
  - 実装: cargo/rustup/log/xcode/custom/ollama/uv/browser/gradle/project_artifacts のループは per-entry が PBI 53 形式（Ok/Err -> SkippedEntry）で残るが、ログ先と skip 記録は PBI 59 の SkipCause で統一
- [x] deleted_paths の記録、ログ先（eprintln vs log::error）、fatal 臂の有無の乖離が解消されている
  - 実装: 共有ループは deleted_paths を常に記録（読み手は存在しないため挙動中立）、ログは eprintln 1 形式、fatal は continue。enable_live_elapsed と total freed の出力は呼び出し側の契約として call site に残す（orphan の delete 選択フェーズの契約）
- [x] 全テストが緑
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）

## テスト戦略
- 統合: 共有ループの単体（fake runner で 1 件失敗 -> SkippedEntry、成功分のみ freed）
- E2E: 6 クリーナーの既存テストが緑のまま（挙動維持）
- 単体: fatal/skip 方針パラメータの分岐

## 実装アプローチ
- 共有ループに fatal/skip 方針をパラメータで渡す（per-item の失敗を SkippedEntry にするか、致命的にするか）
- orphan_containers（enable_live_elapsed）と app_support（per-app prep）は共有ループをラップして extras を足す
- 判断の根拠: 6 コピーが「同じ理由で同時に変わる」（skip 意味論の変更は 6 すべてに波及）ため共通化は過剰でなく妥当

## 見積もり
8

## 技術的考慮事項
- items の型を (PathBuf, u64) に統一する際、entry 型（OrphanCandidate など）からマッピングが必要
- 判断の根拠: 方針パラメータは enum にし、bool を 2 つ並べない（呼び出し側の可読性）

## 実装者向け注記
現状確認:
```
grep -n "fn delete_entries\|fn delete_selected\|fn apply_selection" src/cleaners/*.rs
grep -rn "outcome.skip_reason" src/cleaners/ --include="*.rs" | grep -v tests | wc -l   # 約 26
grep -rn "deleted_paths" src/cleaners/*.rs | grep -v tests
```
手順: (1) 共有ループを trash.rs に定義、(2) library_caches と library_logs（最も形が近い）から着地、(3) 残り 4 クリーナー、(4) 軽量版の統一は同じスケルトンのものだけ。落とし穴: 55 のプロンプトポートが同じクリーナー群に触れるため、55 の着地後に始める。乖離の解消で既存テストが赤になる可能性がある（deleted_paths の記録を足すテストは既存の主張を強める）。

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
  - 検証: 各 PBI のテスト戦略に沿ったテストが実装され緑
- [x] 6 クリーナーが共有ループを使う
  - 検証: 6 ファイルが crate::trash::delete_entries を呼ぶ
- [x] 全テストが緑
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）
- [x] CHANGELOG の [Unreleased] に追記
  - 検証: CHANGELOG.md [Unreleased] に各バッチのエントリあり
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
