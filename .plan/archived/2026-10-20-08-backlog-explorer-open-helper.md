# PBI: Finder 起動 (open) を CommandRunner 経由の共通ヘルパーにまとめる

## ユーザーストーリー
sasurahime の保守者として、`open <path>` で Finder を開く処理を CommandRunner 経由の 1 か所にまとめたい。なぜなら、explorer と Application Support の残骸表示 (app-support 第2段) が別々に open を実装すると、失敗時の扱いとテスト方法が食い違い、片方だけ直し忘れるから。

## ビジネス価値
- Finder 起動の失敗処理 (終了コード、エラー文言) を 1 か所で直せる
- `CommandRunner` のモックで「open が呼ばれたか、失敗したときどうなるか」を単体テストできる (現状 `open_in_finder` はテスト対象外)
- app-support の第2段が open を二重実装せずに済む (設計書 `docs/superpowers/specs/2026-10-20-05-app-support-design.md` 1.2 節で「別 PBI」とされた共通化の受け皿)

## BDD受け入れシナリオ

```gherkin
Scenario: explorer で選んだパスを共通ヘルパー経由で開く
  Given explorer の未管理一覧にパス "~/Library/Caches/foo" がある
  And   ユーザーが "Open in Finder?" に y と答える
  When  共通ヘルパーが呼ばれる
  Then  runner に program "open"、args ["~/Library/Caches/foo" の絶対パス] が 1 回渡される

Scenario: open が非ゼロで終了した場合
  Given runner が open に対して終了コード 1 を返す
  When  共通ヘルパーが呼ばれる
  Then  Err が返り、メッセージに終了コードが含まれる
  And   explorer は "Error opening Finder: ..." を stderr に出して次のエントリへ進む

Scenario: open が実行できない場合
  Given runner の run が Err を返す (open が PATH に無い、タイムアウトなど)
  When  共通ヘルパーが呼ばれる
  Then  Err が返り、呼び出し側はクラッシュしない
```

## 受け入れ基準
- [ ] `open` を呼ぶ処理が `std::process::Command` を直接使わず、`CommandRunner::run` 経由の 1 関数だけになっている
- [ ] 共通ヘルパーは終了コードが非ゼロなら `Err` を返す (現行の `open_in_finder` と同じ契約)
- [ ] explorer の対話フロー (エントリごとの `Confirm`、既定 No) は変わらない
- [ ] ヘルパーの単体テストが、成功、非ゼロ終了、run の Err の 3 通りを MockRunner で検証する
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 追加しない。explorer は TTY 必須 (`run_explore` の冒頭で `is_terminal()` を見て bail する) で、Finder も実際には開けない。既存の `tests/` に explorer の E2E が無いことを確認してから判断する

### 統合テスト
- 不要。ヘルパー単体で完結する

### 単体テスト
- 共通ヘルパーに MockRunner を渡し、呼ばれた program と args を記録して検証する
- 終了コード 0 / 1 / run の Err の 3 ケース
- パスにスペースや日本語を含んでも、1 つの引数として渡る (シェルを経由しない)

## 実装アプローチ
- **Outside-In**: 呼び出し側 (explorer) の挙動が変わらないことを先に固定し、ヘルパーの単体テストから Red にする
- **Red-Green-Refactor**: `open_in_finder` の本体をヘルパーに置き換え、explorer は薄い呼び出しだけにする
- **リファクタリング**: app-support 第2段が同じヘルパーを呼べる公開範囲にする

## 見積もり
1 ストーリーポイント。理由: 対象は 1 関数と 1 呼び出し箇所で、外部仕様は変わらない。テストを足す作業が主。

## 技術的考慮事項
- 依存関係: app-support 第2段 (設計書 05 の Task 8 以降) より前に済ませると二重実装を避けられる。必須ではなく、第2段を先に実装した場合は第2段側の `reveal_paths` をこのヘルパー呼び出しに置き換える
- テスタビリティ: `src/command.rs` の `CommandRunner` (`run` と `exists`) をそのまま使う
- 非機能要件: macOS のみ

## 実装者向け注記

### 現状コードの確認
```bash
cargo test
grep -rn "open_in_finder" src/
grep -rn "Command::new(\"open\")" src/
grep -n "Confirm::new" -B3 -A8 src/explorer.rs
grep -n "pub trait CommandRunner" -A5 src/command.rs
```

事実確認の結果 (ブランチ docs/system-data-specs の HEAD 時点):
- `src/explorer.rs:233` `fn open_in_finder(path: &Path) -> Result<()>` は private。`std::process::Command::new("open").arg(path).status()?` を呼ぶ。
- 想定と違う点: 終了コードは見ている。`anyhow::ensure!(status.success(), "open returned non-zero exit: {:?}", status.code())` で非ゼロを `Err` にしている。見ていないのは「runner を通さない」点と「テストできない」点である。doc コメントにも "not unit-tested" とある。
- 呼び出しは `src/explorer.rs:347` の 1 か所だけ。`dialoguer::Confirm` (`explorer.rs:342`, プロンプト "Open in Finder?", `.default(false)`) で、選んだ未管理エントリごとに 1 回聞き、y のときだけ `open_in_finder` を呼ぶ。失敗は `eprintln!("Error opening Finder: {e}")` で表示して続行する。
- `src/command.rs:13` の `CommandRunner::run(&self, program, args) -> Result<Output>` は `SystemCommandRunner` が stdout を pipe して 30 秒のタイムアウトで kill する (`run_with_timeout`)。`Output.status.success()` で終了コードを判定できる。

### Confirm との違いの整理
| 項目 | explorer (現状) | app-support 第2段 (設計書 05) |
|---|---|---|
| 確認の単位 | エントリごとに 1 回 | 選択が 6 件以上のときだけ、開く前に 1 回 (`open_needs_confirmation`) |
| 確認の既定 | No | 設計書 05 の該当節を読んで合わせる |
| open の呼び方 | `Command` を直接 | runner 経由 (`reveal_paths`, private) |

確認 (Confirm) の仕様は呼び出し側の責務として残し、共通ヘルパーは「1 パスを open して、非ゼロなら Err」だけを担当させる。確認の挙動を共通化しない。

### 再現手順
1. `sasurahime explore` を TTY で実行し、未管理一覧から 1 件選ぶ
2. "Open in Finder?" に y と答えると Finder が開く (実機のみ。自動テスト不可)

### 想定する修正方針
- 推奨案 A: 新規モジュールは作らず、`src/command.rs` に `pub fn open_path(runner: &dyn CommandRunner, path: &Path) -> Result<()>` を置く。理由: runner の隣にあり、explorer と app_support の両方から循環なく参照できる。
- 案 B: `src/explorer.rs` に pub のまま残して app_support が explorer に依存する。却下理由: cleaner が UI 層 (explorer) に依存する向きが逆。
- 案 C: 何もしない。却下理由: 二重実装とテスト不能が残る。

### 落とし穴
- `run_with_timeout` は stdout / stderr を pipe する。`open` は通常出力しないので問題ないが、`status()` が端末に継承していた出力は表示されなくなる
- 30 秒のタイムアウトは、Finder 起動を待つ `open` には十分だが、`-W` などの待機オプションは付けない
- パスは `&str` に変換して渡す必要がある。非 UTF-8 パスは `to_string_lossy` で壊れるため、変換できないときは Err にする

### 検証コマンド
```bash
cargo test open_path
cargo clippy -- -D warnings
cargo fmt --check
grep -rn "Command::new(\"open\")" src/   # 0 件になること
```

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] `grep -rn 'Command::new("open")' src/` が 0 件
