# PBI: Pages 本番デプロイの実機検証 (GitHub Pages)

## ユーザーストーリー

保守者として、`v0.4.7` で出荷した Pages 改善 (S1-S4) が本番 URL で期待どおりに動いていることを確認したい。
なぜなら実装時の検証はローカルの `jekyll build` が中心で、GitHub Pages 側のビルド環境
(Jekyll バージョンや設定の差異) による差異やデプロイ漏れはローカル検証では検出できないから。

## ビジネス価値

- S1-S4 の成果 (アクセシビリティ、レスポンシブ、言語切替、SEO) が実際の訪問者に届いていることを保証できる
- ローカルビルドと本番ビルドの差異に起因する問題を早期発見できる
- 測定方法: 本番 URL への HTTP 取得 + grep 検証、カードバリデータ、実ブラウザでの確認

## BDD受け入れシナリオ

```gherkin
Scenario: 本番サイトが v0.4.7 の成果を配信している
  Given 本番 URL が https://armaniacs.github.io/sasurahime/ である
  When トップページの HTML を取得する
  Then head に og:title と og:description と og:type と theme-color があり、canonical と hreflang (en、ja、x-default) もある

Scenario: 日本語下層ページが本番で到達可能
  Given 訪問者が本番の /ja/ 配下を開いている
  When /ja/HOWTO-USE と /ja/SUPPORTED と /ja/HOWTO-ADD-target を取得する
  Then いずれも 200 で日本語本文が表示され、nav のリンクが /ja/ 配下を指す

Scenario: 言語切替が本番で動く
  Given 訪問者が本番の EN トップページを開いている
  When JA ボタンを押す
  Then 表示が日本語に切り替わり documentElement.lang が ja になる

Scenario: キーボードとモバイルの基本動線が本番で成立する
  Given 訪問者が本番のページを開いている
  When Tab キーを押し、その後幅 360px で targets table のあるページを開く
  Then スキップリンクが現れ、表は横スクロールと sticky ヘッダーで読める
```

## 受け入れ基準

- [ ] 本番トップ (EN / JA) と doc レイアウト全ページの head に og:title、og:description、og:type、theme-color、canonical、hreflang (x-default 含む) がある (HTTP 取得で確認)
- [ ] /ja/ 配下 3 ページが本番で 200 を返す
- [ ] 本番で配信される JS に onclick 直書きが無く、data-lang + addEventListener で動く
- [ ] SNS カードバリデータで本番 URL のカードが正しく出る
- [ ] 実ブラウザ (幅 360px) で表の横スクロールと sticky ヘッダーを確認
- [ ] キーボードのみでスキップリンクから main ランドマークへ到達できる
- [ ] 発見した差異・不具合は修正 PBI として起票するか、軽微であれば本 PBI 内で修正して記録する

## テスト戦略 (t_wadaスタイル)

Rust コードには触れないため `cargo test` / clippy は対象外。本番 URL への HTTP 取得と実ブラウザ確認が主体。

### E2Eテスト

- SNS カードバリデータ / リッチリザルトテストで本番 URL を確認する

### 統合テスト

- curl 等で本番 HTML を取得し、アーカイブ済み S1-S4 と同じ grep 検証を本番に対して実行する

### 単体テスト

- ローカル `bundle exec jekyll build` の生成物と本番 HTML の head を比較し、意図しない差異が無いことを目視確認する

## 検証手順

1. GitHub Pages のデプロイ履歴で v0.4.7 相当のコミットが反映済みであることを確認する
2. curl で本番ページ一式を取得し、S1-S4 と同じ grep 検証を実行する
3. SNS カードバリデータでトップ EN / JA と doc ページを確認する
4. 実ブラウザでキーボード操作と 360px 表示を確認する
5. 差異があれば記録し、修正要否を判断する

## 見積もり

1 SP (要チームでの見積もり)

## 技術的考慮事項

- 依存関係: なし (S1-S4 のリリース後であれば単独で実施可能)
- GitHub Pages のビルドはローカルと Jekyll バージョンやプラグイン集合が異なる可能性がある。`docs/Gemfile.lock` と Pages のビルド設定の差異に注意
- 確認対象の本番 URL: `https://armaniacs.github.io/sasurahime/` (`docs/_config.yml` の url 設定)
- 参照: アーカイブ済み S1-S4 (`.plan/archived/2026-11-16-*.md`) の受け入れ基準を本番 URL に読み替えたものが検証セット

## 実装者向け注記

### 現状コードの確認

(着手前に必ず実行すること)

```bash
# 本番 head に S1-S4 の成果が反映されているか (0 または小さい値なら未反映の疑い)
curl --compressed -fsS https://armaniacs.github.io/sasurahime/ | grep -c "og:title\|hreflang\|theme-color" || echo "NOT FOUND"
# /ja/ 下層の到達性
curl -s -o /dev/null -w "%{http_code}\n" https://armaniacs.github.io/sasurahime/ja/HOWTO-USE.html
```

### 落とし穴

- curl の取得結果は gzip 圧縮に注意する (`--compressed` を付ける)
- カードバリデータは結果をキャッシュすることがある。修正後の再確認はクエリパラメータを付けて行う
- 本番の URL 形態 (ディレクトリ型か `.html` 付きか) を確認してから一括検証スクリプトを書く。pretty_urls の設定差異で 404 判定を誤らないこと

## Definition of Done

- [ ] 全 BDD シナリオが検証手順として実行されパスする
- [ ] 本番 HTML の grep 検証の実行結果が本 PBI に記録されている
- [ ] 発見事項と対応 (修正 PBI 起票 / 記録のみ) が明確になっている
- [ ] レビュー完了
