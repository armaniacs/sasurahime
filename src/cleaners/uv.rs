use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::cleaners::retention::{is_version_dir, keep_highest};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
#[cfg(test)]
use crate::test_helpers::MockRunner;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct UvCleaner {
    cache_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl UvCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cache_dir: home.join(".cache/uv"),
            runner,
        }
    }

    /// Parses "simple-v16" → Some(16), anything else → None.
    pub fn parse_simple_version(name: &str) -> Option<u32> {
        name.strip_prefix("simple-v")?.parse().ok()
    }

    /// Returns paths of simple-vN directories that are older than the highest N found.
    pub fn detect_old_indexes(&self) -> Vec<PathBuf> {
        let entries = match fs::read_dir(&self.cache_dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        // design-it-twice (PBI-85): keep-highest was duplicated here as a
        // hand-rolled len() <= 1 short-circuit + max + retain. Plan A
        // (adopted) borrows the shared pub(crate) keep_highest (gradle.rs's
        // at PBI-85 time; moved to retention.rs by the retention-extract
        // refactor) — the one helper already shared by gradle, browsers and
        // JetBrains (the browsers.rs precedent) — so gradle keeps every line
        // and the gradle/browser regression surface stays zero. A single u32
        // key wraps in vec![n]: a one-element lexicographic comparison equals
        // the numeric one, and the helper's "single candidate → empty"
        // contract absorbs the old short-circuit. Plan B (moving the helper
        // into a common retention module) is deferred to its own PBI once all
        // consumers line up.
        let candidates: Vec<(Vec<u32>, PathBuf)> = entries
            .filter_map(|e| e.ok())
            // GAP-006 (PBI-85): only real directories may become deletion
            // candidates. A regular file named simple-vN is never an index
            // dir, a symlink is a stale / shared link, and an unreadable file
            // type is never touched — the shared is_version_dir (retention.rs)
            // owns all three rules (the browsers cleaner's rule, borrowed the
            // same way).
            .filter(is_version_dir)
            .filter_map(|e| {
                let name = e.file_name();
                let n = Self::parse_simple_version(&name.to_string_lossy())?;
                Some((vec![n], e.path()))
            })
            .collect();

        keep_highest(candidates)
    }
}

impl Cleaner for UvCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

    fn is_available(&self) -> bool {
        self.runner.exists("uv")
    }

    fn name(&self) -> &'static str {
        "uv"
    }

    fn detect(&self) -> ScanResult {
        if !self.cache_dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes = dir_size(&self.cache_dir);
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.cache_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        if !self.runner.exists("uv") {
            println!("uv: not found, skipping");
            return Ok(CleanResult::empty(self.name()));
        }

        // Phase 1 (PBI 77): the simple-vN index deletions ride the delete
        // ledger, which owns the per-entry contracts (progress, skip
        // records, freed sums, CleanResult assembly). There is no
        // caller-side unsafe-path guard here: the seam's scope tripwire
        // refuses the same paths at deletion time, and the ledger records
        // the refusal as a skipped entry (prose reason) and continues — a
        // fatal per-item error no longer aborts the clean before the prune.
        let items: Vec<(PathBuf, u64)> = self
            .detect_old_indexes()
            .into_iter()
            .map(|p| {
                let size = dir_size(&p);
                (p, size)
            })
            .collect();
        if dry_run {
            for (path, _) in &items {
                println!("[dry-run] would remove: {}", path.display());
            }
            println!("[dry-run] would run: uv cache prune --force");
            return Ok(CleanResult::empty(self.name()));
        }
        let phase1 = crate::trash::delete_entries(
            self.name(),
            self.runner.as_ref(),
            reporter,
            opts.trash_mode,
            &items,
        );

        // Phase 2 (PBI 77): prune unused package archives via uv itself.
        // The `before` measurement sits AFTER the ledger so the phase-1
        // deletions are never counted twice — once in the ledger sum and
        // once in the delta; the delta only captures the prune's effect.
        let before = dir_size(&self.cache_dir);
        let output = self.runner.run("uv", &["cache", "prune", "--force"])?;
        if !output.status.success() {
            // PBI 53: a failed prune silently contributes 0 to `freed`
            // (the delta over a failed prune is 0). Warn so the prune
            // failure is not invisible.
            log::warn!(
                "uv cache prune --force exited with {} - the prune was not applied",
                output.status.code().unwrap_or(-1)
            );
        }
        let after = dir_size(&self.cache_dir);
        let phase2 = before.saturating_sub(after);

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: phase1.bytes_freed + phase2,
            uses_trash: phase1.uses_trash,
            skipped: phase1.skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::SkipCause;
    use std::os::unix::process::ExitStatusExt;
    use tempfile::TempDir;

    // ── ledger migration (PBI 77) ──

    /// A runner that records every invocation, so the prune phase running
    /// after a refused index deletion is observable. `uv` exists and
    /// succeeds; `chflags` is permitted for the seam's permanent-mode clear.
    #[derive(Default, Clone)]
    struct RecordingUvRunner {
        calls: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }
    impl CommandRunner for RecordingUvRunner {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, program: &str) -> bool {
            program == "uv" || program == "chflags"
        }
    }

    #[test]
    fn clean_freed_is_the_ledger_sum_plus_the_prune_delta() {
        // Two-phase accounting (PBI 77): phase 1 = the ledger sum for the
        // simple-vN deletions; phase 2 = the dir_size delta measured AFTER
        // the ledger (prune only). The post-ledger before measurement keeps
        // a deleted index from being counted twice — once in the ledger sum
        // and once in the delta.
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v16")).unwrap();
        std::fs::write(cache.join("simple-v16/index.html"), [0u8; 4096]).unwrap();
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();
        std::fs::write(cache.join("simple-v21/index.html"), [0u8; 8192]).unwrap();

        let runner = MockRunner::new().with_success("uv").with_success("chflags");
        let cleaner = UvCleaner::new(tmp.path(), Box::new(runner));
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let r = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        assert!(
            !cache.join("simple-v16").exists(),
            "the old index must delete"
        );
        assert!(
            cache.join("simple-v21").exists(),
            "the newest index must stay"
        );
        assert_eq!(
            r.bytes_freed, 4096,
            "phase 1 sums the deleted index once; phase 2 adds the prune delta (0 for a mock prune) — never 2x"
        );
        assert!(!r.uses_trash, "a permanent resolve must never claim Trash");
        assert!(r.skipped.is_empty());
    }

    #[test]
    fn clean_fatal_index_error_does_not_abort_the_prune_phase() {
        // The hand-rolled loop propagated a fatal per-item error with `?`,
        // aborting the whole clean — the prune phase never ran. The ledger
        // records the refusal as a SkippedEntry (prose reason) and the run
        // continues into the prune. A tempdir under /tmp never passes
        // is_safe_delete_target (the canonicalized path starts with
        // /private/tmp), so both index dirs are refused by the seam.
        let tmp = tempfile::TempDir::new_in("/tmp").unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v16")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();

        let runner = RecordingUvRunner::default();
        let cleaner = UvCleaner::new(tmp.path(), Box::new(runner.clone()));
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let r = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .expect("a fatal per-item error must not abort the clean");
        assert_eq!(
            r.skipped.len(),
            1,
            "the refused index is recorded: {:?}",
            r.skipped
        );
        assert_eq!(r.skipped[0].path, cache.join("simple-v16"));
        assert!(
            matches!(&r.skipped[0].reason, SkipCause::Reason(reason) if reason.contains("refusing to delete unsafe path")),
            "the seam refusal rides as prose: {:?}",
            r.skipped[0].reason
        );
        assert_eq!(r.bytes_freed, 0, "a refused path frees nothing");
        assert_eq!(r.exit_code(), 1, "exit 1 rides the skip record, not an Err");
        let calls = runner.calls.lock().unwrap();
        assert_eq!(
            *calls,
            vec!["uv cache prune --force".to_string()],
            "the prune phase still ran after the refusal: {calls:?}"
        );
        assert!(
            cache.join("simple-v16").exists() && cache.join("simple-v21").exists(),
            "the refused dir and the newest index must survive"
        );
    }

    #[test]
    fn parse_simple_version_valid() {
        assert_eq!(UvCleaner::parse_simple_version("simple-v16"), Some(16));
        assert_eq!(UvCleaner::parse_simple_version("simple-v21"), Some(21));
    }

    #[test]
    fn parse_simple_version_invalid() {
        assert_eq!(UvCleaner::parse_simple_version("archive-v0"), None);
        assert_eq!(UvCleaner::parse_simple_version("simple-vabc"), None);
        assert_eq!(UvCleaner::parse_simple_version(""), None);
    }

    #[test]
    fn detect_old_indexes_returns_all_but_highest() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v16")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v17")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let old = cleaner.detect_old_indexes();
        assert_eq!(old.len(), 2);
        let names: Vec<_> = old
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"simple-v16".to_string()));
        assert!(names.contains(&"simple-v17".to_string()));
    }

    #[test]
    fn detect_old_indexes_single_version_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    // ── GAP-006: symlinks and files are skipped ────────────────────────────
    #[test]
    fn detect_old_indexes_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();
        let target = tmp.path().join("actual-dir");
        std::fs::create_dir_all(&target).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, cache.join("simple-v99")).unwrap();
        }

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        // Without the symlink guard, simple-v99 would be the max and simple-v21 would
        // be marked as old. With the guard, only simple-v21 exists → returns empty.
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    #[test]
    fn detect_old_indexes_skips_regular_files() {
        // PBI-85: a regular file named simple-vN must never become a deletion
        // candidate. The former single-entry version of this test was vacuous
        // (the len() <= 1 short-circuit returned empty before the file ever
        // mattered), so it is rewritten into this 3-way coexistence: on the
        // pre-fix code the file enters the candidate set through the
        // parse_simple_version name match and this fails.
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v16")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();
        std::fs::write(cache.join("simple-v17"), b"not a dir").unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let old = cleaner.detect_old_indexes();
        let names: Vec<_> = old
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(
            !names.contains(&"simple-v17".to_string()),
            "a regular file named simple-vN is never a deletion candidate: {names:?}"
        );
        assert!(
            names.contains(&"simple-v16".to_string()),
            "the older real index stays a deletion candidate: {names:?}"
        );
        assert!(
            !names.contains(&"simple-v21".to_string()),
            "the newest real index is kept: {names:?}"
        );
    }

    #[test]
    fn detect_old_indexes_regular_file_alone_returns_empty() {
        // Detection edge (PBI-85): a lone regular file named simple-vN yields
        // no candidates — the keep-highest contract returns empty for a
        // single candidate.
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("simple-v21"), b"not a dir").unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    #[test]
    fn detect_old_indexes_file_as_fake_highest_keeps_real_index() {
        // BDD scenario (PBI-85): a regular file simple-v99 posing as the
        // highest generation must not mark the real index simple-v21 as old —
        // the file is filtered out before the keep-highest computation.
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();
        std::fs::write(cache.join("simple-v99"), b"not a dir").unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(
            cleaner.detect_old_indexes().is_empty(),
            "a fake-highest file must not put the real index into the candidates"
        );
    }

    #[test]
    fn detect_old_indexes_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        // cache dir does not exist
        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    // ── detect() size coverage ──────────────────────────────────────────────

    #[test]
    fn detect_measures_full_cache_dir() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        // archive-v0 — previously the only thing detect() measured
        std::fs::create_dir_all(cache.join("archive-v0")).unwrap();
        std::fs::write(cache.join("archive-v0/pkg.tar.gz"), [0u8; 4096]).unwrap();
        // simple-vN index — was missed before the fix
        std::fs::create_dir_all(cache.join("simple-v17")).unwrap();
        std::fs::write(cache.join("simple-v17/index.html"), [0u8; 1024]).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();

        let expected = crate::format::dir_size(&cache);
        match result.status {
            ScanStatus::Pruneable(bytes) => assert_eq!(bytes, expected),
            other => panic!("expected Pruneable, got {other:?}"),
        }
    }

    #[test]
    fn detect_returns_not_found_when_cache_missing() {
        let tmp = TempDir::new().unwrap();
        // cache dir does not exist
        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::NotFound),
            "expected NotFound, got {:#?}",
            result.status
        );
    }

    #[test]
    fn clean_reports_zero_freed_when_prune_exits_nonzero() {
        // PBI 53 pin: a failed `uv cache prune --force` silently contributes
        // 0 to `freed` (the warn branch). The failure must not bubble up as
        // an error and must not fabricate freed bytes.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".cache/uv/simple-v21")).unwrap();

        let cleaner = UvCleaner::new(
            tmp.path(),
            Box::new(MockRunner::new().with_exit_code("uv", 1 << 8)),
        );
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .expect("a failed prune must not fail the run");
        assert_eq!(
            result.bytes_freed, 0,
            "a failed prune must not report freed bytes"
        );
        assert!(result.skipped.is_empty(), "nothing was deleted");
        assert!(!result.uses_trash);
    }
}
