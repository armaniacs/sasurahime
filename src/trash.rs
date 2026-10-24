use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

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
    /// is on, otherwise a permanent remove.
    pub fn delete_path(&self, path: &Path) -> Result<()> {
        if self.trash_mode.resolve() {
            trash::delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
        } else if path.is_dir() {
            fs::remove_dir_all(path)
                .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
        } else {
            fs::remove_file(path).map_err(|e| anyhow::anyhow!("remove_file {:?}: {}", path, e))
        }
    }
}

pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}

/// Compat wrapper over [`DeleteCtx`]: reads the process-global mode that
/// `main` sets once per run. Cleaner-level call sites keep using this until
/// the trait carries a `DeleteCtx`.
pub fn delete_path(path: &Path) -> Result<()> {
    DeleteCtx::new(TrashMode::Global).delete_path(path)
}

/// Outcome of a single deletion through the [`safe_delete`] seam. The seam
/// returns its own truth: `uses_trash` is the mode that was actually
/// resolved at deletion time, so a cleaner can never report Trash for a
/// permanent removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteOutcome {
    /// Whether the resolved mode moved the entry to the Trash.
    pub uses_trash: bool,
    /// Set when the delete failed with a skippable error (permission denied,
    /// resource busy, ...): the entry was NOT deleted, but the clean run
    /// should continue. Fatal errors propagate as `Err` instead.
    pub skip_reason: Option<String>,
}

/// The one deletion seam: first enforces the scope check
/// ([`crate::cleaners::generic::is_safe_delete_target`]) so HOME, its
/// ancestors, system roots and the mounted/shared roots (/Volumes, /net,
/// /Users/Shared, /Library, /opt, /private/var/vm) are refused before
/// anything is touched, then clears macOS `uchg` immutable flags before a
/// permanent remove (warning and continuing when the clear fails), deletes
/// according to `mode`, and classifies failures into skippable vs fatal.
///
/// Every deletion in the codebase goes through here so the CLAUDE.md safety
/// rules concentrate in one module instead of living as copy-pasted
/// incantations at each call site: uchg handling, mode resolution and skip
/// classification are the seam's job, and the enforced scope check keeps the
/// deletion safety rules (which paths may be deleted at all) in that same
/// single module.
pub fn safe_delete(
    runner: &dyn crate::command::CommandRunner,
    path: &Path,
    mode: TrashMode,
) -> Result<DeleteOutcome> {
    // Scope tripwire: refuse paths that must never be deleted (HOME, its
    // ancestors, system roots, relative targets). This runs before the
    // chflags call and the delete itself, so a misdirected override (e.g.
    // ACT_CACHE_DIR=$HOME) can never reach the deletion sink.
    if !crate::cleaners::generic::is_safe_delete_target(path) {
        log::warn!("refusing to delete unsafe path: {}", path.display());
        anyhow::bail!("refusing to delete unsafe path: {}", path.display());
    }
    // The uchg rule guards permanent removal (`rm -rf`); a Trash move is
    // reversible and Finder handles flags itself.
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
        Err(e) => {
            if crate::cleaner::is_skippable_error(&e) {
                Ok(DeleteOutcome {
                    uses_trash,
                    skip_reason: Some(format!("{e:#}")),
                })
            } else {
                Err(e)
            }
        }
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
    fn compat_wrapper_reads_the_global_mode() {
        // The wrapper is the only global reader left; one locked test covers it.
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
        // A Trash-mode delete of a nonexistent path fails with a
        // "trash failed: ..." error, which is_skippable_error classifies as
        // skippable; a NotFound in permanent mode is fatal by design.
        let runner = RecordingRunner::new();
        let outcome = safe_delete(
            &runner,
            Path::new("/nonexistent/path/xyz"),
            TrashMode::Trash,
        )
        .unwrap();
        let reason = outcome.skip_reason.expect("trash failure is skippable");
        assert!(
            reason.contains("trash"),
            "skip reason must mention the cause: {reason}"
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
}
