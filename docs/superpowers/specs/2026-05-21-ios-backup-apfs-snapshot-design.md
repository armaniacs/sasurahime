# PBI-012 / PBI-013: iOS Backup & APFS Snapshot Cleaners

Date: 2026-05-21

## Background

macOS の「システムデータ」に計上される領域のうち、既存クリーナーでカバーされていない2カテゴリを追加する。

- **PBI-012**: iOS デバイスバックアップ (`~/Library/Application Support/MobileSync/Backup/`)
- **PBI-013**: APFS ローカルスナップショット (`tmutil` 経由)

Docker (`docker system prune -f`)、Simulator (`xcrun simctl delete unavailable`)、Library Logs はすでに実装済みのため対象外。

---

## PBI-012: `IosCleaner`

### ファイル

`src/cleaners/ios_backup.rs`

### 構造体

```rust
pub struct IosCleaner {
    backup_dir: PathBuf,  // ~/Library/Application Support/MobileSync/Backup/
    runner: Box<dyn CommandRunner>,
}
```

### `detect()`

1. `backup_dir` が存在しなければ `ScanStatus::NotFound`
2. 存在する場合: 直下の各子ディレクトリ（バックアップエントリ）のサイズを合計
3. 合計 > 0 なら `ScanStatus::Pruneable(total_bytes)`、0 なら `ScanStatus::Clean`

### `clean(dry_run)`

1. `backup_dir` が存在しなければ `bytes_freed: 0` で即リターン
2. **常に警告を表示**: `⚠  iOS backups cannot be restored once deleted. Proceed with caution.`
3. `dry_run = true`:
   - 各バックアップを `  would remove: <uuid>  (<size>)` 形式で列挙
   - `bytes_freed: 0` を返す
4. `dry_run = false`:
   - `MultiSelect` でバックアップ一覧を表示（UUID + サイズ）、デフォルト全選択
   - 選択された各パスに対して:
     - `chflags -R nouchg <path>`
     - `trash::delete_path(&path)`
   - `ProgressReporter` でプログレス報告
   - `bytes_freed` を返す

### `name()`

`"ios-backup"`

### 登録先

`main.rs` の `register_cleaners!` マクロに追加:

```rust
IosCleaner : "ios-backup" => "iOS device backups (irreversible)";
(|home, _config| cleaners::ios_backup::IosCleaner::new(home, Box::new(SystemCommandRunner))),
```

### テスト

- `detect()`: backup_dir なし → `NotFound`
- `detect()`: バックアップあり → `Pruneable(合計サイズ)`
- `clean(dry_run=true)`: 削除なし、バイト数 0 を返す
- `clean(dry_run=true)`: 警告メッセージが出力される

---

## PBI-013: `ApfsSnapshotCleaner`

### ファイル

`src/cleaners/apfs_snapshot.rs`

### 構造体

```rust
pub struct ApfsSnapshotCleaner {
    runner: Box<dyn CommandRunner>,
}
```

### `detect()`

1. `tmutil listlocalsnapshots /` を実行
2. コマンドが PATH にない (`CommandError::NotFound` 相当) → `ScanStatus::NotFound`
3. 出力が空（スナップショットなし）→ `ScanStatus::Clean`
4. スナップショット名の行が1行以上ある場合:
   - `/.MobileBackups` が存在すれば `du -sk /.MobileBackups` で計測
   - 存在しない（macOS 13+ では非表示の場合がある）場合は `Pruneable(0)` でスナップショット数だけ報告
   - 計測できた場合は `Pruneable(bytes)`

`tmutil listlocalsnapshots /` の出力形式:

```
com.apple.TimeMachine.2026-05-10-120000.local
com.apple.TimeMachine.2026-05-11-120000.local
```

### `clean(dry_run)`

1. `tmutil listlocalsnapshots /` でスナップショット一覧を取得
2. **常に警告を表示**: `⚠  Deleting snapshots disables local Time Machine protection until the next backup.`
3. `dry_run = true`:
   - スナップショット名一覧を `  would delete: <name>` 形式で表示
   - `bytes_freed: 0` を返す
4. `dry_run = false`:
   - `MultiSelect` でスナップショットを選択（スナップショット名を表示）
   - 選択された各スナップショットを `tmutil deletelocalsnapshot / <name>` で削除
   - `bytes_freed`: 削除前に `du` で計測した合計バイト数（計測不可なら 0）

### `name()`

`"apfs-snapshot"`

### 登録先

`main.rs` の `register_cleaners!` マクロに追加:

```rust
ApfsSnapshotCleaner : "apfs-snapshot" => "APFS local Time Machine snapshots";
(|_home, _config| cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(SystemCommandRunner))),
```

### テスト

- `detect()`: `tmutil` が PATH にない → `NotFound`（モックで `NotFound` エラーを返す）
- `detect()`: 出力なし → `Clean`
- `detect()`: スナップショット名あり → `Pruneable(_)`
- `clean(dry_run=true)`: 削除なし、バイト数 0
- `clean(dry_run=true)`: 警告メッセージが出力される
- `parse_snapshot_names()`: tmutil 出力のパース正確性

---

## 共通事項

### 安全ルール（既存との一貫性）

- `detect()` は副作用なし
- `clean(dry_run=true)` は削除なし
- 削除前に `chflags -R nouchg` を実行（`IosCleaner` のみ; スナップショットは不要）
- 外部コマンドは `CommandRunner` trait 経由でモック可能にする

### Outside-In TDD 順序

1. E2E: `tempdir` をホームとして `sasurahime scan` → `ios-backup` / `apfs-snapshot` が出力に現れる
2. Integration: `IosCleaner::detect()` / `ApfsSnapshotCleaner::detect()` を fake path / mock runner で呼ぶ
3. Unit: `parse_snapshot_names()` などの純粋関数

### 実装順序

PBI-012 (`IosCleaner`) → PBI-013 (`ApfsSnapshotCleaner`) の順。
IosCleaner は既存の `LibraryLogsCleaner` パターンに最も近く、
ApfsSnapshotCleaner は `tmutil` のモックが必要なため後続にする。
