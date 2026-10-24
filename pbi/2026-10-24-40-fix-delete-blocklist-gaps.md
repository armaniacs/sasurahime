# PBI-40: 削除ブロックリストのギャップ解消（/Volumes, /Users/Shared 等）+ doc 正確化

## 概要

is_safe_delete_target のブロックリストが /Volumes（外部/ネットワークマウント）と /Users/Shared を欠き、一方で /usr/local/Caskroom/*（Intel Homebrew）は拒否する。seam doc の「HOME、その祖先、システムルートを拒否する」主張と実装が不整合。

## 発見元

1023a 全体レビュー Major #4（generic.rs:844-870, trash.rs:88-90）。

## Why (5 Whys)

1. なぜ外部ボリューム上の [custom] ターゲットが削除可能か → ブロックリストに /Volumes と /Users/Shared の項目がない。
2. なぜ項目がないか → リストは main 由来で、macOS のマウントポイント/共有ディレクトリが考慮されていなかった。
3. なぜ考慮されていなかったか → トリップワイヤ導入時に「システムルート」を /System /usr /etc 等の固定リストで定義し、/Volumes がルート扱いであるという macOS 特有の事情が抜けた。
4. なぜ抜けたままか → ブロックリストに対応するテストが / /etc /tmp のみで、マウント系のケースがない。
5. なぜテストがないか → 受け入れシナリオがHOME祖先と system root に限定され、ボリューム境界がシナリオ化されていなかった。

## 受け入れ基準

1. is_safe_delete_target が /Volumes、/Users/Shared、/Library、/opt、/net、/private/var/vm を拒否する。各1件の単体テスト付き。
2. seam doc（trash.rs:88-90）と generic.rs のコメントが、実際のブロックリストと HOME parity（main::home は空 HOME を filter しないため「同一のフォールバック」主張は修正する）の実態に一致する。
3. 既存の全テスト（tempdir ベース含む）が green を維持。
4. lib/integration/clippy/fmt の全ゲート green。

## 検証戦略

単体テスト（generic.rs テストモジュール）。TDD: 各新規拒否パスの red → 実装 → green。
