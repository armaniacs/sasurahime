# sasurahime — 対応ターゲット一覧

sasurahime は **32 のクリーンターゲット** を提供します。
全ターゲットは `detect`（読み取り専用、副作用なし）と
`clean`（削除）の両方をサポートし、全ての `clean` サブコマンドに
`--dry-run` が使用できます。

---

## 1. `sasurahime clean uv`

**カテゴリ:** Sprint 1

**削除対象:** `~/.cache/uv/` 内の古い `simple-vN` インデックスディレクトリ
および `uv cache prune --force` の実行。

**detect の動作:**
1. `~/.cache/uv/` のエントリを読み取る。
2. `simple-v<N>` のパターン（例: `simple-v16`, `simple-v21`）に一致する
   名前をフィルタリング。シンボリックリンクはスキップ。
3. 全バージョン番号を収集し、最大値を見つける。
4. 最大値 **以外** の全ディレクトリの合計サイズを報告する。

**clean の動作:**
1. `uv cache prune --force` を実行。
2. 最大バージョン **以外** の全 `simple-vN` ディレクトリを
   `fs::remove_dir_all` で削除。
3. `--dry-run` 時は削除予定のリストを表示し、削除は行わない。

**安全性:** 最も新しい `simple-v<N>` は常に保持されます。

---

## 2. `sasurahime clean brew`

**カテゴリ:** Sprint 1

**削除対象:** Homebrew ダウンロードキャッシュ。
`brew cleanup -s --prune=all` に委譲。

**detect の動作:**
`~/Library/Caches/Homebrew` が存在すれば、その合計サイズを報告する。

**clean の動作:**
1. `brew` が `PATH` にない場合はスキップ（終了コード 0）。
2. `brew cleanup -s --prune=all` を実行。
3. brew の出力（`"freed approximately <N>GB of disk space"`）から
   解放サイズをパース（大文字小文字とスペース区切りに対応）。
4. 解放サイズを報告。

**安全性:** Homebrew CLI 自身が安全に処理します。

---

## 3. `sasurahime clean mise`

**カテゴリ:** Sprint 2

**削除対象:** [mise](https://mise.jdx.dev/) でインストールされた未使用の
ランタイムバージョン（`~/.local/share/mise/installs/<tool>/<version>`）。

**detect の動作:**
1. `mise ls --current` を実行し、現在アクティブな `(tool, version)` ペア
   を取得。
2. `~/.config/mise/config.toml` および HOME 直下の全 `.mise.toml`
   （深さ 5 まで）をスキャンし、ピン留めされたペアを収集。
3. `installs/` ディレクトリツリーを読み取り、アクティブでもピン留め
   でもないバージョンを未使用と判定。

**clean の動作:**
1. 上記と同じ active + pinned 検出。
2. 未使用の各 `(tool, version, path)` に対し:
   - **dry-run:** `[dry-run] would remove: <tool> <version>` を表示。
   - **実削除:** `remove_with_uchg` を実行:
     1. `chflags -R nouchg <path>` でイミュータブルフラグを解除。
     2. `fs::remove_dir_all` を実行。
     3. `chflags` が失敗した場合はエラーを伝播（握り潰さない）。

**安全性（CLAUDE.md §Safety rules 準拠）:**
- `~/.config/mise/config.toml` と HOME 内の全 `.mise.toml`（深さ 5）を
   削除前に横断チェック。
- ピン留めされたバージョンはアクティブでなくても **絶対に削除しない**。
- macOS イミュータブルフラグは `chflags -R nouchg` で自動処理。

---

## 4. `sasurahime clean browsers`

**カテゴリ:** Sprint 2

**削除対象:** [Puppeteer](https://pptr.dev/) と
[Playwright](https://playwright.dev/) の古いブラウザビルド。
ブラウザファミリごとに最新バージョンのみ保持。

**スキャン対象:**

| ラベル                     | パス                                  |
|---------------------------|---------------------------------------|
| puppeteer/chrome           | `~/.cache/puppeteer/chrome`           |
| puppeteer/chrome-headless-shell | `~/.cache/puppeteer/chrome-headless-shell` |
| ms-playwright              | `~/Library/Caches/ms-playwright`      |
| ms-playwright-go           | `~/Library/Caches/ms-playwright-go`   |

**バージョン比較:**
- `BrowserCleaner::version_key` がディレクトリ名から `Vec<u32>` に変換。
- 例: `mac_arm-131.0.6778.204` → `[131, 0, 6778, 204]`、
  `chromium-1208` → `[1208]`。
- Rust 標準の `Vec<u32>::cmp` による辞書順比較。

**clean の動作:**
- グループごとに `find_old_versions` を呼び出し、最新バージョン以外を
  `fs::remove_dir_all` で削除。

**安全性:**
- 最新バージョン（最も新しいブラウザバイナリ）は **常に** 保持。
- シンボリックリンクはスキップ（GAP-005）。
- パース不能なディレクトリ名（例: `nightly`）はスキップ。

---

## 5. `sasurahime clean bun`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [Bun](https://bun.sh/) パッケージキャッシュ。

**方法:** `bun pm cache rm`

**detect:** `bun` が `PATH` にあれば pruneable と報告。

**clean:** `bun` がない場合はスキップ。`bun pm cache rm` を実行。

**安全性:** 公式 `bun` CLI に委譲。

---

## 6. `sasurahime clean go`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [Go](https://go.dev/) ビルドキャッシュ。

**方法:** `go clean -cache`

**detect:** `go` が `PATH` にあれば pruneable と報告。

**clean:** `go` がない場合はスキップ。`go clean -cache` を実行。

**安全性:** 公式 `go` CLI に委譲。

---

## 7. `sasurahime clean pip`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [pip](https://pip.pypa.io/) パッケージキャッシュ。

**方法:** `pip cache purge`

**detect:** `pip` が `PATH` にあれば pruneable と報告。

**clean:** `pip` がない場合はスキップ。`pip cache purge` を実行。

**安全性:** 公式 `pip` CLI に委譲。

---

## 8. `sasurahime clean node-gyp`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [node-gyp](https://github.com/nodejs/node-gyp) ビルド
キャッシュディレクトリ。

**スキャン対象:**
- `~/.cache/node-gyp/`
- `~/Library/Caches/node-gyp/`

**detect:** 存在するディレクトリの合計サイズを報告。

**clean:** `chflags -R nouchg` でイミュータブルフラグを解除後、
`fs::remove_dir_all` を実行（GAP-010）。

**安全性:** macOS `uchg` フラグを自動処理。

---

## 9. `sasurahime clean npm`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [npm](https://www.npmjs.com/) パッケージキャッシュ。

**方法:** `npm cache clean --force`

**detect:** `npm` が `PATH` にあれば pruneable と報告。

**clean:** `npm` がない場合はスキップ。`npm cache clean --force` を実行。

**安全性:** 公式 `npm` CLI に委譲。

---

## 10. `sasurahime clean yarn`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [Yarn](https://yarnpkg.com/) パッケージキャッシュ。

**方法:** `yarn cache clean`

**detect:** `yarn` が `PATH` にあれば pruneable と報告。

**clean:** `yarn` がない場合はスキップ。`yarn cache clean` を実行。

**安全性:** 公式 `yarn` CLI に委譲。

---

## 11. `sasurahime clean pnpm`

**カテゴリ:** Sprint 3 — 汎用キャッシュ

**削除対象:** [pnpm](https://pnpm.io/) ストア。

**方法:** `pnpm store prune`

**detect:** `pnpm` が `PATH` にあれば pruneable と報告。

**clean:** `pnpm` がない場合はスキップ。`pnpm store prune` を実行。

**安全性:** 公式 `pnpm` CLI に委譲。

---

## 12. `sasurahime clean caches`

**カテゴリ:** Sprint 3 — 汎用キャッシュ（一括）

**削除対象:** 全ての汎用キャッシュを一度に削除。
`bun` + `go` + `pip` + `node-gyp` + `npm` + `yarn` + `pnpm` を順次実行。

各サブクリーナーは独立して動作します。インストールされていないツールは
スキップされ、終了コードは 0 です。

---

## 13. `sasurahime clean logs`

**カテゴリ:** Sprint 3

**削除対象:** 既知およびユーザー設定のログディレクトリ内の、
`N` 日より古いログファイル。

**内蔵ログターゲット（自動スキャン）:**

| 名称         | パス                              | 削除除外ファイル    |
|--------------|-----------------------------------|-------------------|
| kilo         | `~/.local/share/kilo/log`         | `dev.log`         |
| opencode     | `~/.local/share/opencode/logs`    | なし               |
| claude-code  | `~/.local/share/claude/logs`      | なし               |

**追加ターゲット** は設定ファイル（`config.toml`）で指定可能。

**detect / clean の動作:**
1. 全ターゲットを反復。
2. `find_old_logs(dir, keep_days, exclude)` を呼び出し、各ターゲットの
   保持期間を超えたファイルを収集。
3. ファイルは `fs::remove_file` で削除。
   (`Dry-run` 時は削除予定リストを表示)

**保持ポリシー:**
- デフォルト: `keep_days = 7`（7日より古いファイルを削除）。
- `--keep-days <N>` フラグで上書き可能。
- 設定ファイル `[logs]\nkeep_days = <N>` でも設定可能。
- CLI フラグが設定より優先。

**安全性:**
- 除外リストに含まれるファイルは **絶対に削除しない**。
- 内蔵の `kilo` ターゲットはデフォルトで `dev.log` を除外。
- 日数比較は `>`（超過）を使用。正確に `N` 日経過したファイルは
   削除 **されない**。
- メタデータが読めないファイルはスキップ。

---

## 14. `sasurahime clean xcode`

**カテゴリ:** Sprint 3

**削除対象:** Xcode DerivedData フォルダ
（`~/Library/Developer/Xcode/DerivedData/`）内のプロジェクトビルド
ディレクトリ。

**detect:**
`DerivedData` が存在すれば合計サイズを報告。

**clean:**
1. `DerivedData` が存在しなければスキップ。
2. Xcode が実行中の場合は警告を表示し、確認を求める。
   `--yes` モードでは確認はスキップ。
3. `DerivedData` 直下のプロジェクトディレクトリを `fs::remove_dir_all`
   で削除。`DerivedData` ルート自体は **決して削除しない**。

**安全性:**
- `DerivedData` ルートは **絶対に削除しない**。
- Xcode 実行中は確認を求める（または中断）。
- ルート直下のサブディレクトリのみを削除対象とする。

---

## 15. `sasurahime clean act`

**カテゴリ:** Sprint 5

**削除対象:** [act](https://github.com/nektos/act) GitHub Actions ローカル
ランナーキャッシュ（`~/.cache/act/`、または `$ACT_CACHE_DIR` が設定されていればそのパス）。

**方法:** ディレクトリ削除。

**detect:**
キャッシュディレクトリが存在すれば合計サイズを報告。

**clean:**
1. `is_safe_delete_target` でパスが安全か検証。
2. `$ACT_CACHE_DIR` が安全でない場合はデフォルトにフォールバック。
3. `fs::remove_dir_all` で削除。

**安全性:** `$ACT_CACHE_DIR` はシステムパスのブロックリストで検証。

---

## 16. `sasurahime clean cargo`

**カテゴリ:** Sprint 5

**削除対象:** [Cargo](https://doc.rust-lang.org/cargo/) レジストリキャッシュ
（`~/.cargo/registry/cache/`）および `target/` ビルドアーティファクト。

**clean:**
1. `~/.cargo/registry/cache/` を削除。
2. プロジェクトの `target/` ディレクトリをスキャンして削除。
3. `chflags -R nouchg` を実行後に削除。

**安全性:** ダウンロード済みキャッシュのみ削除。ソースコードには影響なし。

---

## 17. `sasurahime clean cocoa-pods`

**カテゴリ:** Sprint 5

**削除対象:** [CocoaPods](https://cocoapods.org/) キャッシュ。

**方法:** `pod cache clean --all`

**detect:** `pod` が `PATH` にあれば pruneable と報告。

**clean:** `pod` がない場合はスキップ。`pod cache clean --all` を実行。

**安全性:** 公式 CocoaPods CLI に委譲。

---

## 18. `sasurahime clean conda`

**カテゴリ:** Sprint 5

**削除対象:** [Conda](https://docs.conda.io/) パッケージキャッシュ。

**方法:** `conda clean --all -y`

**detect:** `conda` が `PATH` にあれば pruneable と報告。

**clean:** `conda` がない場合はスキップ。`conda clean --all -y` を実行。

**安全性:** 公式 Conda CLI に委譲。

---

## 19. `sasurahime clean deno`

**カテゴリ:** Sprint 5

**削除対象:** [Deno](https://deno.com/) キャッシュ。

**方法:** `deno cache -r`

**detect:** `deno` が `PATH` にあれば pruneable と報告。

**clean:** `deno` がない場合はスキップ。`deno cache -r` を実行。

**安全性:** 公式 Deno CLI に委譲。

---

## 20. `sasurahime clean docker`

**カテゴリ:** Sprint 5

**削除対象:** [Docker](https://www.docker.com/) の dangling イメージ、
コンテナ、ビルドキャッシュ、ネットワーク。

**方法:** `docker system prune -f`

**detect:** `docker` が `PATH` にあれば pruneable と報告。

**clean:** `docker` がない場合はスキップ。`docker system prune -f` を実行。

**安全性:** dangling イメージのみ削除。タグ付きイメージは保持。`-a` は不使用。

---

## 21. `sasurahime clean downloads`

**カテゴリ:** Sprint 5

**削除対象:** `~/Downloads/` 内の古いファイル。

**detect:** `~/Downloads` が存在すれば合計サイズを報告。

**clean:**
1. `~/Downloads` 直下のアイテムを列挙。
2. `chflags -R nouchg` 後に `remove_dir_all` で削除。
3. 直下のアイテムのみが対象（再帰的削除は行わない）。

**安全性:** `~/Downloads` 直下のみ操作。システムファイルには影響なし。

---

## 22. `sasurahime clean gradle`

**カテゴリ:** Sprint 5

**削除対象:** [Gradle](https://gradle.org/) の古いバージョンキャッシュ
（`~/.gradle/caches/`）。各キャッシュグループの最新版のみ保持。

**detect:**
`~/.gradle/caches/` をスキャンし、古いバージョンの合計サイズを報告。

**clean:**
1. バージョン比較で古いバージョンを特定。
2. `chflags -R nouchg` + `remove_dir_all` で削除。

**安全性:** 各アーティファクトの最新バージョンは常に保持。

---

## 23. `sasurahime clean huggingface`

**カテゴリ:** Sprint 5

**削除対象:** [Hugging Face](https://huggingface.co/) モデルキャッシュ
（`~/.cache/huggingface/hub/` または `$HF_HOME/hub`）。

**detect:**
`hub/` ディレクトリが存在すれば合計サイズを報告。

**clean:**
1. CLI 優先: `huggingface-cli delete-cache --yes` を実行。
2. CLI がない場合は `hub/` の中身を直接削除後にディレクトリを再作成。
3. `$HF_HOME` は `is_safe_delete_target` で検証。

**安全性:** 環境変数のパスはブロックリストで検証。CLI 優先で安全。

---

## 24. `sasurahime clean jetbrains`

**カテゴリ:** Sprint 5

**削除対象:** [JetBrains IDE](https://www.jetbrains.com/) の古いキャッシュ
（IntelliJ IDEA、WebStorm 等、`~/Library/Caches/JetBrains/` 内）。

**detect:**
`~/Library/Caches/JetBrains/` をスキャンし、古い IDE バージョンの
キャッシュサイズを報告。

**clean:**
1. IDE ファミリごとに古いバージョンを特定。
2. `chflags -R nouchg` + `remove_dir_all` で削除。

**安全性:** 各 IDE の最新キャッシュは常に保持。

---

## 25. `sasurahime clean library-logs`

**カテゴリ:** Sprint 5

**削除対象:** `~/Library/Logs/` 内のユーザーログ。ヒューリスティックルールで
削除候補を提示し、**対話的に選択**できます（`--all` で確認スキップ）。

**detect:**
1. `~/Library/Logs/` 直下をスキャン。
2. 各エントリに 2 つのルールを適用:
   - **大容量:** サイズ > 100 MB → `[large]` タグ
   - **経過:** 最終更新 > 90 日前 → `[stale N days]` タグ
3. 少なくとも 1 つのルールに合致したエントリのみ報告。
4. `CrashReporter`、`DiagnosticReports`、ドットエントリは除外。

**clean:**
1. 上記と同じスキャンを実行。
2. **`--dry-run`:** 各エントリを理由タグ付きで表示。
3. **`--all`:** 全候補を確認なしで削除。
4. **デフォルト:** `dialoguer::MultiSelect` で選択式削除。

**安全性:**
- `CrashReporter` / `DiagnosticReports` は **常に** 除外。
- ドットファイル（`.DS_Store` 等）はスキップ。
- 未来のタイムスタンプは現在時刻にクランプ。
- `--dry-run` は副作用ゼロ。

---

## 26. `sasurahime clean orbstack`

**カテゴリ:** Sprint 5

**削除対象:** [Orbstack](https://orbstack.dev/) Docker ランタイムキャッシュ。

**方法:** `orb prune`

**detect:** `orb` が `PATH` にあれば pruneable と報告。

**clean:** `orb` がない場合はスキップ。`orb prune` を実行。

**安全性:** 公式 Orbstack CLI に委譲。

---

## 27. `sasurahime clean pipx`

**カテゴリ:** Sprint 5

**削除対象:** [pipx](https://pypa.github.io/pipx/) キャッシュと未使用パッケージ。

**方法:** `pipx cache purge`

**detect:** `pipx` が `PATH` にあれば pruneable と報告。

**clean:** `pipx` がない場合はスキップ。`pipx cache purge` を実行。

**安全性:** 公式 pipx CLI に委譲。

---

## 28. `sasurahime clean poetry`

**カテゴリ:** Sprint 5

**削除対象:** [Poetry](https://python-poetry.org/) パッケージキャッシュ。

**方法:** `poetry cache clear --all`

**detect:** `poetry` が `PATH` にあれば pruneable と報告。

**clean:** `poetry` がない場合はスキップ。`poetry cache clear --all` を実行。

**安全性:** 公式 Poetry CLI に委譲。

---

## 29. `sasurahime clean pre-commit`

**カテゴリ:** Sprint 5

**削除対象:** [pre-commit](https://pre-commit.com/) hook 環境キャッシュ
（`~/.cache/pre-commit/`、`$PRE_COMMIT_HOME`、または `$XDG_CACHE_HOME/pre-commit`）。

**detect:**
環境変数からキャッシュパスを解決し、合計サイズを報告。

**clean:**
1. CLI 優先: `pre-commit clean` を実行。
2. CLI がない場合はキャッシュディレクトリを直接削除。
3. 環境変数のパスは `is_safe_delete_target` で検証。

**安全性:** 環境変数のパスはブロックリストで検証。CLI 優先。

---

## 30. `sasurahime clean rustup`

**カテゴリ:** Sprint 5

**削除対象:** 未使用の [Rust](https://www.rust-lang.org/) ツールチェーン
（`rustup default` や `rustup override` で選択されていないもの）。

**detect:**
1. `rustup toolchain list` を実行。
2. `(default)` と `(override)` マークのないツールチェーンを未使用と判定。

**clean:**
1. 各未使用ツールチェーンに対して `rustup toolchain remove <name>` を実行。
2. 解放サイズを報告。

**安全性:** デフォルトとオーバーライドのツールチェーンは **絶対に削除しない**。

---

## 31. `sasurahime clean spm`

**カテゴリ:** Sprint 5

**削除対象:** [Swift Package Manager](https://www.swift.org/package-manager/)
のビルドアーティファクトとキャッシュパッケージ。

**削除パス:**
- `~/Library/Caches/org.swift.swiftpm/`
- `~/Library/Developer/Xcode/DerivedData/SourcePackages/`

**clean:**
1. `chflags -R nouchg` 後に `remove_dir_all` を実行。
2. 次回ビルド時にパッケージは再取得される。

**安全性:** キャッシュのみ削除。ソースコードへの影響なし。

---

## 32. `sasurahime clean trash`

**カテゴリ:** Sprint 5

**削除対象:** `~/.Trash` — **スキャンのみ**。サイズは報告しますが、
削除は行いません（Finder で空にするよう案内）。

**detect:** `~/.Trash` が存在すれば合計サイズを報告。

**clean:**
1. **`--dry-run`:** 解放可能なサイズを表示。
2. **実削除:** Finder で空にするよう警告メッセージを表示。削除は行わない。

**安全性:** sasurahime は `~/.Trash` の削除を拒否します。
これは意図的な安全措置です。

---

## スキャン (`sasurahime scan`)

全クリーナーの `detect()` を実行し、`comfy_table` で整形した表を出力。
副作用は一切ありません。

---

## インタラクティブ / 自動モード

| モード | 動作 |
|--------|------|
| `sasurahime`（引数なし, TTY） | `dialoguer::MultiSelect` で選択、確認後に削除。 |
| `sasurahime --yes`（引数なし） | 確認なしで全 pruneable ターゲットを削除。 |
| `sasurahime scan`（非 TTY） | スキャン表のみ出力。 |
| `sasurahime clean <target>` | 特定ターゲットを直接削除。 |
