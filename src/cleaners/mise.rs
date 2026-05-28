use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
#[cfg(test)]
use crate::test_helpers::MockRunner;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MiseCleaner {
    installs_dir: PathBuf,
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    mise_output_cache: std::sync::OnceLock<Option<String>>,
    pinned_cache: std::sync::OnceLock<HashSet<(String, String)>>,
}

impl MiseCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            installs_dir: home.join(".local/share/mise/installs"),
            home: home.to_path_buf(),
            runner,
            mise_output_cache: std::sync::OnceLock::new(),
            pinned_cache: std::sync::OnceLock::new(),
        }
    }

    /// Returns cached stdout from `mise ls --current`, computing it on first call.
    fn get_mise_output(&self) -> Option<String> {
        self.mise_output_cache
            .get_or_init(|| {
                self.runner
                    .run("mise", &["ls", "--current"])
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            })
            .clone()
    }

    /// Returns cached pinned versions, computing them on first call.
    fn get_pinned(&self) -> &HashSet<(String, String)> {
        self.pinned_cache
            .get_or_init(|| Self::scan_pinned_versions(&self.home))
    }

    /// Parses `mise ls --current` stdout into a set of (tool, version) pairs.
    ///
    /// Each output line has whitespace-separated columns: tool  version  source
    /// Both spaces and tabs are accepted as separators.
    pub fn parse_active_versions(stdout: &str) -> HashSet<(String, String)> {
        stdout
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                let tool = parts.next()?.to_string();
                let version = parts.next()?.to_string();
                Some((tool, version))
            })
            .collect()
    }

    /// Returns (tool, version, path) triples for installed versions not in `active`.
    fn unused_versions(
        &self,
        active: &HashSet<(String, String)>,
        pinned: &HashSet<(String, String)>,
    ) -> Vec<(String, String, PathBuf)> {
        let tools = match fs::read_dir(&self.installs_dir) {
            Ok(d) => d,
            Err(_) => return vec![],
        };

        let mut unused = vec![];
        for tool_entry in tools.filter_map(|e| e.ok()) {
            let tool_name = tool_entry.file_name().to_string_lossy().to_string();
            let versions = match fs::read_dir(tool_entry.path()) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for version_entry in versions.filter_map(|e| e.ok()) {
                // Skip non-directory entries (e.g. .DS_Store, .mise.backend)
                if !version_entry
                    .file_type()
                    .map(|t| t.is_dir())
                    .unwrap_or(false)
                {
                    continue;
                }
                let version = version_entry.file_name().to_string_lossy().to_string();
                let pair = (tool_name.clone(), version.clone());
                if !active.contains(&pair) && !pinned.contains(&pair) {
                    unused.push((tool_name.clone(), version, version_entry.path()));
                }
            }
        }
        unused
    }

    /// Scans `~/.config/mise/config.toml` and all `.mise.toml` files under
    /// `home` (max depth 5) and returns a set of pinned `(tool, version)` pairs.
    ///
    /// Respects CLAUDE.md safety rule:
    ///   "mise runtime deletion must cross-check global config.toml AND any
    ///    .mise.toml found within HOME (max depth 5)."
    fn scan_pinned_versions(home: &Path) -> HashSet<(String, String)> {
        let mut pinned = HashSet::new();

        // ── global ──────────────────────────────────────────────────────────
        let global_config = home.join(".config/mise/config.toml");
        if let Ok(content) = fs::read_to_string(global_config) {
            Self::parse_tools_section(&content, &mut pinned);
        }

        // ── per-project (depth ≤ 5) ─────────────────────────────────────────
        for entry in walkdir::WalkDir::new(home)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() && entry.file_name().to_string_lossy() == ".mise.toml" {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    Self::parse_tools_section(&content, &mut pinned);
                }
            }
        }

        pinned
    }

    /// Reads TOML content and collects `tool = "version"` from `[tools]` section.
    fn parse_tools_section(content: &str, out: &mut HashSet<(String, String)>) {
        let mut in_tools = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.trim_end_matches(']') == "[tools" {
                in_tools = true;
                continue;
            }
            if trimmed.starts_with('[') {
                in_tools = false;
                continue;
            }
            if in_tools {
                // matches:  tool_name = "0.1.0"
                if let Some((key, val)) = Self::parse_toml_kv(trimmed) {
                    out.insert((key, val));
                }
            }
        }
    }

    /// Parses a single `key = "value"` line from TOML.
    fn parse_toml_kv(line: &str) -> Option<(String, String)> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, rest) = line.split_once('=')?;
        let key = key.trim().to_string();
        let val = rest.trim().trim_matches('"').to_string();
        if key.is_empty() || val.is_empty() {
            return None;
        }
        Some((key, val))
    }

    /// Clears macOS `uchg` immutable flags then deletes the directory.
    ///
    /// Returns an error if `chflags -R nouchg` fails, so callers get a clear
    /// diagnostic instead of a confusing `remove_dir_all` failure.
    fn remove_with_uchg(path: &Path, runner: &dyn CommandRunner) -> Result<()> {
        let path_str = path.to_string_lossy();
        runner
            .run("chflags", &["-R", "nouchg", &path_str])
            .map_err(|e| anyhow::anyhow!("chflags -R nouchg {:?}: {}", path, e))?;
        crate::trash::delete_path(path)
    }
}

impl Cleaner for MiseCleaner {
    fn is_available(&self) -> bool {
        self.runner.exists("mise")
    }

    fn name(&self) -> &'static str {
        "mise"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("mise") {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let stdout = match self.get_mise_output() {
            Some(ref s) => s.clone(),
            None => return ScanResult::new(self.name(), ScanStatus::NotFound),
        };
        let active = Self::parse_active_versions(&stdout);
        let pinned = self.get_pinned().clone();
        let unused = self.unused_versions(&active, &pinned);
        let bytes: u64 = unused.iter().map(|(_, _, p)| dir_size(p)).sum();
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.installs_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.runner.exists("mise") {
            println!("mise: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: true,
                skipped: vec![],
            deleted_paths: vec![],
            });
        }
        let stdout = match self.get_mise_output() {
            Some(ref s) => s.clone(),
            None => {
                println!("mise: ls --current failed, nothing to clean");
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: true,
                    skipped: vec![],
            deleted_paths: vec![],
                });
            }
        };
        let active = Self::parse_active_versions(&stdout);
        let pinned = self.get_pinned().clone();
        let unused = self.unused_versions(&active, &pinned);

        if !unused.is_empty() && !pinned.is_empty() {
            eprintln!(
                "Note: {} version(s) protected by .mise.toml pinning",
                pinned.len()
            );
        }

        if !dry_run && !unused.is_empty() {
            reporter.progress_init(self.name(), unused.len());
        }

        let mut freed: u64 = 0;
        for (i, (tool, version, path)) in unused.iter().enumerate() {
            let size = dir_size(path);
            if dry_run {
                println!(
                    "[dry-run] would remove: {tool} {version} ({})",
                    crate::format::format_bytes(size)
                );
            } else {
                reporter.progress_tick(path, i + 1, size);
                match Self::remove_with_uchg(path, self.runner.as_ref()) {
                    Ok(()) => {
                        freed += size;
                        println!("Removed: {tool} {version}");
                    }
                    Err(e) => {
                        eprintln!("Error removing {tool} {version}: {e}");
                        // Continue with remaining items on error
                    }
                }
            }
        }

        if !dry_run && !unused.is_empty() {
            reporter.progress_finish();
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped: vec![],
            deleted_paths: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_active_versions_space_separated() {
        let stdout = "node    24.15.0  ~/.config/mise/config.toml\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(active.contains(&("node".to_string(), "24.15.0".to_string())));
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn parse_active_versions_tab_separated() {
        let stdout = "node\t24.15.0\t~/.config/mise/config.toml\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(active.contains(&("node".to_string(), "24.15.0".to_string())));
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn parse_active_versions_multiple_tools() {
        let stdout = "node\t24.15.0\t~/.config/mise/config.toml\npython\t3.12.11\t~/.config/mise/config.toml\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(active.contains(&("node".to_string(), "24.15.0".to_string())));
        assert!(active.contains(&("python".to_string(), "3.12.11".to_string())));
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn parse_active_versions_empty() {
        assert!(MiseCleaner::parse_active_versions("").is_empty());
    }

    #[test]
    fn unused_versions_excludes_active() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let installs = tmp.path().join(".local/share/mise/installs/node");
        fs::create_dir_all(installs.join("20.11.0")).unwrap();
        fs::create_dir_all(installs.join("24.15.0")).unwrap();

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let mut active = std::collections::HashSet::new();
        active.insert(("node".to_string(), "24.15.0".to_string()));

        let pinned = std::collections::HashSet::new();
        let unused = cleaner.unused_versions(&active, &pinned);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].1, "20.11.0");
    }

    #[test]
    fn unused_versions_all_active_returns_empty() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let installs = tmp.path().join(".local/share/mise/installs/node");
        fs::create_dir_all(installs.join("24.15.0")).unwrap();

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let mut active = std::collections::HashSet::new();
        active.insert(("node".to_string(), "24.15.0".to_string()));

        let pinned = std::collections::HashSet::new();
        assert!(cleaner.unused_versions(&active, &pinned).is_empty());
    }

    #[test]
    fn unused_versions_pinned_is_protected() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let installs = tmp.path().join(".local/share/mise/installs/node");
        fs::create_dir_all(installs.join("20.11.0")).unwrap();
        fs::create_dir_all(installs.join("24.15.0")).unwrap();

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let active = std::collections::HashSet::new(); // nothing active

        let mut pinned = std::collections::HashSet::new();
        pinned.insert(("node".to_string(), "20.11.0".to_string()));
        // 24.15.0 is neither active nor pinned → should be removed
        let unused = cleaner.unused_versions(&active, &pinned);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].1, "24.15.0");
    }

    // NOTE: scan_pinned_versions / parse_tools_section / parse_toml_kv are
    // covered by the E2E test `clean_mise_pinned_version_not_deleted` in
    // tests/mise.rs because integration tests cannot access private functions.

    #[test]
    fn unused_versions_skips_non_directories() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let installs = tmp.path().join(".local/share/mise/installs/python");
        fs::create_dir_all(installs.join("3.12.0")).unwrap();
        // Non-directory entries that should be skipped
        fs::write(installs.join(".DS_Store"), b"").unwrap();
        fs::write(installs.join(".mise.backend"), b"").unwrap();

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let active = std::collections::HashSet::new();
        let pinned = std::collections::HashSet::new();
        let unused = cleaner.unused_versions(&active, &pinned);
        // Only 3.12.0 should be in the result, .DS_Store and .mise.backend skipped
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].0, "python");
        assert_eq!(unused[0].1, "3.12.0");
    }

    // ── parse_toml_kv ──

    #[test]
    fn parse_toml_kv_valid_line_returns_key_value() {
        let result = MiseCleaner::parse_toml_kv(r#"node = "20.11.0""#);
        assert_eq!(result, Some(("node".to_string(), "20.11.0".to_string())));
    }

    #[test]
    fn parse_toml_kv_comment_line_returns_none() {
        let result = MiseCleaner::parse_toml_kv("# node = \"20.11.0\"");
        assert_eq!(result, None);
    }

    #[test]
    fn parse_toml_kv_empty_line_returns_none() {
        assert_eq!(MiseCleaner::parse_toml_kv(""), None);
    }

    #[test]
    fn parse_toml_kv_no_equals_returns_none() {
        assert_eq!(MiseCleaner::parse_toml_kv("node 20.11.0"), None);
    }

    // ── parse_tools_section ──

    #[test]
    fn parse_tools_section_with_tools_returns_parsed_entries() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section(
            "[tools]\nnode = \"20.11.0\"\npython = \"3.12.0\"\n",
            &mut result,
        );
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("node".to_string(), "20.11.0".to_string())));
        assert!(result.contains(&("python".to_string(), "3.12.0".to_string())));
    }

    #[test]
    fn parse_tools_section_empty_content_returns_empty() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section("", &mut result);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_tools_section_toolchain_not_confused_with_tools() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section("[toolchain]\nchannel = \"stable\"\n", &mut result);
        assert!(result.is_empty(), "[toolchain] must not match [tools]");
    }

    #[test]
    fn parse_tools_section_devtools_not_confused_with_tools() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section("[devtools]\nnode = \"20.0.0\"\n", &mut result);
        assert!(result.is_empty(), "[devtools] must not match [tools]");
    }

    #[test]
    fn parse_tools_section_second_section_ends_tools() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section("[tools]\nnode = \"22.0.0\"\n[aliases]\n", &mut result);
        assert!(result.contains(&("node".to_string(), "22.0.0".to_string())));
        assert_eq!(result.len(), 1, "[aliases] should end [tools] section");
    }

    // ── scan_pinned_versions ──

    #[test]
    fn scan_pinned_versions_no_config_files_returns_empty() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn scan_pinned_versions_global_config_is_read() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let config_dir = tmp.path().join(".config/mise");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            "[tools]\nnode = \"22.0.0\"\n",
        )
        .unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path());
        assert!(result.contains(&("node".to_string(), "22.0.0".to_string())));
    }

    #[test]
    fn scan_pinned_versions_project_mise_toml_is_read() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join(".mise.toml"),
            "[tools]\npython = \"3.13.0\"\n",
        )
        .unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path());
        assert!(result.contains(&("python".to_string(), "3.13.0".to_string())));
    }

    #[test]
    fn scan_pinned_versions_malformed_toml_does_not_panic() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".mise.toml"), "[[[invalid toml").unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn clean_handles_mise_ls_current_failure_gracefully() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        // even though mise is not found, clean should not crash
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn detect_and_clean_consistent_on_mise_not_found() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        // detect should soft-fail when mise not found
        let detect_result = cleaner.detect();
        assert!(matches!(detect_result.status, ScanStatus::NotFound));
        // clean should also soft-fail
        let clean_result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(clean_result.bytes_freed, 0);
    }
}
