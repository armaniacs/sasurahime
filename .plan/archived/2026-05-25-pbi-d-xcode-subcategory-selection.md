# PBI-D: Xcode サブカテゴリ別部分削除 UI

## ユーザーストーリー

macOS 開発者として、Xcode のキャッシュを種類ごとに選択して削除したい。なぜなら、DerivedData は消したいが Archives（過去のビルド成果物）は残したい、というケースがよくあるから。

## ビジネス価値

- Xcode キャッシュは最大で数十 GB に達する最大の回収源のひとつ
- サブカテゴリ選択により「消しすぎ」を防ぎ、再ビルド時間のロスを減らせる
- ユーザーがリスクなくより積極的にクリーンを実行できるようになる

## 実装ステータス

**Phase 1 (CLI コア): ✅ 完了**
- `XcodeSubcategory` enum (DerivedData, Archives)
- `--sub derived-data|archives` CLI フラグ
- `detect()` でサブカテゴリフィルタリング
- `clean()` で指定サブカテゴリのみ削除
- `--dry-run` サブカテゴリ単位対応
- E2E/統合/単体テスト完備

**Phase 2 (TUI 展開): ✅ 完了**
- `XcodeCleaner::sub_targets()` オーバーライド — DerivedData / Archives の存在チェック + サイズ返却
- TUI でのサブカテゴリチェックボックス展開表示 — interactive.rs の汎用ロジックが自動展開
- 存在しないサブカテゴリは size=0 でフィルタアウト（リストに表示されない）
- 単体テスト: sub_targets_returns_only_existing, sub_targets_filters_zero_size_entries
- E2E テスト: sub_targets_integration_via_yes_cleans_default_subcategory

### スコープノート

Simulators は XcodeCleaner のサブカテゴリとしてではなく、**独立した `simulator` クリーンターゲット** として実装済み（`xcrun simctl delete unavailable` を呼び出す）。
そのため `XcodeSubcategory` enum は DerivedData と Archives の 2 値のみを持つ。

## 現在の実装詳細

### データ構造（実装済み）

```rust
/// 実際のコードベースに存在する定義
pub enum XcodeSubcategory {
    DerivedData,
    Archives,
}
```

### CLI（実装済み）

```
sasurahime clean xcode                  # DerivedData のみ削除（後方互換）
sasurahime clean xcode --sub derived-data  # DerivedData のみ削除
sasurahime clean xcode --sub archives      # Archives のみ削除
sasurahime clean xcode --sub derived-data,archives  # 両方削除（カンマ区切り）
sasurahime clean xcode --dry-run           # DerivedData のドライラン
sasurahime clean xcode --sub archives --dry-run  # Archives のドライラン
```

### TUI（未実装）

`Cleaner` trait に `sub_targets()` メソッドが用意されており、interactive.rs の TUI はこのメソッドの戻り値を使ってサブカテゴリをチェックボックス展開する汎用ロジックを持つ。しかし `XcodeCleaner` はまだ `sub_targets()` をオーバーライドしていないため、TUI で Xcode は単一ターゲットとして表示される。

## TUI サブカテゴリ展開の実装 TODO

```rust
// XcodeCleaner に追加が必要:
impl Cleaner for XcodeCleaner {
    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        self.detect_subcategories()
            .into_iter()
            .filter(|info| info.size > 0)
            .map(|info| (info.sub.display_name(), info.size))
            .collect()
    }
}
```

`sub_targets()` が正しく値を返すようになれば、interactive.rs は自動的に:
1. Xcode を展開し DerivedData / Archives を個別チェックボックスとして表示
2. 各サブカテゴリのサイズを表示（`format_bytes`）
3. 選択されたサブカテゴリを `sasurahime clean xcode --sub <name>` で再実行

## BDD 受け入れシナリオ

```gherkin
Scenario: CLI でサブカテゴリを指定して削除できる
  Given DerivedData, Archives が存在する
  When sasurahime clean xcode --sub derived-data を実行する
  Then DerivedData のみが削除される
  And Archives は保持される

Scenario: カンマ区切りで複数サブカテゴリを指定できる
  Given DerivedData, Archives が存在する
  When sasurahime clean xcode --sub derived-data,archives を実行する
  Then 両方のサブカテゴリが削除される

Scenario: サブカテゴリ未指定時は DerivedData のみ削除（後方互換）
  Given DerivedData, Archives が存在する
  When sasurahime clean xcode を実行する（--sub なし）
  Then DerivedData のみが削除される
  And Archives は保持される

Scenario: --dry-run でサブカテゴリ単位のドライラン
  Given DerivedData が存在する
  When sasurahime clean xcode --sub derived-data --dry-run を実行する
  Then DerivedData は削除されない
  And 削除予定のエントリが表示される

Scenario: サブカテゴリが存在しない場合はスキップされる
  Given Archives ディレクトリが存在しない
  When sasurahime clean xcode --sub archives を実行する
  Then "not found, skipping" が表示される
  And エラー終了しない

Scenario: TUI でサブカテゴリを個別選択できる（未実装）
  Given Xcode の DerivedData, Archives が全て存在する
  When インタラクティブ TUI で Xcode を選択する
  Then DerivedData / Archives が個別チェックボックスで展開される
  And 各項目のサイズが表示される
  And ユーザーは任意の組み合わせを選択できる
```

## 受け入れ基準

- [x] `sasurahime clean xcode --sub <derived-data|archives>` でコマンドライン指定可能
- [x] カンマ区切りで複数指定可能（`--sub derived-data,archives`）
- [x] `--sub` 未指定時は DerivedData のみ削除（後方互換性）
- [x] `--dry-run` でサブカテゴリ単位のドライラン動作
- [x] 存在しないサブカテゴリはスキップ（エラーにならない）
- [x] 無効なサブカテゴリ名は無視される
- [x] Xcode 実行中の警告表示
- [x] TUI で Xcode 選択時にサブカテゴリが展開表示される
- [x] DerivedData / Archives の各サイズを個別に表示する（TUI）
- [x] 任意の組み合わせを選択して削除できる（TUI）
- [ ] 存在しないサブカテゴリは無効化表示（選択不可）

## t_wada スタイル テスト戦略

```
E2Eテスト（実装済み）:
- tempdir に DerivedData/Archives を作成し
  sasurahime clean xcode --sub derived-data を実行 → Archives が残っていることを検証
- sasurahime clean xcode --sub archives を実行 → DerivedData が残っていることを検証
- sasurahime clean xcode --sub なし → DerivedData のみ削除される
- sasurahime clean xcode --dry-run で何も削除されないことを検証
- DerivedData 不在でも exit 0 になることを検証

統合テスト（実装済み）:
- XcodeCleaner::detect_subcategories() が各パスのサイズを正しく返すことをテスト
- XcodeCleaner::clean(sub: &[XcodeSubcategory]) が指定カテゴリのみ削除することをテスト
- XcodeCleaner::is_xcode_running() が pgrep 結果を正しく反映することをテスト

単体テスト（実装済み）:
- XcodeSubcategory::all() が 2 値を返すこと
- XcodeSubcategory::path(&self, home: &Path) -> PathBuf の純関数テスト
- XcodeSubcategory::from_str() のパーステスト（derived-data, deriveddata, archives, 無効値）
- XcodeSubcategory::display_name() の可読名テスト
- サブカテゴリが存在しない場合の検出テスト

TUI テスト（実装済み）:
- sub_targets_integration_via_yes_cleans_default_subcategory: --yes で全サブカテゴリが正しく処理されることを E2E 検証
- sub_targets_returns_only_existing: 存在するサブカテゴリのみ返すことを検証
- sub_targets_filters_zero_size_entries: 空のパスはフィルタアウトされることを検証
```

## 実装アプローチ

Phase 1 (完了):
1. Red: `--sub derived-data` の E2E テストを作成
2. Green: `XcodeSubcategory` enum 追加、`XcodeCleaner::clean` にサブカテゴリフィルタリング実装、CLI パース追加
3. Refactor: detect() と clean() のサブカテゴリ分岐を整理

Phase 2 (完了):
1. `XcodeCleaner::sub_targets()` をオーバーライド — `detect_subcategories()` をラップ
2. 単体テスト追加（existing + zero-size filtering）
3. E2E テスト追加（--yes 経由のサブカテゴリ結合動作検証）
4. `detect_subcategories()` の `#[allow(dead_code)]` 除去

## 技術的考慮事項

- **依存関係**: `dialoguer` の `MultiSelect` — この PBI の TUI 部分では階層チェックボックスが必要。現在の実装ではフラットリストにサブカテゴリを `  xcode > DerivedData` 形式で表示
- **Archives のパス**: `~/Library/Developer/Xcode/Archives/`
- **DerivedData のパス**: `~/Library/Developer/Xcode/DerivedData/`
- **パス所有権**: `XcodeCleaner` は `derived_data` と `archives` の 2 フィールドを `PathBuf` として保持（`XcodeSubcategory::path()` はテスト用ヘルパー兼用）
- **sub_targets() の設計**: `Cleaner` trait のデフォルト実装は空の Vec を返す。オーバーライドにより任意のクリーナーがサブカテゴリを提供可能（Xcode 以外の将来拡張にも対応）
- **TUI 削除の仕組み**: interactive.rs は選択されたサブカテゴリに対して `sasurahime clean <name> --sub <sub_name>` を子プロセスとして再実行する
- **後方互換性**: `--sub` なし = DerivedData のみ削除。既存の `sasurahime clean xcode` の動作を変更しない

## 変更履歴（PBI 更新）

| 日付 | 変更内容 |
|------|---------|
| 2026-05-26 | Phase 2 (TUI) 完了: `sub_targets()` オーバーライド + テスト追加。全 PBI-D 完了 |
| 2026-05-25 | Phase 1 (CLI コア) 完了。Simulators をスコープ外に（独立 cleaner で対応）。Phase 2 (TUI) は未着手のまま PBI を更新 |

## 見積もり

**5 SP**（Phase 1: 3 SP, Phase 2: 2 SP）

## Definition of Done

Phase 1:
- [x] `sasurahime clean xcode --sub derived-data` で DerivedData のみ削除される
- [x] `sasurahime clean xcode --sub archives` で Archives のみ削除される
- [x] カンマ区切り複数指定が動作する
- [x] `--sub` 未指定時の後方互換性が保たれる
- [x] `--dry-run` で削除されない
- [x] `cargo test` 全パス
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン

Phase 2 (TUI):
- [x] TUI で Xcode 選択時に DerivedData / Archives が展開表示される
- [x] 各サブカテゴリのサイズが表示される
- [x] 任意の組み合わせが選択可能
- [x] `sub_targets()` フィルタ（存在しないサブカテゴリは非表示）
- [x] コードレビュー完了
