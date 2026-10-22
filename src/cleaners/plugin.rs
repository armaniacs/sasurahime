use crate::cleaner::{CleanResult, Cleaner, Risk, ScanResult, ScanStatus};
use anyhow::Result;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// PATH prefix every plugin binary must carry. `sasurahime` itself never
/// matches: the prefix requires a suffix.
pub const PLUGIN_PREFIX: &str = "sasurahime-";

/// Default per-plugin timeout: a hung plugin is killed and reported
/// Unavailable instead of blocking the whole scan.
pub const DEFAULT_PLUGIN_TIMEOUT_SECS: u64 = 10;

/// Env override for the timeout (mainly for tests).
pub const TIMEOUT_ENV: &str = "SASURAHIME_PLUGIN_TIMEOUT_SECS";

/// Extracts the plugin name from a file name: `sasurahime-foo` -> `foo`.
pub fn plugin_name_from(file_name: &str) -> Option<String> {
    file_name
        .strip_prefix(PLUGIN_PREFIX)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Effective per-plugin timeout in seconds.
pub fn timeout_secs() -> u64 {
    std::env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PLUGIN_TIMEOUT_SECS)
}

/// The minimal plugin protocol. A plugin is an executable `sasurahime-<name>`
/// on PATH that answers:
/// - `<plugin> scan --json` -> stdout `{"status": "...", "bytes": <n>}` and
///   exit 0. Status is one of `pruneable`, `clean`, `notfound`, `unavailable`.
/// - `<plugin> clean [--dry-run]` -> performs (or plans) the clean, exit 0.
///
/// Any non-zero exit, invalid output or timeout maps to `Unavailable`, so one
/// broken plugin can never take the whole scan down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolStatus {
    pub status: String,
    pub bytes: u64,
}

/// Parses the `scan --json` protocol output.
pub fn parse_protocol_output(stdout: &str) -> Option<ProtocolStatus> {
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).ok()?;
    let status = value.get("status")?.as_str()?.to_string();
    let bytes = value.get("bytes").and_then(|b| b.as_u64()).unwrap_or(0);
    Some(ProtocolStatus { status, bytes })
}

fn status_from_protocol(protocol: &ProtocolStatus) -> Option<ScanStatus> {
    match protocol.status.as_str() {
        "pruneable" => Some(if protocol.bytes > 0 {
            ScanStatus::Pruneable(protocol.bytes)
        } else {
            ScanStatus::Clean
        }),
        "clean" => Some(ScanStatus::Clean),
        "notfound" => Some(ScanStatus::NotFound),
        "unavailable" => Some(ScanStatus::Unavailable),
        _ => None,
    }
}

pub struct PluginCleaner {
    static_name: &'static str,
    pub path: PathBuf,
}

impl PluginCleaner {
    pub fn new(name: &str, path: PathBuf) -> Self {
        // One leak per discovered plugin, once per process.
        let static_name: &'static str = Box::leak(name.to_string().into_boxed_str());
        Self { static_name, path }
    }

    /// Discovers `sasurahime-*` executables on PATH. The first match on PATH
    /// wins per plugin name.
    pub fn discover() -> Vec<PluginCleaner> {
        let path_var = std::env::var("PATH").unwrap_or_default();
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for dir in path_var.split(':') {
            if dir.is_empty() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let Some(name) = plugin_name_from(&file_name) else {
                    continue;
                };
                if seen.contains(&name) {
                    continue;
                }
                let path = entry.path();
                if !is_plugin_candidate(&path) {
                    continue;
                }
                seen.push(name.clone());
                out.push(PluginCleaner::new(&name, path));
            }
        }
        out.sort_by(|a, b| a.static_name.cmp(b.static_name));
        out
    }

    /// Runs a protocol command with the plugin timeout. `None` on timeout or
    /// spawn failure.
    fn run_protocol(&self, args: &[&str]) -> Option<(i32, String)> {
        let mut child = Command::new(&self.path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok()?;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs());
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return None,
            }
        };
        let mut stdout = String::new();
        if let Some(mut handle) = child.stdout.take() {
            let _ = handle.read_to_string(&mut stdout);
        }
        Some((status.code().unwrap_or(-1), stdout))
    }
}

impl Cleaner for PluginCleaner {
    fn name(&self) -> &'static str {
        self.static_name
    }

    fn detect(&self) -> ScanResult {
        let unavailable = || ScanResult::new(self.name(), ScanStatus::Unavailable);
        let Some((code, stdout)) = self.run_protocol(&["scan", "--json"]) else {
            return unavailable(); // timeout or spawn failure: isolated
        };
        if code != 0 {
            return unavailable();
        }
        match parse_protocol_output(&stdout)
            .as_ref()
            .and_then(status_from_protocol)
        {
            Some(status) => ScanResult::new(self.name(), status),
            None => unavailable(),
        }
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn crate::progress::ProgressReporter,
    ) -> Result<CleanResult> {
        let mut args = vec!["clean"];
        if dry_run {
            args.push("--dry-run");
        }
        match self.run_protocol(&args) {
            // The minimal protocol reports success by exit code only; bytes
            // are unknown, so nothing is recorded as freed.
            Some((0, _)) => Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            }),
            Some((code, _)) => Err(anyhow::anyhow!("plugin exited with code {code}")),
            None => Err(anyhow::anyhow!("plugin timed out or failed to spawn")),
        }
    }

    fn is_available(&self) -> bool {
        self.path.exists()
    }

    fn advisory(&self) -> Option<String> {
        Some("External plugin: sasurahime does not review third-party plugin behavior.".to_string())
    }

    fn risk(&self) -> Risk {
        Risk::Moderate
    }
}

/// Whether `path` looks like a plugin for discovery purposes (name + exec bit).
pub fn is_plugin_candidate(path: &Path) -> bool {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return false;
    };
    if plugin_name_from(&name).is_none() {
        return false;
    }
    path.metadata()
        .map(|m| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                m.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                m.is_file()
            }
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_name_extraction() {
        assert_eq!(plugin_name_from("sasurahime-foo"), Some("foo".to_string()));
        assert_eq!(
            plugin_name_from("sasurahime-foo-bar"),
            Some("foo-bar".to_string())
        );
        assert_eq!(
            plugin_name_from("sasurahime"),
            None,
            "no suffix: not a plugin"
        );
        assert_eq!(
            plugin_name_from("sasurahime-"),
            None,
            "empty suffix rejected"
        );
        assert_eq!(plugin_name_from("other-binary"), None);
    }

    #[test]
    fn protocol_output_parsing() {
        let ok = parse_protocol_output(r#"{"status": "pruneable", "bytes": 123}"#).unwrap();
        assert_eq!(ok.status, "pruneable");
        assert_eq!(ok.bytes, 123);

        let clean = parse_protocol_output(r#"{"status": "clean"}"#).unwrap();
        assert_eq!(clean.bytes, 0, "bytes default to 0");

        assert!(parse_protocol_output("not json").is_none());
        assert!(
            parse_protocol_output(r#"{"bytes": 5}"#).is_none(),
            "status required"
        );
    }

    #[test]
    fn status_mapping_covers_the_protocol() {
        let pruneable = ProtocolStatus {
            status: "pruneable".into(),
            bytes: 4096,
        };
        assert!(matches!(
            status_from_protocol(&pruneable),
            Some(ScanStatus::Pruneable(4096))
        ));
        let zero = ProtocolStatus {
            status: "pruneable".into(),
            bytes: 0,
        };
        assert!(matches!(
            status_from_protocol(&zero),
            Some(ScanStatus::Clean)
        ));
        let nf = ProtocolStatus {
            status: "notfound".into(),
            bytes: 0,
        };
        assert!(matches!(
            status_from_protocol(&nf),
            Some(ScanStatus::NotFound)
        ));
        let unknown = ProtocolStatus {
            status: "weird".into(),
            bytes: 0,
        };
        assert!(status_from_protocol(&unknown).is_none());
    }

    #[test]
    fn plugin_candidate_requires_exec_bit() {
        let tmp = tempfile::TempDir::new().unwrap();
        let exec = tmp.path().join("sasurahime-ok");
        std::fs::write(&exec, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&exec, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(is_plugin_candidate(&exec));

        let plain = tmp.path().join("sasurahime-plain");
        std::fs::write(&plain, b"#!/bin/sh\n").unwrap();
        assert!(!is_plugin_candidate(&plain), "no exec bit");

        let wrong_name = tmp.path().join("not-a-plugin");
        std::fs::write(&wrong_name, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&wrong_name, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(!is_plugin_candidate(&wrong_name), "name prefix required");
    }

    #[test]
    fn detect_isolates_failures_as_unavailable() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A plugin that exits 1 on scan: the row becomes unavailable.
        let failing = tmp.path().join("sasurahime-failing");
        std::fs::write(&failing, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let cleaner = PluginCleaner::new("failing", failing);
        assert!(matches!(cleaner.detect().status, ScanStatus::Unavailable));
        // clean() reports the failure without taking the process down.
        let reporter = crate::progress::DeepSuppressReporter;
        assert!(cleaner.clean(false, &reporter).is_err());
    }

    #[test]
    fn detect_isolates_timeouts_as_unavailable() {
        let tmp = tempfile::TempDir::new().unwrap();
        // A plugin that sleeps past the (overridden) timeout.
        let hung = tmp.path().join("sasurahime-hung");
        std::fs::write(&hung, "#!/bin/sh\nsleep 30\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hung, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let _guard = crate::test_helpers::EnvGuard::set(TIMEOUT_ENV, "1");
        let cleaner = PluginCleaner::new("hung", hung);
        assert!(matches!(cleaner.detect().status, ScanStatus::Unavailable));
    }

    #[test]
    fn working_plugin_roundtrips_the_protocol() {
        let tmp = tempfile::TempDir::new().unwrap();
        let good = tmp.path().join("sasurahime-good");
        std::fs::write(
            &good,
            "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nexit 0\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&good, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let cleaner = PluginCleaner::new("good", good);
        assert!(matches!(
            cleaner.detect().status,
            ScanStatus::Pruneable(512)
        ));
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(cleaner.name(), "good");
    }
}
