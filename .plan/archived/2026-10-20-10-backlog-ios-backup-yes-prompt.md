# PBI: --yes で ios-backup / apfs-snapshot の確認プロンプトが出る問題を解消する

## ユーザーストーリー
sasurahime のユーザーとして、`sasurahime --yes` では破壊的で不可逆な cleaner (ios-backup、apfs-snapshot) が一切動かないでほしい。なぜなら、ドキュメントは「--yes では実行されない」と約束しているのに、TTY 上では選択プロンプトが開き、Enter を押すだけで全件削除が実行されるから。

## ビジネス価値
- ドキュメント (`docs/SUPPORTED.md`) と実挙動の食い違いをなくし、「--yes は安全」という約束を守る
- iOS バックアップとローカル Time Machine スナップショットという、消すと戻せないデータの誤削除を防ぐ

## BDD受け入れシナリオ

```gherkin
Scenario: TTY 上の --yes で ios-backup が動かない
  Given iOS バックアップが 1 件以上ある
  And   標準入力が TTY である
  When  ユーザーが `sasurahime --yes` を実行する
  Then  ios-backup の選択プロンプトは表示されない
  And   バックアップは削除されない

Scenario: TTY 上の --yes で apfs-snapshot が動かない
  Given ローカルスナップショットが 1 件以上ある
  And   標準入力が TTY である
  When  ユーザーが `sasurahime --yes` を実行する
  Then  apfs-snapshot の選択プロンプトは表示されない
  And   tmutil deletelocalsnapshots は呼ばれない

Scenario: 明示的な clean は従来どおり対話できる
  Given iOS バックアップが 1 件以上ある
  When  ユーザーが TTY で `sasurahime clean ios-backup` を実行する
  Then  選択プロンプトが表示される

Scenario: --yes 以外の入口でも一覧には出る
  Given iOS バックアップがある
  When  ユーザーが `sasurahime scan` を実行する
  Then  ios-backup は削除候補として表に出る
```

## 受け入れ基準
- [ ] `Cleaner::auto_clean_allowed()` が ios-backup と apfs-snapshot で false を返す
- [ ] `run_auto` は `auto_clean_allowed() == false` の cleaner の `clean` を呼ばない
- [ ] `clean ios-backup` / `clean apfs-snapshot` と TUI (`run_interactive`) の挙動は変わらない
- [ ] `docs/SUPPORTED.md` の記述と実挙動が一致する (TTY の有無によらず `--yes` では動かない)
- [ ] `--yes` が ios-backup / apfs-snapshot をスキップしたことが出力でわかる (1 行)

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 一時 HOME に擬似 `MobileSync/Backup/<id>/` を作り、`sasurahime --yes` を実行して、ディレクトリが残ることを検証する。TTY を使わないので、TTY 上の再現は単体テストで補う

### 統合テスト
- fake の `Cleaner` (`auto_clean_allowed` が false) を `run_auto` に渡し、`clean` が呼ばれないことを検証する (呼び出し回数を数える stub)

### 単体テスト
- `IosCleaner::auto_clean_allowed()` と `ApfsSnapshotCleaner::auto_clean_allowed()` が false
- `Cleaner` の既定実装が true

## 実装アプローチ
- **Outside-In**: E2E (ディレクトリが残る) と run_auto の stub テストを Red にする
- **Red-Green-Refactor**: trait に既定 true のメソッドを足し、run_auto のフィルタ、2 cleaner の override の順に Green にする
- **リファクタリング**: ios_backup 内の「非 TTY なら skip」のガードはそのまま残す (多層防御)

## 見積もり
1 ストーリーポイント。理由: 設計書 04 の S2 が `auto_clean_allowed` を導入済みであれば、override 2 件と run_auto の確認だけ。導入前に着手する場合は 2 SP 相当に増える (本 PBI は S2 後を前提とする)。

## 技術的考慮事項
- 依存関係: 設計書 `docs/superpowers/specs/2026-10-20-04-docker-and-scan-coverage-design.md` の Task S2 (`Cleaner::auto_clean_allowed()`、既定 true、`run_auto` が参照) が前提。設計書 04 の冒頭は「ios-backup は --yes で TTY 上だと確認プロンプトを出します。今回は触りません」と明記しており、本 PBI がその受け皿
- 04 の S3 で `ManualOnly` ラッパーが入るが、ios-backup と apfs-snapshot は特殊 7 件 (マクロ化しない) に含まれる。ラッパーで包むか、cleaner 自身が false を返すかは S3 の実装に合わせる。推奨は cleaner 自身が false を返す (設計書 04 の 2.5 節の 02/03/05 と同じ方針)
- 非機能要件: なし

## 実装者向け注記

### 現状コードの確認
```bash
cargo test
grep -n "is_terminal" src/cleaners/ios_backup.rs src/cleaners/apfs_snapshot.rs
grep -n "pub fn run_auto" -A60 src/interactive.rs
grep -n "auto_clean_allowed" -r src docs
grep -n "never runs in .--yes" docs/SUPPORTED.md
```

事実確認の結果 (HEAD 時点):
- `src/interactive.rs:74` `run_auto` は `is_available()` が true の cleaner を `detect()` し、`ScanStatus::Pruneable(_)` のもの全部に `cleaners[i].clean(false, &reporter)` を呼ぶ (140 行付近)。cleaner の種別での除外は無い。直前に `set_skip_confirm(true)` で `GenericCleaner` 系の確認だけ抑制する。
- `src/cleaners/ios_backup.rs:112-124` の `clean` は `!std::io::stdin().is_terminal()` のときだけ "not a terminal - skipping" で 0 を返す。つまり TTY 上では `interactive_select` (`ios_backup.rs:173`, `dialoguer::MultiSelect`, `defaults(&vec![true; n])`) が開く。全件が最初からチェック済みなので、Enter だけで全件削除に進む。
- `docs/SUPPORTED.md` は 64 行の表で「対話的のみ、`--yes` では非実行」、1138 行で "Interactive only: never runs in `--yes` mode. Non-TTY invocations are safely skipped" と書く。非 TTY の `--yes` では確かに実行されないが、TTY 上では食い違う。
- `auto_clean_allowed` はコードにも未導入 (grep は設計書にしかヒットしない)。
- apfs-snapshot にも同じ問題がある: `src/cleaners/apfs_snapshot.rs:147` は `!self.all && !is_terminal()` のときだけ skip する。`run_auto` は `with_all(false)` の cleaner を使うので、TTY 上の `--yes` では `interactive_select` (`apfs_snapshot.rs:258`, 全件チェック済み) が開く。`docs/SUPPORTED.md` 1181 行は "never runs in `--yes` mode" と書いており、同じ食い違い。
- 補足: apfs-snapshot の `detect` はスナップショットがあれば `Pruneable(0)` を返す (`/.MobileBackups` が無いと 0)。0 でも Pruneable なので `run_auto` の対象に入る。

### 再現手順
1. iOS バックアップがある機で TTY から `sasurahime --yes` を実行する
2. "Scan complete..." 後に ios-backup の MultiSelect が表示される (実機のみ)。自動テストでは再現できないので stub テストで代替する

### 想定する修正方針
- 推奨案 A: `auto_clean_allowed()` を 2 cleaner が false で override し、`run_auto` がそれを見る。理由: 設計書 04 の方針と一致し、`clean` 内の TTY 判定に依存しない
- 案 B: `run_auto` が `set_non_interactive(true)` のグローバルを立て、各 cleaner が見る。却下: `set_skip_confirm` と同種のグローバル状態が増える
- 案 C: ドキュメントを「TTY 上では動く」に直す。却下: 不可逆な操作に対して安全側の約束を弱める

### 落とし穴
- `run_auto` のスキップは `detect()` 後・`clean()` 前に行う。表示される "Found N item(s), ~X will be permanently deleted" の合計にスキップ分を含めない
- `defaults(true)` のまま変えない (`clean ios-backup` の挙動を変える別議論)
- `ios_backup.rs` の非 TTY ガードは多層防御として残す

### 検証コマンド
```bash
cargo test auto_clean
cargo test --test interactive
cargo clippy -- -D warnings
cargo fmt --check
```

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] `docs/SUPPORTED.md` と実挙動が一致していることを確認済み
