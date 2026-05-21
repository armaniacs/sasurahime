# Running-Process Manual Command Hint

Date: 2026-05-22

## Background

`~/Library/Caches/` には Microsoft Edge (1.8 GB)、VSCode (920 MB)、ms-playwright (278 MB) など、
`~/Library/Application Support/` には Slack/Cache (228 MB) など、
`~/Library/Logs/` には Claude (62 MB)、zoom.us (824 KB) など、
常駐プロセスが保持するキャッシュ・ログが大量に存在する。sasurahime はこれらを自動削除できないが、
ユーザーが手動で実行できるコマンドを案内することで回収機会を提供できる。

**方針:**

- 起動中のプロセスを調査して、対応するキャッシュが削除困難かを判定する
- 3 つのスキャン対象を合算し、64 MB 超のもの上位 5 件についてヒントを表示する
- sasurahime 自身は一切削除しない（ヒント表示のみ）

---

## 検出対象

スキャンベースディレクトリは 3 つ。各テーブルでマッチしないエントリは無視する。

### `~/Library/Caches/`

| ディレクトリ名（前方一致） | プロセス名（pgrep） | 推奨コマンド |
|---|---|---|
| `Microsoft Edge` | `Microsoft Edge Helper` | `rm -rf ~/Library/Caches/Microsoft\ Edge` ※終了後に実行 |
| `com.microsoft.VSCode.ShipIt` | — | `rm -rf ~/Library/Caches/com.microsoft.VSCode.ShipIt` |
| `ms-playwright` | — | `rm -rf ~/Library/Caches/ms-playwright` |
| `ms-playwright-go` | — | `rm -rf ~/Library/Caches/ms-playwright-go` |
| `electron` | — | `rm -rf ~/Library/Caches/electron` |
| `BraveSoftware` | `Brave Browser` | `rm -rf ~/Library/Caches/BraveSoftware` ※終了後に実行 |
| `typescript` | — | `rm -rf ~/Library/Caches/typescript` |
| `gopls` | — | `rm -rf ~/Library/Caches/gopls` |
| `ort.pyke.io` | — | `rm -rf ~/Library/Caches/ort.pyke.io` |
| `GeoServices` | `locationd` | **スキップ**（OS 管理） |
| `Homebrew` | — | `brew cleanup -s --prune=all` |

### `~/Library/Application Support/`

VSCode は複数サブディレクトリを **1 エントリにまとめて案内する**（集約エントリ）。
集約エントリはサイズを複数パスの合算で算出し、コマンドも複数行で出力する。

| ディレクトリパス（前方一致） | プロセス名（pgrep） | 推奨コマンド | 備考 |
|---|---|---|---|
| `Slack/Cache` | `Slack` | `rm -rf ~/Library/Application\ Support/Slack/Cache` | ※終了後に実行 |
| `Claude/Cache` | `Claude` | `rm -rf ~/Library/Application\ Support/Claude/Cache` | ※終了後に実行 |
| `obsidian/Cache` | `Obsidian` | `rm -rf ~/Library/Application\ Support/obsidian/Cache` | ※終了後に実行 |
| `Code/Cache` + `Code/CachedExtensionVSIXs` + `Code/CachedData` | `Code` | （下記・集約） | まとめて案内 |
| `Google/Chrome` (Cache_Data 配下) | `Google Chrome` | `rm -rf ~/Library/Application\ Support/Google/Chrome/Default/Cache_Data` | ※終了後に実行 |

**VSCode 集約コマンド（3 行まとめて表示）:**

```
$ rm -rf ~/Library/Application\ Support/Code/Cache
$ rm -rf ~/Library/Application\ Support/Code/CachedExtensionVSIXs
$ rm -rf ~/Library/Application\ Support/Code/CachedData
```

### `~/Library/Logs/`

しきい値は **1 MB** に下げる（ログは小さくても削除価値あり）。

| ディレクトリ名（前方一致） | プロセス名（pgrep） | 推奨コマンド |
|---|---|---|
| `Claude` | `Claude` | `rm -rf ~/Library/Logs/Claude` ※終了後に実行 |
| `zoom.us` | `zoom.us` | `rm -rf ~/Library/Logs/zoom.us` ※終了後に実行 |
| `DiagnosticReports` | — | `rm -rf ~/Library/Logs/DiagnosticReports` |
| `LM Studio` | `LM Studio` | `rm -rf ~/Library/Logs/LM\ Studio` ※終了後に実行 |
| `CrashReporter` | — | `rm -rf ~/Library/Logs/CrashReporter` |
| `fsck_hfs.log` | — | `rm -f ~/Library/Logs/fsck_hfs.log` |

**注:** `GeoServices`、`PhotosSearch.aapbz`、`PhotosUpgrade.aapbz`、`ZoomPhone`、`VMware` はテーブル未登録のため自動的にスキップされる。

---

## 実装方針

### 新モジュール `src/hint.rs`

```rust
pub struct HintEntry {
    pub base_dir: BaseDir,         // Caches / AppSupport / Logs
    pub path_suffixes: &'static [&'static str], // 集約対象パス（複数可）
    pub display_name: &'static str,
    pub process_name: Option<&'static str>,     // pgrep 対象
    pub commands: &'static [&'static str],      // 推奨コマンド（複数行対応）
    pub threshold_bytes: u64,      // この値を超えたときのみ表示
    pub skip: bool,                // true = 常にスキップ
}

pub struct ProcessHint {
    pub entry: &'static HintEntry,
    pub size_bytes: u64,           // path_suffixes の合算
    pub running: bool,
}
```

**`collect_hints(home: &Path, runner: &dyn CommandRunner) -> Vec<ProcessHint>`**

1. 各 `BaseDir` のディレクトリを走査し、`KNOWN_ENTRIES` にマッチするエントリのサイズを `du -sk` で取得
2. 集約エントリは `path_suffixes` 全パスのサイズを合算する
3. `skip = true` のエントリを除外
4. `threshold_bytes` 超のもののみ残す
5. `process_name` があれば `pgrep -x <name>` を呼んで `running` を設定
6. `size_bytes` 降順でソートし、上位 5 件を返す

**`print_hints(hints: &[ProcessHint])`**

```
─────────────────────────────────────────────────────────
 Tip: The following caches can be freed manually:
─────────────────────────────────────────────────────────
  Microsoft Edge     1.8 GB  [running — quit first]
    $ rm -rf ~/Library/Caches/Microsoft\ Edge

  Slack/Cache        228 MB  [running — quit first]
    $ rm -rf ~/Library/Application\ Support/Slack/Cache

  ms-playwright      278 MB
    $ rm -rf ~/Library/Caches/ms-playwright

  Claude logs         62 MB  [running — quit first]
    $ rm -rf ~/Library/Logs/Claude

  Homebrew            75 MB
    $ brew cleanup -s --prune=all
─────────────────────────────────────────────────────────
```

`running = true` のエントリには `[running — quit first]` を付与する。
ヒントが 0 件の場合は何も出力しない。

**`offer_auto_clean(hints: &[ProcessHint], home: &Path, runner: &dyn CommandRunner)`**

`running = true` のエントリについてのみ、ユーザーに個別確認して自動終了・削除・（アプリによって）再起動を行う。

```
Quit VSCode and clear cache? (693 MB will be freed) [y/N] y
  Quitting VSCode...
  Clearing cache...  [OK]
  Restarting VSCode...  [OK]

Quit Slack and clear cache? (228 MB will be freed) [y/N] y
  Quitting Slack...
  Clearing cache...  [OK]
  (Slack will not be restarted — log in manually if needed)
```

### `HintEntry` への追加フィールド

```rust
pub struct HintEntry {
    ...
    pub quit_command: Option<&'static str>,    // None = 自動終了不可
    pub relaunch_app: Option<&'static str>,    // None = 再起動しない
}
```

`quit_command`: macOS では `osascript -e 'quit app "AppName"'` を使う。
`relaunch_app`: `open -a "AppName"` に渡すアプリ名。

### アプリ別の再起動方針

| アプリ | 再起動 | 理由 |
|---|---|---|
| VSCode (`Code`) | あり | `open -a "Visual Studio Code"` |
| Microsoft Edge | あり | `open -a "Microsoft Edge"` |
| Slack | **なし** | ログイン処理が重く、ユーザーが任意タイミングで起動すべき |
| Claude (desktop) | **なし** | セッション状態があるため |
| Obsidian | あり | `open -a "Obsidian"` — vault 再接続のみ |
| Brave Browser | あり | `open -a "Brave Browser"` |
| Google Chrome | あり | `open -a "Google Chrome"` |
| zoom.us | **なし** | ミーティング中の可能性がある |
| LM Studio | あり | `open -a "LM Studio"` |

### 自動クリーン処理の流れ（running エントリのみ）

1. `osascript -e 'quit app "AppName"'` で終了要求
2. 最大 10 秒 `pgrep -x` でポーリング（1 秒間隔）
3. まだ起動中なら警告して skip
4. `rm -rf` で各 `path_suffixes` を削除
5. `relaunch_app` があれば `open -a "AppName"` で再起動

---

## 呼び出し箇所

| 箇所 | ファイル | タイミング |
|---|---|---|
| `scanner::run_scan()` 末尾 | `main.rs` (scan アーム) | scan コマンド終了前 |
| 対話モード・`--yes` モード終了後 | `main.rs` (None アーム) | clean 完了後 |

---

## テスト方針

| テスト | 内容 |
|---|---|
| `collect_hints_filters_below_threshold` | threshold_bytes 未満のエントリが除外されること ✅ |
| `collect_hints_limits_to_top5` | 6 件以上あっても上位 5 件のみ返ること ✅ |
| `collect_hints_excludes_skip_entries` | `skip = true` のエントリ（GeoServices 等）が含まれないこと ✅ |
| `collect_hints_sets_running_flag` | pgrep が 0 を返すエントリで `running = true` になること ✅ |
| `collect_hints_aggregates_vscode_dirs` | VSCode の 3 ディレクトリがサイズ合算で 1 エントリになること ✅ |
| `collect_hints_includes_logs` | `~/Library/Logs/Claude` が 1 MB 超で検出されること ✅ |
| `print_hints_empty` | ヒントなしの場合は何も出力されないこと ✅ |
| `print_hints_running_shows_quit_first` | `running = true` のエントリに `quit first` が含まれること ✅ |
| `auto_clean_skips_if_quit_times_out` | 終了タイムアウト時に削除をスキップしてエラーメッセージを出すこと |
| `auto_clean_deletes_paths_after_quit` | 正常終了後にすべての `path_suffixes` が削除されること |
| `auto_clean_relaunches_when_configured` | `relaunch_app` があれば `open -a` が呼ばれること |
| `auto_clean_skips_relaunch_for_slack` | Slack は `relaunch_app = None` なので `open -a` が呼ばれないこと |

`osascript` / `pgrep` / `open` 呼び出しはすべて `CommandRunner` 経由でモック可能。

---

## 実装しないこと

- GeoServices など OS が管理するキャッシュの削除
- ヒント表示のオン/オフ設定（初版では常に表示）
