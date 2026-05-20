use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MiseCleaner {
    installs_dir: PathBuf,
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl MiseCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            installs_dir: home.join(".local/share/mise/installs"),
            home: home.to_path_buf(),
            runner,
        }
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
            if trimmed.starts_with('[') && trimmed.contains("tools") {
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
    fn name(&self) -> &'static str {
        "mise"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("mise") {
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let output = match self.runner.run("mise", &["ls", "--current"]) {
            Ok(o) => o,
            Err(_) => {
                return ScanResult {
                    name: self.name(),
                    status: ScanStatus::NotFound,
                }
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let active = Self::parse_active_versions(&stdout);
        let pinned = Self::scan_pinned_versions(&self.home);
        let unused = self.unused_versions(&active, &pinned);
        let bytes: u64 = unused.iter().map(|(_, _, p)| dir_size(p)).sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        if !self.runner.exists("mise") {
            println!("mise: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
            });
        }
        let output = self.runner.run("mise", &["ls", "--current"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let active = Self::parse_active_versions(&stdout);
        let pinned = Self::scan_pinned_versions(&self.home);
        let unused = self.unused_versions(&active, &pinned);

        if !unused.is_empty() && !pinned.is_empty() {
            eprintln!(
                "Note: {} version(s) protected by .mise.toml pinning",
                pinned.len()
            );
        }

        let mut freed: u64 = 0;
        for (tool, version, path) in &unused {
            let size = dir_size(path);
            if dry_run {
                println!(
                    "[dry-run] would remove: {tool} {version} ({})",
                    crate::format::format_bytes(size)
                );
            } else {
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
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopRunner;
    impl CommandRunner for NoopRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            unimplemented!()
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

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

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(NoopRunner));
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

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(NoopRunner));
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

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(NoopRunner));
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

        let cleaner = MiseCleaner::new(tmp.path(), Box::new(NoopRunner));
        let active = std::collections::HashSet::new();
        let pinned = std::collections::HashSet::new();
        let unused = cleaner.unused_versions(&active, &pinned);
        // Only 3.12.0 should be in the result, .DS_Store and .mise.backend skipped
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].0, "python");
        assert_eq!(unused[0].1, "3.12.0");
    }
}
