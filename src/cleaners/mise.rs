use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MiseCleaner {
    installs_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl MiseCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            installs_dir: home.join(".local/share/mise/installs"),
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
    fn unused_versions(&self, active: &HashSet<(String, String)>) -> Vec<(String, String, PathBuf)> {
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
                let version = version_entry.file_name().to_string_lossy().to_string();
                if !active.contains(&(tool_name.clone(), version.clone())) {
                    unused.push((tool_name.clone(), version, version_entry.path()));
                }
            }
        }
        unused
    }

    /// Clears macOS `uchg` immutable flags then deletes the directory.
    fn remove_with_uchg(path: &Path, runner: &dyn CommandRunner) -> Result<()> {
        let path_str = path.to_string_lossy();
        let _ = runner.run("chflags", &["-R", "nouchg", &path_str]);
        fs::remove_dir_all(path)
            .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
    }
}

impl Cleaner for MiseCleaner {
    fn name(&self) -> &'static str {
        "mise"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("mise") {
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        let output = match self.runner.run("mise", &["ls", "--current"]) {
            Ok(o) => o,
            Err(_) => return ScanResult { name: self.name(), status: ScanStatus::NotFound },
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let active = Self::parse_active_versions(&stdout);
        let unused = self.unused_versions(&active);
        let bytes: u64 = unused.iter().map(|(_, _, p)| dir_size(p)).sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 { ScanStatus::Pruneable(bytes) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        if !self.runner.exists("mise") {
            println!("mise: not found, skipping");
            return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
        }
        let output = self.runner.run("mise", &["ls", "--current"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let active = Self::parse_active_versions(&stdout);
        let unused = self.unused_versions(&active);

        let mut freed: u64 = 0;
        for (tool, version, path) in &unused {
            let size = dir_size(path);
            if dry_run {
                println!("[dry-run] would remove: {tool} {version} ({})", crate::format::format_bytes(size));
            } else {
                Self::remove_with_uchg(path, self.runner.as_ref())?;
                freed += size;
                println!("Removed: {tool} {version}");
            }
        }
        Ok(CleanResult { name: self.name(), bytes_freed: freed })
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

        let unused = cleaner.unused_versions(&active);
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

        assert!(cleaner.unused_versions(&active).is_empty());
    }
}
