use crate::cleaner::{CleanOptions, CleanResult, Cleaner, ScanResult, ScanStatus};
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

/// Default max depth for the `.mise.toml` search under HOME (AGENTS.md rule).
const DEFAULT_PINNED_MAX_DEPTH: usize = 5;

/// Directory names the pinned-version scan never descends into.
const PINNED_SCAN_EXCLUDED_DIRS: &[&str] = &["node_modules", ".git", "target"];

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
    /// `None` when the command fails, times out, or exits non-zero.
    fn get_mise_output(&self) -> Option<String> {
        self.mise_output_cache
            .get_or_init(|| {
                self.runner
                    .run("mise", &["ls", "--current"])
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            })
            .clone()
    }

    /// Returns cached pinned versions, computing them on first call.
    fn get_pinned(&self) -> &HashSet<(String, String)> {
        self.pinned_cache
            .get_or_init(|| Self::scan_pinned_versions(&self.home, DEFAULT_PINNED_MAX_DEPTH))
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

    /// True when `mise ls --current` printed data whose format the parser
    /// could not understand.
    ///
    /// Treating such output as "nothing active" would mark every installed
    /// runtime deletable (an unparsable line is dropped by
    /// [`Self::parse_active_versions`]), so the cleaner must fail closed
    /// instead of deleting. An empty stdout is the legitimate "no active
    /// versions" case and does NOT trip this guard. Residual risk: output
    /// that still parses into bogus pairs (e.g. a header line with 3+
    /// tokens) evades this check; current `mise ls --current` prints only
    /// `tool version [source]` lines, so the check is false-positive free
    /// for the formats mise emits today.
    fn output_unparseable(stdout: &str, active: &HashSet<(String, String)>) -> bool {
        if stdout.trim().is_empty() {
            return false;
        }
        if active.is_empty() {
            return true;
        }
        // A non-empty line with a single token cannot yield a (tool, version)
        // pair and is silently dropped: treat it as a format change.
        stdout
            .lines()
            .any(|line| !line.trim().is_empty() && line.split_whitespace().count() < 2)
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
    /// `home` (up to `max_depth`) and returns a set of pinned `(tool, version)`
    /// pairs.
    ///
    /// Respects AGENTS.md safety rule:
    ///   "mise runtime deletion must cross-check global config.toml AND any
    ///    .mise.toml found within HOME (max depth 5)."
    ///
    /// Vendored/build directories (`node_modules`, `.git`, `target`) are never
    /// descended into: their `.mise.toml` files are vendor junk, not pins.
    fn scan_pinned_versions(home: &Path, max_depth: usize) -> HashSet<(String, String)> {
        let mut pinned = HashSet::new();

        // ── global ──────────────────────────────────────────────────────────
        let global_config = home.join(".config/mise/config.toml");
        if let Ok(content) = fs::read_to_string(global_config) {
            Self::parse_tools_section(&content, &mut pinned);
        }

        // ── per-project (depth ≤ max_depth) ─────────────────────────────────
        for entry in walkdir::WalkDir::new(home)
            .max_depth(max_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !Self::is_excluded_dir(e))
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

    /// True for directories the pinned-version scan never descends into.
    fn is_excluded_dir(entry: &walkdir::DirEntry) -> bool {
        entry.file_type().is_dir()
            && PINNED_SCAN_EXCLUDED_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
    }

    /// Extracts the version strings a `[tools]` spec value may contain.
    fn spec_versions(spec: &toml::Value) -> Vec<String> {
        match spec {
            toml::Value::String(v) => vec![v.clone()],
            toml::Value::Array(arr) => arr
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect(),
            toml::Value::Table(t) => t
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| vec![s.to_string()])
                .unwrap_or_default(),
            toml::Value::Integer(i) => vec![i.to_string()],
            toml::Value::Float(f) => vec![f.to_string()],
            _ => vec![],
        }
    }

    /// Reads TOML content and collects `tool = "version"` pairs from the
    /// `[tools]` section via the `toml` crate, so inline comments, arrays and
    /// table-form specs parse correctly.
    fn parse_tools_section(content: &str, out: &mut HashSet<(String, String)>) {
        let Ok(value) = content.parse::<toml::Table>() else {
            return;
        };
        let Some(tools) = value.get("tools").and_then(|t| t.as_table()) else {
            return;
        };
        for (tool, spec) in tools {
            for v in Self::spec_versions(spec) {
                out.insert((tool.clone(), v));
            }
        }
    }

    /// The shared clean body. `mode` is the resolved deletion mode for every
    /// per-item deletion (one per unused tool version): the seam outcome and
    /// the deletion agree by construction.
    fn clean_with_mode(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        if !self.runner.exists("mise") {
            println!("mise: not found, skipping");
            return Ok(CleanResult::empty(self.name()));
        }
        let stdout = match self.get_mise_output() {
            Some(ref s) => s.clone(),
            None => {
                println!("mise: ls --current failed, nothing to clean");
                return Ok(CleanResult::empty(self.name()));
            }
        };
        let active = Self::parse_active_versions(&stdout);
        if Self::output_unparseable(&stdout, &active) {
            // Fail closed (PBI 53): an unparseable `ls --current` output must
            // not be read as "nothing active", which would mark every
            // installed runtime deletable.
            eprintln!(
                "mise: ls --current output could not be parsed - refusing to delete anything"
            );
            return Ok(CleanResult::empty(self.name()));
        }
        let pinned = self.get_pinned().clone();
        let unused = self.unused_versions(&active, &pinned);

        if !unused.is_empty() && !pinned.is_empty() {
            eprintln!(
                "Note: {} version(s) protected by .mise.toml pinning",
                pinned.len()
            );
        }

        // The caller owns the per-item size computation; the ledger owns
        // the per-item contracts (progress, skip records, freed sums,
        // CleanResult assembly). The `{tool} {version}` display label is
        // the path itself, so the ledger's standard per-entry line names
        // the same runtime.
        let items: Vec<(std::path::PathBuf, u64)> = unused
            .iter()
            .map(|(_, _, path)| (path.clone(), dir_size(path)))
            .collect();

        if dry_run {
            for ((tool, version, _), (_, size)) in unused.iter().zip(items.iter()) {
                println!(
                    "[dry-run] would remove: {tool} {version} ({})",
                    crate::format::format_bytes(*size)
                );
            }
            return Ok(CleanResult::empty(self.name()));
        }

        // The seam adapter fixes safe_delete as the per-item deletion, so
        // the uchg rule and the scope tripwire stay the seam's job.
        Ok(crate::trash::delete_ledger(
            self.name(),
            reporter,
            &items,
            crate::trash::seam_delete(self.runner.as_ref(), mode),
        ))
    }
}

impl Cleaner for MiseCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

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
            // mise is installed but the command failed/timed out: the target is
            // present, so report it as unavailable instead of `not found`.
            None => return ScanResult::new(self.name(), ScanStatus::Unavailable),
        };
        let active = Self::parse_active_versions(&stdout);
        if Self::output_unparseable(&stdout, &active) {
            // Fail closed (PBI 53): report the target as unavailable rather
            // than pruneable when the active-version parse cannot be trusted.
            return ScanResult::new(self.name(), ScanStatus::Unavailable);
        }
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

    /// Cleans with the run's options: `opts.trash_mode` is the resolved
    /// deletion mode, so `uses_trash` and the actual deletion can never
    /// disagree (the `CleanOptions` contract).
    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.clean_with_mode(dry_run, reporter, opts.trash_mode)
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

    // ── complex TOML (toml crate parity) ──

    #[test]
    fn parse_tools_section_inline_comment_is_ignored() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section("[tools]\nnode = \"20.11.0\" # active\n", &mut result);
        assert!(
            result.contains(&("node".to_string(), "20.11.0".to_string())),
            "inline comment must not corrupt the version, got {result:?}"
        );
    }

    #[test]
    fn parse_tools_section_array_of_versions_is_parsed() {
        let mut result = std::collections::HashSet::new();
        MiseCleaner::parse_tools_section(
            "[tools]\nnode = [\"20.11.0\", \"18.19.0\"]\n",
            &mut result,
        );
        assert!(result.contains(&("node".to_string(), "20.11.0".to_string())));
        assert!(result.contains(&("node".to_string(), "18.19.0".to_string())));
    }

    // ── scan_pinned_versions ──

    #[test]
    fn scan_pinned_versions_no_config_files_returns_empty() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
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
        let result = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
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
        let result = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
        assert!(result.contains(&("python".to_string(), "3.13.0".to_string())));
    }

    #[test]
    fn scan_pinned_versions_malformed_toml_does_not_panic() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".mise.toml"), "[[[invalid toml").unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
        assert!(result.is_empty());
    }

    #[test]
    fn scan_pinned_versions_ignores_node_modules_git_target() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        // Pins inside vendored/build directories must not be honored.
        for dir in ["node_modules", ".git", "target"] {
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
            fs::write(
                tmp.path().join(dir).join(".mise.toml"),
                "[tools]\nnode = \"19.0.0\"\n",
            )
            .unwrap();
        }
        // A real project .mise.toml is honored.
        fs::write(
            tmp.path().join(".mise.toml"),
            "[tools]\nnode = \"20.11.0\"\n",
        )
        .unwrap();
        let result = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
        assert!(result.contains(&("node".to_string(), "20.11.0".to_string())));
        assert!(
            !result.contains(&("node".to_string(), "19.0.0".to_string())),
            "pins inside node_modules/.git/target must be ignored, got {result:?}"
        );
    }

    #[test]
    fn scan_pinned_versions_respects_max_depth_boundary() {
        use tempfile::TempDir;
        // home(0)/a(1)/b(2)/c(3)/d(4)/.mise.toml(5) is exactly at depth 5.
        let tmp = TempDir::new().unwrap();
        let deep = tmp.path().join("a/b/c/d");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join(".mise.toml"), "[tools]\nnode = \"21.0.0\"\n").unwrap();

        let at_limit = MiseCleaner::scan_pinned_versions(tmp.path(), 5);
        assert!(
            at_limit.contains(&("node".to_string(), "21.0.0".to_string())),
            "a .mise.toml exactly at max_depth must be found"
        );

        let below = MiseCleaner::scan_pinned_versions(tmp.path(), 4);
        assert!(
            !below.contains(&("node".to_string(), "21.0.0".to_string())),
            "a .mise.toml beyond max_depth must be skipped"
        );
    }

    #[test]
    fn clean_handles_mise_ls_current_failure_gracefully() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        // even though mise is not found, clean should not crash
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
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
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(clean_result.bytes_freed, 0);
    }

    #[test]
    fn detect_returns_unavailable_when_mise_command_fails() {
        // mise is installed but its command fails (e.g. timed out after 30s):
        // the target is present, so scan must say `unavailable`, not `not found`.
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".local/share/mise/installs/node/20.11.0")).unwrap();
        let runner = MockRunner::new().with_failure("mise");
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        let detect_result = cleaner.detect();
        assert!(
            matches!(detect_result.status, ScanStatus::Unavailable),
            "expected Unavailable, got {:?}",
            detect_result.status
        );
    }

    // ── fail-closed on unparseable `ls --current` output (PBI 53) ──

    #[test]
    fn clean_does_not_delete_when_active_set_is_unparseable() {
        // `mise ls --current` printed data the parser cannot understand: the
        // semantic-empty active set must NOT mark every installed runtime
        // deletable (PBI 53 fail-closed).
        let tmp = TempDir::new().unwrap();
        let version = tmp.path().join(".local/share/mise/installs/node/20.11.0");
        fs::create_dir_all(&version).unwrap();
        let runner = MockRunner::new()
            .with_stdout("mise", "node=20.11.0;active\n")
            .with_success("chflags");
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "nothing may be deleted");
        assert!(version.exists(), "the installed runtime must survive");
    }

    #[test]
    fn detect_returns_unavailable_when_active_set_is_unparseable() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".local/share/mise/installs/node/20.11.0")).unwrap();
        let runner = MockRunner::new().with_stdout("mise", "node=20.11.0;active\n");
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        let detect_result = cleaner.detect();
        assert!(
            matches!(detect_result.status, ScanStatus::Unavailable),
            "expected Unavailable, got {:?}",
            detect_result.status
        );
    }

    #[test]
    fn clean_still_deletes_when_mise_ls_prints_nothing() {
        // Empty stdout is the legitimate "no active versions" case: the
        // fail-closed guard must not trip and installed versions stay
        // deletable (the trash_mode pin tests rely on this). The opts pin
        // Permanent so the sandboxed trashItemAtURL TCC limit cannot
        // interfere with the deletion assertion.
        let tmp = TempDir::new().unwrap();
        let version = tmp.path().join(".local/share/mise/installs/node/20.11.0");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("cache.bin"), vec![0u8; 1024]).unwrap();
        let runner = MockRunner::new()
            .with_success("mise")
            .with_success("chflags");
        let cleaner = MiseCleaner::new(tmp.path(), Box::new(runner));
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        assert!(!version.exists(), "an unused runtime stays deletable");
        assert!(result.bytes_freed > 0);
    }

    #[test]
    fn output_unparseable_empty_stdout_is_legitimate() {
        assert!(!MiseCleaner::output_unparseable("", &HashSet::new()));
    }

    #[test]
    fn output_unparseable_total_parse_failure_is_true() {
        let stdout = "node=20.11.0;active\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(MiseCleaner::output_unparseable(stdout, &active));
    }

    #[test]
    fn output_unparseable_parseable_output_is_false() {
        let stdout = "node\t24.15.0\t~/.config/mise/config.toml\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(!MiseCleaner::output_unparseable(stdout, &active));
    }

    #[test]
    fn output_unparseable_single_token_line_is_true() {
        // A format change that breaks SOME lines must fail closed too.
        let stdout = "node\t24.15.0\t~/.config/mise/config.toml\nbroken-line\n";
        let active = MiseCleaner::parse_active_versions(stdout);
        assert!(MiseCleaner::output_unparseable(stdout, &active));
    }

    // ── trash_mode pin contract ──

    /// One installed node version plus its cleaner: `mise ls --current`
    /// returns empty stdout, so the version is unused and deletable.
    fn unused_version_cleaner(tmp: &TempDir) -> (MiseCleaner, PathBuf) {
        let version = tmp.path().join(".local/share/mise/installs/node/20.11.0");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("cache.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = MiseCleaner::new(
            tmp.path(),
            Box::new(
                MockRunner::new()
                    .with_success("mise")
                    .with_success("chflags"),
            ),
        );
        (cleaner, version)
    }

    fn pin_opts(trash_mode: crate::trash::TrashMode) -> crate::cleaner::CleanOptions {
        crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode,
        }
    }

    #[test]
    fn clean_with_opts_pinned_permanent_deletes_for_real() {
        // The CleanOptions.trash_mode pin contract: a pinned Permanent must
        // delete for real and claim no Trash.
        let tmp = TempDir::new().unwrap();
        let (cleaner, version) = unused_version_cleaner(&tmp);
        let opts = pin_opts(crate::trash::TrashMode::Permanent);
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        assert!(
            !version.exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !result.uses_trash,
            "uses_trash must come from the pinned mode, not the global"
        );
        assert!(
            result.bytes_freed > 0,
            "the pinned deletion must count bytes"
        );
    }

    #[test]
    fn clean_with_opts_pinned_trash_claims_trash() {
        // A pinned Trash must resolve the deletion seam to trash. The
        // global is pinned to permanent so only the pin can claim Trash.
        // mise drops the seam outcome's claim on a skip (uses_trash only
        // accumulates for successful deletions and `skipped` stays empty),
        // so the assertion follows the seam truth: a landed Trash move
        // claims Trash and counts bytes, a TCC-skipped one (this sandbox)
        // claims neither.
        let tmp = TempDir::new().unwrap();
        let (cleaner, _version) = unused_version_cleaner(&tmp);
        let opts = pin_opts(crate::trash::TrashMode::Trash);
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        let deletions_happened = result.bytes_freed > 0;
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
        let tmp = TempDir::new().unwrap();
        let (cleaner, _version) = unused_version_cleaner(&tmp);
        let opts = pin_opts(crate::trash::TrashMode::default());
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        let deletions_happened = result.bytes_freed > 0;
        assert_eq!(
            result.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    #[test]
    fn clean_all_skipped_reports_exit_code_one() {
        // Ledger pin (PBI 70, the same contract as delete_entries'
        // all-skipped pin): with the test-only backend refusing every trash
        // move, the ledger records the deletion as a skip and the clean
        // reports failure (exit 1). The pre-ledger loop assembled a bare
        // freed result that could not express the failure, and its fatal
        // branch recorded no skip at all.
        let _backend = crate::trash::TrashBackendOverrideGuard::set();
        let tmp = TempDir::new().unwrap();
        let (cleaner, version) = unused_version_cleaner(&tmp);
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "a skipped entry is never summed");
        assert_eq!(result.skipped.len(), 1, "the refused move is a skip");
        assert!(
            matches!(
                &result.skipped[0].reason,
                crate::cleaner::SkipCause::Delete(_)
            ),
            "the seam's typed refusal rides in the skip: {:?}",
            result.skipped[0].reason
        );
        assert!(version.exists(), "the refused runtime is untouched");
        assert!(!result.uses_trash, "a skipped move never claims Trash");
        assert_eq!(result.exit_code(), 1, "all-skipped reports failure");
    }
}
