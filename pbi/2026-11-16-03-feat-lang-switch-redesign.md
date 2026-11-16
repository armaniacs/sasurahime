# PBI: 言語切替 JS 再設計と /ja/ 下層整備 (GitHub Pages)

## ユーザーストーリー

日英両言語の訪問者として、言語切替が確実に動き、日本語 URL 直下でも日本語ドキュメントを読めるようにしたい。
なぜなら現状は `onclick` 直書きと `indexOf` 判定の脆い実装で、EN 本文 3 件 (HOWTO-USE / SUPPORTED / HOWTO-ADD-target) に日本語版が無く、JS による後付けラッパーも保守性を下げているから。

## ビジネス価値

- 言語切替の誤動作 (判定漏れや `lang` 属性の不整合) が無くなる
- 日本語話者が `/ja/` 配下で主要ドキュメント 3 件を日本語で読めるようになる
- 保守者が Jekyll / CSS 側で構造を追えるようになり、JS の応急処置が減る
- 測定方法: EN / JA 各ページで切替操作が期待どおり動くこと、`/ja/` 配下に 3 ページが存在すること

## BDD受け入れシナリオ

```gherkin
Scenario: 訪問者が言語を切り替える
  Given 訪問者が EN ページを開いていてブラウザ言語が日本語である
  When JA ボタンを押す
  Then ページ表示が日本語になり、documentElement.lang が ja になって状態が保持される

Scenario: 初回訪問者がブラウザ言語で表示される
  Given 訪問者が初めて docs を開き、明示的な言語選択をしたことがない
  When ページが読み込まれる
  Then navigator.language に応じた言語で表示され、localStorage には何も書かれない

Scenario: 日本語訪問者が /ja/ 配下の使い方を読む
  Given 訪問者が /ja/ 配下の HOWTO-USE 日本語ページを開いている
  When ページの内容とナビゲーションを確認する
  Then 本文が日本語で、nav のリンクが /ja/ 配下を指している

Scenario: 脆い判定が残っていない
  Given 実装者が言語切替コードを開いている
  When 文字列の indexOf 判定や onclick 直書きを探す
  Then どちらも存在せず、data-lang 属性と addEventListener で実装されている
```

## 受け入れ基準

- [ ] `doc.html` の `onclick` 直書きが無く、`addEventListener` 化されている
- [ ] 言語ボタン等に `data-lang` 属性があり、`indexOf` 判定が残っていない
- [ ] 切替時に `documentElement.lang` が同期される
- [ ] 既定表示は `navigator.language` 追随で、明示選択時のみ `localStorage` に保存される (初回訪問では書かれない)
- [ ] JS による `.doc-body` 後付けラッパーが無く、Jekyll / CSS 側で解決されている
- [ ] EN md 3 件 (HOWTO-USE / SUPPORTED / HOWTO-ADD-target) の本文が `_includes` に切り出されている
- [ ] `docs/ja/` 配下に `lang: ja` の JA 版 3 ページが新設されている
- [ ] トップと doc レイアウトの nav href が言語対応 (`/ja/` 配下では `/ja/` リンク) になっている
- [ ] `bundle exec jekyll build` が成功する

## テスト戦略 (t_wadaスタイル)

静的サイトのため Outside-In を次の形で適用する。Rust コードには触れないため `cargo test` / clippy は対象外。

### E2Eテスト

- ビルド後サイトをブラウザで開き、EN / JA 切替、リロード後の保持、初回訪問の既定言語を手動シナリオで確認する
- `/ja/` 配下の 3 ページを EN 版と見比べて内容と言語が正しいことを確認する

### 統合テスト

- ビルド後の `/ja/` 配下に 3 ページの HTML が生成されることを確認する
- ビルド後 HTML に `onclick="switchLang` が残っていないことを `grep` で検証する
- ビルド後 HTML の `html lang` 属性がページ言語と一致することを検証する

### 単体テスト

- 変更差分の目視レビュー: `data-lang` の読み取り、`localStorage` の読み書き条件分岐が意図どおりであること
- `_includes` 切り出し前後で EN ページの表示内容が変わっていないこと (差分比較)

## 検証手順

1. `bundle exec jekyll build` が通ること (docs/ ディレクトリで実行)
2. 目視確認項目:
   - EN ページで JA ボタンを押すと日本語表示になり、`html lang` が ja になること
   - リロード後も選択言語が保持されること
   - プライベートウィンドウ (localStorage 空) ではブラウザ言語に従うこと
   - `/ja/` 配下 3 ページの本文が日本語で、nav が `/ja/` リンクを指すこと
   - ブラウザコンソールにエラーが出ないこと

## 実装アプローチ

- **Outside-In**: 先に受け入れの grep 検証 (onclick 残存チェック等) を決め、失敗を確認してから実装
- **Red-Green-Refactor**: JS 再設計、_includes 切り出し、JA 3 ページ新設、nav 言語対応の順に小さく適用し、各段階でビルドする
- **リファクタリング**: グリーンになるたびに言語判定ロジックの重複を整理する

## 見積もり

5 SP (要チームでの見積もり)

## 技術的考慮事項

- 依存関係: なし (本 PBI 単独で出荷可能。S1 の `aria-pressed` 等と同時実装する場合はボタン属性の競合に注意)
- テスタビリティ: ビルド後 HTML への grep 検証とブラウザ手動シナリオが主体
- 非機能要件: 既存 EN ページの URL を変えない (`/ja/` は新規追加のみ)。`localStorage` キー名は既存利用者への影響を考えて決める
- 参照元 (modern-web-guidance ガイド): `html` (lang 属性)、`navbar` (言語対応ナビ)

## 実装者向け注記

### 現状コードの確認

(着手前に必ず実行すること。2026-11-16 時点では旧実装であることを確認済み)

```bash
# onclick 直書きの存在を確認
grep -rn "onclick" docs/_layouts/doc.html docs/index.html docs/ja/index.html
# data-lang / addEventListener が無いことを確認
grep -rn "data-lang\|addEventListener" docs/_layouts/doc.html docs/index.html docs/ja/index.html || echo "NOT FOUND"
# _includes が無く ja 配下に下層ページが無いことを確認
ls docs/_includes 2>/dev/null || echo "(no _includes)"
ls docs/ja/
```

### 実装手順

1. `doc.html` の言語ボタンの `onclick="switchLang(...)"` を `data-lang="en" / "ja"` 属性に置き換える
2. `switchLang` を `addEventListener` 登録に変え、`document.querySelectorAll('[data-lang]')` で束ねる。`indexOf` 判定をやめ、完全一致で判定する
3. 切替処理で `document.documentElement.lang` を同期させる
4. 初期化処理を `navigator.language` 参照に変え、`localStorage` への保存は明示選択 (クリック) 時のみにする
5. JS による `.doc-body` 後付けラッパー (`document.createElement` / `wrap` 系) を削除し、Jekyll レイアウト側でラッパーを出力するよう `doc.html` と CSS を直す
6. EN md 3 件 (docs/HOWTO-USE.md、docs/SUPPORTED.md、docs/HOWTO-ADD-target.md) の本文を `docs/_includes/` に切り出す (front matter は各ページに残す)
7. `docs/ja/` 配下に `lang: ja` の JA 版 3 ページを新設し、切り出した include に対応する日本語本文を持たせる
8. トップ (`index.html`、`ja/index.html`) と `doc.html` の nav href を言語対応にする (`/ja/` 配下では `/ja/...` を指す)
9. `bundle exec jekyll build` を実行し、受け入れ基準の grep 検証とブラウザ目視を行う

### 落とし穴

- `navigator.language` は `ja-JP` のように地域付きで返る。`startsWith('ja')` 等の前方一致で判定し、完全一致にしないこと
- `_includes` 切り出し時に front matter (`layout`、`title`、`lang`) を巻き込まないこと。巻き込むと JA ページの属性が壊れる
- `.doc-body` ラッパーを JS と Jekyll の両方で出すと二重になる。JS 側を消した後にビルド後 HTML で 1 つだけあることを確認すること
- `localStorage` が使えない環境 (無効化時) で例外にならないよう try-catch 等で守ること

## Definition of Done

- [ ] 全 BDD シナリオが検証手順として実行されパスする
- [ ] `bundle exec jekyll build` が成功する
- [ ] EN ページの表示内容が切り出し前後で変わっていない
- [ ] コードレビュー完了
- [ ] `/ja/` 新設 3 ページの日本語内容がレビュー済みである
