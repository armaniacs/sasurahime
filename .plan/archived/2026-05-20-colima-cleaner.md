# PBI: Colima VM キャッシュクリーナー

## ユーザーストーリー
Colima でコンテナを動かす macOS 開発者として、Colima の VM ディスクキャッシュを掃除したい、なぜなら `~/.colima/_lima/` が 9GB 以上に膨れ上がることがあり、手動で掃除するのが面倒だから。

## ビジネス価値
- 対象マシンで 9.3GB の回収が確認されている（実測、araki 環境）
- Colima は Docker Desktop の代替として広く使われており、macOS 開発者のうち Homebrew + Colima 構成のユーザーが恩恵を受ける
- `colima prune --all` の安全なラッパーを提供することで、CLI オプションを知らないユーザーでも確実に実行できる
- 最悪ケースのモデル: macOS Docker 開発者の 30% が Colima 利用（Homebrew 経由）、平均 5〜10GB の VM ディスクキャッシュを持つ

## BDD受け入れシナリオ

```gherkin
Scenario: Colima キャッシュサイズを scan で表示する
  Given ~/.colima/ に VM ディスクキャッシュが存在する
  When  sasurahime scan を実行する
  Then  colima の項目にキャッシュサイズが表示される

Scenario: colima prune --all を実行する
  Given colima コマンドが PATH に存在する
  And   ~/.colima/ にキャッシュが存在する
  When  sasurahime clean colima を実行する
  Then  colima prune --all が実行される
  And   解放サイズが報告される

Scenario: dry-run では削除されない
  Given colima コマンドが PATH に存在する
  And   ~/.colima/ にキャッシュが存在する
  When  sasurahime clean colima --dry-run を実行する
  Then  colima prune --all は実行されない
  And   削除予定のキャッシュサイズが表示される

Scenario: Colima がインストールされていない場合はスキップする
  Given colima コマンドが PATH に存在しない
  When  sasurahime clean colima を実行する
  Then  "colima: not found" と表示されて正常終了する

Scenario: ~/.colima/ が存在しない場合は NotFound を返す
  Given ~/.colima/ が存在しない
  When  sasurahime scan を実行する
  Then  colima の項目は NotFound と表示される
```

## 受け入れ基準
- [ ] `sasurahime scan` に colima が表示される
- [ ] `sasurahime clean colima` で `colima prune --all` が実行される
- [ ] `--dry-run` で削除されないことを確認できる
- [ ] colima 未インストール時はスキップ（エラーにならない）
- [ ] `~/.colima/` 不在時は scan で NotFound 表示

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- **偽の `colima` スクリプト**を bin に配置し、`colima prune --all` が呼ばれることを確認（`docker` / `brew` / `bun` と同じ既存パターン）
- `~/.colima/` にダミーディレクトリを作成し、scan がサイズを認識することを確認
- `colima` が PATH にない場合にスキップメッセージが出て終了コード 0 になることを確認

### 統合テスト
- `GenericCleaner` の `CleanMethod::Command` 経由で `colima prune --all` が正しく実行されることの確認
- `dir_size(~/.colima/)` がディレクトリサイズを正しく返すことの確認

### 単体テスト
- `colima` コマンド存在確認のテスト（`CommandRunner::exists` モック）
- `~/.colima/` パス解決のテスト（`home.join(".colima")`）

## 実装アプローチ
- **Outside-In**: E2Eテスト（偽の colima スクリプト）から開始
- **Red-Green-Refactor**: 失敗→実装→グリーン→リファクタリング
- **GenericCleaner パターン**: `docker` / `brew` / `bun` と同一の外部CLI委譲パターンを踏襲
  - detect: `dir_size(~/.colima/)` でキャッシュサイズを報告
  - clean: `colima prune --all` を実行（`--dry-run` 対応）
  - フォールバック: colima 不在時はスキップ

### 追加ファイル
`src/cleaners/generic.rs` に以下のファクトリメソッドを追加:

```rust
pub fn colima_prune(runner: Box<dyn CommandRunner>) -> Self {
    let detect_dir = dirs::home_dir()
        .map(|h| h.join(".colima"))
        .unwrap_or_else(|| PathBuf::from("~/.colima"));
    Self {
        display_name: "colima",
        method: CleanMethod::Command {
            program: "colima",
            args: &["prune", "--all"],
        },
        runner,
    }
}
```

ただし detect で `~/.colima/` のサイズを報告するには `DeleteDirs` と `Command` のハイブリッドが必要。代わりに、`GenericCleaner` に `detect_dir` フィールドを追加して、detect 時はそのディレクトリのサイズを報告し、clean 時は CLI コマンドを実行する、という拡張を行う。

または、`GenericCleaner` に新しい `CleanMethod::CommandWithDetectDir` variant を追加する:

```rust
pub enum CleanMethod {
    Command { program: &'static str, args: &'static [&'static str] },
    CommandWithDetectDir { program: &'static str, args: &'static [&'static str], detect_dir: PathBuf },
    DeleteDirs(Vec<PathBuf>),
}
```

`detect` が `CommandWithDetectDir` の場合は `detect_dir` の `dir_size` を報告する。`clean` は通常の `Command` と同じ動作。

## 見積もり
1 ストーリーポイント

## 技術的考慮事項
- 依存関係: なし（既存の `GenericCleaner` + `CommandRunner` のみ）
- テスタビリティ: 偽の `colima` スクリプトで E2E テスト可能（既存パターン流用）
- 非機能要件: `colima prune --all` には Colima が起動していなくても使える（キャッシュアセットのみ削除）
- 注意点: `colima prune --all` は未使用の VM イメージテンプレートを削除するが、実行中の Colima インスタンスには影響しない

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `sasurahime targets` に `colima` が表示される
- [ ] `sasurahime scan` で `~/.colima/` のサイズが表示される
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` 全パス
- [ ] `cargo fmt --check` 全パス
