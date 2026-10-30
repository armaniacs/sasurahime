# PBI-79: installed_apps module を切り出して「インストール済みアプリ」に home を与える

出典: /improve-codebase-architecture ラウンド 2 候補 6（深掘り 2026-10-30）

## ユーザーストーリー

As a sasurahime ユーザー
I want 「このマシンに何がインストールされているか」の判定が src/installed_apps.rs 1 本に集約され、/Applications/Utilities と /System/Applications/Utilities の 2 階層ネストまで列挙されること
so that インストール済みのアプリのコンテナが誤って orphan と報告されて受け入れで削除される事故がなくなる

## 優先度

RICE: Reach 2 × Impact 1.5 × Confidence 85% / Effort 0.75 人週 = スコア 3.4（本ラウンド 3 位）

根拠:
- 実害トリガーは狭い: /Applications/Utilities/<Folder>/Foo.app と /System/Applications/Utilities/<Folder>/Foo.app の 2 階層ネストにサードパーティアプリを置いている環境でのみ false orphan（受け入れで削除）が発生し得る。1 階層の /Applications/Utilities/Foo.app は list_app_bundles が 1 サブフォルダを辿るため現行でも列挙され、影響外
- ただし破壊的な削除に直結する誤判定の温床であり、正 canonical な root list の手維持コピーが 2 本あり静かにドリフトし続けている（README に対して orphan 側は狭く、app_support 側はわずかに広い）
- 相互参照の module cycle（app_support.rs:837 → orphan_containers::collect_app_identities、orphan_containers.rs:211 → app_support::list_app_bundles）の解消はテスタビリティと AI-navigability にも効く

## BDD 受け入れシナリオ

Scenario: Utilities 2 階層ネストのサードパーティアプリは orphan と報告されない
  Given /Applications/Utilities/Adobe Installers/X.app がインストール済みで、そのコンテナが存在する
  When orphan 検出を実行する
  Then そのコンテナは orphan として報告されない

Scenario: Utilities 1 階層のアプリは従来どおりインストール済みと判定される
  Given /Applications/Utilities/Foo.app が存在し、そのコンテナが存在する
  When orphan 検出を実行する
  Then そのアプリはインストール済みと判定される（既存の traversal 境界 pin を維持）

Scenario: 同一 bundle path で plutil は 1 回だけ実行される
  Given /Applications/Utilities/Foo.app を app_support と orphan の両方が列挙する状態がある
  When インストール済みアプリの identity を読む
  Then plutil は bundle path ごとに 1 回だけ実行される

Scenario: CI 環境では canonical root list がトリムされる
  Given 環境変数 CI が設定されている
  When default_app_roots(home) を呼ぶ
  Then canonical root list は home/Applications のみにトリムされる

Scenario: README と canonical root list の乖離は pin テストで検出される
  Given README の root list 宣言と installed_apps の default_app_roots が一致しなくなった
  When pin テストを実行する
  Then pin テストが落ちる

## 受け入れ基準

- [ ] src/installed_apps.rs が新設され、「.app 列挙 → identity 読み → インストール済み集合 + 正 canonical root list」を 1 本に所有する
- [ ] AppIdentity { bundle_id, name, display_name } + PLIST_KEY_* + xml_string_value + parse_app_identity_from_xml + read_app_identity を orphan_containers.rs:140-198 から移動する（README promise コメントも逐語移動）
- [ ] is_app_bundle / list_app_bundles / app_bundle_stems を app_support.rs:783-830 から移動する
- [ ] collect_app_identities / collect_installed_ids を orphan_containers.rs:205-222 から移動する
- [ ] default_app_roots(home) を 2 本の list を統合した正 canonical にする。app_support 側の上位集合（5 root: /Applications, /Applications/Utilities, /System/Applications, /System/Applications/Utilities, ~/Applications）を採用し、orphan 側に寄せて leftover 保護を退歩させない
- [ ] is_ci() トリムを 1 箇所に統合する
- [ ] list_app_bundles に bundle path の dedup（HashSet）を入れ、二重 plutil spawn を消す（結果不変）
- [ ] 依存方向を installed_apps → crate::command + std::fs のみにする。app_support → installed_apps、orphan_containers → installed_apps とし、cleaner 同士の辺をゼロにする
- [ ] per-cleaner 拡張は constructor 注入（既存 #[cfg(test)] with_app_roots / with_app_dirs がそのまま拡張点）にする。per-cleaner の静的 root レジストリは作らない（YAGNI）
- [ ] 照合ポリシーは各 cleaner に残留させる: orphan は normalize_container_id / is_owned_by_installed / team_id_prefix、app_support は InstalledApps + matches_installed
- [ ] README.md:216（en）と :453（ja）を canonical root list に同期する（同一 PR で）
- [ ] 挙動変更を (a) orphan の installed-set が Utilities 2 root を得て false orphan が減る、(b) list_app_bundles の bundle path dedup（結果不変）の 2 点に限定する

## テスト戦略

Outside-In TDD:

1. pin テスト（最外・契約）: canonical 5-root の exhaustive pin を新設する（EnvGuard CI=false + TEST_LOCK パターンを必ず保持。c08a6f8 で CI 環境漏れで壊れた前科あり）。canonical 化前は失敗（赤）を確認する
2. 統合: traversal 境界テスト（1 階層の /Applications/Utilities/Foo.app は列挙、2 階層ネストは canonical で列挙）+ dedup テスト（plutil 呼び出しが bundle path ごとに 1 回）
3. seam テスト: app_support.rs:1648-1664 orphan_containers_identity_collector_is_available を installed_apps への seam テストとして再目標化
4. 単体: identity 解析/読み取りテスト群を orphan_containers.rs から名前を保ったまま移動（three keys、missing bundle id、key-without-string、empty、ignores other keys、exactly-three-fields、xml/binary/garbage/missing、collect_installed_ids 3 本）
5. 実装（新 module 新設 + 2 cleaner の再配線）で全テストをグリーンにする

## 見積もり

0.75 人週（0.6 から 1.0）

## 実装計画

### 問題（深掘り結果）

「このマシンに何がインストールされているか」を理解するのに 2 モジュールを行き来が必要で、相互参照の module cycle（app_support.rs:837 → orphan_containers::collect_app_identities、orphan_containers.rs:211 → app_support::list_app_bundles）。正 canonical な root list の手維持コピーが 2 本ある:

- 実害ドリフト: orphan 側は /Applications/Utilities と /System/Applications/Utilities を見ない（orphan_containers.rs:232-236）→ /Applications/Utilities/<Folder>/Foo.app の 2 階層ネストに入れたサードパーティアプリは orphan 検出から不可視で、インストール済みのままコンテナが orphan と報告され得る（受け入れで削除）。/Applications/Utilities/Foo.app（1 階層）は list_app_bundles が 1 サブフォルダを辿るため列挙される（orphan_containers.rs:919-928 のテストが境界を pin）― 不可視は 2 階層ネストのみ
- 副次症状: app_support の 5 root は Utilities の 1 階層アプリを二重列挙 → バイナリ plist で plutil を 2 回 spawn（HashSet で結果は正しいが無駄 I/O）
- README にもう一つのずれ: README.md:216（日本語版 :453）の promise は「/Applications, ~/Applications, /System/Applications とその 1 フォルダ下」。orphan は README より狭く、app_support は README よりわずかに広い
- CI トリム処理は 2 箇所にほぼ逐語重複（app_support.rs:250-253 / orphan_containers.rs:226-229）
- orphan_containers.rs:988-1004 default_app_dirs_lists_three_roots がドリフトした 3 root を「本番デフォルト」として pin していた（ドリフトを固定していた）

### 5 Whys の根本原因

2026-10-20 の同日に PBI-04（orphan-containers）と PBI-06/07（app-support）が着地し、先に書いた側の部品を最短経路で相互 import した（fac3fad は抽出ではなく存在固定テストを作った）。共有化自体は正しかった（7b2fc09 は probe を意図的 dedup）― 誤りは置き場所。概念に所有 module がなく、Rust は crate 内 module cycle を黙って許す。

### 解

src/installed_apps.rs を新設し「.app 列挙 → identity 読み → インストール済み集合 + 正 canonical root list」を 1 本に所有:

- AppIdentity { bundle_id, name, display_name } + PLIST_KEY_* + xml_string_value + parse_app_identity_from_xml + read_app_identity（orphan_containers.rs:140-198 から移動、README promise コメントも逐語移動）
- is_app_bundle / list_app_bundles / app_bundle_stems（app_support.rs:783-830 から移動）
- collect_app_identities / collect_installed_ids（orphan_containers.rs:205-222 から移動）
- default_app_roots(home): 2 本の list を統合した正 canonical。app_support 側の上位集合（5 root: /Applications, /Applications/Utilities, /System/Applications, /System/Applications/Utilities, ~/Applications）を採用（orphan 側に寄せると leftover 保護が退歩する）。is_ci() トリムも 1 箇所に統合
- list_app_bundles に bundle path の dedup（HashSet）を入れ二重 plutil を消す（結果不変）
- 依存方向: installed_apps → crate::command + std::fs のみ。app_support → installed_apps、orphan_containers → installed_apps。cleaner 同士の辺はゼロになる
- per-cleaner 拡張は constructor 注入（既存 #[cfg(test)] with_app_roots / with_app_dirs がそのまま拡張点）。per-cleaner の静的 root レジストリは作らない（YAGNI）
- 照合ポリシーは各 cleaner に残留: orphan は normalize_container_id / is_owned_by_installed / team_id_prefix、app_support は InstalledApps + matches_installed

### 決定事項（やらないこと）

- threshold/process_name のテーブル構築時解決（LazyLock）はやらない。const eval 不可・22 要素の線形 find は計測不可の差・沈黙フォールバックは双方向 pin（app_support.rs:911/925）で既に塞がれている・第二の真実源になる
- library_caches::known_entry_for の統合もしない。キー意味が違う（path_suffixes 完全一致 vs display_name 一致）。第三の照合様式が出現した時点で再検討

### 実装順

1. src/installed_apps.rs 新設（コード移動 + canonical root 統合 + lib.rs に pub mod 1 行）
2. orphan_containers.rs 再配線（移動済み項目削除、use crate::installed_apps、new を canonical へ、identity テスト移動）
3. app_support.rs 再配線（probe/stems 削除、collect_installed_apps を installed_apps 経由に、traversal/collect テスト移動、:1648 の availability テストを installed_apps への seam テストとして再目標化）
4. pin テスト新設（canonical 5-root の exhaustive pin（EnvGuard CI=false + TEST_LOCK パターン保持）+ traversal 境界テスト + dedup テスト）
5. README.md:216（en）と :453（ja）を canonical root list に同期（同一 PR で。promise は読み取り範囲の宣言であり、コードがそれを超えて読む状態を残さない）
6. 検証ゲート
7. 完了後 .plan/archived/ へ移動とインデックス追記（closer が実施）

## pin（壊してはいけない契約）

- app_support.rs:1648-1664 orphan_containers_identity_collector_is_available ― seam テストとして再目標化して維持
- app_support.rs:1762-1849 traversal テスト + collect_installed_apps 3 本 ― 名前を保ったまま移動
- orphan_containers.rs:787-985 identity 解析/読み取りテスト群（three keys、missing bundle id、key-without-string、empty、ignores other keys、exactly-three-fields、xml/binary/garbage/missing、collect_installed_ids 3 本）
- orphan_containers.rs:919-928 traversal 境界 pin ― canonical に対して再 pin
- orphan_containers.rs:988-1004 default_app_dirs_lists_three_roots ― canonical 化で更新必須（3 root → 5 root、installed_apps 側へ移設）。CI=false EnvGuard + TEST_LOCK のパターンは必ず保持
- app_support.rs:911-930 every_target_has_matching_hint_entry / every_app_support_hint_entry_has_a_target ― 双方向 pin
- library_caches の known_entry テスト + GeoServices skip ガード
- README.md:216 / :453 の privacy promise（Info.plist からは bundle ID とアプリ名のみ）。read_app_identity をコードベース唯一の Info.plist reader に保つ
- 振る舞い pin: detect() 側作用なし、leftovers 表示のみ・TTY-only ガード（app_support.rs:301-326）、orphan の scan_roots 挙動（orphan:1019-1171 の 10 本）、format_item/unreadable_hint の出力文言、TrashMode/delete_entries 契約（apply_selection app_support.rs:469-521）

## scope fence

1. 挙動変更は 2 点に限定: (a) orphan の installed-set が Utilities 2 root を得て false orphan が減る（本体）、(b) list_app_bundles の bundle path dedup（結果不変・二重 plutil 除去のみ）
2. 新しいクリーニング対象・削除経路・root（~/Applications 以外の追加）は足さない
3. config 由来の root 注入は作らない
4. hint テーブル・TARGETS・AppSupportTarget・hint_entry/known_entry_for のコード変更なし
5. InstalledApps / matches_installed は移動しない（app_support 固有の照合ポリシー）
6. DI、plist crate、cleaner 名・CLI フラグ・出力形式の変更なし
7. generic.rs / main.rs / trash.rs / prompt.rs / cleaner.rs / hint/* への接触なし（lib.rs の 1 行を除く）
8. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD

- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
