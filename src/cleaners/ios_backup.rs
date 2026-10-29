use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use crate::prompt::PromptPort;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct IosCleaner {
    pub backup_dir: PathBuf,
    pub runner: Box<dyn CommandRunner>,
}

impl IosCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            backup_dir: home.join("Library/Application Support/MobileSync/Backup"),
            runner,
        }
    }
}

impl Cleaner for IosCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Risky
    }

    fn name(&self) -> &'static str {
        "ios-backup"
    }

    // Deleting an iOS backup is irreversible, and its selection prompt would
    // otherwise accept exactly one Enter, so `--yes` must never pick it.
    fn auto_clean_allowed(&self) -> bool {
        false
    }

    fn detect(&self) -> ScanResult {
        if !self.backup_dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = match fs::read_dir(&self.backup_dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| crate::format::dir_size(&e.path()))
                .sum(),
            Err(_) => 0,
        };
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.backup_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        if !self.backup_dir.exists() {
            return Ok(CleanResult::empty(self.name()));
        }

        let entries: Vec<(PathBuf, u64)> = match fs::read_dir(&self.backup_dir) {
            Ok(r) => r
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| {
                    let size = crate::format::dir_size(&e.path());
                    (e.path(), size)
                })
                .filter(|(_, size)| *size > 0)
                .collect(),
            Err(_) => vec![],
        };

        if entries.is_empty() {
            println!("[ios-backup] nothing to clean");
            return Ok(CleanResult::empty(self.name()));
        }

        eprintln!("⚠  iOS backups contain personal data (contacts, messages, photos, etc.) and cannot be restored once deleted. Proceed with caution.");
        eprintln!("    iOS バックアップには個人データ（連絡先・メッセージ・写真など）が含まれており、削除後は復元できません。注意して実行してください。");

        if dry_run {
            println!("[ios-backup] dry-run: {} backup(s) found", entries.len());
            for (path, size) in &entries {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                println!(
                    "  would remove: {}  ({})",
                    name,
                    crate::format::format_bytes(*size)
                );
            }
            return Ok(CleanResult::empty(self.name()));
        }

        // Safety: iOS backups are irreversible and must only be deleted
        // interactively. If stdin is not a terminal (e.g. via --yes), refuse.
        if !crate::prompt::gate_interactive("ios-backup") {
            return Ok(CleanResult::empty(self.name()));
        }

        let selected = self.interactive_select(reporter, &entries)?;
        if selected.is_empty() {
            println!("[ios-backup] nothing selected");
            return Ok(CleanResult::empty(self.name()));
        }

        // The shared delete ledger owns the per-entry contracts. The mode is
        // Global: the deletion scope was already gated interactively above.
        let result = crate::trash::delete_entries(
            self.name(),
            self.runner.as_ref(),
            reporter,
            crate::trash::TrashMode::Global,
            &selected,
        );
        println!(
            "[ios-backup] total freed: {}",
            crate::format::format_bytes(result.bytes_freed)
        );
        // The full result propagates: skipped entries and deleted paths
        // reach the caller, so an all-skipped run exits 1 (the same
        // contract every other ledger user pins) and --verbose keeps the
        // audit trail.
        Ok(result)
    }
}

impl IosCleaner {
    fn interactive_select(
        &self,
        reporter: &dyn ProgressReporter,
        entries: &[(PathBuf, u64)],
    ) -> Result<Vec<(PathBuf, u64)>> {
        let items: Vec<String> = entries
            .iter()
            .map(|(path, size)| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                format!("{:<40}  {}", name, crate::format::format_bytes(*size))
            })
            .collect();
        let defaults = vec![true; entries.len()];
        println!("\niOS device backups in ~/Library/Application Support/MobileSync/Backup/:\n");
        let selections =
            crate::prompt::TtyPrompt.multi_select(reporter, None, &items, &defaults)?;
        Ok(selections.into_iter().map(|i| entries[i].clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use tempfile::TempDir;

    struct FakeRunner;
    impl CommandRunner for FakeRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    fn make_backup_dir(tmp: &TempDir) -> PathBuf {
        let d = tmp
            .path()
            .join("Library/Application Support/MobileSync/Backup");
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_cleaner(tmp: &TempDir) -> IosCleaner {
        IosCleaner::new(tmp.path(), Box::new(FakeRunner))
    }

    // detect tests
    #[test]
    fn detect_returns_not_found_when_dir_absent() {
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            make_cleaner(&tmp).detect().status,
            ScanStatus::NotFound
        ));
    }

    #[test]
    fn detect_returns_clean_when_backup_dir_is_empty() {
        let tmp = TempDir::new().unwrap();
        make_backup_dir(&tmp);
        assert!(matches!(
            make_cleaner(&tmp).detect().status,
            ScanStatus::Clean
        ));
    }

    #[test]
    fn detect_returns_pruneable_when_backups_present() {
        let tmp = TempDir::new().unwrap();
        let backup_dir = make_backup_dir(&tmp);
        let entry = backup_dir.join("AABBCCDD-EEFF-0011-2233-445566778899");
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join("Manifest.db"), b"fakedata").unwrap();
        assert!(matches!(
            make_cleaner(&tmp).detect().status,
            ScanStatus::Pruneable(_)
        ));
    }

    #[test]
    fn detect_name_is_ios_backup() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(make_cleaner(&tmp).detect().name, "ios-backup");
    }

    // clean(dry_run=true) tests
    #[test]
    fn clean_dry_run_returns_zero_bytes_freed() {
        let tmp = TempDir::new().unwrap();
        let backup_dir = make_backup_dir(&tmp);
        let entry = backup_dir.join("AABBCCDD-EEFF-0011-2233-445566778899");
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join("Manifest.db"), b"fakedata").unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let result = make_cleaner(&tmp)
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn clean_dry_run_does_not_delete_backup_directories() {
        let tmp = TempDir::new().unwrap();
        let backup_dir = make_backup_dir(&tmp);
        let entry = backup_dir.join("AABBCCDD-EEFF-0011-2233-445566778899");
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join("Manifest.db"), b"fakedata").unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        make_cleaner(&tmp)
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert!(
            entry.exists(),
            "dry-run must not delete the backup directory"
        );
    }

    #[test]
    fn clean_when_backup_dir_absent_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let result = make_cleaner(&tmp)
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn ios_backup_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!make_cleaner(&tmp).auto_clean_allowed());
    }
}
