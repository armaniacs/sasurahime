# colima クリーナーの実解放修正 - 設計

- 日付: 2026-10-27
- 状態: 承認済み(ユーザー選択: prune + fstrim)
- 対象: src/cleaners/generic.rs、src/disk_image.rs(COLIMA_REMEDY 参照)、tests/

## 背景

- `scan` は colima を pruneable 1.4 GB と報告するが、clean を実行すると `Total freed: 0 B` になる。
- `colima prune --all --force` はダウンロード済みアセットのキャッシュだけを掃除する(colima prune --help → "Prune cached downloaded assets")。VM ディスクには触らない。
- 検出の 1.4 GB は VM ディスクの物理割り当てである。実機の証拠: `~/.colima/_lima/colima/disk` が見かけ 20 GB / 物理 1.4 GB。`dir_size` は `blocks() * 512` で物理を合算する(src/format.rs:23)。
- したがって size_after == size_before となり、解放量 0 が正しく報告される(src/cleaners/generic.rs:629 の diff は正しい)。
- 正しい remedy はクリーナー自身の advisory に既にある(src/disk_image.rs:10): `colima ssh -- sudo fstrim -av`(非破壊、VM 起動時のみ)または `colima delete`(破壊的)。
- 確認メッセージが "This will delete ALL stopped Colima VM disk data (containers, images, volumes)." となっており、`colima delete` の説明で実態と不一致。
- ユーザーの VM は起動中(colima status → running using macOS Virtualization.Framework)。fstrim は今すぐ効く。

## 根因

コマンド(prune)が検出対象(VM ディスクの物理割り当て)を解放できない構造的不一致。

## 設計

### 1. CliCleaner に後続コマンドを追加できるようにする

- `extra_commands: Vec<(String, Vec<String>)>` フィールドを追加する。既存の `program` / `args` はメインコマンドのまま。
- colima ビルダーは、メインに `colima prune --all --force`、後続に `colima ssh -- sudo fstrim -av` を設定する。
- clean はメインコマンド実行後に各後続コマンドを実行する。後続コマンドの失敗は非致命とする: warning と COLIMA_REMEDY を表示して処理を続け、クラッシュしない。
- 解放量は既存の物理ブロック diff がそのまま報告する。fstrim で VM ディスクの物理割り当てが縮むと `freed > 0` になる。

### 2. VM 停止時の扱い

- `colima ssh` は VM 停止時に失敗する。その場合は既存の warning 経路で VM が起動していない旨と remedy(`colima start` して再実行、または `colima delete`)を表示し、prune 分だけを報告して正常終了する。

### 3. 確認メッセージの修正

- 実態を説明する文に修正する。prune + fstrim は非破壊で、コンテナ/イメージ/ボリュームは消えない。

### 4. 変更しないもの

- 検出: `~/.colima` の物理サイズのまま。fstrim で縮むので "pruneable" は正直。
- Risk: `Risk::Risky` のまま。PBI 42 の無人実行保護を維持し、TUI からは選択可能。
- advisory: `DiskImageNotice` は既に正しい案内なのでそのまま。

## テスト

- 統合(mock CommandRunner):
  - 両コマンド(メインと後続)が実行されること。
  - mock の `run()` 内で検出ディレクトリ内の canary ファイルを消し、物理 diff が `bytes_freed > 0` を報告すること。
  - 後続コマンド失敗(VM 停止相当)でも warning 表示と正常終了、かつ prune 分の報告が崩れないこと。
- E2E(hermetic PATH): tempdir bin に fake `colima` スクリプトを置いて clean が成功すること(既存の install_fake_uv パターン)。
- 既存回帰: docker クリーナーなど他の `CliCleaner` 利用箇所が壊れないこと。

## ドキュメントとブランチ

- CHANGELOG [Unreleased] に追記する(ドキュメント文字種規約に従う)。
- ブランチ: 現在の `feat/orphan-trash-ux` は READY(CI 緑、TTY 手元確認待ち)なので、本修正は別ブランチで行う。現ブランチの仕上げ後、main から切り出す。
