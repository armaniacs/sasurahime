use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Per-cleaner filter configuration from `[cleaner.<name>]` sections.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PerCleanerConfig {
    pub older_than_days: Option<u32>,
    pub larger_than_mb: Option<u64>,
}

/// A user-defined custom cache target from the config file (`[[custom]]`).
#[derive(Debug, Clone, Deserialize)]
pub struct CustomTarget {
    pub name: String,
    pub path: String,
}

/// A user-defined log target from the config file.
/// Kept separate from `cleaners::log::LogTarget` to avoid a cross-module dep.
/// `#[allow(dead_code)]` on fields: consumed by Task 3 (LogCleaner wiring).
#[derive(Debug, Clone, Deserialize)]
pub struct ExtraLogTarget {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct LogsSection {
    keep_days: Option<u32>,
    #[serde(default)]
    targets: Vec<ExtraLogTarget>,
}

/// `[project-artifacts]` section: roots, depth and recency for stale
/// project artifact detection.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ProjectArtifactsSection {
    #[serde(default)]
    roots: Vec<String>,
    max_depth: Option<usize>,
    recency_days: Option<u32>,
}

/// `[policy]` section: the declarative unattended-apply policy. Conservative
/// by construction: irreversible (Risky) targets never run unattended.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PolicySection {
    #[serde(default)]
    older_than_days: Option<u32>,
    #[serde(default)]
    larger_than_mb: Option<u64>,
    #[serde(default)]
    include_manual: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfig {
    // Option (not #[serde(default)] on a plain value): present must be true
    // only when the file really contains a [policy] section. A
    // `#[serde(default)]` PolicySection deserializes an absent section into
    // default values indistinguishable from a written one, which silently
    // unlocked the unattended `auto` gate for any pre-existing config
    // (PBI-41).
    #[serde(default)]
    policy: Option<PolicySection>,
    #[serde(default, rename = "project-artifacts", alias = "project_artifacts")]
    project_artifacts: ProjectArtifactsSection,
    #[serde(default)]
    logs: LogsSection,
    trash_mode: Option<bool>,
    suppress: Option<bool>,
    deep_suppress: Option<bool>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    custom: Vec<CustomTarget>,
    #[serde(default, rename = "cleaner")]
    per_cleaner: HashMap<String, PerCleanerConfig>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub logs_keep_days: u32,
    pub logs_extra_targets: Vec<ExtraLogTarget>,
    pub trash_mode: bool,
    pub suppress: bool,
    pub deep_suppress: bool,
    pub exclude: Vec<String>,
    pub custom: Vec<CustomTarget>,
    pub per_cleaner: HashMap<String, PerCleanerConfig>,
    pub project_artifacts: ProjectArtifactsConfig,
    pub policy: Policy,
}

/// Resolved `[policy]` settings. `None` means the section is absent, so the
/// unattended `auto` command refuses to run (no policy, no unattended deletes).
#[derive(Debug, Clone, Default)]
pub struct Policy {
    pub present: bool,
    pub older_than_days: Option<u32>,
    pub larger_than_mb: Option<u64>,
    pub include_manual: bool,
}

/// Resolved `[project-artifacts]` settings with defaults applied.
#[derive(Debug, Clone)]
pub struct ProjectArtifactsConfig {
    pub roots: Vec<PathBuf>,
    pub max_depth: usize,
    pub recency_days: u32,
}

impl Default for ProjectArtifactsConfig {
    fn default() -> Self {
        Self {
            roots: vec![PathBuf::from("~/Playground")],
            max_depth: 5,
            recency_days: 7,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            logs_keep_days: 7,
            logs_extra_targets: vec![],
            trash_mode: true,
            suppress: false,
            deep_suppress: false,
            exclude: vec![],
            custom: vec![],
            per_cleaner: HashMap::new(),
            project_artifacts: ProjectArtifactsConfig::default(),
            policy: Policy::default(),
        }
    }
}

/// Resolves the current user's home directory with the crate-wide chain:
/// a non-empty `HOME` environment variable first, then `dirs::home_dir()`
/// (the passwd fallback on macOS). Returns `None` when both fail; each call
/// site keeps its own failure semantics — `main::home` stops with `expect`,
/// `is_safe_delete_target` stays fail-closed, config/history fall back to a
/// default. One acquisition chain for the whole crate: an empty `HOME` never
/// resolves to `PathBuf::from("")`, which would silently point every
/// downstream cache/history/tilde path at the process CWD.
pub fn resolved_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

/// True when the process runs on CI. GitHub Actions and most CI systems set
/// the `CI` env var. The real system roots (`/Applications`,
/// `/System/Applications`, ...) cost minutes of cold metadata I/O on a fresh
/// CI runner, so the installed-apps collection limits itself to the
/// HOME-derived root there; fixtures and sandboxed runs put their apps in
/// the HOME tree, so test assertions keep passing.
pub fn is_ci() -> bool {
    match std::env::var_os("CI") {
        Some(v) => !v.is_empty() && v != "0" && v != "false",
        None => false,
    }
}

/// Reads and parses a config.toml file at the given path.
/// Returns defaults if the file does not exist.
fn parse_config_file(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("cannot read {:?}: {}", path, e))?;
    let raw: RawConfig = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("config parse error in {:?}: {}", path, e))?;
    Ok(Config {
        logs_keep_days: raw.logs.keep_days.unwrap_or(7),
        logs_extra_targets: raw.logs.targets,
        trash_mode: raw.trash_mode.unwrap_or(true),
        suppress: raw.suppress.unwrap_or(false),
        deep_suppress: raw.deep_suppress.unwrap_or(false),
        exclude: raw.exclude,
        custom: raw.custom,
        per_cleaner: raw.per_cleaner,
        policy: Policy {
            // True only when the file actually contains a [policy] section:
            // the unattended `auto` gate refuses to run otherwise (no policy,
            // no unattended deletes).
            present: raw.policy.is_some(),
            older_than_days: raw.policy.as_ref().and_then(|p| p.older_than_days),
            larger_than_mb: raw.policy.as_ref().and_then(|p| p.larger_than_mb),
            include_manual: raw.policy.is_some_and(|p| p.include_manual),
        },
        project_artifacts: ProjectArtifactsConfig {
            roots: if raw.project_artifacts.roots.is_empty() {
                ProjectArtifactsConfig::default().roots
            } else {
                let home = resolved_home_dir().unwrap_or_default();
                raw.project_artifacts
                    .roots
                    .iter()
                    .map(|r| Config::expand_tilde(r, &home))
                    .collect()
            },
            max_depth: raw
                .project_artifacts
                .max_depth
                .unwrap_or_else(|| ProjectArtifactsConfig::default().max_depth),
            recency_days: raw
                .project_artifacts
                .recency_days
                .unwrap_or_else(|| ProjectArtifactsConfig::default().recency_days),
        },
    })
}

impl Config {
    /// Loads config from `config_dir/config.toml`.
    /// Returns defaults if the file does not exist.
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join("config.toml");
        parse_config_file(&path)
    }

    /// Loads config from an explicit file path.
    /// Returns defaults if the file does not exist.
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            log::warn!("config file {:?} not found, using defaults", path);
        }
        parse_config_file(path)
    }

    /// Returns the effective logs keep_days: per-cleaner override if set,
    /// otherwise the top-level `logs_keep_days` value.
    pub fn effective_logs_keep_days(&self) -> u32 {
        self.per_cleaner
            .get("logs")
            .and_then(|p| p.older_than_days)
            .unwrap_or(self.logs_keep_days)
    }

    /// Expands a leading `~` to `home`. Other paths are returned unchanged.
    pub fn expand_tilde(path: &str, home: &Path) -> PathBuf {
        if let Some(rest) = path.strip_prefix("~/") {
            home.join(rest)
        } else if path == "~" {
            home.to_path_buf()
        } else {
            PathBuf::from(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_keep_days_is_7() {
        assert_eq!(Config::default().logs_keep_days, 7);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert_eq!(cfg.logs_keep_days, 7);
        assert!(cfg.logs_extra_targets.is_empty());
    }

    #[test]
    fn policy_absent_marks_present_false() {
        let tmp = TempDir::new().unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert!(!cfg.policy.present, "no section: auto must refuse to run");
    }

    #[test]
    fn existing_file_without_policy_section_marks_present_false() {
        // A pre-existing config.toml with no [policy] section (e.g. only
        // trash_mode) must NOT flip policy.present to true: the unattended
        // auto gate refuses to run without the section.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "trash_mode = false\n").unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert!(
            !cfg.policy.present,
            "existing file without [policy]: auto must refuse to run"
        );
    }

    #[test]
    fn existing_file_with_policy_section_marks_present_true() {
        // The counterpart: a parseable file that really contains [policy]
        // keeps present == true, so auto may run.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "[policy]\nolder_than_days = 30\n").unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert!(
            cfg.policy.present,
            "existing file with [policy]: auto may run"
        );
        assert_eq!(cfg.policy.older_than_days, Some(30));
    }

    #[test]
    fn policy_section_is_resolved_with_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            "[policy]\nolder_than_days = 30\nlarger_than_mb = 100\n",
        )
        .unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert!(cfg.policy.present);
        assert_eq!(cfg.policy.older_than_days, Some(30));
        assert_eq!(cfg.policy.larger_than_mb, Some(100));
        assert!(!cfg.policy.include_manual, "conservative default");
    }

    #[test]
    fn policy_include_manual_opt_in_resolves() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "[policy]\ninclude_manual = true\n").unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert!(cfg.policy.present);
        assert!(cfg.policy.include_manual);
    }

    #[test]
    fn keep_days_from_config() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "[logs]\nkeep_days = 30\n").unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert_eq!(cfg.logs_keep_days, 30);
    }

    #[test]
    fn extra_log_target_loaded() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[[logs.targets]]\nname = \"my-tool\"\npath = \"~/.local/share/my-tool/logs\"\n",
        )
        .unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert_eq!(cfg.logs_extra_targets.len(), 1);
        assert_eq!(cfg.logs_extra_targets[0].name, "my-tool");
    }

    #[test]
    fn invalid_toml_returns_error() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "not valid toml :::").unwrap();
        assert!(Config::load(tmp.path()).is_err());
    }

    #[test]
    fn expand_tilde_home_relative() {
        let expanded = Config::expand_tilde("~/.local/share/kilo/log", Path::new("/Users/test"));
        assert_eq!(expanded, PathBuf::from("/Users/test/.local/share/kilo/log"));
    }

    #[test]
    fn expand_tilde_tilde_alone() {
        let expanded = Config::expand_tilde("~", Path::new("/Users/test"));
        assert_eq!(expanded, PathBuf::from("/Users/test"));
    }

    #[test]
    fn expand_tilde_absolute_unchanged() {
        let expanded = Config::expand_tilde("/absolute/path", Path::new("/Users/test"));
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn config_default_trash_mode_is_true() {
        let cfg = Config::default();
        assert!(cfg.trash_mode, "default trash_mode must be true");
    }

    #[test]
    fn config_loads_trash_mode_true() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "trash_mode = true\n").unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert!(cfg.trash_mode, "trash_mode from config must be true");
    }

    #[test]
    fn config_loads_suppress_true() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "suppress = true\n").unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert!(cfg.suppress, "suppress from config must be true");
    }

    #[test]
    fn config_default_suppress_is_false() {
        let cfg = Config::default();
        assert!(!cfg.suppress, "default suppress must be false");
    }

    #[test]
    fn config_loads_deep_suppress_true() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "deep_suppress = true\n").unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert!(cfg.deep_suppress, "deep_suppress from config must be true");
    }

    #[test]
    fn default_exclude_is_empty() {
        let cfg = Config::default();
        assert!(cfg.exclude.is_empty(), "default exclude must be empty");
    }

    #[test]
    fn exclude_parsed_from_config() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "exclude = [\"act\", \"uv\"]\n",
        )
        .unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert_eq!(cfg.exclude, vec!["act".to_string(), "uv".to_string()]);
    }

    #[test]
    fn load_from_path_works() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("custom.toml");
        std::fs::write(&path, "exclude = [\"ollama\"]\n").unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg.exclude, vec!["ollama".to_string()]);
    }

    #[test]
    fn load_from_path_missing_returns_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.toml");
        let cfg = Config::load_from_path(&path).unwrap();
        assert!(cfg.exclude.is_empty());
        assert_eq!(cfg.logs_keep_days, 7);
    }

    #[test]
    fn load_from_path_invalid_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.toml");
        std::fs::write(&path, "not valid toml :::").unwrap();
        assert!(Config::load_from_path(&path).is_err());
    }

    #[test]
    fn per_cleaner_older_than_days_parsed() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[cleaner.uv]\nolder_than_days = 30\n",
        )
        .unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        let uv = cfg.per_cleaner.get("uv").unwrap();
        assert_eq!(uv.older_than_days, Some(30));
        assert_eq!(uv.larger_than_mb, None);
    }

    #[test]
    fn per_cleaner_larger_than_mb_parsed() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[cleaner.brew]\nlarger_than_mb = 500\n",
        )
        .unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        let brew = cfg.per_cleaner.get("brew").unwrap();
        assert_eq!(brew.larger_than_mb, Some(500));
        assert_eq!(brew.older_than_days, None);
    }

    #[test]
    fn per_cleaner_multiple_filters() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("config.toml"),
            "[cleaner.\"my-custom\"]\nolder_than_days = 7\nlarger_than_mb = 100\n",
        )
        .unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        let my = cfg.per_cleaner.get("my-custom").unwrap();
        assert_eq!(my.older_than_days, Some(7));
        assert_eq!(my.larger_than_mb, Some(100));
    }

    #[test]
    fn per_cleaner_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("config.toml"), "trash_mode = false\n").unwrap();
        let cfg = Config::load(tmp.path()).unwrap();
        assert!(cfg.per_cleaner.is_empty());
    }

    #[test]
    fn per_cleaner_default_is_empty() {
        let cfg = Config::default();
        assert!(cfg.per_cleaner.is_empty());
    }

    // ── The crate-wide HOME resolver (PBI-48) ──

    #[test]
    fn resolved_home_dir_filters_an_empty_home_var() {
        // An empty HOME env var must be rejected by the resolver itself, so
        // PathBuf::from("") can never leak downstream and resolve cache,
        // history and tilde paths against the process CWD. TEST_LOCK
        // serializes the process-global HOME mutation against the other
        // HOME-scoped tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = crate::test_helpers::EnvGuard::set("HOME", "");
        let got = resolved_home_dir();
        let expected = dirs::home_dir().expect("test host must have a home dir");
        assert_eq!(
            got,
            Some(expected),
            "an empty HOME must fall back to dirs::home_dir(), got {:?}",
            got
        );
    }

    #[test]
    fn resolved_home_dir_falls_back_to_dirs_when_home_unset() {
        // With HOME removed entirely the resolver must still find a home via
        // dirs::home_dir() — the acquisition chain's second arm. A genuine
        // dirs failure (no passwd entry) cannot be forced in-process; the
        // Option return is what call sites match on to pick their failure
        // semantics (expect / fail-closed / unwrap_or_default).
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = crate::test_helpers::EnvGuard::unset("HOME");
        let got = resolved_home_dir();
        let expected = dirs::home_dir().expect("test host must have a home dir");
        assert_eq!(
            got,
            Some(expected),
            "with HOME unset the resolver must fall back to dirs::home_dir(), got {:?}",
            got
        );
    }

    #[test]
    fn resolved_home_dir_prefers_a_nonempty_home_var() {
        // The env arm wins over dirs: HOME set to a real directory resolves
        // to exactly that directory.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", tmp.path().to_str().unwrap());
        assert_eq!(resolved_home_dir(), Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn is_ci_detects_the_ci_env_var() {
        // The installed-apps collection gates its real system-root scan on
        // this flag, so the recognized values must be exact: GitHub Actions
        // sets CI=true; "false"/"0"/"" must not trip it.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = crate::test_helpers::EnvGuard::set("CI", "true");
        assert!(is_ci(), "CI=true must be detected");
        drop(_guard);
        let _guard = crate::test_helpers::EnvGuard::set("CI", "false");
        assert!(!is_ci(), "CI=false must not be detected");
        drop(_guard);
        let _guard = crate::test_helpers::EnvGuard::unset("CI");
        assert!(!is_ci(), "an unset CI must not be detected");
    }
}
