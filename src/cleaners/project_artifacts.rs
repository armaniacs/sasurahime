use crate::cleaner::{CleanResult, Cleaner, Risk, ScanResult, ScanStatus};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Artifact directory name + adjacent project manifest pairs. An artifact
/// directory is only detected when its manifest sits next to it; `dist` is
/// intentionally out of scope for the first cut.
pub const ARTIFACT_RULES: &[(&str, &str)] = &[
    ("node_modules", "package.json"),
    ("target", "Cargo.toml"),
    (".build", "Package.swift"),
];

/// Directories never inspected for project recency: artifact trees are huge
/// (double-walk cost) and VCS internals do not count as user activity.
const RECENCY_SKIP_DIRS: &[&str] = &["node_modules", "target", ".build", ".git"];

pub fn manifest_for(artifact_name: &str) -> Option<&'static str> {
    ARTIFACT_RULES
        .iter()
        .find(|(artifact, _)| *artifact == artifact_name)
        .map(|(_, manifest)| *manifest)
}

/// Pure recency decision: a project whose newest source update is no older
/// than `days` counts as recent. Exactly `days` old counts as recent
/// (conservative: kept).
pub fn is_recent(mtime: SystemTime, now: SystemTime, days: u32) -> bool {
    now.duration_since(mtime)
        .map(|age| age <= Duration::from_secs(u64::from(days) * 86_400))
        .unwrap_or(true)
}

fn now() -> SystemTime {
    SystemTime::now()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedArtifact {
    pub path: PathBuf,
    pub size: u64,
}

pub struct ProjectArtifactsCleaner {
    roots: Vec<PathBuf>,
    max_depth: usize,
    recency_days: u32,
    /// Global size filter: projects smaller than this are skipped.
    larger_than_mb: Option<u64>,
}

impl ProjectArtifactsCleaner {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            max_depth: 5,
            recency_days: 7,
            larger_than_mb: None,
        }
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn with_recency_days(mut self, recency_days: u32) -> Self {
        self.recency_days = recency_days;
        self
    }

    fn detect_artifacts(&self) -> Vec<DetectedArtifact> {
        self.detect_artifacts_inner(true)
    }

    /// `use_cache` is false for the deletion path: removed sizes and the
    /// size==0 skip must never ride on scan-cache freshness.
    fn detect_artifacts_inner(&self, use_cache: bool) -> Vec<DetectedArtifact> {
        let mut out = Vec::new();
        for root in &self.roots {
            if !root.exists() {
                continue;
            }
            let mut it = walkdir::WalkDir::new(root)
                .max_depth(self.max_depth)
                .into_iter();
            while let Some(entry) = it.next() {
                let Ok(entry) = entry else { continue };
                if !entry.file_type().is_dir() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().to_string();
                let Some(manifest) = manifest_for(&name) else {
                    continue;
                };
                // Do not descend into a detected artifact tree.
                it.skip_current_dir();
                let Some(project_dir) = entry.path().parent() else {
                    continue;
                };
                // The manifest must be a real file: a symlinked manifest is
                // not evidence the directory belongs to a project here.
                let is_real_manifest = std::fs::symlink_metadata(project_dir.join(manifest))
                    .map(|m| m.is_file())
                    .unwrap_or(false);
                if !is_real_manifest {
                    continue;
                }
                if self.project_is_recent(project_dir) {
                    continue;
                }
                let size = if use_cache {
                    crate::scan_cache::cached_dir_size(entry.path())
                } else {
                    crate::format::dir_size(entry.path())
                };
                if size == 0 {
                    continue;
                }
                if let Some(mb) = self.larger_than_mb {
                    if size < mb.saturating_mul(1_048_576) {
                        continue;
                    }
                }
                out.push(DetectedArtifact {
                    path: entry.path().to_path_buf(),
                    size,
                });
            }
        }
        out
    }

    /// Newest source mtime in the project, from FILE mtimes only: directory
    /// mtimes are contaminated by artifact creation (creating node_modules
    /// updates the parent dir's mtime). Artifact trees and VCS internals are
    /// never descended into. Unknown mtimes count as recent (conservative).
    fn project_is_recent(&self, project_dir: &Path) -> bool {
        let now = now();
        let newest = walkdir::WalkDir::new(project_dir)
            .max_depth(self.max_depth)
            .into_iter()
            .filter_entry(|e| {
                if !e.file_type().is_dir() {
                    return true;
                }
                let name = e.file_name().to_string_lossy().to_string();
                !RECENCY_SKIP_DIRS.contains(&name.as_str())
            })
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok())
            .filter_map(|m| m.modified().ok())
            .max();
        match newest {
            Some(mtime) => is_recent(mtime, now, self.recency_days),
            // Nothing measurable: treat as recent so nothing is deleted.
            None => true,
        }
    }

    /// The shared clean body. `mode` is the deletion mode for the seam:
    /// `Global` reads the process-global mode; `Trash` / `Permanent` pin it
    /// so the seam outcome and the deletion agree. The reporter is threaded
    /// through to the delete ledger, which owns the per-entry contracts
    /// (progress, skip records, freed sums, CleanResult assembly).
    fn clean_with_mode(
        &self,
        dry_run: bool,
        reporter: &dyn crate::progress::ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        // Caller-specific decisions stay here: the artifact walk and the
        // dry-run printing. There is no caller-side unsafe-path guard here:
        // the seam's scope tripwire refuses the same paths at deletion time,
        // and the ledger records the refusal as a skipped entry (prose
        // reason) and continues — the skip channel and the deleted-path
        // audit trail stay consistent with every other ledger user.
        let items: Vec<(PathBuf, u64)> = self
            .detect_artifacts_inner(false)
            .into_iter()
            .map(|a| (a.path, a.size))
            .collect();
        if dry_run {
            for (path, _) in &items {
                println!("[dry-run] would remove: {}", path.display());
            }
            return Ok(CleanResult::empty(self.name()));
        }
        // The cleaner has no runner field: the seam only needs one for the
        // permanent-mode uchg clear, which the system runner performs.
        Ok(crate::trash::delete_entries(
            self.name(),
            &crate::command::SystemCommandRunner,
            reporter,
            mode,
            &items,
        ))
    }
}

impl Cleaner for ProjectArtifactsCleaner {
    fn name(&self) -> &'static str {
        "project-artifacts"
    }

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn crate::progress::ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        // Stricter wins: the global age filter tightens the recency window,
        // and the global size filter applies to whole project dirs.
        let effective = ProjectArtifactsCleaner {
            roots: self.roots.clone(),
            max_depth: self.max_depth,
            recency_days: self
                .recency_days
                .max(opts.older_than_days.unwrap_or(self.recency_days)),
            larger_than_mb: opts.larger_than_mb,
        };
        // The pinned `opts.trash_mode` must reach the deletion seam so
        // `uses_trash` and the actual deletion can never disagree; `None`
        // keeps the resolved mode.
        let mode = opts.trash_mode;
        effective.clean_with_mode(dry_run, reporter, mode)
    }

    fn detect(&self) -> ScanResult {
        let artifacts = self.detect_artifacts();
        if artifacts.is_empty() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = artifacts.iter().map(|a| a.size).sum();
        ScanResult::new(self.name(), ScanStatus::Pruneable(total))
    }

    fn is_available(&self) -> bool {
        true
    }

    fn advisory(&self) -> Option<String> {
        Some(
            "Protected by mtime only: uncommitted changes are NOT checked. Review `git status` before cleaning."
                .to_string(),
        )
    }

    fn risk(&self) -> Risk {
        Risk::Manual
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::SkipCause;

    fn write_file(dir: &Path, name: &str, bytes: usize) {
        std::fs::write(dir.join(name), vec![0u8; bytes]).unwrap();
    }

    fn make_old(path: &Path) {
        let status = std::process::Command::new("touch")
            .args(["-t", "202001010000"])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn manifest_lookup() {
        assert_eq!(manifest_for("node_modules"), Some("package.json"));
        assert_eq!(manifest_for("target"), Some("Cargo.toml"));
        assert_eq!(manifest_for(".build"), Some("Package.swift"));
        assert_eq!(manifest_for("dist"), None, "dist is out of scope");
        assert_eq!(manifest_for("build"), None);
    }

    #[test]
    fn is_recent_boundary_exactly_seven_days() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
        let exactly = now - Duration::from_secs(7 * 86_400);
        let older = now - Duration::from_secs(7 * 86_400 + 1);
        let fresher = now - Duration::from_secs(7 * 86_400 - 1);
        assert!(is_recent(exactly, now, 7), "exactly 7 days is recent");
        assert!(!is_recent(older, now, 7), "older than 7 days is stale");
        assert!(is_recent(fresher, now, 7));
    }

    #[test]
    fn detects_old_project_artifacts_with_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("old-app");
        std::fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
        write_file(&project, "package.json", 100);
        write_file(&project.join("node_modules/pkg"), "lib.js", 4096);
        write_file(&project, "src.rs", 10);
        make_old(&project.join("src.rs"));
        make_old(&project.join("package.json"));

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
        assert!(result.bytes_for_test() > 0);
        assert_eq!(cleaner.name(), "project-artifacts");
    }

    #[test]
    fn skips_projects_without_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let orphan = tmp.path().join("not-a-project");
        std::fs::create_dir_all(orphan.join("node_modules")).unwrap();
        write_file(&orphan.join("node_modules"), "x.js", 4096);
        // No package.json next to node_modules.

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn skips_recent_projects() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("active-app");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        write_file(&project, "package.json", 100);
        write_file(&project.join("node_modules"), "lib.js", 4096);
        // Fresh mtimes: updated 2 "days" ago is not testable without a clock,
        // but files created now are within any threshold.

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn depth_limit_excludes_deeper_artifacts() {
        let tmp = tempfile::TempDir::new().unwrap();
        // node_modules 7 levels below the root: beyond the default depth 5.
        let deep = tmp.path().join("a/b/c/d/e/deep-app");
        std::fs::create_dir_all(deep.join("node_modules")).unwrap();
        write_file(&deep, "package.json", 100);
        write_file(&deep.join("node_modules"), "x.js", 4096);
        write_file(&deep, "src.rs", 10);
        make_old(&deep.join("package.json"));
        make_old(&deep.join("src.rs"));

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));

        let wider = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]).with_max_depth(10);
        assert!(matches!(wider.detect().status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn clean_dry_run_deletes_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("old-app");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        write_file(&project, "package.json", 100);
        write_file(&project.join("node_modules"), "x.js", 4096);
        make_old(&project.join("package.json"));

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(project.join("node_modules").exists());
    }

    #[test]
    fn risk_is_manual_and_not_auto_clean_allowed() {
        let cleaner = ProjectArtifactsCleaner::new(vec![PathBuf::from("/nonexistent")]);
        assert_eq!(cleaner.risk(), Risk::Manual);
        assert!(!cleaner.auto_clean_allowed());
        assert!(cleaner.is_available());
    }

    #[test]
    fn symlinked_manifest_is_not_a_manifest() {
        // A symlinked package.json is not a real manifest: the artifact dir
        // next to it must not be detected.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/stale.js"), b"code").unwrap();
        std::fs::write(project.join("package.json"), b"{}").unwrap();
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));
        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        assert_eq!(
            cleaner.detect_artifacts().len(),
            1,
            "a real manifest detects the artifact"
        );
        // Replace the manifest with a symlink to an outside file.
        let outside = tmp.path().join("outside.json");
        std::fs::write(&outside, b"{}").unwrap();
        std::fs::remove_file(project.join("package.json")).unwrap();
        std::os::unix::fs::symlink(&outside, project.join("package.json")).unwrap();
        assert_eq!(
            cleaner.detect_artifacts().len(),
            0,
            "a symlinked manifest must not count"
        );
    }

    #[test]
    fn clean_uses_fresh_sizes_not_the_cache() {
        // The deletion path must not ride on scan-cache freshness: a poisoned
        // cache entry claiming size 0 must not suppress the removal.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/stale.js"), b"code").unwrap();
        std::fs::write(project.join("package.json"), b"{}").unwrap();
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));
        // Poison the cache: the artifact dir claims size 0.
        let cache_path = tmp.path().join("cache.json");
        let mut cache = crate::scan_cache::ScanCache::default();
        cache.record(&project.join("node_modules"), 0);
        cache.save(&cache_path);
        crate::scan_cache::init(&cache_path);
        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let reporter = crate::progress::DeepSuppressReporter;
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            !project.join("node_modules").exists(),
            "the stale artifact must be removed even with a poisoned cache"
        );
        assert!(
            result.bytes_freed > 0,
            "freed must use the fresh size, not the cached 0"
        );
    }

    #[test]
    fn project_is_recent_walk_respects_max_depth() {
        // A source file deeper than max_depth is invisible to the recency
        // walk, which must conservatively count the project as recent.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("proj");
        let deep = project.join("a/b/c/d/e/f");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("old.rs"), b"code").unwrap();
        make_old(&deep.join("old.rs"));
        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        assert!(
            cleaner.project_is_recent(&project),
            "a source beyond max_depth counts as recent (conservative)"
        );
        // Within max_depth the file is seen and the project counts as old.
        let cleaner3 =
            ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]).with_max_depth(4);
        std::fs::create_dir_all(project.join("a/b/c")).unwrap();
        std::fs::write(project.join("a/b/c/old.rs"), b"code").unwrap();
        make_old(&project.join("a/b/c/old.rs"));
        assert!(
            !cleaner3.project_is_recent(&project),
            "a source within max_depth makes the project old"
        );
    }

    #[test]
    fn clean_with_opts_applies_global_filters() {
        // The global age filter must tighten the recency window: a project
        // 10 days old is deletable under recency_days=7 but kept when the
        // policy demands older_than_days=30.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/stale.js"), b"code").unwrap();
        std::fs::write(project.join("package.json"), b"{}").unwrap();
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));
        // Newest source: 10 days old.
        let ten_days_ago = {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 10 * 86_400;
            let (y, mo, d, h, mi) = epoch_to_touch(secs);
            format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}")
        };
        std::fs::write(project.join("src.rs"), b"code").unwrap();
        let status = std::process::Command::new("touch")
            .args(["-t", &ten_days_ago])
            .arg(project.join("src.rs"))
            .status()
            .unwrap();
        assert!(status.success());

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let reporter = crate::progress::DeepSuppressReporter;
        let permanent = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        // Plain clean (recency_days=7): the project is deletable. The opts
        // pin Permanent: the deletion must land regardless of the sandbox's
        // TCC stance on the trash backend.
        cleaner.clean(false, &reporter, &permanent).unwrap();
        assert!(
            !project.join("node_modules").exists(),
            "recency_days=7 deletes a 10-day-old project"
        );
        // Rebuild and apply the stricter global filter.
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/stale.js"), b"code").unwrap();
        make_old(&project.join("node_modules"));
        std::fs::write(project.join("src.rs"), b"code").unwrap();
        let status = std::process::Command::new("touch")
            .args(["-t", &ten_days_ago])
            .arg(project.join("src.rs"))
            .status()
            .unwrap();
        assert!(status.success());
        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: Some(30),
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Permanent,
        };
        cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            project.join("node_modules").exists(),
            "the global older_than_days=30 must keep a 10-day-old project"
        );
    }

    // ── trash_mode pin contract ──

    /// An old one-artifact project under `tmp` plus its cleaner: the newest
    /// source file is the aged `package.json`, so the artifact is deletable.
    fn old_artifact_cleaner(tmp: &tempfile::TempDir) -> ProjectArtifactsCleaner {
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/stale.js"), b"code").unwrap();
        std::fs::write(project.join("package.json"), b"{}").unwrap();
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));
        ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()])
    }

    #[test]
    fn clean_with_opts_pinned_permanent_deletes_for_real() {
        // The CleanOptions.trash_mode pin contract: a pinned Permanent must
        // delete for real and claim no Trash.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = old_artifact_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Permanent,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            !tmp.path().join("proj/node_modules").exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !result.uses_trash,
            "uses_trash must come from the pinned mode, not the global"
        );
    }

    #[test]
    fn clean_with_opts_pinned_trash_claims_trash() {
        // A pinned Trash must resolve the deletion seam to trash. The
        // global is pinned to permanent so only the pin can claim Trash.
        // The seam outcome is the one owner of the claim: a deleted outcome
        // claims Trash, a skipped one (the TCC-denied move in this sandbox)
        // does not — either way the assertion follows the seam truth.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = old_artifact_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Trash,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        let deletions_happened = result.skipped.is_empty();
        assert_eq!(
            result.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    #[test]
    fn clean_options_default_carries_trash() {
        // The CleanOptions default carries the concrete Trash mode (the
        // historical default). The seam outcome is the one owner of the
        // claim: a deleted outcome claims Trash, a skipped one (the
        // TCC-denied move in this sandbox) does not — either way the
        // assertion follows the seam truth.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = old_artifact_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions::default();
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        let deletions_happened = result.skipped.is_empty();
        assert_eq!(
            result.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    // ── ledger migration (PBI 77) ──

    /// A reporter that records the progress calls, so the threading of the
    /// reporter from `clean` into the delete ledger is observable.
    struct RecordingReporter {
        inits: std::sync::Mutex<Vec<(String, usize)>>,
        ticks: std::sync::Mutex<Vec<(PathBuf, usize, u64)>>,
        finishes: std::sync::atomic::AtomicUsize,
    }

    impl Default for RecordingReporter {
        fn default() -> Self {
            Self {
                inits: std::sync::Mutex::new(Vec::new()),
                ticks: std::sync::Mutex::new(Vec::new()),
                finishes: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl crate::progress::ProgressReporter for RecordingReporter {
        fn show_spinner(&self) -> bool {
            false
        }
        fn progress_init(&self, label: &str, total: usize) {
            self.inits.lock().unwrap().push((label.to_string(), total));
        }
        fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64) {
            self.ticks
                .lock()
                .unwrap()
                .push((path.to_path_buf(), current, size_bytes));
        }
        fn progress_finish(&self) {
            self.finishes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn clean_threads_the_reporter_into_the_ledger() {
        // progress_init carries the label and items.len() (PBI 68) and every
        // deletion ticks: the reporter is threaded from clean into
        // clean_with_mode through to delete_entries. Before the ledger
        // migration the reporter had no path into clean_with_mode, so the
        // progress tick was structurally absent for project-artifacts.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("old-app");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        write_file(&project, "package.json", 100);
        write_file(&project.join("node_modules"), "x.js", 4096);
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let reporter = RecordingReporter::default();
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        cleaner.clean(false, &reporter, &opts).unwrap();
        let inits = reporter.inits.lock().unwrap();
        assert_eq!(
            *inits,
            vec![("project-artifacts".to_string(), 1)],
            "progress_init runs once with the label and items.len()"
        );
        let ticks = reporter.ticks.lock().unwrap();
        assert_eq!(ticks.len(), 1, "one tick per deleted artifact: {ticks:?}");
        assert_eq!(ticks[0].0, project.join("node_modules"));
    }

    #[test]
    fn clean_records_an_unsafe_artifact_path_as_a_skipped_entry() {
        // The caller-side unsafe-path pre-check printed a bare warning and
        // recorded nothing, so the skip channel and the exit code could not
        // see the refusal. The seam's scope tripwire refuses the same path at
        // deletion time and the ledger records it as a skipped entry (prose
        // reason) — same as every other ledger user (generic.rs precedent).
        // A tempdir under /tmp never passes is_safe_delete_target (the
        // canonicalized path starts with /private/tmp), so the detected
        // artifact is refused by the seam, not deleted.
        let tmp = tempfile::TempDir::new_in("/tmp").unwrap();
        let project = tmp.path().join("proj");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        write_file(&project, "package.json", 100);
        write_file(&project.join("node_modules"), "x.js", 4096);
        make_old(&project.join("package.json"));
        make_old(&project.join("node_modules"));

        let cleaner = ProjectArtifactsCleaner::new(vec![tmp.path().to_path_buf()]);
        let reporter = crate::progress::DeepSuppressReporter;
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            project.join("node_modules").exists(),
            "an unsafe path must survive"
        );
        assert_eq!(result.bytes_freed, 0, "a refused path frees nothing");
        assert_eq!(
            result.skipped.len(),
            1,
            "the refusal must be recorded as a SkippedEntry: {:?}",
            result.skipped
        );
        assert_eq!(result.skipped[0].path, project.join("node_modules"));
        assert!(
            matches!(&result.skipped[0].reason, SkipCause::Reason(reason) if reason.contains("refusing to delete unsafe path")),
            "the seam refusal rides as prose: {:?}",
            result.skipped[0].reason
        );
        assert_eq!(
            result.exit_code(),
            1,
            "the refusal must surface through the exit code"
        );
    }

    /// Formats seconds since epoch as a `touch -t` stamp (YYYYMMDDHHMM).
    fn epoch_to_touch(secs: u64) -> (u64, u64, u64, u64, u64) {
        let days = secs / 86_400;
        let rem = secs % 86_400;
        let (h, mi) = (rem / 3600, (rem % 3600) / 60);
        // Civil-from-days (Howard Hinnant's algorithm).
        let z = days as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        (y as u64, m as u64, d as u64, h, mi)
    }
}
