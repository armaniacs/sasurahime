use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XcodeSubcategory {
    DerivedData,
    Archives,
}

#[allow(dead_code)]
impl XcodeSubcategory {
    pub fn all() -> Vec<Self> {
        vec![Self::DerivedData, Self::Archives]
    }

    pub fn path(&self, home: &Path) -> PathBuf {
        match self {
            Self::DerivedData => home.join("Library/Developer/Xcode/DerivedData"),
            Self::Archives => home.join("Library/Developer/Xcode/Archives"),
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "derived-data" | "deriveddata" => Some(Self::DerivedData),
            "archives" => Some(Self::Archives),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::DerivedData => "DerivedData",
            Self::Archives => "Archives",
        }
    }
}

#[allow(dead_code)]
pub struct SubcategoryInfo {
    pub sub: XcodeSubcategory,
    pub path: PathBuf,
    pub size: u64,
}

pub struct XcodeCleaner {
    derived_data: PathBuf,
    archives: PathBuf,
    runner: Box<dyn CommandRunner>,
    subs: Option<Vec<XcodeSubcategory>>,
}

impl XcodeCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            derived_data: home.join("Library/Developer/Xcode/DerivedData"),
            archives: home.join("Library/Developer/Xcode/Archives"),
            runner,
            subs: None,
        }
    }

    pub fn with_subcategories(mut self, subs: Vec<XcodeSubcategory>) -> Self {
        self.subs = Some(subs);
        self
    }

    pub fn detect_subcategories(&self) -> Vec<SubcategoryInfo> {
        XcodeSubcategory::all()
            .into_iter()
            .map(|sub| {
                let path = match sub {
                    XcodeSubcategory::DerivedData => self.derived_data.clone(),
                    XcodeSubcategory::Archives => self.archives.clone(),
                };
                let size = if path.exists() { dir_size(&path) } else { 0 };
                SubcategoryInfo { sub, path, size }
            })
            .collect()
    }

    /// Returns true if an Xcode process is currently running.
    #[allow(dead_code)]
    pub fn is_xcode_running(&self) -> bool {
        self.runner
            .run("pgrep", &["-x", "Xcode"])
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Cleaner for XcodeCleaner {
    fn name(&self) -> &'static str {
        "xcode"
    }

    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        self.detect_subcategories()
            .into_iter()
            .filter(|info| info.size > 0)
            .map(|info| (info.sub.display_name(), info.size))
            .collect()
    }

    fn detect(&self) -> ScanResult {
        if let Some(ref subs) = self.subs {
            let total: u64 = subs
                .iter()
                .map(|sub| {
                    let path = match sub {
                        XcodeSubcategory::DerivedData => &self.derived_data,
                        XcodeSubcategory::Archives => &self.archives,
                    };
                    if path.exists() {
                        dir_size(path)
                    } else {
                        0
                    }
                })
                .sum();
            let mut r = ScanResult::new(
                self.name(),
                if total > 0 {
                    ScanStatus::Pruneable(total)
                } else {
                    ScanStatus::NotFound
                },
            );
            if crate::context::is_verbose() {
                r = r.with_target(format!("{:?}", subs));
            }
            return r;
        }
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
        let subs: Vec<XcodeSubcategory> = match self.subs {
            Some(ref s) => s.clone(),
            None => vec![XcodeSubcategory::DerivedData],
        };

        if subs.contains(&XcodeSubcategory::DerivedData)
            && self.derived_data.exists()
            && self.is_xcode_running()
        {
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

        let mut total_freed = 0u64;
        let mut all_skipped = vec![];

        for sub in subs {
            let (dir, label) = match sub {
                XcodeSubcategory::DerivedData => (&self.derived_data, "DerivedData"),
                XcodeSubcategory::Archives => (&self.archives, "Archives"),
            };

            if !dir.exists() {
                println!("Xcode {label}: not found, skipping");
                continue;
            }

            let entries = match fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };

            let dirs: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect();

            if dirs.is_empty() {
                continue;
            }

            if !dry_run {
                reporter.progress_init(&format!("xcode/{label}"), dirs.len());
            }

            for (i, entry) in dirs.iter().enumerate() {
                let size = dir_size(entry);
                let entry_name = entry.file_name().unwrap_or_default().to_string_lossy();
                if dry_run {
                    println!(
                        "[dry-run] would remove: {label}/{entry_name} ({})",
                        crate::format::format_bytes(size)
                    );
                } else {
                    reporter.progress_tick(entry, i + 1, size);
                    if let Err(e) = crate::trash::delete_path(entry) {
                        if crate::cleaner::is_skippable_error(&e) {
                            all_skipped.push(crate::cleaner::SkippedEntry {
                                path: entry.to_path_buf(),
                                reason: format!("{e:#}"),
                            });
                        } else {
                            return Err(e);
                        }
                    } else {
                        total_freed += size;
                        println!("Removed: {label}/{entry_name}");
                    }
                }
            }

            if !dry_run {
                reporter.progress_finish();
            }
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: total_freed,
            uses_trash: true,
            skipped: all_skipped,
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
    fn subcategory_all_returns_two() {
        let all = XcodeSubcategory::all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&XcodeSubcategory::DerivedData));
        assert!(all.contains(&XcodeSubcategory::Archives));
    }

    #[test]
    fn subcategory_path_derived_data() {
        let home = Path::new("/Users/test");
        let p = XcodeSubcategory::DerivedData.path(home);
        assert!(p.ends_with("Library/Developer/Xcode/DerivedData"));
    }

    #[test]
    fn subcategory_path_archives() {
        let home = Path::new("/Users/test");
        let p = XcodeSubcategory::Archives.path(home);
        assert!(p.ends_with("Library/Developer/Xcode/Archives"));
    }

    #[test]
    fn from_str_derived_data_variants() {
        assert_eq!(
            XcodeSubcategory::from_str("derived-data"),
            Some(XcodeSubcategory::DerivedData)
        );
        assert_eq!(
            XcodeSubcategory::from_str("deriveddata"),
            Some(XcodeSubcategory::DerivedData)
        );
    }

    #[test]
    fn from_str_archives() {
        assert_eq!(
            XcodeSubcategory::from_str("archives"),
            Some(XcodeSubcategory::Archives)
        );
    }

    #[test]
    fn from_str_invalid_returns_none() {
        assert_eq!(XcodeSubcategory::from_str("invalid"), None);
        assert_eq!(XcodeSubcategory::from_str("simulators"), None);
    }

    #[test]
    fn display_name_is_readable() {
        assert_eq!(XcodeSubcategory::DerivedData.display_name(), "DerivedData");
        assert_eq!(XcodeSubcategory::Archives.display_name(), "Archives");
    }

    #[test]
    fn detect_subcategories_returns_correct_sizes() {
        let tmp = TempDir::new().unwrap();
        let dd = tmp.path().join("Library/Developer/Xcode/DerivedData");
        fs::create_dir_all(dd.join("ProjectA")).unwrap();
        fs::write(dd.join("ProjectA").join("f"), b"x").unwrap();

        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(NoopRunner));
        let infos = cleaner.detect_subcategories();
        assert_eq!(infos.len(), 2);
        let dd_info = infos
            .iter()
            .find(|i| i.sub == XcodeSubcategory::DerivedData)
            .unwrap();
        assert!(dd_info.size > 0, "DerivedData should have size > 0");
        let archives_info = infos
            .iter()
            .find(|i| i.sub == XcodeSubcategory::Archives)
            .unwrap();
        assert_eq!(archives_info.size, 0, "Archives should be size 0");
    }

    #[test]
    fn sub_targets_returns_only_existing_subcategories() {
        let tmp = TempDir::new().unwrap();
        let dd = tmp.path().join("Library/Developer/Xcode/DerivedData");
        fs::create_dir_all(dd.join("ProjectA")).unwrap();
        fs::write(dd.join("ProjectA").join("f"), b"x").unwrap();
        // Archives not created — should be filtered out

        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(NoopRunner));
        let targets = cleaner.sub_targets();
        assert_eq!(targets.len(), 1, "only DerivedData should appear");
        assert_eq!(targets[0].0, "DerivedData");
        assert!(targets[0].1 > 0, "should have size > 0");
    }

    #[test]
    fn sub_targets_filters_zero_size_entries() {
        let tmp = TempDir::new().unwrap();
        // Neither DerivedData nor Archives exist
        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(NoopRunner));
        let targets = cleaner.sub_targets();
        assert!(
            targets.is_empty(),
            "no subcategories should appear when none exist"
        );
    }

    #[test]
    fn clean_selected_subcategory_only_deletes_that_one() {
        let tmp = TempDir::new().unwrap();
        let dd = tmp.path().join("Library/Developer/Xcode/DerivedData");
        let arch = tmp.path().join("Library/Developer/Xcode/Archives");
        fs::create_dir_all(dd.join("P")).unwrap();
        fs::write(dd.join("P").join("f"), b"x").unwrap();
        fs::create_dir_all(arch.join("A")).unwrap();
        fs::write(arch.join("A").join("f"), b"x").unwrap();

        let cleaner = XcodeCleaner::new(tmp.path(), Box::new(PgrepRunner { running: false }))
            .with_subcategories(vec![XcodeSubcategory::DerivedData]);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(result.bytes_freed > 0, "should have freed bytes");
        assert!(
            !dd.join("P").exists(),
            "DerivedData subdirectory should be removed"
        );
        assert!(arch.exists(), "Archives should still exist");
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
