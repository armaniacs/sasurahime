# PBI-51: サイズパーサ統一（ollama の黙って 0 を返す挙動の解消）

## ユーザーストーリー

保守担当者として、バイナリ単位のサイズパースが 1 つの実装にまとまっていてほしい。なぜなら ollama.rs:60-74 の parse_model_size は brew.rs:23-50 の弱いコピーで、ゴミ入力に対して黙って 0 を返し（unwrap_or(0.0)）、大文字小文字も区別しないから。

## 発見元

アーキテクチャ deepening 後日候補（pbi/2026-10-23-27 索引、2026-10-24 再評価）。RICE: Reach 6 / Impact 1 / Conf 80% / Effort 1.0 = 約 5。

## BDD受け入れシナリオ

```gherkin
Scenario: brew と ollama が同じパーサを使い、ollama がゴミ入力を None で扱う
  Given format.rs に単位フラグ付きの共有パーサが置かれている
  When  ollama がゴミ文字列（例: "unknown"）をパースする
  Then  パーサは None を返し、呼び出し側の既定化が明示的に記録される
  And   黙って 0 を返す挙動が消える

Scenario: apfs_snapshot の 10 進パーサは tmutil 規約のまま別実装に保たれる
  Given apfs_snapshot.rs:64-95 の parse_size_decimal が tmutil 規約（10GB = 10,000,000,000）を実装している
  When  共有パーサへの統一を検討する
  Then  10 進版はバイナリ側にマージされない
  And   既存の 10 進単位テストが green を維持する
```

## 受け入れ基準

- [ ] format.rs に単位フラグ付きの共有 parse_size_str が置かれ、brew と ollama がそれを使う
- [ ] ollama の黙って 0 を返す挙動が明示的な None 扱いに変わる（呼び出し側でデフォルト化するなら記録する）
- [ ] apfs_snapshot.rs:64-95 の 10 進パーサ（tmutil 規約）はマージされず別実装のまま
- [ ] 既存ユニットテストが green（brew 10 件: 141-197 / ollama 2 件: 288-297 / apfs 約 20 件: 1020-1128）、brew の空白区切り形式を保持
- [ ] 全ゲート green

## テスト戦略（t_wadaスタイル）

- 単体: 共有パーサの単位ケース（バイナリ / 大文字小文字 / 空白区切り / ゴミ入力 → None）
- 回帰: apfs 10 進テストの移動なし維持

## 実装者向け注記

### 現状コードの確認（2026-10-24 調査済み）
- src/cleaners/brew.rs:23-50 parse_size_str（バイナリ単位、大文字小文字非依存、空白区切り + 連結両対応）+ 54-65 wrapper、tests 141-197
- src/cleaners/ollama.rs:60-74 parse_model_size（バイナリ単位、連結形のみ、unwrap_or(0.0) で黙って 0、used at 38、tests 288-297）
- src/cleaners/apfs_snapshot.rs:64-95 parse_size_decimal（10 進単位 per tmutil、GiB/0/負/non-finite を拒否、1 PB cap、called from main.rs:1222、tests 1020-1128）
- 正準フォーマッタ: src/format.rs:34 format_bytes（バイナリ、150 参照）

### 実装手順
1. format.rs に単位フラグ（Binary / Decimal）付きの共有パーサを置く
2. brew と ollama を移行する（ollama は呼び出し側で既定化を明示）
3. apfs は名前と単位を保ったまま残す（main.rs:1222 に影響させない）

### 落とし穴
- apfs の 10 進単位をバイナリ側にマージするとバグになる（tmutil は 10GB = 10,000,000,000）
- PBI 38-47 のホットファイル（main.rs / generic.rs / batch.rs）に触れないこと。main.rs:1222 は名前維持で影響なし
