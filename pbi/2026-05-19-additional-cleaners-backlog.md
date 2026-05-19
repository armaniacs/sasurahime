# PBI: Additional Cleaners Backlog（中優先度・低優先度）

調査で発見した追加クリーナー候補の一覧。実装優先度ごとにまとめる。
araki 環境での実測サイズは「(未確認)」と記載（ツール未インストール）。

---

## 中優先度

macOS 開発者に広く該当するが、ツールが入っていないマシンも多い。

### simulator — iOS/watchOS Simulator キャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/Library/Developer/CoreSimulator/Caches/` `~/Library/Developer/CoreSimulator/Devices/` |
| クリーン方法 | `xcrun simctl delete unavailable` |
| 実測サイズ | (未確認) ／ 典型的に数GB |
| 依存 | Xcode インストール必須 |

**BDD シナリオ（概要）**
- scan: Caches + 未使用 Devices のサイズを合算して表示
- clean: `xcrun simctl delete unavailable` を実行し、解放サイズを報告
- Xcode 未インストールの場合は `NotFound` を返す

**実装ノート**
- `xcrun simctl list devices` でデバイス一覧を取得し、unavailable なものを特定
- Devices 削除は `simctl delete` が担当するため直接 rm しない

---

### xcode-device-support — Xcode デバイスシンボル

| 項目 | 内容 |
|------|------|
| パス | `~/Library/Developer/Xcode/{iOS,watchOS,visionOS,tvOS} DeviceSupport/` |
| クリーン方法 | 最新N世代を残して古いOSバージョンのディレクトリを削除 |
| 実測サイズ | (未確認) ／ バージョンあたり 1〜3 GB |
| 依存 | Xcode インストール、実機接続履歴 |

**BDD シナリオ（概要）**
- scan: 各プラットフォームの DeviceSupport ディレクトリ合計サイズを表示
- clean: デフォルトで最新2世代を保持し、それより古いものを削除
- `--keep N` オプションで保持世代数を指定可能

**実装ノート**
- ディレクトリ名は `"16.4 (20E247)"` 形式。メジャーバージョンでソートして末尾N件を保持
- 削除前に `is_xcode_running()` チェック（既存パターン流用）

---

### maven — Maven ローカルリポジトリ

| 項目 | 内容 |
|------|------|
| パス | `~/.m2/repository/` |
| クリーン方法 | `mvn dependency:purge-local-repository` または直接削除 |
| 実測サイズ | (未確認) ／ 典型的に数百MB〜数GB |
| 依存 | Java/Maven プロジェクトを触る場合のみ |

**BDD シナリオ（概要）**
- scan: `~/.m2/repository/` のサイズを表示
- clean: `mvn` が PATH にあれば `purge-local-repository` を実行、なければ直接削除
- Maven 未インストールの場合は直接削除にフォールバック

**実装ノート**
- `GenericCleaner` パターンで実装可能
- `~/.m2/settings.xml` の `<localRepository>` タグでパスが変更されている場合に対応（XML パース不要、grep で抽出）

---

### ollama — Ollama モデルキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.ollama/models/` |
| クリーン方法 | `ollama list` で確認後、`ollama rm <model>` で個別削除 |
| 実測サイズ | (未確認) ／ モデルあたり 1〜70 GB |
| 依存 | Ollama インストール・使用歴 |

**BDD シナリオ（概要）**
- scan: `~/.ollama/models/` のサイズを表示
- clean: `ollama list` でモデル一覧を取得し、対話的に削除対象を選択（TUI）
- `--all` フラグで全モデル削除

**実装ノート**
- `ollama rm` は安全に動作する（実行中のモデルは削除不可）ため外部 CLI 委任が安全
- サイズが非常に大きくなりうるため scan での視認性が特に重要
- TUI モード（PBI-008）と連携して対話的に削除対象を選ばせる設計が望ましい

---

### terraform — Terraform プロバイダープラグインキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.terraform.d/plugin-cache/` |
| クリーン方法 | ディレクトリ直接削除（`terraform init` で再ダウンロード） |
| 実測サイズ | (未確認) ／ 典型的に数百MB |
| 依存 | Terraform 使用歴 |

**BDD シナリオ（概要）**
- scan: `~/.terraform.d/plugin-cache/` のサイズを表示
- clean: `plugin-cache/` 以下を削除（`.terraform.d/` 自体は残す）
- ディレクトリが存在しない場合は `NotFound` を返す

**実装ノート**
- `TF_PLUGIN_CACHE_DIR` 環境変数でパスが変更される場合に対応
- `GenericCleaner` パターンで実装可能（外部 CLI 不要）

---

### flutter — Flutter/Dart パッケージキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.pub-cache/` |
| クリーン方法 | `dart pub cache clean` |
| 実測サイズ | (未確認) ／ 典型的に数百MB |
| 依存 | Flutter/Dart SDK インストール |

**BDD シナリオ（概要）**
- scan: `~/.pub-cache/` のサイズを表示
- clean: `dart pub cache clean` を実行。`dart` が PATH にない場合は直接削除
- `flutter pub cache repair` との混同に注意（clean が正しいコマンド）

**実装ノート**
- `$PUB_CACHE` 環境変数でパスが変更される場合に対応
- `GenericCleaner` パターンで実装可能

---

## 低優先度

ニッチなツールまたはサイズが小さく ROI が低い。実装は後回し。

### volta — Volta Node.js バージョンマネージャキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.volta/cache/` |
| クリーン方法 | ディレクトリ直接削除（Volta が再ダウンロード） |
| 実測サイズ | (未確認) |
| 備考 | nvm/fnm と競合しないため使用率は低め |

---

### sbt — Scala/sbt ビルドキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.sbt/` `~/.ivy2/cache/` |
| クリーン方法 | `sbt clean` または直接削除 |
| 実測サイズ | (未確認) |
| 備考 | Scala 開発者以外には不要。Maven と同一 JVM エコシステム |

---

### tree-sitter — tree-sitter パーサーコンパイルキャッシュ

| 項目 | 内容 |
|------|------|
| パス | `~/.cache/tree-sitter/` |
| クリーン方法 | ディレクトリ直接削除 |
| 実測サイズ | araki 環境: **2 MB**（ROI 低） |
| 備考 | Neovim/Helix ユーザーが対象。サイズが小さすぎて独立 PBI には不向き |

**実装ノート**: 独立ターゲットではなく `logs` や将来の `editor-caches` にまとめる案もある。

---

### vscode-logs — VSCode ログファイル

| 項目 | 内容 |
|------|------|
| パス | `~/Library/Application Support/Code/logs/` |
| クリーン方法 | `logs/` 以下を直接削除 |
| 実測サイズ | araki 環境: **23 MB** |
| 備考 | 既存の `LogCleaner`（PBI-007）を拡張する形で実装できる |

**実装ノート**: 独立ターゲットではなく PBI-007 の `logs` ターゲットに VSCode・Zed などのエディタログを追加するオプションとして実装するのが自然。

---

## コリマの発見と追加（2026-05-20）

Colima VM ディスクキャッシュ（`~/.colima/_lima/`）が 9.3 GB 使用中であることが判明した。
`colima prune --all` で安全に削除可能であり、PBI を作成した。

- PBI: `pbi/2026-05-20-colima-cleaner.md`
- 状態: 実装計画済み
- パターン: `GenericCleaner` + `CleanMethod::CommandWithDetectDir`（新規 variant）

## 実装順序の推奨（2026-05-20 更新）

```
実装予定:
1. Colima           ← 実測 9.3 GB、PBI 作成済み、実装計画済み
   - PBI: pbi/2026-05-20-colima-cleaner.md
   - パターン: GenericCleaner + CleanMethod::CommandWithDetectDir

次候補（環境依存）:
2. ollama           ← 最大 70 GB/モデル、影響大。library-logs と同様の対話的選択パターン
3. simulator        ← xcrun simctl 一発、シンプル
4. xcode-device-support ← 世代管理、browsers パターン

簡易実装（環境にあれば）:
5. maven            ← GenericCleaner CLI + DeleteDirs（CLI+fallback パターン）
6. terraform        ← GenericCleaner DeleteDirs（要 $TF_PLUGIN_CACHE_DIR 対応）
7. flutter          ← GenericCleaner CLI + fallback（要 $PUB_CACHE 対応）

低優先度（保留）:
- volta / sbt     → ユーザー要望があれば対応
- tree-sitter     → 2MB のみ。editor-caches としてまとめて実装
- vscode-logs     → PBI-007 の拡張として実装（24MB）
```
