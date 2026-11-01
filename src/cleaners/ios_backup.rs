use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
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
    // Deleting an iOS backup is irreversible; its selection prompt accepts exactly one Enter.
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Risky
    }

    fn name(&self) -> &'static str {
        "ios-backup"
    }

    // `--yes` must never pick this target.
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
        opts: &crate::cleaner::CleanOptions,
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
        // interactively. multi_select_or_skip owns both skips - the port's
        // non-TTY remedy and the empty-selection skip - so the clean skips
        // instead of hanging on a hidden prompt.
        let Some(selected) = self.interactive_select(reporter, &entries)? else {
            return Ok(CleanResult::empty(self.name()));
        };

        // The shared delete ledger owns the per-entry contracts. The mode is
        // Global: the deletion scope was already gated interactively above.
        let result = self.delete_selected(&selected, reporter, opts.trash_mode)?;
        println!(
            "[ios-backup] total freed: {}",
            crate::format::format_bytes(result.bytes_freed)
        );
        // The full result propagates: skipped entries reach the caller,
        // so an all-skipped run exits 1 (the same contract every other
        // ledger user pins).
        Ok(result)
    }
}

impl IosCleaner {
    fn interactive_select(
        &self,
        reporter: &dyn ProgressReporter,
        entries: &[(PathBuf, u64)],
    ) -> Result<Option<Vec<(PathBuf, u64)>>> {
        let items: Vec<String> = entries
            .iter()
            .map(|(path, size)| {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                format!("{:<40}  {}", name, crate::format::format_bytes(*size))
            })
            .collect();
        let defaults = vec![true; entries.len()];
        println!("\niOS device backups in ~/Library/Application Support/MobileSync/Backup/:\n");
        let picks =
            crate::prompt::multi_select_or_skip("ios-backup", reporter, None, &items, &defaults)?;
        Ok(picks.map(|selections| selections.into_iter().map(|i| entries[i].clone()).collect()))
    }

    // PBI 97: freed is the delete-time dir_size (the interactive select
    // above can wait unbounded), so re-measure right before the deletion;
    // the ledger keeps its caller-owned size contract, and the collected
    // sizes stay the dry-run / selection-list numbers.
    fn delete_selected(
        &self,
        selected: &[(PathBuf, u64)],
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        let items: Vec<(PathBuf, u64)> = selected
            .iter()
            .map(|(path, _)| {
                let size = crate::format::dir_size(path);
                (path.clone(), size)
            })
            .collect();
        Ok(crate::trash::delete_entries(
            self.name(),
            self.runner.as_ref(),
            reporter,
            mode,
            &items,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::IsTerminal;
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

    #[test]
    fn clean_skips_interactive_select_without_a_terminal() {
        // The per-cleaner non-TTY pin (PBI 99): on a non-TTY stdin the
        // shared select ritual skips the interactive path, so nothing is
        // deleted and the clean never hangs on a hidden prompt.
        if std::io::stdin().is_terminal() {
            // An interactive `cargo test` would otherwise hang on the prompt.
            return;
        }
        let tmp = TempDir::new().unwrap();
        let backup_dir = make_backup_dir(&tmp);
        let entry = backup_dir.join("AABBCCDD-EEFF-0011-2233-445566778899");
        fs::create_dir_all(&entry).unwrap();
        fs::write(entry.join("Manifest.db"), b"fakedata").unwrap();
        let result = make_cleaner(&tmp)
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(
            result.bytes_freed, 0,
            "non-terminal stdin must skip the interactive select"
        );
        assert!(entry.exists(), "nothing is deleted without a terminal");
    }

    #[test]
    fn delete_selected_freed_is_remeasured_after_the_scan_drifts() {
        // PBI 97 drift pin (ios_backup): freed must be the delete-time
        // dir_size. The interactive select can wait on a human for
        // unbounded time, so growth that lands between the collect above
        // and the deletion (a device syncing its backup while the screen
        // is open) must be reflected; the collected size is the scan-time
        // value and stays the dry-run / selection-list number.
        let tmp = TempDir::new().unwrap();
        let backup_dir = make_backup_dir(&tmp);
        let entry = backup_dir.join("AABBCCDD-EEFF-0011-2233-445566778899");
        fs::create_dir_all(entry.join("logs")).unwrap();
        fs::write(entry.join("logs/a.log"), vec![1u8; 20_000]).unwrap();
        let runner = crate::test_helpers::MockRunner::new().with_success("chflags");
        let cleaner = IosCleaner::new(tmp.path(), Box::new(runner));
        let scan_time_size = crate::format::dir_size(&entry);
        let entries = vec![(entry.clone(), scan_time_size)];

        // The backup grows inside a subdirectory: the top-level mtime is
        // untouched, the same fixture shape the PBI 93/97 drift pins use.
        fs::write(entry.join("logs/b.log"), vec![1u8; 4096]).unwrap();
        let drifted = crate::format::dir_size(&entry);
        assert!(
            drifted > scan_time_size,
            "fixture: the backup must have grown after the scan ({drifted} <= {scan_time_size})"
        );

        let r = cleaner
            .delete_selected(
                &entries,
                &crate::progress::DeepSuppressReporter,
                crate::trash::TrashMode::Permanent,
            )
            .unwrap();
        assert!(!entry.exists());
        assert_eq!(
            r.bytes_freed, drifted,
            "freed must be the delete-time measurement, not the scan-time size"
        );
        assert!(r.skipped.is_empty());
    }

    #[test]
    fn the_empty_selection_line_is_owned_by_the_shared_select_path() {
        // PBI 99: the empty-selection skip must be owned by the shared
        // select path, not re-printed per cleaner. The drift cannot surface
        // as behaviour (an empty selection needs a TTY), so the pin greps
        // the source: Red before the helper, green after the migration. The
        // needle is assembled from two pieces so the pin's own source does
        // not contain the contiguous line it greps for.
        let needle = String::from("] nothing") + " selected";
        let src = include_str!("ios_backup.rs");
        assert!(
            !src.contains(&needle),
            "the empty-selection line must be owned by the shared select path"
        );
        assert!(
            src.contains("multi_select"),
            "the interactive select must flow through the shared select path"
        );
    }
}
