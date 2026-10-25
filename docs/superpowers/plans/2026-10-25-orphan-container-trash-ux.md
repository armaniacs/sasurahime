# orphan-containers 削除 UX 改善 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** orphan-containers の削除で trash クレートの NsFileManager バックエンドを使い TCC ダイアログ待ちのハングを根因除去し、選択リストにフルパスを表示し、スピナーに経過時間タイマーを常時表示する。

**Architecture:** `DeleteCtx::delete_path` (src/trash.rs) が `TrashContextExtMacos::set_delete_method(DeleteMethod::NsFileManager)` でバックエンドを指定する。`format_item` (src/cleaners/orphan_containers.rs) がフルパスを中間省略で表示する。`spinner_style()` と `with_parallel_scan` (src/progress.rs) の indicatif テンプレートに `{elapsed}` を足す。挙動の境界は既存のシーム (trash テスト、format_item テスト) で守る。

**Tech Stack:** Rust (macOS 専用)、trash 5.2.6 (vendored、`pub mod macos` は `#[cfg(target_os = "macos")]`)、indicatif 0.18.6、dialoguer 0.11。

**仕様書:** `docs/superpowers/specs/2026-10-25-orphan-container-trash-ux-design.md`

**ブランチ:** main から `feat/orphan-trash-ux` を切って実装する。

---

### Task 1: NsFileManager バックエンド切替 (根因除去)

**Files:**
- Modify: `src/trash.rs:1-4` (imports)
- Modify: `src/trash.rs:47-62` (`DeleteCtx::delete_path` の trash 分岐)

- [ ] **Step 1: ブランチを作る**

```bash
cd /Users/yaar/Playground/sasurahime
git checkout -b feat/orphan-trash-ux main
git log --oneline -1
```

Expected: `26d5100` (または main の先頭コミット) から分岐する。

- [ ] **Step 2: import に trash::macos を足す**

`src/trash.rs` の先頭 (1-4 行目) を次のようにする:

```rust
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use trash::macos::{DeleteMethod, TrashContextExtMacos};
```

- [ ] **Step 3: delete_path の trash 分岐を差し替える**

`src/trash.rs` の `DeleteCtx::delete_path` (trash 分岐の `trash::delete(path)` 行) を次のようにする:

```rust
    pub fn delete_path(&self, path: &Path) -> Result<()> {
        if self.trash_mode.resolve() {
            #[cfg(test)]
            if trash_backend_override_enabled() {
                return Err(anyhow::anyhow!(
                    "trash failed: simulated refusing backend (test override)"
                ));
            }
            // NsFileManager (trashItemAtURL) instead of the default Finder
            // backend: the Finder backend spawns `osascript`, which can sit
            // for minutes on a hidden TCC permission dialog with no output.
            // The NSFileManager call needs no extra permissions, is faster
            // and is silent.
            let mut ctx = trash::TrashContext::new();
            ctx.set_delete_method(DeleteMethod::NsFileManager);
            ctx.delete(path)
                .map_err(|e| anyhow::anyhow!("trash failed: {e}"))
        } else if path.is_dir() {
            fs::remove_dir_all(path)
                .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
        } else {
            fs::remove_file(path).map_err(|e| anyhow::anyhow!("remove_file {:?}: {}", path, e))
        }
    }
```

- [ ] **Step 4: 既存の trash テストが通ることを確認する**

Run: `cargo test --lib trash:: 2>&1 | tail -3`
Expected: `test result: ok` (既存の `delete_ctx_trash_mode_moves_to_trash`、`delete_ctx_permanent_mode_removes_directory` などがそのまま通る。NsFileManager バックエンドでもダイアログが出ないのでテストは速い)

- [ ] **Step 5: コミット**

```bash
git add src/trash.rs
git commit -m "fix(trash): move to Trash with NsFileManager instead of Finder's osascript

The Finder backend spawns osascript, which can sit for minutes on a
hidden TCC permission dialog with no output (36 minutes observed). The
NSFileManager trashItemAtURL call needs no extra permissions, is faster
and is silent. Deletion stays a Trash move."
```

---

### Task 2: Put Back の断りを 1 行足す

**Files:**
- Modify: `src/cleaners/orphan_containers.rs:397-410` (`interactive_select` のヘッダー)

- [ ] **Step 1: ヘッダーに 1 行追加する**

`src/cleaners/orphan_containers.rs` の `interactive_select` を次のようにする:

```rust
    fn interactive_select(candidates: &[OrphanCandidate]) -> Result<Vec<OrphanCandidate>> {
        use dialoguer::MultiSelect;

        let items: Vec<String> = candidates.iter().map(format_item).collect();
        println!("\nContainers of apps that are not installed (nothing is selected by default):\n");
        println!(
            "Moved items may not show \"Put Back\" in Finder's context menu. \
             Restore by dragging out of the Trash.\n"
        );
        let selections = MultiSelect::new()
            .with_prompt("Select folders to move to Trash  [space to toggle, enter to confirm]")
            .items(&items)
            .interact()?;
        Ok(selections
            .into_iter()
            .map(|i| candidates[i].clone())
            .collect())
    }
```

- [ ] **Step 2: ビルドを確認する**

Run: `cargo check 2>&1 | tail -2`
Expected: エラーなし (この println は TTY 専用プロンプトの一部で、自動テストからは届かない。手元確認は Task 5)

- [ ] **Step 3: コミット**

```bash
git add src/cleaners/orphan_containers.rs
git commit -m "feat(orphan-containers): note the Put Back limitation before selecting

The NsFileManager backend does not show Put Back in Finder's context
menu on some systems. State the limitation once, before the selection."
```

---

### Task 3: 選択リストにフルパスを表示する

**Files:**
- Modify: `src/cleaners/orphan_containers.rs:365-372` (`format_item`)
- Modify: `src/cleaners/orphan_containers.rs:1193-1204` (`format_item_contains_name_size_and_kind` テスト)
- Test: `src/cleaners/orphan_containers.rs` (同モジュールの `#[cfg(test)]`)

- [ ] **Step 1: 失敗するテストを書く**

`src/cleaners/orphan_containers.rs` の tests モジュールで既存の
`format_item_contains_name_size_and_kind` を次のように差し替える:

```rust
    #[test]
    fn format_item_contains_path_size_and_kind() {
        let c = OrphanCandidate {
            path: PathBuf::from("/x/com.example.gone"),
            name: "com.example.gone".into(),
            kind: ContainerKind::GroupContainers,
            size: 2048,
        };
        let s = format_item(&c);
        assert!(s.contains("/x/com.example.gone"), "{s}");
        assert!(s.contains("2.0 KB"), "{s}");
        assert!(s.contains("[Group Containers]"), "{s}");
    }

    #[test]
    fn format_item_truncates_a_long_path_in_the_middle() {
        let long = format!("/Users/u/Library/Containers/{}", "x".repeat(80));
        let c = OrphanCandidate {
            path: PathBuf::from(&long),
            name: "long".into(),
            kind: ContainerKind::Containers,
            size: 8,
        };
        let s = format_item(&c);
        assert!(s.contains("..."), "middle ellipsis missing: {s}");
        assert!(!s.contains(&long), "full path must be truncated: {s}");
        assert!(s.contains("[Containers]"), "{s}");
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --lib format_item 2>&1 | tail -4`
Expected: FAIL (`format_item_contains_path_size_and_kind` は `c.name` を表示するので `/x/` が引っかからない)

- [ ] **Step 3: format_item を実装する**

`src/cleaners/orphan_containers.rs` の `format_item` (365 行目付近) を次のようにする:

```rust
/// Selection-row width for the path column: paths wider than this ellipsize
/// the middle so the size and kind columns stay aligned.
const ITEM_PATH_WIDTH: usize = 56;

/// Shortens a path to at most `width` chars by cutting the middle:
/// `/Users/u/Library/Containers/xxxx...` becomes
/// `/Users/u/Library/Conta...xxxx`.
fn truncate_path_middle(path: &str, width: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= width {
        return path.to_string();
    }
    let keep = width.saturating_sub(3).max(1);
    let head = keep * 2 / 3;
    let tail = keep - head;
    let head_str: String = chars[..head].iter().collect();
    let tail_str: String = chars[chars.len() - tail..].iter().collect();
    format!("{head_str}...{tail_str}")
}

pub fn format_item(c: &OrphanCandidate) -> String {
    format!(
        "{:<56}  {:>10}  [{}]",
        truncate_path_middle(&c.path.display().to_string(), ITEM_PATH_WIDTH),
        format_bytes(c.size),
        c.kind.label()
    )
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test --lib format_item 2>&1 | tail -4`
Expected: PASS (2 テスト)

- [ ] **Step 5: コミット**

```bash
git add src/cleaners/orphan_containers.rs
git commit -m "feat(orphan-containers): show full paths in the selection list

The selection list showed folder names only, so what would be deleted
was not visible before confirming. Show the full path, ellipsizing the
middle of long ones so the size and kind columns stay aligned."
```

---

### Task 4: スピナーに経過時間タイマーを常時表示する

**Files:**
- Modify: `src/progress.rs:15-22` (`spinner_style`)
- Modify: `src/progress.rs:176-190` (`with_parallel_scan`)
- Test: `src/progress.rs` (同モジュールの `#[cfg(test)]`)

- [ ] **Step 1: 失敗するテストを書く**

`src/progress.rs` の tests モジュールに足す:

```rust
    #[test]
    fn templates_with_elapsed_time_are_valid() {
        assert!(SPINNER_TEMPLATE.contains("{elapsed}"), "{SPINNER_TEMPLATE}");
        assert!(SCAN_TEMPLATE.contains("{elapsed}"), "{SCAN_TEMPLATE}");
        assert!(
            ProgressStyle::default_spinner().template(SPINNER_TEMPLATE).is_ok(),
            "spinner template must be a valid indicatif template"
        );
        assert!(
            ProgressStyle::default_bar().template(SCAN_TEMPLATE).is_ok(),
            "scan template must be a valid indicatif template"
        );
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test --lib templates_with_elapsed 2>&1 | tail -4`
Expected: FAIL (`SPINNER_TEMPLATE` が未定義でコンパイルエラー、または `{elapsed}` を含まない)

- [ ] **Step 3: テンプレートを定数化して {elapsed} を足す**

`src/progress.rs` の `spinner_style` (15 行目付近) を次のようにする:

```rust
/// Spinner line: `{msg}` is the contextual status the cleaner swaps per
/// deletion, `({elapsed})` is the always-visible elapsed time counter.
const SPINNER_TEMPLATE: &str = "{spinner:.green} {msg} ({elapsed})";

/// Parallel-scan line: same elapsed counter as the spinner.
const SCAN_TEMPLATE: &str = "{spinner:.green} [{bar:20}] {msg} ({elapsed})";

fn spinner_style() -> &'static ProgressStyle {
    static STYLE: OnceLock<ProgressStyle> = OnceLock::new();
    STYLE.get_or_init(|| {
        ProgressStyle::default_spinner()
            .template(SPINNER_TEMPLATE)
            .expect("valid indicatif template")
    })
}
```

`with_parallel_scan` (176 行目付近) の `set_style` を次のようにする:

```rust
    pb.set_style(
        ProgressStyle::default_bar()
            .template(SCAN_TEMPLATE)
            .expect("valid template")
            .progress_chars("=> "),
    );
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test --lib templates_with_elapsed 2>&1 | tail -4`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add src/progress.rs
git commit -m "feat(progress): show the elapsed time counter on every spinner

Align the spinner with the four elements Claude Code shows: spinner,
contextual status, elapsed time counter and in-place update. indicatif's
{elapsed} placeholder makes the counter always visible and updates the
line every second, so no phase can sit silently for 10+ seconds."
```

---

### Task 5: 最終検証

**Files:** 変更なし (検証のみ)

- [ ] **Step 1: 全テストとゲート**

```bash
cargo test --lib 2>&1 | grep "^test result"
cargo test --test orphan_containers 2>&1 | grep "^test result"
cargo fmt --check && echo "fmt: CLEAN"
cargo clippy --all-targets 2>&1 | grep -cE "^warning|^error"
```

Expected: lib は既知の TCC 依存 3-4 件以外 0 failed、orphan_containers 0 failed、fmt CLEAN、clippy 0

- [ ] **Step 2: dry-run E2E でフルパスを確認する**

```bash
cargo build 2>&1 | tail -1
./target/debug/sasurahime clean orphan-containers --dry-run 2>&1 | head -6
```

Expected: `would remove:` 行にフルパス (`/Users/.../Library/...`) が出る。中間省略は 56 文字超のパスでのみ効く

- [ ] **Step 3: 手元確認 (TTY)**

手動で TTY から実行し、次の 3 点を確認する:

```bash
./target/debug/sasurahime clean orphan-containers
```

1. 選択プロンプトの上に "Moved items may not show \"Put Back\"..." の 1 行が出る
2. 選択リストにフルパスが出る
3. 削除確定後、スピナーに `({elapsed})` が付いて 1 秒ごとに更新される。TCC ダイアログは出ない

Expected: ハングせず即座に完了する (Group Containers 2 件なら数秒)

- [ ] **Step 4: push**

```bash
git push -u origin feat/orphan-trash-ux 2>&1 | tail -1
```

Expected: `feat/orphan-trash-ux` が push される
