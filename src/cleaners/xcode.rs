use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct XcodeCleaner {
    derived_data: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl XcodeCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            derived_data: home.join("Library/Developer/Xcode/DerivedData"),
            runner,
        }
    }

    /// Returns true if an Xcode process is currently running.
    #[allow(dead_code)]
    pub fn is_xcode_running(&self) -> bool {
        self.runner
            .run("pgrep", &["-x", "Xcode"])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn project_dirs(&self) -> Vec<PathBuf> {
        let entries = match fs::read_dir(&self.derived_data) {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect()
    }
}

impl Cleaner for XcodeCleaner {
    fn name(&self) -> &'static str {
        "xcode"
    }

    fn detect(&self) -> ScanResult {
        if !self.derived_data.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes = dir_size(&self.derived_data);
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.derived_data.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.derived_data.exists() {
            println!("Xcode DerivedData: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        if self.is_xcode_running() {
            eprintln!("Warning: Xcode is running. DerivedData deletion may cause issues.");
            eprint!("Continue? [y/N] ");
            use std::io::Write;
            std::io::stderr().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                });
            }
        }

        let dirs = self.project_dirs();
        if !dry_run && !dirs.is_empty() {
            reporter.progress_init(self.name(), dirs.len());
        }

        let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];
        let mut freed: u64 = 0;
        for (i, dir) in dirs.iter().enumerate() {
            let size = dir_size(dir);
            let entry_name = dir.file_name().unwrap_or_default().to_string_lossy();
            if dry_run {
                println!(
                    "[dry-run] would remove: DerivedData/{entry_name} ({})",
                    crate::format::format_bytes(size)
                );
            } else {
                reporter.progress_tick(dir, i + 1, size);
                if let Err(e) = crate::trash::delete_path(dir) {
                    if crate::cleaner::is_skippable_error(&e) {
                        skipped.push(crate::cleaner::SkippedEntry {
                            path: dir.to_path_buf(),
                            reason: format!("{e:#}"),
                        });
                    } else {
                        return Err(e);
                    }
                } else {
                    freed += size;
                    println!("Removed: DerivedData/{entry_name}");
                }
            }
        }

        if !dry_run && !dirs.is_empty() {
            reporter.progress_finish();
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: true,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct NoopRunner;
    impl CommandRunner for NoopRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            unimplemented!()
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

    struct PgrepRunner {
        running: bool,
    }
    impl CommandRunner for PgrepRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            use std::os::unix::process::ExitStatusExt;
            let status = std::process::ExitStatus::from_raw(if self.running { 0 } else { 256 });
            Ok(std::process::Output {
                status,
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn detect_not_found_when_missing() {
        let tmp = TempDir::new().unwrap();
        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(NoopRunner));
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_pruneable_when_content_exists() {
        let tmp = TempDir::new().unwrap();
        let derived = tmp
            .path()
            .join("Library/Developer/Xcode/DerivedData/ProjectA");
        fs::create_dir_all(&derived).unwrap();
        fs::write(derived.join("f"), b"x").unwrap();

        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(NoopRunner));
        assert!(matches!(cleaner.detect().status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn is_xcode_running_reflects_pgrep_result() {
        let tmp = TempDir::new().unwrap();
        let cleaner_running =
            XcodeCleaner::new(tmp.path(), Box::new(PgrepRunner { running: true }));
        let cleaner_stopped =
            XcodeCleaner::new(tmp.path(), Box::new(PgrepRunner { running: false }));
        assert!(cleaner_running.is_xcode_running());
        assert!(!cleaner_stopped.is_xcode_running());
    }
}
