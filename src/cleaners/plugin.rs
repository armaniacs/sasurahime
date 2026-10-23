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

/// Extracts the last balanced top-level JSON object from noisy stdout: real
/// plugins print log lines around the object. Returns the whole trimmed input
/// when no balanced object is found (the caller's parse then fails closed).
pub fn extract_last_json_object(s: &str) -> &str {
    let bytes = s.as_bytes();
    let (mut depth, mut start, mut last) = (0i32, None, None);
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s0) = start {
                        last = Some((s0, i + 1));
                    }
                }
            }
            _ => {}
        }
    }
    match last {
        Some((a, b)) => &s[a..b],
        None => s.trim(),
    }
}

/// Parses the `scan --json` protocol output.
pub fn parse_protocol_output(stdout: &str) -> Option<ProtocolStatus> {
    let value: serde_json::Value = serde_json::from_str(extract_last_json_object(stdout)).ok()?;
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
    /// wins per plugin name. `reserved` lists built-in target names: a plugin
    /// named like a built-in would impersonate it in scan output, so it is
    /// never discovered.
    pub fn discover_excluding(reserved: &[&str]) -> Vec<PluginCleaner> {
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
                if reserved.contains(&name.as_str()) {
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
    /// spawn failure. Both pipes are drained concurrently with a 1 MiB cap, so
    /// a chatty plugin cannot deadlock on a full pipe buffer, and the deadline
    /// bounds the total runtime: a grandchild that inherits the pipes must not
    /// hang the CLI past the timeout.
    fn run_protocol(&self, args: &[&str]) -> Option<(i32, String)> {
        use std::os::unix::process::CommandExt;
        const PIPE_CAP: usize = 1 << 20;
        let mut child = Command::new(&self.path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0) // a kill never reaches sasurahime's own group
            .spawn()
            .ok()?;
        let deadline = Instant::now() + Duration::from_secs(timeout_secs());
        // Reader threads with a cap: stdout holds the protocol JSON, stderr
        // the diagnostics that would otherwise be discarded.
        let stdout_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let stderr_buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let remaining = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(spawn_reader(
                out,
                stdout_buf.clone(),
                remaining.clone(),
                PIPE_CAP,
            ));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(spawn_reader(
                err,
                stderr_buf.clone(),
                remaining.clone(),
                PIPE_CAP,
            ));
        }
        let timed_out = |deadline: Instant| Instant::now() >= deadline;
        // Wait for the child within the deadline.
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if timed_out(deadline) {
                        let _ = child.kill();
                        return None; // reader threads detach; the CLI returns
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(_) => return None,
            }
        };
        // The child exited; bound the read too. A grandchild inheriting the
        // pipes keeps them open, so the readers must not be waited on forever.
        while remaining.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            if timed_out(deadline) {
                let _ = child.kill();
                return None; // detach the blocked readers
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        for r in readers {
            let _ = r.join();
        }
        let stdout = String::from_utf8_lossy(&stdout_buf.lock().unwrap_or_else(|p| p.into_inner()))
            .to_string();
        Some((status.code().unwrap_or(-1), stdout))
    }
}

/// Drains one pipe into a capped buffer, decrementing `remaining` on exit.
fn spawn_reader(
    mut stream: impl Read + Send + 'static,
    buf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    remaining: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    cap: usize,
) -> std::thread::JoinHandle<()> {
    remaining.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut guard = buf.lock().unwrap_or_else(|p| p.into_inner());
                    if guard.len() < cap {
                        let room = cap - guard.len();
                        guard.extend_from_slice(&chunk[..n.min(room)]);
                    }
                    // Past the cap: keep draining but stop storing, so the
                    // child never deadlocks on a full pipe.
                }
            }
        }
        remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    })
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
                m.is_file() && m.permissions().mode() & 0o111 != 0
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
    fn parse_protocol_output_extracts_json_from_noisy_output() {
        // Real-world plugins print log lines around the JSON object.
        let ok =
            parse_protocol_output("[plugin] starting...\n{\"status\": \"clean\"}\n[plugin] done")
                .unwrap();
        assert_eq!(ok.status, "clean");
        // Nested braces inside the object survive the extraction.
        let nested =
            parse_protocol_output("log {\"x\": 1}\n{\"status\": \"pruneable\", \"bytes\": 512}")
                .unwrap();
        assert_eq!(nested.status, "pruneable");
        assert_eq!(nested.bytes, 512);
    }

    #[test]
    fn is_plugin_candidate_rejects_directories() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("sasurahime-dirname");
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(
            !is_plugin_candidate(&dir),
            "a directory named like a plugin is not a plugin"
        );
    }

    #[test]
    fn discover_rejects_builtin_name_collisions() {
        // A `sasurahime-uv` executable on PATH must not impersonate the
        // trusted `uv` target.
        let tmp = tempfile::TempDir::new().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let plugin = bin_dir.join("sasurahime-uv");
        std::fs::write(&plugin, b"#!/bin/sh\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&plugin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = bin_dir.to_string_lossy().to_string();
        let _guard = crate::test_helpers::EnvGuard::set("PATH", &path);
        assert_eq!(
            PluginCleaner::discover_excluding(&["uv"]).len(),
            0,
            "a built-in name must never be discovered as a plugin"
        );
        assert_eq!(
            PluginCleaner::discover_excluding(&[]).len(),
            1,
            "without reservation the plugin is discovered"
        );
    }

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
