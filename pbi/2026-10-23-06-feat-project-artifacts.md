# PBI: プロジェクト生成物のパージ(node_modules / target 等)

## ユーザーストーリー
macOS開発者として、使っていない古いプロジェクトの `node_modules` や `target` をまとめて見つけて消したい、なぜなら ~/Playground 配下の再生成可能な生成物が数十GBになっても、グローバルキャッシュ系のクリーナーでは見えないから

## ビジネス価値
開発者のディスク消費で最大級の領域に手が届く。測定指標は、1回の scan で検出できる回収可能サイズ。

## 優先度
- 順位: 02 / 5
- RICEスコア: 315(Reach=600 / Impact=3 / Confidence=70% / Effort=4.0人週)
- 根拠: 影響が大きい。誤削除リスクが高いため、保護策を受け入れ基準に含める。

## BDD受け入れシナリオ

```gherkin
Scenario: 古いプロジェクトの生成物を検出する
  Given ~/Playground/old-app に node_modules と package.json があり、プロジェクト内の最終更新が30日前
  When  `sasurahime scan` を実行する
  Then  `project-artifacts` に old-app/node_modules のサイズが表示される

Scenario: 直近7日に更新があるプロジェクトは除外する
  Given ~/Playground/active-app のソースに2日前の更新がある
  When  `sasurahime clean project-artifacts` を実行する
  Then  active-app の生成物は対象に含まれず、除外理由が表示される

Scenario: マニフェストがない場合は触らない
  Given `target` というディレクトリだけがあり、隣に Cargo.toml がない
  When  `sasurahime scan` を実行する
  Then  そのディレクトリは検出対象にならない

Scenario: --yes の既定対象に含めない
  When  `sasurahime --yes` を実行する
  Then  `project-artifacts` は実行されない
```

## 受け入れ基準
- [ ] 検出ルールは「生成物ディレクトリ名 + 隣接マニフェスト」の組(node_modules+package.json、target+Cargo.toml、.build+Package.swift。dist は初回対象外)
- [ ] 探索ルートと深さを config.toml で指定できる(既定は ~/Playground、深さ5)
- [ ] プロジェクト内の最終更新が直近N日(既定7日)なら除外する。生成物自身の mtime は判定に使わない
- [ ] 保護はmtimeのみであり、未コミット変更の有無は見ないことを README に明記する
- [ ] 既存の exclude、Trash モード、`--dry-run` に従う
- [ ] `--yes` の既定対象に含めない

## テスト戦略(t_wadaスタイル)

### E2Eテスト
- tempdir に新旧2プロジェクトを作り、scan と `clean --dry-run` の出力を検証

### 統合テスト
- 偽ルートで `detect()` と `clean(dry_run=true)` を呼び、何も削除されないことを検証
- exclude と Trash モードが効くこと

### 単体テスト
- マニフェスト対応表の判定
- 最終更新判定(`is_older_than`)の境界値(ちょうど7日)
- 探索深さの上限

## 実装アプローチ
- **Outside-In**: E2E から開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 各レイヤーで適用
- **リファクタリング**: グリーンのたびに既存の走査ユーティリティとの重複を除去

## 見積もり
8 SP(要チームでの見積もり)。上限ちょうどのため、実装中に膨らむ場合は「検出のみ」と「削除」で分割する。

## 技術的考慮事項
- 依存関係: なし。`2026-10-23-01` の永続キャッシュ導入後はキャッシュキーに載せる
- テスタビリティ: 探索ルートを引数化し tempdir で差し替える
- 非機能要件: 生成物の内部は走査しない(巨大ツリーの二重walkを避ける)

## 実装者向け注記

### 現状コードの確認
確認済み: `src/` に `node_modules` は存在しない(2026-10-23 時点)。
```bash
grep -rn "node_modules" src/
ls src/cleaners/
grep -n "exclude" src/config.rs
```

### 実装手順
1. E2E: 新旧2プロジェクトでの scan テストを書く(Red)
2. 統合: `detect()` / `clean(dry_run=true)` のテストを書く(Red)
3. 単体: マニフェスト判定と最終更新判定のテストを書く(Red)
4. `src/cleaners/project_artifacts.rs` に探索ルール表と判定を実装(Green)
5. `cleaners/mod.rs` と `main.rs` に登録(Green)
6. 既存走査との共通化(Refactor)

### 落とし穴
- 最終更新の算出で生成物の中まで走査すると、node_modules の巨大ツリーで遅くなる
- 対象外ディレクトリ(`.git` 内など)の mtime を拾わない

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] E2E/統合/単体のすべてが揃っている
- [ ] コードレビュー完了
- [ ] リファクタリング完了(グリーン後)
- [ ] README のターゲット一覧を更新済み
- [ ] `cargo clippy -- -D warnings` と `cargo fmt --check` が通る
