use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use trash::macos::{DeleteMethod, TrashContextExtMacos};

static TRASH_MODE: AtomicBool = AtomicBool::new(true);

/// Deletion mode carried through the clean flow. `Global` reads the
/// process-global mode that `main` pins once per run; `Trash` and `Permanent`
/// pin the mode so `uses_trash` and the actual deletion can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashMode {
    /// Read the process-global mode (pinned once per run by `main`).
    Global,
    /// Move to Trash.
    Trash,
    /// Delete permanently.
    Permanent,
}

impl TrashMode {
    /// Resolves `Global` against the process-global mode.
    pub fn resolve(self) -> bool {
        match self {
            TrashMode::Global => TRASH_MODE.load(Ordering::Relaxed),
            TrashMode::Trash => true,
            TrashMode::Permanent => false,
        }
    }
}

/// Deletion context: carries the deletion mode as an argument instead of
/// reading process-global state, so parallel tests never leak modes into each
/// other. New code should prefer this over the global wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteCtx {
    pub trash_mode: TrashMode,
}

impl DeleteCtx {
    pub fn new(trash_mode: TrashMode) -> Self {
        Self { trash_mode }
    }

    /// Deletes `path` according to this context: macOS Trash when trash mode
    /// is on, otherwise a permanent remove. Failures are typed
    /// ([`crate::cleaner::DeleteError`]) so the skip/fatal classification
    /// matches on variant, never on message text (PBI 59).
    pub fn delete_path(&self, path: &Path) -> Result<(), crate::cleaner::DeleteError> {
        if self.trash_mode.resolve() {
            #[cfg(test)]
            if trash_backend_override_enabled() {
                return Err(crate::cleaner::DeleteError::TrashRefused(
                    "simulated refusing backend (test override)".to_string(),
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
                .map_err(|e| crate::cleaner::DeleteError::TrashRefused(e.to_string()))
        } else if path.is_dir() {
            fs::remove_dir_all(path).map_err(crate::cleaner::classify_io_error)
        } else {
            fs::remove_file(path).map_err(crate::cleaner::classify_io_error)
        }
    }
}

pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}

// ── test-only backend override ──

#[cfg(test)]
static TRASH_BACKEND_OVERRIDE: AtomicBool = AtomicBool::new(false);

/// Simulates a refusing trash backend so the seam's skip classification and
/// claims can be exercised deterministically. The platform backends (Finder
/// via osascript, `trashItemAtURL` via trashd) move entries out of read-only
/// parent directories on modern macOS, so a chmod alone no longer forces the
/// refusal the tests need. Install under TEST_LOCK; the guard restores the
/// default backend on drop.
#[cfg(test)]
pub fn set_trash_backend_override(enabled: bool) {
    TRASH_BACKEND_OVERRIDE.store(enabled, Ordering::Relaxed);
}

#[cfg(test)]
fn trash_backend_override_enabled() -> bool {
    TRASH_BACKEND_OVERRIDE.load(Ordering::Relaxed)
}

/// Installs the test-only override and restores the default backend on drop.
#[cfg(test)]
pub struct TrashBackendOverrideGuard;

#[cfg(test)]
impl TrashBackendOverrideGuard {
    pub fn set() -> Self {
        set_trash_backend_override(true);
        Self
    }
}

#[cfg(test)]
impl Drop for TrashBackendOverrideGuard {
    fn drop(&mut self) {
        set_trash_backend_override(false);
    }
}

/// Outcome of a single deletion through the [`safe_delete`] seam. The seam
/// returns its own truth: `uses_trash` is the mode that was actually
/// resolved at deletion time, so a cleaner can never report Trash for a
/// permanent removal, and a skip carries the typed [`DeleteError`]
/// classification, not prose (PBI 59).
#[derive(Debug)]
pub struct DeleteOutcome {
    /// Whether the resolved mode moved the entry to the Trash.
    pub uses_trash: bool,
    /// Set when the delete failed with a skippable error (a refused trash
    /// move, permission denied, resource busy, ...): the entry was NOT
    /// deleted, but the clean run should continue. Fatal errors propagate as
    /// `Err` instead. The classification is typed — matched on by variant.
    pub skip_reason: Option<crate::cleaner::DeleteError>,
}

/// The one deletion seam: first enforces the scope check
/// ([`crate::safety::is_safe_delete_target`]) so HOME, its
/// ancestors, system roots and the mounted/shared roots (/Volumes, /net,
/// /Users/Shared, /Library, /opt, /private/var/vm) are refused before
/// anything is touched, then clears macOS `uchg` immutable flags before a
/// permanent remove (warning and continuing when the clear fails), deletes
/// according to `mode`, and classifies failures into skippable vs fatal.
///
/// Every PathCleaner-based deletion in the codebase goes through here so the
/// CLAUDE.md safety rules concentrate in one module instead of living as
/// copy-pasted incantations at each call site. External tools delete
/// themselves outside this seam (`uv cache prune`, `brew cleanup`,
/// `ollama rm` and friends run their own removals via
/// [`crate::command::CommandRunner`]), so uchg handling, mode resolution and
/// skip classification are the seam's job for the deletions it mediates, and
/// the enforced scope check keeps the deletion safety rules (which paths may
/// be deleted at all) in that same single module.
pub fn safe_delete(
    runner: &dyn crate::command::CommandRunner,
    path: &Path,
    mode: TrashMode,
) -> Result<DeleteOutcome> {
    // Scope tripwire: refuse paths that must never be deleted (HOME, its
    // ancestors, system roots, relative targets). This runs before the
    // chflags call and the delete itself, so a misdirected override (e.g.
    // ACT_CACHE_DIR=$HOME) can never reach the deletion sink.
    if !crate::safety::is_safe_delete_target(path) {
        log::warn!("refusing to delete unsafe path: {}", path.display());
        anyhow::bail!("refusing to delete unsafe path: {}", path.display());
    }
    // The uchg rule guards permanent removal (`rm -rf`). Under trash mode no
    // chflags is run: the vendored `trash` crate (5.2.6) performs no flag
    // handling itself — its default Finder backend spawns `osascript -e 'tell
    // application "Finder" to delete { POSIX file "..." }'` and its
    // NsFileManager backend calls `NSFileManager::trashItemAtURL`, both
    // delegating the move (and any flag behavior) outside the crate, so
    // whether Finder clears or refuses a uchg-locked item is not decidable
    // from the crate source. If the backend refuses, the error reaches this
    // crate as the typed [`DeleteError::TrashRefused`] variant, which
    // [`DeleteError::is_skippable`] classifies as skippable: the locked
    // entry is skipped and the clean run continues.
    if !mode.resolve() {
        let path_str = path.to_string_lossy();
        if let Err(e) = runner.run("chflags", &["-R", "nouchg", &path_str]) {
            log::warn!("chflags -R nouchg {} failed: {e}", path.display());
        }
    }
    let uses_trash = mode.resolve();
    match DeleteCtx::new(mode).delete_path(path) {
        Ok(()) => Ok(DeleteOutcome {
            uses_trash,
            skip_reason: None,
        }),
        // The classification is typed (PBI 59): matched on by variant, so a
        // reworded message can never flip a skip into a fatal failure.
        Err(e) => {
            if e.is_skippable() {
                Ok(DeleteOutcome {
                    uses_trash,
                    skip_reason: Some(e),
                })
            } else {
                Err(anyhow::Error::new(e))
            }
        }
    }
}

/// The one delete loop: runs [`safe_delete`] per entry with progress ticks,
/// records failures as skipped entries (a fatal per-entry error never aborts
/// the run), sums the freed bytes, and reports the seam's `uses_trash` truth.
///
/// Every cleaner that deletes a list of scanned paths goes through here so
/// the loop's contracts concentrate in one place instead of living as
/// copy-pasted loops at each call site: a vanished path is reported and the
/// remaining entries still process, a skippable failure is a `SkippedEntry`,
/// and `uses_trash` is the resolved mode at deletion time. `items` pairs each
/// path with its scanned byte size (the value the run reports as freed);
/// `label` names the cleaner in the per-entry log lines.
pub fn delete_entries(
    label: &'static str,
    runner: &dyn crate::command::CommandRunner,
    reporter: &dyn crate::progress::ProgressReporter,
    mode: TrashMode,
    items: &[(std::path::PathBuf, u64)],
) -> crate::cleaner::CleanResult {
    let mut freed: u64 = 0;
    let mut uses_trash = false;
    let mut skipped = Vec::new();
    let mut deleted_paths = Vec::new();
    reporter.progress_init(label, items.len());
    for (i, (path, size)) in items.iter().enumerate() {
        reporter.progress_tick(path, i + 1, *size);
        let outcome = match safe_delete(runner, path, mode) {
            Ok(outcome) => outcome,
            // A fatal per-entry error (e.g. the path vanished between scan
            // and delete) must not abort the whole run: report it and
            // continue with the remaining entries.
            Err(err) => {
                eprintln!("[{label}] error removing {}: {err:#}", path.display());
                skipped.push(crate::cleaner::SkippedEntry {
                    path: path.clone(),
                    reason: crate::cleaner::SkipCause::Reason(format!("{err:#}")),
                });
                continue;
            }
        };
        if let Some(reason) = outcome.skip_reason {
            eprintln!("[{label}] error removing {}: {reason}", path.display());
            skipped.push(crate::cleaner::SkippedEntry {
                path: path.clone(),
                reason: crate::cleaner::SkipCause::Delete(reason),
            });
            continue;
        }
        uses_trash |= outcome.uses_trash;
        freed += size;
        deleted_paths.push(path.clone());
        println!(
            "[{label}] removed: {} (freed {})",
            path.display(),
            crate::format::format_bytes(*size)
        );
    }
    reporter.progress_finish();
    crate::cleaner::CleanResult {
        name: label,
        bytes_freed: freed,
        uses_trash,
        skipped,
        deleted_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // DeleteCtx-based tests carry the mode as an argument: no global, no
    // TEST_LOCK, safe to run fully in parallel.

    #[test]
    fn delete_ctx_trash_mode_moves_to_trash() {
        let ctx = DeleteCtx::new(TrashMode::Trash);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("dir_to_trash");
        fs::create_dir_all(&d).unwrap();
        ctx.delete_path(&d).unwrap();
        assert!(!d.exists(), "trash mode removes the source path");
    }

    #[test]
    fn delete_ctx_permanent_mode_removes_directory() {
        let ctx = DeleteCtx::new(TrashMode::Permanent);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("testdir");
        fs::create_dir_all(&d).unwrap();
        ctx.delete_path(&d).unwrap();
        assert!(!d.exists(), "directory must be removed");
    }

    #[test]
    fn delete_ctx_permanent_mode_removes_file() {
        let ctx = DeleteCtx::new(TrashMode::Permanent);
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("test.txt");
        fs::write(&f, b"hello").unwrap();
        ctx.delete_path(&f).unwrap();
        assert!(!f.exists(), "file must be removed in permanent mode");
    }

    #[test]
    fn delete_ctx_trash_mode_returns_error_on_failure() {
        let ctx = DeleteCtx::new(TrashMode::Trash);
        let result = ctx.delete_path(Path::new("/nonexistent/path/that/cannot/be/trashed"));
        assert!(result.is_err(), "trash of nonexistent path must return Err");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("trash"),
            "error must mention trash, got: {err}"
        );
    }

    #[test]
    fn delete_ctx_permanent_mode_returns_error_on_missing_path() {
        let ctx = DeleteCtx::new(TrashMode::Permanent);
        let result = ctx.delete_path(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err(), "removing a missing path must return Err");
    }

    #[test]
    fn delete_ctx_is_copiable_and_comparable() {
        let trash_ctx = DeleteCtx::new(TrashMode::Trash);
        let perm_ctx = DeleteCtx::new(TrashMode::Permanent);
        assert_eq!(trash_ctx, DeleteCtx::new(TrashMode::Trash));
        assert_ne!(trash_ctx, perm_ctx);
    }

    #[test]
    fn global_trash_mode_pin_round_trips() {
        // The process-global pin is read by `TrashMode::Global` (the seam's
        // compat path); one locked test covers the setter/getter pair.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        set_trash_mode(true);
        assert!(is_trash_mode());
        set_trash_mode(false);
        assert!(!is_trash_mode());
        set_trash_mode(true);
        assert!(is_trash_mode());
    }

    // ── safe_delete seam tests ──

    /// Records every (program, args) invocation so the seam's chflags
    /// behavior is assertable. Fails for programs listed in `fail_programs`.
    struct RecordingRunner {
        calls: std::sync::Mutex<Vec<(String, Vec<String>)>>,
        fail_programs: Vec<String>,
    }

    impl RecordingRunner {
        fn new() -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
                fail_programs: Vec::new(),
            }
        }

        fn that_fails(mut self, program: &str) -> Self {
            self.fail_programs.push(program.to_string());
            self
        }

        fn ran(&self, program: &str) -> bool {
            self.calls.lock().unwrap().iter().any(|(p, _)| p == program)
        }
    }

    impl crate::command::CommandRunner for RecordingRunner {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            self.calls.lock().unwrap().push((
                program.to_string(),
                args.iter().map(|a| a.to_string()).collect(),
            ));
            if self.fail_programs.iter().any(|p| p == program) {
                anyhow::bail!("failed to spawn `{program}`: command failed");
            }
            Ok(std::process::Output {
                status: std::os::unix::process::ExitStatusExt::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }

        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    #[test]
    fn safe_delete_permanent_mode_clears_uchg_then_removes() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("uchg_dir");
        fs::create_dir_all(&d).unwrap();
        let runner = RecordingRunner::new();
        let outcome = safe_delete(&runner, &d, TrashMode::Permanent).unwrap();
        assert!(
            runner.ran("chflags"),
            "permanent mode must clear uchg flags before removing"
        );
        assert!(!d.exists(), "directory must be removed");
        assert!(!outcome.uses_trash, "permanent delete must not claim trash");
        assert!(outcome.skip_reason.is_none());
    }

    #[test]
    fn safe_delete_permanent_mode_removes_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("file.txt");
        fs::write(&f, b"hi").unwrap();
        let runner = RecordingRunner::new();
        let outcome = safe_delete(&runner, &f, TrashMode::Permanent).unwrap();
        assert!(runner.ran("chflags"));
        assert!(!f.exists());
        assert!(!outcome.uses_trash);
    }

    #[test]
    fn safe_delete_chflags_failure_warns_and_still_deletes() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("dir");
        fs::create_dir_all(&d).unwrap();
        let runner = RecordingRunner::new().that_fails("chflags");
        let outcome = safe_delete(&runner, &d, TrashMode::Permanent).unwrap();
        assert!(
            runner.ran("chflags"),
            "the clear attempt must still be made"
        );
        assert!(!d.exists(), "deletion proceeds after a failed flag clear");
        assert!(outcome.skip_reason.is_none());
    }

    #[test]
    fn safe_delete_skippable_error_returns_skip_reason() {
        // A Trash-mode delete of a nonexistent path fails with the typed
        // TrashRefused classification, which is skippable; a NotFound in
        // permanent mode is fatal by design.
        let runner = RecordingRunner::new();
        let outcome = safe_delete(
            &runner,
            Path::new("/nonexistent/path/xyz"),
            TrashMode::Trash,
        )
        .unwrap();
        let reason = outcome.skip_reason.expect("trash failure is skippable");
        assert!(
            matches!(reason, crate::cleaner::DeleteError::TrashRefused(_)),
            "the skip must carry the typed TrashRefused classification: {:?}",
            reason
        );
    }

    #[test]
    fn safe_delete_trash_mode_reports_trash_truth() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("dir");
        fs::create_dir_all(&d).unwrap();
        let runner = RecordingRunner::new();
        let outcome = safe_delete(&runner, &d, TrashMode::Trash).unwrap();
        assert!(outcome.uses_trash, "trash mode must report trash");
        // Whether the underlying `trash` crate succeeds and whether the dir
        // actually moves are the documented TCC-environmental test's job.
    }

    #[test]
    fn safe_delete_global_mode_resolves_global_pin() {
        // Global defers to the process-global pin, so a locked test is needed.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("dir");
        fs::create_dir_all(&d).unwrap();
        let runner = RecordingRunner::new();
        set_trash_mode(true);
        let outcome = safe_delete(&runner, &d, TrashMode::Global).unwrap();
        assert!(outcome.uses_trash);
        // The trash move's success is TCC-environment-dependent; the seam's
        // contract under test is only the resolved truth.
    }

    // ── safe_delete scope tripwire (PBI 28) ──

    #[test]
    fn safe_delete_refuses_home_itself() {
        // The refusal must happen BEFORE any deletion, so HOME is pointed at
        // a throwaway tempdir: even a tripwire-less build can only lose that
        // tempdir, never a real home directory. TEST_LOCK serializes the
        // process-global HOME mutation against the other HOME-scoped tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        let runner = RecordingRunner::new();
        let err = safe_delete(&runner, &home, TrashMode::Permanent)
            .expect_err("HOME itself must be refused before any deletion attempt");
        assert!(
            err.to_string().contains("refusing to delete unsafe path"),
            "the refusal error must say so, got: {err:#}"
        );
        assert!(home.exists(), "the refused path must still exist");
        assert!(
            !runner.ran("chflags"),
            "no chflags may run on a refused path"
        );
    }

    #[test]
    fn safe_delete_refuses_filesystem_root() {
        let runner = RecordingRunner::new();
        let err = safe_delete(&runner, Path::new("/"), TrashMode::Trash)
            .expect_err("/ must be refused before any deletion attempt");
        assert!(
            err.to_string().contains("refusing to delete unsafe path"),
            "the refusal error must say so, got: {err:#}"
        );
        assert!(Path::new("/").exists(), "the refused path must still exist");
        assert!(
            !runner.ran("chflags"),
            "no chflags may run on a refused path"
        );
    }

    #[test]
    fn safe_delete_refuses_etc() {
        let runner = RecordingRunner::new();
        let err = safe_delete(&runner, Path::new("/etc"), TrashMode::Trash)
            .expect_err("/etc must be refused before any deletion attempt");
        assert!(
            err.to_string().contains("refusing to delete unsafe path"),
            "the refusal error must say so, got: {err:#}"
        );
        assert!(
            Path::new("/etc").exists(),
            "the refused path must still exist"
        );
        assert!(
            !runner.ran("chflags"),
            "no chflags may run on a refused path"
        );
    }

    // ── NsFileManager backend source pin ──

    #[test]
    fn delete_ctx_delete_path_selects_the_nsfilemanager_backend() {
        // Source pin (the same style as progress.rs's steady-tick pins): the
        // trash branch must keep selecting DeleteMethod::NsFileManager. The
        // default Finder backend spawns `osascript`, which can sit for
        // minutes on a hidden TCC permission dialog -- the 36-minute hang
        // this choice removed.
        let src = include_str!("trash.rs");
        let start = src
            .find("pub fn delete_path(&self, path: &Path)")
            .expect("DeleteCtx::delete_path source present");
        let end = src[start..]
            .find("pub fn set_trash_mode")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            body.contains("set_delete_method(DeleteMethod::NsFileManager)"),
            "DeleteCtx::delete_path must keep selecting the NsFileManager backend (no osascript, no TCC dialog)"
        );
        assert!(
            !body.contains("DeleteMethod::Finder"),
            "the default Finder backend must not come back"
        );
    }

    // ── shared delete loop (PBI 56) ──

    #[test]
    fn delete_entries_continues_past_a_vanished_entry_and_sums_the_rest() {
        // The shared loop's contract: a fatal per-entry error (a path that
        // vanished between scan and delete) is reported as a SkippedEntry and
        // the remaining entries still delete; the freed sum and the seam's
        // uses_trash truth come out of the same loop every cleaner uses.
        // The global pin is permanent here: a Permanent resolve must never
        // claim Trash.
        let _guard = crate::test_helpers::TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        for d in [&a, &b] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("cache.bin"), vec![0u8; 1024]).unwrap();
        }
        fs::remove_dir_all(&a).unwrap(); // the vanished first entry
        let runner = crate::test_helpers::MockRunner::new().with_success("chflags");
        let items = vec![(a.clone(), 1024), (b.clone(), 1024)];
        let r = delete_entries(
            "loop-test",
            &runner,
            &crate::progress::DeepSuppressReporter,
            TrashMode::Permanent,
            &items,
        );
        assert!(!b.exists(), "the surviving entry must still delete");
        assert!(!a.exists(), "the vanished entry stays vanished");
        assert_eq!(r.bytes_freed, 1024, "only the surviving entry is summed");
        assert_eq!(r.skipped.len(), 1, "the vanished entry is a skip");
        assert_eq!(r.skipped[0].path, a);
        assert!(!r.uses_trash, "a permanent resolve must never claim Trash");
        assert_eq!(r.deleted_paths, vec![b], "the deleted path is recorded");
    }

    #[test]
    fn delete_entries_empty_items_is_a_zero_result() {
        let runner = crate::test_helpers::MockRunner::new();
        let r = delete_entries(
            "loop-test",
            &runner,
            &crate::progress::DeepSuppressReporter,
            TrashMode::Global,
            &[],
        );
        assert_eq!(r.bytes_freed, 0);
        assert!(r.skipped.is_empty());
        assert!(!r.uses_trash);
    }

    #[test]
    fn delete_entries_all_skipped_reports_exit_code_one() {
        // Pinned intentionally (review 2026-10-29): the shared loop records
        // skips, so a run where every entry is skipped exits 1. v0.4.2's
        // ios-backup/device-support loops recorded no skip and exited 0 in
        // the same situation - the flip is the loop's documented contract,
        // not an accident, and scripts gating on the exit code rely on it.
        let _backend = TrashBackendOverrideGuard::set();
        let runner = crate::test_helpers::MockRunner::new().with_success("chflags");
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        fs::create_dir_all(&a).unwrap();
        let r = delete_entries(
            "loop-test",
            &runner,
            &crate::progress::DeepSuppressReporter,
            TrashMode::Trash,
            &[(a, 1024)],
        );
        assert_eq!(r.bytes_freed, 0, "a skipped entry is never summed");
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.exit_code(), 1, "all-skipped reports failure");
    }

    #[test]
    fn delete_entries_fatal_error_does_not_abort_the_remaining_entries() {
        // Pinned intentionally (review 2026-10-29): a fatal per-entry error
        // (an unsafe path here) is recorded as a skipped entry and the run
        // continues. v0.4.2's ios-backup/device-support loops propagated the
        // fatal error with `?` and aborted the whole clean; the shared loop
        // converges on continue (the same contract every other cleaner uses).
        let _guard = crate::test_helpers::TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("cache.bin"), vec![0u8; 1024]).unwrap();
        let runner = crate::test_helpers::MockRunner::new().with_success("chflags");
        // A relative path never passes is_safe_delete_target: safe_delete
        // bails fatally, and the loop must skip it, not abort.
        let items = vec![
            (std::path::PathBuf::from("relative/unsafe"), 100),
            (real.clone(), 1024),
        ];
        let r = delete_entries(
            "loop-test",
            &runner,
            &crate::progress::DeepSuppressReporter,
            TrashMode::Permanent,
            &items,
        );
        assert!(!real.exists(), "the safe entry still deletes");
        assert_eq!(r.bytes_freed, 1024, "the safe entry is summed");
        assert_eq!(r.skipped.len(), 1, "the unsafe path is a skip");
        assert_eq!(
            r.skipped[0].path,
            std::path::PathBuf::from("relative/unsafe")
        );
        assert!(
            matches!(&r.skipped[0].reason, crate::cleaner::SkipCause::Reason(_)),
            "the fatal bail is a prose skip: {:?}",
            r.skipped[0].reason
        );
    }
}
