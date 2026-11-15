<details{% if page.lang == 'ja' %} open{% endif %} markdown="1" data-lang="ja" lang="ja">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

<div class="doc-body" markdown="1">

sasurahime は **55 のクリーンターゲット** を提供しています。すべてのターゲットは
`detect`（読み取り専用、副作用なし）と `clean`（削除）の両方に対応しており、
すべての `clean` サブコマンドは `--dry-run` をサポートしています。
削除されるのはキャッシュや古いバージョンのみで、現在使用中のランタイムやパッケージには触れません。

ターゲットの一覧は `src/main.rs` の `define_cleaners!` 定義表から生成されます
（唯一の源: `command_name` / `dry_run` / `is_special` / `ALL_TARGETS` とこの一覧は
すべて行から導かれます）。追加方法は `docs/HOWTO-ADD-target.md` を参照してください。

### クイックリファレンス

| ターゲット | 削除対象 |
|-----------|---------|
| `uv` | 古い `simple-vN` インデックスディレクトリ + `uv cache prune --force` |
| `brew` | `brew cleanup -s --prune=all` |
| `mise` | 未使用ランタイムバージョン（先に `.mise.toml` をクロスチェック） |
| `browsers` | 古い Puppeteer/Playwright ビルド（ファミリーごとに最新を保持） |
| `cargo` | Cargo レジストリキャッシュ + `target/` ディレクトリ |
| `bun` | `bun pm cache rm` |
| `go` | `go clean -cache` |
| `pip` | `pip cache purge` |
| `npm` | `npm cache clean --force` |
| `yarn` | `yarn cache clean` |
| `pnpm` | `pnpm store prune` |
| `deno` | `deno cache -r` |
| `rustup` | 未使用の Rust ツールチェーン |
| `flutter` | `dart pub cache clean` |
| `conda` | `conda clean --all -y` |
| `pipx` | `pipx cache purge` |
| `poetry` | `poetry cache clear --all` |
| `gem` | `gem cleanup` |
| `bundle` | `bundle clean` |
| `dotnet` | `dotnet nuget locals all --clear` |
| `caches` | bun, go, pip, node-gyp, npm, yarn, pnpm をまとめて実行 |
| `node-gyp` | `~/.cache/node-gyp/` |
| `xcode` | Xcode DerivedData プロジェクトディレクトリ |
| `device-support` | 古い Xcode DeviceSupport ディレクトリ（最新 N を保持） |
| `simulator` | `xcrun simctl delete unavailable` |
| `spm` | SwiftPM キャッシュディレクトリ |
| `cocoa-pods` | `pod cache clean --all` |
| `gradle` | Gradle の古いバージョンキャッシュ |
| `maven` | `mvn dependency:purge-local-repository` |
| `sbt` | Scala/sbt + Ivy キャッシュ |
| `jetbrains` | 古い JetBrains IDE キャッシュ |
| `vscode-extensions` | `~/.vscode/extensions/` |
| `docker` | `docker system prune -f`（dangling イメージのみ） |
| `colima` | `colima prune --all --force` |
| `orbstack` | `orb prune` |
| `huggingface` | HF モデルキャッシュ（`hub/`） |
| `ollama` | Ollama モデルキャッシュ（対話的選択） |
| `act` | `~/.cache/act/` |
| `pre-commit` | pre-commit フック環境キャッシュ |
| `terraform` | Terraform プロバイダプラグインキャッシュ |
| `tree-sitter` | tree-sitter パーサーキャッシュ |
| `volta` | `~/.volta/cache/` |
| `logs` | N 日より古いログファイル |
| `library-logs` | `~/Library/Logs/`（ヒューリスティック、対話的） |
| `downloads` | `~/Downloads/` の古いファイル |
| `ios-backup` | iOS バックアップ ― 対話的のみ、`--yes` では非実行 |
| `apfs-snapshot` | APFS ローカル Time Machine スナップショット ― 対話的、`--all`、または `--thin` |
| `orphan-containers` | アンインストール済みアプリの `~/Library/Containers` と `~/Library/Group Containers` の残骸 - 対話的のみ、`--yes` では実行しない |
| `library-caches` | `~/Library/Caches/` のアプリ別一覧。既知アプリのキャッシュのみ削除可能 - 対話的のみ、`--yes` では非実行 |
| `app-support` | `~/Library/Application Support/` 内の既知アプリのキャッシュ、対話的のみ、`--yes` では非実行 |
| `trash` | `~/.Trash` のサイズを報告（スキャンのみ） |

---

## 1. `sasurahime clean uv`

**カテゴリ:** Sprint 1

**削除対象:** `~/.cache/uv/` 内の古い `simple-vN` インデックスディレクトリ、
および `uv cache prune --force` の実行。

**detect の動作:**
1. `~/.cache/uv/` 内のエントリを読み取ります。
2. `simple-v<N>` にマッチする名前を `UvCleaner::parse_simple_version` でフィルタリングします。シンボリックリンクはスキップされます。
3. すべてのバージョン番号を収集し、最大値を見つけます。
4. 最大値 **以外** の全バージョンのディスクサイズ合計を削除可能量として報告します。

**clean の動作:**
1. `uv cache prune --force` を呼び出します（孤立した/展開済みアーカイブを削除）。
2. 最大バージョン **以外** のすべての `simple-vN` ディレクトリを `fs::remove_dir_all` で削除します。
3. `--dry-run` が設定されている場合：削除予定のリストを表示し、削除は行いません。

**安全性:** 最も新しい `simple-v<N>`（最新）は常に保持され、古いバージョンのみ削除されます。

---

## 2. `sasurahime clean brew`

**カテゴリ:** Sprint 1

**削除対象:** Homebrew のダウンロードキャッシュ。`brew cleanup -s --prune=all` に委譲します。

**detect の動作:**
1. `~/Library/Caches/Homebrew` が存在するか確認します。
2. 存在する場合、キャッシュディレクトリの合計サイズを報告します。

**clean の動作:**
1. `brew` が `PATH` にない場合、メッセージを表示して終了します (0)。
2. `brew cleanup -s --prune=all` を実行します。
3. brew の出力から解放サイズをパースします（`BrewCleaner::parse_brew_freed_bytes` を使用）。
4. パースした解放バイト数を報告します。

**安全性:** Homebrew CLI 自身が安全性を確保します。

---

## 3. `sasurahime clean mise`

**カテゴリ:** Sprint 2

**削除対象:** [mise](https://mise.jdx.dev/) がインストールした未使用のランタイムバージョン（`~/.local/share/mise/installs/<tool>/<version>` 内）。

**detect の動作:**
1. `mise ls --current` を実行し、現在アクティブな `(tool, version)` ペアを取得します。
2. `~/.config/mise/config.toml` と HOME 下のすべての `.mise.toml`（最大深さ 5）をスキャンし、固定 (pinned) されたバージョンを収集します。
3. `~/.local/share/mise/installs/` 以下のディレクトリツリーを読み取ります。
4. アクティブセット **にも** 固定セット **にも** 含まれないインストール済みバージョンを未使用とみなし、そのディスクサイズを合計します。

**clean の動作:**
1. detect と同じアクティブ＋固定検出を行います。
2. 未使用の `(tool, version, path)` トリプルごとに：
   - **dry-run:** `[dry-run] would remove: <tool> <version>` を表示します。
   - **実実行:** `MiseCleaner::remove_with_uchg` を呼び出し、`chflags -R nouchg` で macOS 不変フラグを解除後、`fs::remove_dir_all` を実行します。
3. 解放バイト数の合計を報告します。

**安全性:**
- グローバル設定と HOME 内の `.mise.toml`（最大深さ 5）の **両方** をクロスチェックします。
- 固定 (pinned) されたバージョンは現在アクティブでなくても **決して** 削除されません。

---

## 4. `sasurahime clean browsers`

**カテゴリ:** Sprint 2

**削除対象:** [Puppeteer](https://pptr.dev/) と [Playwright](https://playwright.dev/) が使用する古いブラウザエンジンビルド。ブラウザファミリーごとに最新バージョンのみ保持します。

**スキャン対象パス:**

| ラベル                     | パス                                   |
|----------------------------|----------------------------------------|
| puppeteer/chrome           | `~/.cache/puppeteer/chrome`            |
| puppeteer/chrome-headless-shell | `~/.cache/puppeteer/chrome-headless-shell` |
| ms-playwright              | `~/Library/Caches/ms-playwright`       |
| ms-playwright-go           | `~/Library/Caches/ms-playwright-go`    |

**バージョン比較:**
- `BrowserCleaner::version_key` はディレクトリ名を `Vec<u32>` に変換します（ASCII 数字の連続を抽出）。
- 例: `mac_arm-131.0.6778.204` → `[131, 0, 6778, 204]`、`chromium-1208` → `[1208]`。
- 比較はベクタの辞書順（標準の Rust `Vec<u32>::cmp`）で行われ、semver 形式とフラットなビルド番号形式の両方を正しく処理します。

**detect の動作:**
1. グループごとに `BrowserCleaner::find_old_versions(parent)` を呼び出します。
2. ディレクトリを読み取り、シンボリックリンクと解析不能な名前をスキップし、最新バージョン **以外** のすべてのエントリを返します。
3. 0 または 1 エントリ → 削除対象なし。2 以上 → 古いバージョンのサイズ合計。

**clean の動作:**
1. グループごとに `find_old_versions` を呼び出します。
2. `--dry-run` の場合：削除予定を表示します。
3. それ以外：各古いバージョンパスに対して `fs::remove_dir_all` を実行します。
4. 解放バイト数を報告します。

**安全性:**
- 最新バージョン（最新のブラウザバイナリ）は **常に** 保持されます。
- シンボリックリンクはスキップされ、古いリンク経由の削除を防ぎます。
- `nightly` など解析不能な名前のディレクトリはスキップされます。

---

## 5. `sasurahime clean bun`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [Bun](https://bun.sh/) のパッケージキャッシュ。

**方法:** `bun pm cache rm`

**detect の動作:** `bun` が `PATH` にあるか確認します。見つかった場合は削除可能として報告します（サイズ不明）。

**clean の動作:**
1. `bun` が見つからない場合、メッセージを表示して終了します (0)。
2. `bun pm cache rm` を実行します。
3. 解放バイト数として 0 を報告します（ツールが解放容量を報告しないため）。

**安全性:** 公式の `bun` CLI に委譲します。

---

## 6. `sasurahime clean go`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [Go](https://go.dev/) のビルドキャッシュ。

**方法:** `go clean -cache`

**detect の動作:** `go` が `PATH` にあるか確認します。

**clean の動作:**
1. `go` が見つからない場合、メッセージを表示して終了します (0)。
2. `go clean -cache` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `go` CLI に委譲します。

---

## 7. `sasurahime clean pip`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [pip](https://pip.pypa.io/) のパッケージキャッシュ。

**方法:** `pip cache purge`

**detect の動作:** `pip` が `PATH` にあるか確認します。

**clean の動作:**
1. `pip` が見つからない場合、メッセージを表示して終了します (0)。
2. `pip cache purge` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `pip` CLI に委譲します。

---

## 8. `sasurahime clean node-gyp`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [node-gyp](https://github.com/nodejs/node-gyp) のビルドキャッシュディレクトリ。

**スキャン対象パス:**
- `~/.cache/node-gyp/`
- `~/Library/Caches/node-gyp/`

**detect の動作:**
1. 2 つのディレクトリのうち存在するものを確認します。
2. それぞれの `dir_size` を合計します。

**clean の動作:**
1. 削除前に `chflags -R nouchg <path>` を実行します（macOS 不変フラグを解除）。
2. 既存の各ディレクトリに対して `fs::remove_dir_all` を実行します。
3. 解放バイト数を報告します。

**安全性:** macOS の `uchg` フラグは自動的に処理されます。

---

## 9. `sasurahime clean npm`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [npm](https://www.npmjs.com/) のパッケージキャッシュ。

**方法:** `npm cache clean --force`

**detect の動作:** `npm` が `PATH` にあるか確認します。

**clean の動作:**
1. `npm` が見つからない場合、メッセージを表示して終了します (0)。
2. `npm cache clean --force` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `npm` CLI に委譲します。

---

## 10. `sasurahime clean yarn`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [Yarn](https://yarnpkg.com/) のパッケージキャッシュ。

**方法:** `yarn cache clean`

**detect の動作:** `yarn` が `PATH` にあるか確認します。

**clean の動作:**
1. `yarn` が見つからない場合、メッセージを表示して終了します (0)。
2. `yarn cache clean` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `yarn` CLI に委譲します。

---

## 11. `sasurahime clean pnpm`

**カテゴリ:** Sprint 3 ― Generic caches

**削除対象:** [pnpm](https://pnpm.io/) ストア。

**方法:** `pnpm store prune`

**detect の動作:** `pnpm` が `PATH` にあるか確認します。

**clean の動作:**
1. `pnpm` が見つからない場合、メッセージを表示して終了します (0)。
2. `pnpm store prune` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の `pnpm` CLI に委譲します。

---

## 12. `sasurahime clean caches`

**カテゴリ:** Sprint 3 ― Generic caches（集約）

**削除対象:** **すべての** ジェネリックキャッシュを 1 コマンドで削除します。
`bun`、`go`、`pip`、`node-gyp`、`npm`、`yarn`、`pnpm` を順次実行するのと同等です。

各サブクリーナーは独立して呼び出されます。ツールがインストールされていない場合はスキップされます（終了コードは 0）。

---

## 13. `sasurahime clean logs`

**カテゴリ:** Sprint 3

**削除対象:** 既知およびユーザー設定のログディレクトリ内の `N` 日より古いログファイル。

**組み込みログターゲット（自動スキャン）:**

| 名前         | パス                                 | 削除除外ファイル        |
|--------------|--------------------------------------|-------------------------|
| kilo         | `~/.local/share/kilo/log`            | `dev.log`               |
| opencode     | `~/.local/share/opencode/logs`       | _(なし)_                |
| claude-code  | `~/.local/share/claude/logs`         | _(なし)_                |
| vscode-logs  | `~/Library/Application Support/Code/logs` | _(なし)_          |

**追加ログターゲット** は設定ファイルで追加できます。各ターゲットは独自のパスと除外リストを持ちます。

**detect の動作:**
1. すべてのターゲット（組み込み＋設定追加）を反復処理します。
2. ターゲットごとに `LogCleaner::find_old_logs(dir, keep_days, exclude)` を呼び出します。
3. ディレクトリを読み取り、通常ファイルのみにフィルタリングし、除外リストに一致するファイルを除外して、各ファイルの mtime を確認します。
4. `keep_days` より古いすべてのファイルのバイトサイズを合計します。

**clean の動作:**
1. detect と同じ `find_old_logs` ロジックを使用します。
2. `--dry-run` の場合：各古いファイルに対して削除予定を表示します。
3. それ以外：各古いファイルに対して `fs::remove_file` を呼び出します。
4. 要約を表示：`Removed <N> log files`。
5. 解放バイト数を報告します。

**保持ポリシー:**
- デフォルト: `keep_days = 7`（7 日より古いファイルを削除）。
- `--keep-days <N>` フラグで上書き可能。
- 設定ファイルでも上書き可能：`[logs]\nkeep_days = <N>`。
- CLI フラグが設定より優先されます。

**安全性:**
- 除外リストのファイルは **決して** 削除されません。
- 組み込みの `kilo` ターゲットはデフォルトで `dev.log` を除外します。
- `is_older_than` は厳密な `>` 比較を使用します（正確に `N` 日前のファイルは削除 **されません**）。
- ファイルメタデータが読み取れない場合はスキップされます。

---

## 14. `sasurahime clean xcode`

**カテゴリ:** Sprint 3

**削除対象:** Xcode DerivedData フォルダ内のプロジェクトビルドアーティファクト（`~/Library/Developer/Xcode/DerivedData/`）。

**detect の動作:**
1. `DerivedData` ディレクトリが存在するか確認します。
2. 存在する場合、合計サイズを報告します。

**clean の動作:**
1. `DerivedData` が存在しない場合、メッセージを表示して終了します (0)。
2. Xcode が実行中の場合は警告を表示し、確認を求めます。
3. `DerivedData` のサブディレクトリを一覧表示します。
4. `--dry-run` の場合：プロジェクトごとに削除予定を表示します。
5. それ以外：各プロジェクトディレクトリに対して `fs::remove_dir_all` を呼び出します。`DerivedData` ルート自体は **決して** 削除されません。
6. 解放バイト数を報告します。

**安全性:**
- `DerivedData` ルートディレクトリは **決して** 削除されません。
- Xcode 実行中は確認を求められます。
- ルートのファイルはスキップされ、サブディレクトリのみが削除対象となります。

---

## 15. `sasurahime clean act`

**カテゴリ:** Sprint 5

**削除対象:** [act](https://github.com/nektos/act) GitHub Actions ローカルランナーキャッシュ（`~/.cache/act/`、または `$ACT_CACHE_DIR` が設定されている場合はそのパス）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. キャッシュディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `is_safe_delete_target` を実行してパスがシステムディレクトリでないことを確認します。
2. 安全でないパスの場合はデフォルトにフォールバックします。
3. `fs::remove_dir_all` でディレクトリを削除します。

**安全性:** `$ACT_CACHE_DIR` はシステムパスのブロックリストに対して検証されます。安全でない環境変数値は拒否されます。

---

## 16. `sasurahime clean cargo`

**カテゴリ:** Sprint 5

**削除対象:** [Cargo](https://doc.rust-lang.org/cargo/) レジストリキャッシュ（`~/.cargo/registry/cache/`）と `target/` ビルドアーティファクトディレクトリ。

**detect のスキャン対象:**
1. `~/.cargo/registry/cache/` ― ダウンロード済みクレートアーカイブ。
2. `~/src/`、`~/work/`、`~/dev/` 以下のユーザープロジェクトの `target/` ディレクトリ。

**clean の動作:**
1. `~/.cargo/registry/cache/` の内容を削除します。
2. 一般的なプロジェクトルート下の `target/` ディレクトリをスキャンして削除します。
3. 削除前に `chflags -R nouchg` を実行します。

---

## 17. `sasurahime clean cocoa-pods`

**カテゴリ:** Sprint 5

**削除対象:** [CocoaPods](https://cocoapods.org/) キャッシュ。

**方法:** `pod cache clean --all`

**detect の動作:** `pod` が `PATH` にあるか確認します。

**clean の動作:**
1. `pod` が見つからない場合、メッセージを表示して終了します (0)。
2. `pod cache clean --all` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の CocoaPods CLI に委譲します。

---

## 18. `sasurahime clean conda`

**カテゴリ:** Sprint 5

**削除対象:** [Conda](https://docs.conda.io/) パッケージキャッシュ。

**方法:** `conda clean --all -y`

**detect の動作:** `conda` が `PATH` にあるか確認します。

**clean の動作:**
1. `conda` が見つからない場合、メッセージを表示して終了します (0)。
2. `conda clean --all -y` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Conda CLI に委譲します。

---

## 19. `sasurahime clean deno`

**カテゴリ:** Sprint 5

**削除対象:** [Deno](https://deno.com/) キャッシュ。

**方法:** `deno cache -r`（キャッシュ再読み込み）

**detect の動作:** `deno` が `PATH` にあるか確認します。

**clean の動作:**
1. `deno` が見つからない場合、メッセージを表示して終了します (0)。
2. `deno cache -r` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Deno CLI に委譲します。

---

## 20. `sasurahime clean docker`

**カテゴリ:** Sprint 5

**削除対象:** [Docker](https://www.docker.com/) の dangling イメージ、コンテナ、ビルドキャッシュ、ネットワーク。

**方法:** `docker system prune -f`

**detect の動作:** `docker` が `PATH` にあるか確認します。

**clean の動作:**
1. `docker` が見つからない場合、メッセージを表示して終了します (0)。
2. `docker system prune -f` を実行します（dangling イメージのみ ― タグ付きイメージは保持）。
3. 解放バイト数として 0 を報告します。

**仮想ディスクの注意:**
`sasurahime scan` は表の下に Docker Desktop の仮想ディスク
(`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` と旧版の配置)
の実際のディスク使用量と最大サイズを表示します。`docker system prune` は VM 内の
領域を空けますが、ファイルは必ずしも縮みません。`clean docker` の前後でディスク
使用量を比較し、縮まなかった場合は Docker Desktop の Settings > Resources > Disk
image size を小さくするか、Troubleshoot > Clean / Purge data (全イメージ、コンテナ、
ボリュームを削除) を案内します。detect は docker デーモンに問い合わせないため、
デーモン停止中でも scan はハングしません。

**安全性:** dangling（タグなし）イメージのみ削除します。明示的に `-a` フラグを避け、タグ付きの未使用イメージを保持します。

---

## 21. `sasurahime clean downloads`

**カテゴリ:** Sprint 5

**削除対象:** `~/Downloads/` 内のファイル。

**detect の動作:**
1. `~/Downloads` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `~/Downloads` の直下の子項目を一覧表示します。
2. `chflags -R nouchg` + `remove_dir_all` で削除します。
3. **安全性:** `~/Downloads/` の直下の項目のみ削除します（再帰なし）。

---

## 22. `sasurahime clean gradle`

**カテゴリ:** Sprint 5

**削除対象:** [Gradle](https://gradle.org/) の古いバージョンキャッシュ（`~/.gradle/caches/`）。キャッシュされた各アーティファクトの最新バージョンのみ保持します。

**detect の動作:**
1. `~/.gradle/caches/` をスキャンしてバージョンディレクトリを確認します。
2. アーティファクトグループごとに最新バージョンを保持し、それ以前の全バージョンのサイズ合計を報告します。

**clean の動作:**
1. バージョン比較を使用して古いバージョンを特定します。
2. `chflags -R nouchg` + `remove_dir_all` で古いディレクトリを削除します。

**安全性:** 各キャッシュアーティファクトの最新バージョンは常に保持されます。

---

## 23. `sasurahime clean huggingface`

**カテゴリ:** Sprint 5

**削除対象:** [Hugging Face](https://huggingface.co/) モデルキャッシュ（`~/.cache/huggingface/hub/` または `$HF_HOME/hub`）。

**detect の動作:**
1. `hub/` ディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. まず CLI を試行：`huggingface-cli delete-cache --yes` を実行します。
2. CLI が PATH にない場合は、`hub/` の内容を直接削除します（削除後に `hub/` ディレクトリを再作成）。
3. `$HF_HOME` は `is_safe_delete_target` で検証されます。

**安全性:** `$HF_HOME` はシステムパスのブロックリストに対して検証されます。CLI が直接削除より優先されます。

---

## 24. `sasurahime clean jetbrains`

**カテゴリ:** Sprint 5

**削除対象:** [JetBrains IDEs](https://www.jetbrains.com/)（IntelliJ IDEA、WebStorm など）の古いバージョンキャッシュ（`~/Library/Caches/JetBrains/` 内）。

**方法:** Gradle キャッシュと同様に、各 IDE キャッシュの最新バージョンを保持します。

**detect の動作:**
1. `~/Library/Caches/JetBrains/` をスキャンして IDE 単位・バージョン単位のディレクトリを確認します。
2. 古いバージョンを報告します。

**clean の動作:**
1. IDE ファミリーごとに古いバージョンを特定します。
2. `chflags -R nouchg` + `remove_dir_all` で削除します。

**安全性:** 各 IDE キャッシュの最新バージョンは常に保持されます。

---

## 25. `sasurahime clean library-logs`

**カテゴリ:** Sprint 5

**削除対象:** `~/Library/Logs/` 下のユーザーログファイル。ヒューリスティックルールを使用して削除候補を提案します。**インタラクティブ** ― `--all` が使用されない限り選択プロンプトが表示されます。

**detect の動作:**
1. `~/Library/Logs/` の直下の子項目を読み取ります。
2. 各エントリについて：`dir_size` と `last_modified` 時間を測定します。
3. 2 つのヒューリスティックルールを適用します：
   - **大容量:** サイズ > 100 MB → `[large]` タグ
   - **古い:** 最終変更 > 90 日前 → `[stale N days]` タグ
4. 少なくとも 1 つのルールに該当するエントリを結果に含めます。
5. `CrashReporter`、`DiagnosticReports`、ドットエントリを除外します。

**clean の動作:**
1. detect と同じスキャンを実行します。
2. `--dry-run`：各エントリとその理由タグを表示します。
3. `--all`：確認なしですべての候補を削除します。
4. それ以外：`dialoguer::MultiSelect` ですべてのエントリを事前選択して表示し、ユーザーが確認して削除します。

**安全性:**
- `CrashReporter` と `DiagnosticReports` は **常に** 除外されます。
- ドットファイル・ディレクトリ（`.DS_Store`、`.localized` など）はスキップされます。
- 時計のずれによる未来のタイムスタンプは `SystemTime::now()` にクランプされます。
- `--dry-run` は副作用ゼロを保証します。

---

## 26. `sasurahime clean orbstack`

**カテゴリ:** Sprint 5

**削除対象:** [Orbstack](https://orbstack.dev/) Docker ランタイムキャッシュ。

**方法:** `orb prune`

**detect の動作:** `orb` が `PATH` にあるか確認します。

**clean の動作:**
1. `orb` が見つからない場合、メッセージを表示して終了します (0)。
2. `orb prune` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Orbstack CLI に委譲します。

---

## 27. `sasurahime clean pipx`

**カテゴリ:** Sprint 5

**削除対象:** [pipx](https://pypa.github.io/pipx/) キャッシュと未使用パッケージ。

**方法:** `pipx cache purge`

**detect の動作:** `pipx` が `PATH` にあるか確認します。

**clean の動作:**
1. `pipx` が見つからない場合、メッセージを表示して終了します (0)。
2. `pipx cache purge` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の pipx CLI に委譲します。

---

## 28. `sasurahime clean poetry`

**カテゴリ:** Sprint 5

**削除対象:** [Poetry](https://python-poetry.org/) パッケージキャッシュ。

**方法:** `poetry cache clear --all`

**detect の動作:** `poetry` が `PATH` にあるか確認します。

**clean の動作:**
1. `poetry` が見つからない場合、メッセージを表示して終了します (0)。
2. `poetry cache clear --all` を実行します。
3. 解放バイト数として 0 を報告します。

**安全性:** 公式の Poetry CLI に委譲します。

---

## 29. `sasurahime clean pre-commit`

**カテゴリ:** Sprint 5

**削除対象:** [pre-commit](https://pre-commit.com/) フック環境キャッシュ（`~/.cache/pre-commit/`、`$PRE_COMMIT_HOME`、または `$XDG_CACHE_HOME/pre-commit`）。

**detect の動作:**
1. 環境変数からキャッシュディレクトリを解決します：`$PRE_COMMIT_HOME` → `$XDG_CACHE_HOME/pre-commit` → `~/.cache/pre-commit`。
2. 合計サイズを報告します。

**clean の動作:**
1. まず CLI を試行：`pre-commit clean` を実行します。
2. CLI が PATH にない場合はキャッシュディレクトリを直接削除します。
3. 環境変数パスは `is_safe_delete_target` で検証されます。

**安全性:** 環境変数パスは検証されます。CLI が直接削除より優先されます。

---

## 30. `sasurahime clean rustup`

**カテゴリ:** Sprint 5

**削除対象:** 未使用の [Rust](https://www.rust-lang.org/) ツールチェーンバージョン（`rustup default` や `rustup override` で選択されていないもの）。

**detect の動作:**
1. `rustup toolchain list` を実行してインストール済みツールチェーンを列挙します。
2. デフォルトツールチェーン（`(default)` マーク）とオーバーライドツールチェーン（`(override)` マーク）を特定します。
3. デフォルト **でも** オーバーライド **でもない** ツールチェーンを未使用として報告します。

**clean の動作:**
1. detect と同じ検出ロジックを使用します。
2. 未使用のツールチェーンごとに：`rustup toolchain remove <name>` を実行します。
3. 解放バイト数を報告します（rustup 出力からパース）。

**安全性:** デフォルトとオーバーライドのツールチェーンは **決して** 削除されません。

---

## 31. `sasurahime clean spm`

**カテゴリ:** Sprint 5

**削除対象:** [Swift Package Manager](https://www.swift.org/package-manager/) のビルドアーティファクトとキャッシュされたパッケージ。

**方法:** `~/Library/Caches/org.swift.swiftpm/` と `~/Library/Developer/Xcode/DerivedData/SourcePackages/` を削除します。

**detect の動作:**
1. SPM キャッシュディレクトリが存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. キャッシュディレクトリに対して `chflags -R nouchg` を実行します。
2. 各キャッシュディレクトリに対して `fs::remove_dir_all` を呼び出します。
3. キャッシュされたパッケージチェックアウトとリポジトリクローンが削除されます（次回ビルド時に再取得されます）。

---

## 32. `sasurahime clean trash`

**カテゴリ:** Sprint 5

**削除対象:** `~/.Trash` ― **スキャンのみ**。sasurahime はゴミ箱ディレクトリのサイズを報告しますが、削除は行いません（ユーザーは Finder からゴミ箱を空にしてください）。

**detect の動作:**
1. `~/.Trash` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. **`--dry-run`:** detect スタイルのスキャンを実行し、解放されるサイズを表示します。
2. **それ以外:** Finder からゴミ箱を空にするよう指示する警告を表示します。ファイルは削除されません。

**安全性:** sasurahime は `~/.Trash` の内容を削除することを拒否します。これは意図的な安全対策です。

---

## 33. `sasurahime clean volta`

**カテゴリ:** Sprint 5

**削除対象:** [Volta](https://volta.sh/) Node.js バージョンマネージャのキャッシュ（`~/.volta/cache/`）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. `~/.volta/cache/` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `~/.volta/cache/` ディレクトリを `fs::remove_dir_all` で削除します。
2. 解放バイト数を報告します。

**安全性:** `cache/` サブディレクトリのみ削除され、`~/.volta/` 自体とツールチェーンバイナリは保持されます。

---

## 34. `sasurahime clean sbt`

**カテゴリ:** Sprint 5

**削除対象:** [sbt](https://www.scala-sbt.org/) ビルドキャッシュ（`~/.sbt/`）と [Ivy](https://ant.apache.org/ivy/) 依存関係キャッシュ（`~/.ivy2/cache/`）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. `~/.sbt/` または `~/.ivy2/cache/` が存在するか確認します。
2. 既存のディレクトリの合計サイズを報告します。

**clean の動作:**
1. `~/.sbt/` と `~/.ivy2/cache/` を `fs::remove_dir_all` で削除します。
2. 解放バイト数を報告します。

**安全性:** ディレクトリは完全に削除されます。依存関係は次回 `sbt compile` 実行時に再取得されます。

---

## 35. `sasurahime clean tree-sitter`

**カテゴリ:** Sprint 5

**削除対象:** [tree-sitter](https://tree-sitter.github.io/) パーサーコンパイルキャッシュ（`~/.cache/tree-sitter/`）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. `~/.cache/tree-sitter/` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `~/.cache/tree-sitter/` を `fs::remove_dir_all` で削除します。
2. 解放バイト数を報告します。

**安全性:** パーサーは次回エディタ（Neovim、Helix など）使用時に再コンパイルされます。

---

## 36. `sasurahime clean simulator`

**カテゴリ:** Sprint 5

**削除対象:** iOS シミュレータキャッシュ。`xcrun simctl delete unavailable` で利用不可のシミュレータランタイムを削除します。

**detect の動作:**
1. `~/Library/Developer/CoreSimulator` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `xcrun` が `PATH` にない場合、メッセージを表示して終了します (0)。
2. `xcrun simctl delete unavailable` を実行します。
3. シミュレータキャッシュのサイズ変化から解放バイト数を報告します。

**安全性:** 公式の `xcrun` CLI に委譲します。

---

## 37. `sasurahime clean device-support`

**カテゴリ:** Sprint 5

**削除対象:** `~/Library/Developer/Xcode/* DeviceSupport/` 内の古い Xcode DeviceSupport ディレクトリ。プラットフォームごとに最新 N バージョンを保持（デフォルト: 2）。

**スキャン対象パス:**
- `~/Library/Developer/Xcode/iOS DeviceSupport/`
- `~/Library/Developer/Xcode/watchOS DeviceSupport/`
- その他の `* DeviceSupport/` ディレクトリ

**バージョン比較:**
- プラットフォームディレクトリごとにバージョン番号のサブディレクトリ（例: `17.0`、`18.0`）を収集。メジャーバージョン（`.` の前の数値）で比較します。
- メジャーバージョンの降順でソートし、上位 N 個を保持します。

**detect の動作:**
1. `~/Library/Developer/Xcode/` 内の `* DeviceSupport/` ディレクトリをスキャンします。
2. 各ディレクトリ内のバージョンサブディレクトリを収集・ソートします。
3. 最新 N バージョン以外の全バージョンのディスクサイズ合計を報告します。

**clean の動作:**
1. detect と同じスキャンロジックを使用します。
2. `--dry-run` の場合：削除予定のディレクトリを表示します。
3. それ以外：`chflags -R nouchg` を実行後、ゴミ箱へ移動（または完全削除）します。
4. 解放バイト数を報告します。

**安全性:**
- プラットフォームごとに最新 N バージョンは常に保持されます。
- `DeviceSupport` ルートディレクトリは決して削除されません。

---

## 38. `sasurahime clean vscode-extensions`

**カテゴリ:** Sprint 5

**削除対象:** VS Code 拡張機能キャッシュ（`~/.vscode/extensions/`）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. `~/.vscode/extensions/` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `~/.vscode/extensions/` を `fs::remove_dir_all` で削除します。
2. 拡張機能は次回 VS Code 起動時に再ダウンロードされます。

**安全性:** 拡張機能キャッシュのみ削除され、VS Code の設定は保持されます。

---

## 39. `sasurahime clean maven`

**カテゴリ:** Sprint 5

**削除対象:** Maven ローカルリポジトリキャッシュ。`mvn dependency:purge-local-repository` で削除します。

**detect の動作:**
1. `~/.m2/repository` が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `mvn` が `PATH` にない場合、メッセージを表示して終了します (0)。
2. `mvn dependency:purge-local-repository` を実行します。
3. リポジトリサイズの変化から解放バイト数を報告します。

**安全性:** 公式の Maven CLI に委譲します。依存関係は次回ビルド時に再ダウンロードされます。

---

## 40. `sasurahime clean terraform`

**カテゴリ:** Sprint 5

**削除対象:** Terraform プロバイダプラグインキャッシュ（`~/.terraform.d/plugin-cache/` または `$TF_PLUGIN_CACHE_DIR`）。

**方法:** ディレクトリ削除。

**detect の動作:**
1. プラグインキャッシュディレクトリを解決します：`$TF_PLUGIN_CACHE_DIR` が設定されていればそのパス、そうでなければ `~/.terraform.d/plugin-cache/`。
2. 合計サイズを報告します。

**clean の動作:**
1. プラグインキャッシュディレクトリを `fs::remove_dir_all` で削除します。
2. プロバイダは次回 `terraform init` 実行時に再ダウンロードされます。

**安全性:** `$TF_PLUGIN_CACHE_DIR` は `is_safe_delete_target` で検証されます。プロバイダキャッシュのみ削除され、Terraform のステートファイルと設定は保持されます。

---

## 41. `sasurahime clean flutter`

**カテゴリ:** Sprint 5

**削除対象:** Flutter/Dart pub キャッシュ。`dart pub cache clean` で削除します。

**detect の動作:**
1. pub キャッシュディレクトリ（`$PUB_CACHE` または `~/.pub-cache/`）が存在するか確認します。
2. 合計サイズを報告します。

**clean の動作:**
1. `dart` が `PATH` にない場合、メッセージを表示して終了します (0)。
2. `dart pub cache clean` を実行します。
3. キャッシュサイズの変化から解放バイト数を報告します。

**安全性:** 公式の Dart CLI に委譲します。公開パッケージは次回 `dart pub get` または `flutter pub get` 実行時に再ダウンロードされます。

---

## 42. `sasurahime clean colima`

**カテゴリ:** Sprint 5

**削除対象:** [Colima](https://github.com/abiosoft/colima) VM ディスクキャッシュ。`colima prune --all --force` で削除します。

**detect の動作:**
1. `~/.colima/` が存在するか確認します。
2. 合計サイズを報告します。
   *注: これは推定最大値です。実際の解放量は
   どの VM ディスクイメージが削除されるかに依存します。*

**clean の動作:**
1. `colima` が `PATH` にない場合、`~/.colima/` をゴミ箱経由で直接削除します
   （`chflags -R nouchg` 処理あり）。解放バイト数を報告します。
2. `colima` が見つかった場合：
   - インタラクティブ TTY では：停止中の VM ディスクデータ（コンテナ、イメージ、
     ボリューム）がすべて削除される旨を警告する確認プロンプトを表示します。
     ユーザーが拒否した場合は中断します。
   - `--yes`（非 TTY）モードでは：確認プロンプトを自動的にスキップします。
3. `colima prune --all --force` を実行します。
4. `colima prune` が非ゼロ終了した場合、サイレントに成功とみなす代わりに、
   終了コードと標準エラー出力を含む警告を表示します。
5. ディレクトリサイズの変化から解放バイト数を報告します。

**安全性:**
- インタラクティブ TTY での確認プロンプトが誤削除を防ぎます。
- CLI がない場合の直接削除フォールバックにより、`colima` CLI がインストール
  されていない環境でもキャッシュを削除できます。

---

## 43. `sasurahime clean ollama`

**カテゴリ:** Sprint 5

**削除対象:** [Ollama](https://ollama.com/) モデルキャッシュ（`~/.ollama/models/`）。

**detect の動作:**
1. `ollama` CLI が利用可能な場合、`ollama list` を実行しモデルサイズをパースして報告します。
2. CLI が見つからない場合は `~/.ollama/models/` の `dir_size` にフォールバックします。

**clean の動作:**
1. `ollama` CLI が利用可能な場合：
   - インストール済みモデルを一覧表示します。
   - 対話的な `dialoguer::MultiSelect` プロンプトで削除するモデルを選択します（`--dry-run` 時を除く）。
   - 選択されたモデルごとに `ollama rm <model>` を実行します。
2. `ollama` CLI が利用できない場合：
   - `~/.ollama/models/` ディレクトリを `chflags -R nouchg` + `remove_dir_all`（またはゴミ箱）で削除します。
3. 解放バイト数の合計を報告します。

**安全性:**
- 対話的な選択により大規模モデルの誤削除を防止します。
- CLI が直接削除より優先され、適切なモデル管理が行われます。
- `--dry-run` モードではモデルが一覧表示されるのみで削除は行われません。

---

## 44. `sasurahime clean ios-backup`

**カテゴリ:** Sprint 6

**削除対象:** `~/Library/Application Support/MobileSync/Backup/` 内の iOS
デバイスバックアップ。各デバイスは UUID のような名前のサブディレクトリとして
保存されています。

**detect の動作:**
1. バックアップディレクトリが存在するか確認します。
2. サブディレクトリ（UUID 名のエントリ）を読み取り、各ディレクトリの
   `dir_size` を測定します。
3. すべてのバックアップディレクトリの合計サイズを報告します。

**clean の動作:**
1. すべてのバックアップディレクトリをサイズ付きで一覧表示します。
2. 対話的な `dialoguer::MultiSelect` プロンプトで削除するバックアップを
   選択します（`--dry-run` 時を除く）。
3. `chflags -R nouchg` で macOS 不変フラグを解除します。
4. 選択されたバックアップをゴミ箱に移動します。
5. 解放バイト数の合計を報告します。

**安全性:**
- iOS バックアップは **元に戻せません** ― 一度削除すると復元できません。
- **インタラクティブのみ:** `--yes` モードでは決して実行されません。非 TTY
  の呼び出しは安全にスキップされます。
- `--dry-run` モードではバックアップが一覧表示されるのみで削除は行われません。
- macOS 不変フラグ（`uchg`）は自動的に処理されます。

---

## 45. `sasurahime clean apfs-snapshot`

**カテゴリ:** Sprint 6

**削除対象:** `tmutil deletelocalsnapshots <date>` による APFS ローカル
Time Machine スナップショット（`<date>` は `YYYY-MM-DD-HHMMSS`）。

**detect の動作:**
1. `tmutil` がシステムで利用可能か確認します。
2. `tmutil listlocalsnapshots /` を実行し、出力を `parse_snapshot_names`
   でパースします（`com.apple.TimeMachine.<date>.local` 形式の行のみを残し、
   `Snapshots for disk /:` のヘッダ行は無視します）。暦の範囲外（月 1-12、
   日 1-31、時 0-23、分秒 0-59）の日付は除外します。
3. `/.MobileBackups` のディスク使用量（存在する場合）をサイズ見積もりとして
   測定します。
4. 測定サイズ付きで `Pruneable` を返します（`/.MobileBackups` が存在しない
   場合は 0）。

**clean の動作:**
1. フラグから 1 つのモードだけを決定します: 削除モード（既定）か thin モード
   （`--thin`）。`--all` と `--thin` は同時指定できず、`--urgency` は
   `--thin` が必須です。競合や単独指定は、スナップショットコマンドを実行する
   前に明示エラー（終了コード 1）になります。
2. 削除モードは `tmutil listlocalsnapshots` からローカルスナップショットを
   一覧表示します。`--all` なしの場合は対話的な `dialoguer::MultiSelect`
   プロンプトを開き（`--dry-run` 時を除く）、`--all` ありの場合はプロンプト
   なしで全件を選択し、非 TTY でも実行できます。その後、選択された
   スナップショットごとに `tmutil deletelocalsnapshots <date>` を実行します。
3. thin モードは `tmutil thinlocalsnapshots / <target-bytes> <urgency>`
   （urgency の既定は 2、4 は追加確認が必要）を実行し、どのスナップショットを
   削除するかは macOS に任せます。
4. 解放バイト数は、実行前後の `/` の空き容量の増加分として報告します。
   purgeable 領域の反映が遅れるため、実際より小さい値になることがあります。

**権限:** `tmutil deletelocalsnapshots` は root 権限が必要です。sasurahime は
`sudo` を実行しません。削除に失敗したスナップショットごとに、手動で実行する
`sudo tmutil deletelocalsnapshots <date>` を表示します。Time Machine 形式
（`com.apple.TimeMachine.<date>.local`）以外のスナップショットは一覧に含めません。
削除に失敗したものは skipped として返し、reason には `tmutil` のエラー出力
（起動に失敗した場合はそのエラー）が入ります。1 件も削除できなかった場合の
終了コードは 1 です。

**安全性:**
- スナップショットの削除は、次回のバックアップまでローカル Time Machine
  保護を無効にします。
- **1 回の実行に 1 つのモード:** `--all` と `--thin` は同時指定できず、
  `--urgency` は `--thin` が必須です。競合するフラグは、どちらかを黙って
  選ぶのではなく明示エラーになります。
- **既定はインタラクティブ:** `--yes` モードでは決して実行されません。
  `--all` なしの非 TTY 呼び出しは安全にスキップされます。
- `--dry-run` モードでは（`--all` の有無にかかわらず）スナップショットが
  一覧表示されるのみで削除は行われません。

---

## 46. `sasurahime clean gem`

**カテゴリ:** Sprint 5

**削除対象:** [RubyGems](https://rubygems.org/) の古い gem バージョンを
`gem cleanup` で削除します。

**削除の仕組み:**
1. `gem cleanup` を実行し、`~/.gem/` にある古い gem バージョンを削除します。
2. 次回 `gem install` 時に最新バージョンが自動的に取得されます。

**安全性:** 公式の `gem` CLI に委譲します。Gem ソース自体には影響しません。

---

## 47. `sasurahime clean bundle`

**カテゴリ:** Sprint 5

**削除対象:** [Bundler](https://bundler.io/) のキャッシュを `bundle clean` で削除します。

**削除の仕組み:**
1. `bundle clean` を実行し、Bundler のキャッシュ済み gem ファイルを削除します。
2. 次回 `bundle install` 時に再ダウンロードされます。

**安全性:** 公式の `bundle` CLI に委譲します。`Gemfile.lock` は変更されません。

---

## 48. `sasurahime clean dotnet`

**カテゴリ:** Sprint 5

**削除対象:** [.NET SDK](https://dotnet.microsoft.com/) NuGet キャッシュを
`dotnet nuget locals all --clear` で削除します。

**削除の仕組み:**
1. `dotnet nuget locals all --clear` を実行し、全 NuGet キャッシュ
   (http-cache, packages-cache, temp, plugins-cache) を削除します。
2. 次回 `dotnet restore` 時に再ダウンロードされます。

**安全性:** 公式の `dotnet` CLI に委譲します。プロジェクト内の `packages/`
ディレクトリには影響しません。

---

## 49. `sasurahime clean orphan-containers`

**カテゴリ:** Sprint 6

**削除対象:** 対応するアプリがインストールされていない `~/Library/Containers` と
`~/Library/Group Containers` 配下のフォルダ。選択したフォルダはゴミ箱へ移動します
（`--permanent` を付けると完全削除します）。

**detect の動作:**
1. `/Applications`、`~/Applications`、`/System/Applications` の直下と、その 1 段下の
   サブフォルダ（例 `/Applications/Utilities`）にある `*.app` の `CFBundleIdentifier`
   を集めます。`Info.plist` を読めない .app は無視します。
2. `Containers` のフォルダは、名前がインストール済み ID と一致するか、そのドット区切り
   の子（`com.foo.App.Helper` は `com.foo.App` に属します。`com.foo.AppX` は属しません）
   であれば除外します。大文字小文字は区別しません。
3. `Group Containers` は、先頭のチーム ID（英数字大文字 10 文字 + `.`）と、続く
   `group.` / `groups.` を外してから同じ照合をします。あるチームのフォルダが 1 つでも
   インストール済みアプリと一致する場合、そのチームの全フォルダを除外します。
4. `com.apple.*`（`group.com.apple.*` を含む）と `com.docker.*` は報告しません。
5. 候補の合計サイズで `Pruneable` を返します。候補がなければ `Clean` です。
   読み取れないフォルダ（Full Disk Access なし）は数えません。

**clean の動作:**
1. `--dry-run` は候補を一覧表示するだけで、何も削除しません。
2. `--dry-run` なしでは `dialoguer::MultiSelect` を開きます。既定では何も選択されて
   いません。選択したフォルダだけをゴミ箱へ移動します。
3. `--all` はありません。非 TTY の場合と `--yes` では何も削除しません。
4. 読み取れなかったフォルダがあるときは、その件数と、ターミナルに Full Disk Access を
   付与すると全件を調べられる旨を表示します。sasurahime が権限を変更することはありません。

**制限:** Apple シリコンで動作する iOS アプリ（`Wrapper/*.app`）のコンテナは、
インストール済みアプリと対応付けられず候補に出ることがあります。選択前に一覧を確認してください。

**安全性:**
- 既定では何も選択されません。`--yes` ではこの cleaner は実行されません。
- `--dry-run` は副作用ゼロです。

---

## 50. `sasurahime clean library-caches`

**カテゴリ:** Sprint 6

**削除対象:** `~/Library/Caches/` 直下の既知アプリのキャッシュ (内蔵のアプリ表のうち基準ディレクトリが `Caches` のもの。例: `typescript`、`gopls`、`electron`、`Microsoft Edge`、`BraveSoftware`)。**対話的のみ。** 未知のアプリは一覧に表示しますが、削除はしません。

**`caches` との違い:** `sasurahime clean caches` は CLI ベースの cleaner (bun、go、pip、node-gyp、npm、yarn、pnpm) をまとめて実行します。`library-caches` は `~/Library/Caches/` 直下のアプリ別ディレクトリを直接扱います。

**detect の動作:**
1. `~/Library/Caches/` の直下を読み取ります。
2. ドットエントリ、シンボリックリンク、他の cleaner が担当するディレクトリ (下表) を除外します。サイズを測るのは既知アプリだけです。
3. 閾値 (64 MB) を超える既知アプリのキャッシュ合計を pruneable として報告します。未知アプリは合計に含めません。

**clean の動作:**
1. すべてのエントリをサイズ順に表示します。タグは `[running]`、`[below threshold]`、`[display only]` です。
2. `--dry-run`: 削除可能なエントリごとに `would remove:` を表示して終了します。
3. stdin が端末でない場合は、メッセージを表示して何も削除しません。
4. それ以外は `dialoguer::MultiSelect` を (何も選択しない状態で) 表示し、選択したものをゴミ箱へ移動します (`--permanent` では完全削除)。
5. 対象アプリが実行中のエントリはスキップして報告します。アプリの自動終了は行いません。

**除外 (他の cleaner が担当。一覧に出さない):**

| ディレクトリ | 担当 cleaner |
|---|---|
| `Homebrew` | `brew` |
| `ms-playwright*` | `browsers` |
| `node-gyp` | `node-gyp` |
| `org.swift.swiftpm` | `spm` |
| `JetBrains` | `jetbrains` |
| `CocoaPods` | `cocoa-pods` |
| `Yarn` | `yarn` |
| `pip` | `pip` |
| `go-build` | `go` |
| `pypoetry` | `poetry` |
| `deno` | `deno` |
| `com.apple.*` | OS 管理 |

**安全性:**
- 既定では何も選択されません。`--yes` ではこの cleaner は実行されません。
- 未知のアプリは表示のみです。
- `--dry-run` は副作用ゼロを保証します。

---

## 51. `sasurahime clean app-support`

**カテゴリ:** Sprint 6

**削除対象:** `~/Library/Application Support/` 下の既知アプリ(Slack、Claude
デスクトップ、Obsidian、VS Code、Google Chrome)のキャッシュ用サブディレクトリ。
固定の許可リスト(`Cache`、`Code Cache`、`GPUCache`、`Service Worker/CacheStorage`、
`logs` など)にあるパスだけが削除できます。設定、ログイン情報、IndexedDB、
Local Storage、ワークスペースには一切触れません。

**detect の動作:** アプリごとに許可リストのディレクトリ合計を測ります。合計が
64 MiB を超えたアプリだけを対象にします。後述の残骸一覧は合計に含めません。

**clean の動作:**
1. `--dry-run` は候補ディレクトリを表示して終了します。
2. それ以外は `dialoguer::MultiSelect` を開きます。既定は全部未選択です。
3. 実行中のアプリは、終了してよいか確認します。承認がなければそのアプリの
   ディレクトリはスキップします。sasurahime が終了させたアプリは、hint 機能で
   再起動が設定されているものに限り、削除後に再起動します。
4. 選択したディレクトリはゴミ箱へ移動します(`--permanent` では完全削除)。

**残骸の表示(表示のみ):** `Application Support` 直下のフォルダのうち、インストール済み
`.app` の名前にも bundle ID の末尾要素にも一致しないものを一覧にします。削除は
しません。選択したものを Finder で開きます。6 件以上を選んだ場合は、ウィンドウを
開く前に確認します。

1 MiB 未満のもの、直近 30 日以内に更新されたもの、シンボリックリンク、Apple 製のフォルダ、
ベンダー親フォルダ(`Google`、`Microsoft`、`Adobe`、`JetBrains` など)は一覧に出しません。

**安全性:**
- 許可リストのみ。`..`、絶対パス、シンボリックリンクは拒否します。
- `--yes` では決して実行しません。非 TTY 呼び出しは何もしません。
- `--dry-run` は副作用ゼロです。

---

## 52. `sasurahime stats`

**カテゴリ:** Sprint 5

**機能:** 削除履歴と統計を表示します。

**動作:**
1. `sasurahime clean <target>` がバイト数を解放するたびに、`~/.local/share/sasurahime/history.json` に履歴を追記します（`--dry-run` は除く）。
2. 各エントリには `timestamp`、`cleaner`、`freed_bytes`、`skipped_count` が含まれます。
3. ファイルはアトミック書き込み（一時ファイル + リネーム）で更新されるため、破損しにくい設計です。
4. `sasurahime stats` はファイルを読み込み、集計結果を表示します。
5. 履歴ファイルがない場合や壊れている場合でも、親切なメッセージとともに終了コード 0 で正常終了します。

**オプション:**
- `--last N` ― 直近 N 件のみ表示（デフォルト: 10）。

**出力例:**
```
$ sasurahime stats
Total freed:  12.5 GB
Runs:         15

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-27 10:30   uv             500.0 MB
  2  2026-05-26 22:15   brew           1.2 GB
```

**実装:** `src/history.rs`（362行）、9 unit tests + 6 E2E tests。

---

## 設定ファイル: `~/.config/sasurahime/config.toml`

**カテゴリ:** Sprint 5

設定ファイルは必須ではありません。存在しない場合は適切なデフォルト値が使われます。

**設定例:**
```toml
# スキャン/TUI から除外するクリーナー
exclude = ["huggingface", "ollama"]

# ユーザー定義のキャッシュディレクトリ
[[custom]]
name = "my-project"
path = "~/work/.cache"

# クリーナーごとのフィルタ（DeleteDirs のみ有効）
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

# ログ保持期間
[logs]
keep_days = 14
```

**設定フィールド:**

| フィールド | 型 | デフォルト | 説明 |
|-----------|-----|-----------|------|
| `trash_mode` | boolean | `true` | 削除ファイルをデフォルトでゴミ箱へ |
| `exclude` | string[] | `[]` | スキャン/TUI から除外するクリーナー |
| `[[custom]]` | table[] | `[]` | ユーザー定義キャッシュディレクトリ |
| `custom[].name` | string | 必須 | 表示名 |
| `custom[].path` | string | 必須 | パス（`~` 展開対応） |
| `[cleaner.<name>]` | table | ― | クリーナーごとのフィルタ設定 |
| `cleaner.<name>.older_than_days` | integer | 未設定 | N 日より古いエントリのみ削除 |
| `cleaner.<name>.larger_than_mb` | integer | 未設定 | N MB より大きいエントリのみ削除 |
| `[logs]` | table | ― | ログ保持設定 |
| `logs.keep_days` | integer | `7` | グローバルログ保持期間 |
| `[[logs.targets]]` | table[] | `[]` | 追加ログディレクトリ |
| `logs.targets[].name` | string | 必須 | 表示名 |
| `logs.targets[].path` | string | 必須 | パス（`~` 展開対応） |
| `logs.targets[].exclude` | string[] | `[]` | 削除しないファイル名 |

**CLI フラグ:**
```bash
sasurahime scan --config /path/to/config.toml
```

---

## スキャン (`sasurahime scan`)

すべてのクリーナーで `detect()` を実行し、`comfy_table` を使ってフォーマットされたテーブルを表示します。行はサイズの大きい順に並び、サイズのない対象（clean または not found）は末尾に表示します。副作用はありません ― ファイルの作成、変更、削除は一切行いません。

---

## インタラクティブ / 自動モード

| モード | 動作 |
|--------|------|
| `sasurahime`（引数なし、TTY） | インタラクティブなチェックボックスリストを起動します。ターゲットを選択して確認すると、選択したターゲットがクリーンされます。 |
| `sasurahime --yes`（引数なし、任意） | 無人実行の対象 (uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support) のうち pruneable なものを確認なしで削除します。それ以外のターゲット (docker, npm, cargo, downloads など) は `scan` と対話リストには表示されますが、`--yes` では実行されません。何も見つからなければ 0 で終了します。 |
| `sasurahime scan`（非 TTY） | スキャン結果の一覧のみを表示します。 |
| `sasurahime clean <target>` | 特定のターゲットを直接クリーンします。 |


</div>

</details>
