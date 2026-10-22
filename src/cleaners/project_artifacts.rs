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
}

impl ProjectArtifactsCleaner {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            max_depth: 5,
            recency_days: 7,
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
                if !project_dir.join(manifest).exists() {
                    continue;
                }
                if self.project_is_recent(project_dir) {
                    continue;
                }
                let size = crate::scan_cache::cached_dir_size(entry.path());
                if size == 0 {
                    continue;
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
}

impl Cleaner for ProjectArtifactsCleaner {
    fn name(&self) -> &'static str {
        "project-artifacts"
    }

    fn detect(&self) -> ScanResult {
        let artifacts = self.detect_artifacts();
        if artifacts.is_empty() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = artifacts.iter().map(|a| a.size).sum();
        ScanResult::new(self.name(), ScanStatus::Pruneable(total))
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn crate::progress::ProgressReporter,
    ) -> Result<CleanResult> {
        let mut freed: u64 = 0;
        for artifact in self.detect_artifacts() {
            if dry_run {
                println!("[dry-run] would remove: {}", artifact.path.display());
                continue;
            }
            if !crate::cleaners::generic::is_safe_delete_target(&artifact.path) {
                eprintln!(
                    "[project-artifacts] warning: unsafe path, skipping: {}",
                    artifact.path.display()
                );
                continue;
            }
            crate::trash::delete_path(&artifact.path)?;
            freed += artifact.size;
            println!("Removed: {}", artifact.path.display());
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: crate::trash::is_trash_mode(),
            skipped: vec![],
            deleted_paths: vec![],
        })
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
        let result = cleaner.clean(true, &reporter).unwrap();
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
}
