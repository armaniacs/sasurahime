# PBI: Docker と Colima の仮想ディスクの実サイズを scan に表示し、縮小方法を案内する

## ユーザーストーリー
Docker Desktop または Colima を使う開発者として、`sasurahime scan` で仮想ディスクが実際にディスク上で何 GB を占めているかを知り、`docker system prune` で縮まないときの対処を案内してほしい。なぜなら、prune しても `Docker.raw` のようなファイルは縮まないことが多く、「システムデータ」が減らない原因が分からないから。

## ビジネス価値
- 数十 GB に膨らむ仮想ディスクという最大級の原因を、scan の 1 画面で見える化する
- 「縮まないのに解放できると表示する」嘘を避ける (Size 列は変えず、案内は表の下の Notes に出す)
- docker デーモンに問い合わせないため、デーモン停止中でも scan がハングしない
- 測定方法: 疎ファイルの物理サイズと最大サイズが Notes に出ること、scan が 25 秒以内に終わることを E2E で確認

## BDD受け入れシナリオ

```gherkin
Scenario: Docker Desktop の仮想ディスクが scan の下に案内される
  Given HOME に Docker.raw (物理 2 MiB、論理 64 MiB 相当の疎ファイル) があり、docker コマンドが PATH にある
  When  sasurahime scan を実行する
  Then  表の下に Notes が出て、docker の仮想ディスクの "on disk" と "maximum size" と対処案が表示される
  And   表の docker 行の Size 列は 0 B / pruneable のまま変わらない

Scenario: docker が無ければ案内も出ない
  Given PATH に docker が無い
  When  sasurahime scan を実行する
  Then  docker の行も仮想ディスクの案内も出ない

Scenario: 仮想ディスクが 1 MiB 未満なら案内しない
  Given Docker.raw の物理使用量が 1 MiB 未満である
  When  sasurahime scan を実行する
  Then  案内は出ない

Scenario: docker デーモンが応答しなくても scan がハングしない
  Given PATH の docker が 60 秒 sleep するスクリプトである
  When  sasurahime scan を実行する
  Then  25 秒以内に終了する

Scenario: Colima の複数ディスクが列挙される
  Given ~/.colima/_lima/colima/disk (論理 20G) と ~/.colima/_lima/_disks/colima/datadisk (論理 100G) がある
  When  sasurahime scan を実行する
  Then  両方のパスの最大サイズが Notes に出る

Scenario: clean docker の後に縮まなかったことを案内する
  Given clean docker の前後で Docker.raw の物理サイズが変わらない
  When  clean docker が成功する
  Then  "縮まなかった" 旨と Docker Desktop の Disk image size の変更手順が案内される
  And   docker の終了コードが非 0 のときは縮小判定を出さず警告だけ出す

Scenario: scan に標準ターゲット全件が出る
  Given 標準 39 件の fake ツールが PATH にある
  When  sasurahime scan を実行する
  Then  STANDARD_SCAN_NAMES の 39 件がすべて出る
  And   cargo の進捗行 (println) が scan の出力に混ざらない
```

## 受け入れ基準
- [ ] Docker Desktop の 4 候補パスと Colima の 3 種のパス (`_lima/*/disk`、`_lima/*/diffdisk`、`_lima/_disks/*/datadisk`) を、存在するものだけ列挙する
- [ ] 物理使用量 (`st_blocks * 512`) を主に、論理サイズを "maximum size" として併記する
- [ ] 物理 1 MiB 未満は案内しない。OrbStack には案内を付けない
- [ ] detect は docker を実行しない (`Pruneable(0)` のまま)。案内は scan の表の下だけ。TUI には出さない
- [ ] `clean docker` は終了コードが 0 のときだけ前後の物理サイズを比べて案内する
- [ ] `CargoCleaner::detect` の `println!` を `log::info!` に置き換え、scan の出力が混ざらない
- [ ] docs/SUPPORTED.md の件数が 48、README の Data access に Docker と Colima の 1 行 (英日)、`--yes` の説明が実態 (15 件のみ) に更新されている
- [ ] `sasurahime targets | wc -l` が 48

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `tests/scan_coverage.rs` (新規): `scan_shows_docker_virtual_disk_advice`、`scan_omits_docker_advice_when_docker_is_not_installed`、`scan_does_not_hang_when_docker_daemon_is_unresponsive` (D3)、`scan_lists_every_standard_target`、`scan_does_not_print_cargo_progress_lines` (T4)
- HOME は tempdir、PATH は fake ツールのディレクトリだけにする (実機の docker や npm を見せない)

### 統合テスト
- `src/cleaners/generic.rs` の `docker_advisory_reports_virtual_disk`、`docker_advisory_is_none_without_virtual_disk`、`docker_detect_does_not_run_commands_and_stays_pruneable_zero`、`colima_advisory_reports_disk_images`、`non_vm_cleaner_has_no_advisory`

### 単体テスト
- `src/disk_image.rs`: `probe_reports_physical_below_logical_for_sparse_file`、`probe_returns_none_for_missing_or_directory`、`find_ignores_images_below_threshold`、`render_is_none_without_images`、`render_lists_path_sizes_and_remedy`、`after_clean_message_reports_shrink`、`after_clean_message_warns_when_not_shrunk`、`after_clean_message_is_none_when_nothing_was_measured`、`docker_candidates_start_with_current_layout`、`colima_candidates_find_disk_and_datadisk`、`colima_candidates_empty_without_colima_dir`

## 実装アプローチ
- **Outside-In**: D3 の E2E の意図 (scan に案内が出る) から逆算し、D1 の純関数、D2 の統合、D3 の出力の順に積む (設計書の順序)
- **Red-Green-Refactor**: 各タスクで失敗するテスト、失敗確認、実装、成功確認、コミット
- **リファクタリング**: 案内の文面生成を `disk_image` に集め、cleaner 側は呼ぶだけにする

## 見積もり
5 ストーリーポイント (根拠: 設計書のタスク 5 本 (D1, D2, D3, T4, T5)。D1 の純関数と E2E の fake ツール整備が中心で、新規モジュールと 2 つの E2E ファイルを含む)

## 技術的考慮事項
- 依存関係: PBI-02 (S2 の `advisory()` と S3 の生成切替が前提)
- 設計書の依存順序: D1 と D2 は S 系列と独立だが、D2 が main.rs の docker factory を変えるため S3 の後。D3 は D2 の後。T4 は S3 と D3 の後。T5 は T4 の後
- テスタビリティ: 疎ファイルは `set_len` で作る。fake ツールは実行権限付きシェルスクリプト
- 非機能: scan はデーモンに問い合わせない。物理 1 MiB 未満は出さない。Docker Desktop の 4 候補パスは開発機に Docker Desktop が無く実在未確認 (外れても案内が出ないだけ)

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-04-docker-and-scan-coverage-design.md`: 1 章、2.6 節 (仮想ディスクの実情)、4 章 (公開する型)、5 章、6 章 (依存順序)、10 章 (D1)、11 章 (D2)、12 章 (D3)、13 章 (T4)、14 章 (T5)、15 から 20 章
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` (1.4, 1.6, 11.5)

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"        # 全件 ok
grep -n "fn standard_cleaners" src/main.rs                     # 1 行 (PBI-02 完了の確認)
grep -n "fn advisory" src/cleaner.rs                           # 1 行 (PBI-02 完了の確認)
ls src/disk_image.rs tests/scan_coverage.rs                    # No such file なら未実装
grep -rn "disk_image\|Virtual disk note" src docs README.md    # 出力なしなら未実装
```
どれか期待と違えば着手せず報告する。

### 前提となる PBI
PBI-02 (scan 網羅)。

### 実装の進め方
設計書のタスクを D1, D2, D3, T4, T5 の順に進める。1 タスク 1 コミット。T4 の `scan_lists_every_standard_target` が失敗したとき、足りない fake ツール名を推測で足さず報告する。

### 落とし穴
- detect で `docker system df` や `docker info` を呼ばない (デーモン停止時に最大 30 秒ブロックする)
- 論理サイズを「使用量」として表示しない。主表示は物理。案内のバイト数を `Pruneable` に加算しない
- 縮小判定は docker の終了コードが 0 のときだけ。非 0 なら警告のみ
- E2E で HOME だけ差し替えて PATH を差し替えない、または実機の `~/` に `clean` を実行しない
- `FAKE_TOOLS` の fake を HOME 外のディレクトリに置く。ACT_CACHE_DIR など環境変数が設定されていると HOME 外を走査するので `env_remove` が必要な場合がある
- OrbStack に案内を足さない。TUI に案内を出さない
- D3 の完了時と T4、T5 は警告ゼロ。D1 から D2 の間の dead_code だけ許容
- T5 のドキュメントは Shift-JIS で表せる文字だけ。README の「内容を読むファイル」の一覧は変更しない (メタデータしか読まないため)

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo build
./target/debug/sasurahime targets | wc -l                 # 48
./target/debug/sasurahime --no-unicode scan                # 読み取りのみ。Colima があれば表の下に Notes
time ./target/debug/sasurahime --no-unicode scan           # 数秒から十数秒
./target/debug/sasurahime clean docker --dry-run           # dry-run のみ
grep -n "49 clean targets" docs/SUPPORTED.md               # 出力なし
git status --short
```
`sasurahime clean ...` (`--dry-run` なし) と `sasurahime --yes` は実行しない。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 設計書 18 章の完了条件 1 から 8 を満たす
- [ ] D1, D2, D3, T4, T5 が個別にコミットされている
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告
- [ ] コードレビュー完了
- [ ] リファクタリング完了
- [ ] ドキュメント更新済み (SUPPORTED.md、README、HOWTO-USE.md、HOWTO-ADD-target.md)
