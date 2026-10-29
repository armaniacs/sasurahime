use anyhow::Result;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use trash::macos::{DeleteMethod, TrashContextExtMacos};

/// Deletion mode carried through the clean flow. `main` resolves it once
/// per run (`--permanent` or config.trash_mode = false pins `Permanent`)
/// and the resolved value flows down as an argument from there on, so
/// `uses_trash` and the actual deletion can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrashMode {
    /// Move to Trash.
    Trash,
    /// Delete permanently.
    Permanent,
}

impl Default for TrashMode {
    /// The historical default: move to Trash (the static's
    /// `AtomicBool::new(true)`).
    fn default() -> Self {
        TrashMode::Trash
    }
}

/// Deletion context: carries the resolved deletion mode as an argument
/// instead of reading process-global state, so parallel tests never leak
/// modes into each other. New code should prefer this over any global.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteCtx {
    pub trash_mode: TrashMode,
}

impl DeleteCtx {
    pub fn new(trash_mode: TrashMode) -> Self {
        Self { trash_mode }
    }

    /// Deletes `path` according to this context: macOS Trash when this
    /// context carries the trash mode, otherwise a permanent remove.
    /// Failures are typed ([`crate::cleaner::DeleteError`]) so the
    /// skip/fatal classification matches on variant, never on message
    /// text (PBI 59).
    pub fn delete_path(&self, path: &Path) -> Result<(), crate::cleaner::DeleteError> {
        if self.trash_mode == TrashMode::Trash {
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
/// `set` takes TEST_LOCK and holds it until Drop: two parallel tests can
/// never install the override at the same time, so one guard's release can
/// never clear the flag under another test's run (the lock-free release
/// window that flaked under `cargo test --lib`).
#[cfg(test)]
pub struct TrashBackendOverrideGuard {
    /// Held for the guard's whole lifetime: the override is installed and
    /// cleared while the lock is held, so a parallel override user blocks
    /// on `set` instead of racing the flag.
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl TrashBackendOverrideGuard {
    pub fn set() -> Self {
        let lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        set_trash_backend_override(true);
        Self { _lock: lock }
    }
}

#[cfg(test)]
impl Drop for TrashBackendOverrideGuard {
    fn drop(&mut self) {
        // Cleared while the struct's lock field is still alive: a parallel
        // test cannot acquire TEST_LOCK between the clear and the release,
        // so it never observes the cleared flag as its own install.
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
///
/// Crate-internal since the ledger migration (PBI 77): the cleaner layer
/// rides [`seam_delete`] / [`delete_entries`] instead of calling this seam
/// directly, so only this module and the hint flow (PBI 80) reach it.
pub(crate) fn safe_delete(
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
    if mode == TrashMode::Permanent {
        let path_str = path.to_string_lossy();
        if let Err(e) = runner.run("chflags", &["-R", "nouchg", &path_str]) {
            log::warn!("chflags -R nouchg {} failed: {e}", path.display());
        }
    }
    let uses_trash = mode == TrashMode::Trash;
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

/// Outcome of one per-item deletion handed back to the [`delete_ledger`].
/// The ledger owns the loop, so a deletion closure returns its own truth:
/// `Deleted` carries the resolved trash truth (a run that deleted nothing
/// can never claim Trash, PBI 43), and a skip carries either the seam's
/// typed classification or a prose reason (a CLI-delegated deletion's
/// failure — never disguised as a [`crate::cleaner::DeleteError`]). Fatal
/// failures propagate as `Err`; the ledger turns them into a prose skip
/// and continues instead of aborting the run.
#[derive(Debug)]
pub enum DeletionOutcome {
    /// The entry was deleted. `uses_trash` is whether the resolved mode
    /// moved it to the Trash.
    Deleted {
        /// Whether the resolved mode moved the entry to the Trash.
        uses_trash: bool,
    },
    /// The entry was NOT deleted: the seam classified the failure as
    /// skippable (a refused trash move, a permission denial, a busy
    /// target).
    Skipped(crate::cleaner::DeleteError),
    /// The entry was NOT deleted, with a prose reason (a failed CLI
    /// deletion such as `ollama rm`).
    SkippedReason(String),
}

/// The seam adapter: fixes [`safe_delete`] as the per-item deletion a
/// [`delete_ledger`] runs and maps the seam's outcome into the ledger's.
/// Every path-deleting cleaner rides this adapter (or a CLI adapter like
/// ollama's `ollama rm` closure), so the uchg rule, the scope tripwire,
/// mode resolution and skip classification stay the seam's job for every
/// deletion the ledger mediates.
pub fn seam_delete(
    runner: &dyn crate::command::CommandRunner,
    mode: TrashMode,
) -> impl FnMut(&Path, u64) -> Result<DeletionOutcome> + '_ {
    move |path: &Path, _size: u64| {
        let outcome = safe_delete(runner, path, mode)?;
        match outcome.skip_reason {
            None => Ok(DeletionOutcome::Deleted {
                uses_trash: outcome.uses_trash,
            }),
            Some(reason) => Ok(DeletionOutcome::Skipped(reason)),
        }
    }
}

/// The one delete ledger: the loop every cleaner's per-entry deletions
/// ride, so the bookkeeping contract concentrates in one interface instead
/// of living as hand-rolled loops at each call site. Per item it ticks the
/// reporter, runs the caller's deletion closure, and owns the whole
/// ledger contract: a fatal closure error is recorded as a skipped entry
/// (prose reason) and the run continues, a skippable or prose skip is
/// recorded as a `SkippedEntry`, `freed` only sums entries whose deletion
/// succeeded, `uses_trash` is the OR-in of the outcomes' truth
/// (deletion successes only, PBI 43), and `progress_init` runs with
/// `items.len()` —
/// the count of deletable entries only, the PBI 68 pin — before the
/// ticks. An all-skipped ledger reports failure through
/// [`crate::cleaner::CleanResult::exit_code`].
///
/// `items` pairs each path with the byte count the run reports as freed
/// when its deletion succeeds; the caller owns the per-item size
/// computation and display labels (the ledger never re-walks a path).
/// `label` names the cleaner in the progress and per-entry log lines.
/// dry-run never reaches the ledger: callers print their own dry-run
/// lines and return before calling it. `seam_delete` is the adapter for
/// path deletions via [`safe_delete`]; a CLI-delegated deletion (like
/// `ollama rm`) rides the same ledger through its own closure.
pub fn delete_ledger(
    label: &'static str,
    reporter: &dyn crate::progress::ProgressReporter,
    items: &[(std::path::PathBuf, u64)],
    mut delete: impl FnMut(&Path, u64) -> Result<DeletionOutcome>,
) -> crate::cleaner::CleanResult {
    let mut freed: u64 = 0;
    let mut uses_trash = false;
    let mut skipped = Vec::new();
    reporter.progress_init(label, items.len());
    for (i, (path, size)) in items.iter().enumerate() {
        reporter.progress_tick(path, i + 1, *size);
        let outcome = match delete(path, *size) {
            Ok(outcome) => outcome,
            // A fatal per-item error (e.g. the path vanished between scan
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
        match outcome {
            DeletionOutcome::Deleted { uses_trash: used } => {
                // OR-in only when the deletion actually happened: a skipped
                // move never claims Trash (PBI 43).
                uses_trash |= used;
                freed += size;
                println!(
                    "[{label}] removed: {} (freed {})",
                    path.display(),
                    crate::format::format_bytes(*size)
                );
            }
            DeletionOutcome::Skipped(reason) => {
                eprintln!("[{label}] error removing {}: {reason}", path.display());
                skipped.push(crate::cleaner::SkippedEntry {
                    path: path.clone(),
                    reason: crate::cleaner::SkipCause::Delete(reason),
                });
            }
            DeletionOutcome::SkippedReason(reason) => {
                eprintln!("[{label}] error removing {}: {reason}", path.display());
                skipped.push(crate::cleaner::SkippedEntry {
                    path: path.clone(),
                    reason: crate::cleaner::SkipCause::Reason(reason),
                });
            }
        }
    }
    reporter.progress_finish();
    crate::cleaner::CleanResult {
        name: label,
        bytes_freed: freed,
        uses_trash,
        skipped,
    }
}

/// The seam-fixed convenience wrapper over [`delete_ledger`]: pairs every
/// item with the [`safe_delete`] seam via [`seam_delete`]. The seam users
/// keep this exact shape; cleaners whose deletion is not path-based (or
/// whose per-item behavior differs, like `ollama rm`) call
/// [`delete_ledger`] with their own closure instead.
pub fn delete_entries(
    label: &'static str,
    runner: &dyn crate::command::CommandRunner,
    reporter: &dyn crate::progress::ProgressReporter,
    mode: TrashMode,
    items: &[(std::path::PathBuf, u64)],
) -> crate::cleaner::CleanResult {
    delete_ledger(label, reporter, items, seam_delete(runner, mode))
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
    fn trash_mode_default_is_trash() {
        // The Default contract (PBI 71): the concrete mode defaults to
        // Trash, matching the static's historical AtomicBool::new(true).
        assert_eq!(TrashMode::default(), TrashMode::Trash);
        assert_ne!(TrashMode::default(), TrashMode::Permanent);
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
            .find("// ── test-only backend override ──")
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

    // ── ledger bypass source pin (PBI 77) ──

    #[test]
    fn the_ledger_users_may_not_call_safe_delete_directly() {
        // Source pin (the same style as the NsFileManager pin above): the 5
        // ledger users (uv, project_artifacts, cli_fallback, generic,
        // ollama) must ride delete_entries / delete_ledger with the
        // seam_delete closure and never call safe_delete directly — the
        // ledger owns the per-entry contracts, and a direct call bypasses
        // them. The seam itself is crate-internal: only this module and the
        // hint flow (PBI 80) reach it.
        let self_src = include_str!("trash.rs");
        // The pinned literal is split across a format call so this test's
        // own source does not contain it and the assertion cannot
        // self-satisfy.
        let pinned = format!("pub{} fn safe_delete(", "(crate)");
        assert!(
            self_src.contains(&pinned),
            "safe_delete must stay crate-internal (pub(crate)); the ledger users ride the adapter"
        );
        for (file, src) in [
            ("uv.rs", include_str!("cleaners/uv.rs")),
            (
                "project_artifacts.rs",
                include_str!("cleaners/project_artifacts.rs"),
            ),
            ("cli_fallback.rs", include_str!("cli_fallback.rs")),
            ("generic.rs", include_str!("cleaners/generic.rs")),
            ("ollama.rs", include_str!("cleaners/ollama.rs")),
        ] {
            assert!(
                !src.contains("trash::safe_delete"),
                "{file} must not call trash::safe_delete directly — ride the delete ledger instead"
            );
        }
    }

    // ── shared delete loop (PBI 56) ──

    #[test]
    fn delete_entries_continues_past_a_vanished_entry_and_sums_the_rest() {
        // The shared loop's contract: a fatal per-entry error (a path that
        // vanished between scan and delete) is reported as a SkippedEntry and
        // the remaining entries still delete; the freed sum and the seam's
        // uses_trash truth come out of the same loop every cleaner uses.
        // The mode is pinned permanent here: a Permanent resolve must never
        // claim Trash.
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
    }

    #[test]
    fn delete_entries_empty_items_is_a_zero_result() {
        let runner = crate::test_helpers::MockRunner::new();
        let r = delete_entries(
            "loop-test",
            &runner,
            &crate::progress::DeepSuppressReporter,
            TrashMode::Trash,
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

    // ── delete_ledger contract (PBI 70) ──

    #[test]
    fn delete_ledger_fatal_closure_error_is_a_prose_skip_and_continues() {
        // The generalized ledger's contract, driven through a custom
        // closure (no seam involved): a fatal per-item error is recorded as
        // a prose skip and the remaining entries still delete. The ledger
        // owns the loop, so no call site can reintroduce a `?` abort.
        let tmp = TempDir::new().unwrap();
        let fatal = tmp.path().join("fatal");
        let survivor = tmp.path().join("survivor");
        fs::create_dir_all(&survivor).unwrap();
        fs::write(survivor.join("cache.bin"), vec![0u8; 512]).unwrap();
        let items = vec![(fatal.clone(), 1024), (survivor.clone(), 512)];
        let r = delete_ledger(
            "ledger-test",
            &crate::progress::DeepSuppressReporter,
            &items,
            |path: &Path, _size: u64| {
                if path.file_name().and_then(|n| n.to_str()) == Some("fatal") {
                    anyhow::bail!("simulated fatal deletion failure");
                }
                fs::remove_dir_all(path)?;
                Ok(DeletionOutcome::Deleted { uses_trash: false })
            },
        );
        assert!(!survivor.exists(), "the surviving entry still deletes");
        assert_eq!(r.bytes_freed, 512, "only the surviving entry is summed");
        assert_eq!(r.skipped.len(), 1, "the fatal entry is a skip");
        assert_eq!(r.skipped[0].path, fatal);
        assert!(
            matches!(&r.skipped[0].reason, crate::cleaner::SkipCause::Reason(reason) if reason.contains("simulated fatal")),
            "the fatal error's prose rides in the skip: {:?}",
            r.skipped[0].reason
        );
    }

    #[test]
    fn delete_ledger_prose_skip_records_the_reason_and_exit_code_one() {
        // A CLI-delegated deletion's contract (ollama rm): the failure
        // keeps its prose reason — never disguised as a DeleteError — a
        // skipped entry is never summed, no Trash claim is made, and an
        // all-skipped ledger reports failure (exit 1).
        let tmp = TempDir::new().unwrap();
        let model = tmp.path().join("llama3.2:3b");
        let items = vec![(model.clone(), 2048)];
        let r = delete_ledger(
            "cli-test",
            &crate::progress::DeepSuppressReporter,
            &items,
            |_path: &Path, _size: u64| {
                Ok(DeletionOutcome::SkippedReason(
                    "ollama rm exited with 1".to_string(),
                ))
            },
        );
        assert_eq!(r.bytes_freed, 0, "a skipped entry is never summed");
        assert!(!r.uses_trash, "a skipped CLI deletion never claims Trash");
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.skipped[0].path, model);
        assert!(
            matches!(&r.skipped[0].reason, crate::cleaner::SkipCause::Reason(reason) if reason == "ollama rm exited with 1"),
            "the CLI failure's prose rides in the skip: {:?}",
            r.skipped[0].reason
        );
        assert_eq!(r.exit_code(), 1, "all-skipped reports failure");
    }

    #[test]
    fn delete_ledger_deletions_sum_freed() {
        // The success path: freed sums the items' sizes, uses_trash is the
        // OR-in of the outcomes' truth.
        let tmp = TempDir::new().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        for d in [&a, &b] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("cache.bin"), vec![0u8; 256]).unwrap();
        }
        let items = vec![(a.clone(), 256), (b.clone(), 512)];
        let r = delete_ledger(
            "ledger-test",
            &crate::progress::DeepSuppressReporter,
            &items,
            |path: &Path, _size: u64| {
                fs::remove_dir_all(path)?;
                Ok(DeletionOutcome::Deleted { uses_trash: true })
            },
        );
        assert_eq!(r.bytes_freed, 256 + 512);
        assert!(r.uses_trash, "the outcomes' truth is OR-in");
        assert!(r.skipped.is_empty());
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn delete_ledger_empty_items_is_a_zero_result() {
        let r = delete_ledger(
            "ledger-test",
            &crate::progress::DeepSuppressReporter,
            &[],
            |_: &Path, _: u64| unreachable!("no items, no deletions"),
        );
        assert_eq!(r.bytes_freed, 0);
        assert!(r.skipped.is_empty());
        assert!(!r.uses_trash);
        assert_eq!(r.exit_code(), 0);
    }
}
