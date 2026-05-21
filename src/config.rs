use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// A user-defined log target from the config file.
/// Kept separate from `cleaners::log::LogTarget` to avoid a cross-module dep.
/// `#[allow(dead_code)]` on fields: consumed by Task 3 (LogCleaner wiring).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
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

#[derive(Debug, Clone, Deserialize, Default)]
struct RawConfig {
    #[serde(default)]
    logs: LogsSection,
    trash_mode: Option<bool>,
    suppress: Option<bool>,
    deep_suppress: Option<bool>,
}

#[derive(Debug, Clone)]
/// `#[allow(dead_code)]`: fields consumed by Task 3 (LogCleaner wiring).
#[allow(dead_code)]
pub struct Config {
    pub logs_keep_days: u32,
    pub logs_extra_targets: Vec<ExtraLogTarget>,
    pub trash_mode: bool,
    pub suppress: bool,
    pub deep_suppress: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            logs_keep_days: 7,
            logs_extra_targets: vec![],
            trash_mode: true,
            suppress: false,
            deep_suppress: false,
        }
    }
}

impl Config {
    /// Loads config from `config_dir/config.toml`.
    /// Returns defaults if the file does not exist.
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("cannot read {:?}: {}", path, e))?;
        let raw: RawConfig = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("config parse error in {:?}: {}", path, e))?;
        Ok(Self {
            logs_keep_days: raw.logs.keep_days.unwrap_or(7),
            logs_extra_targets: raw.logs.targets,
            trash_mode: raw.trash_mode.unwrap_or(true),
            suppress: raw.suppress.unwrap_or(false),
            deep_suppress: raw.deep_suppress.unwrap_or(false),
        })
    }

    /// Expands a leading `~` to `home`. Other paths are returned unchanged.
    /// `#[allow(dead_code)]`: used by Task 3 (main.rs LogCleaner wiring).
    #[allow(dead_code)]
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
}
