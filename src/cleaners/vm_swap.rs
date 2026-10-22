use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Reports the size of `/private/var/vm` (swap files + sleep image) and the
/// machine uptime. Read-only: `clean` is a no-op because swap files cannot be
/// deleted by the user; they are re-initialized on reboot.
pub struct VmSwapCleaner {
    vm_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
    /// Note computed during `detect` so `advisory` never runs a command.
    note: OnceLock<Option<String>>,
}

impl VmSwapCleaner {
    pub fn new(vm_dir: PathBuf, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            vm_dir,
            runner,
            note: OnceLock::new(),
        }
    }
}

/// Parses the boot time (seconds since epoch) from `sysctl -n kern.boottime`.
/// Returns `None` on output that does not contain a `sec = N` pair.
pub(crate) fn parse_boottime_secs(output: &str) -> Option<u64> {
    let idx = output.find("sec")?;
    let rest = &output[idx..];
    let eq = rest.find('=')?;
    let tail = rest[eq + 1..].trim_start();
    let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Formats the uptime in whole days. Both arguments are seconds since epoch.
pub(crate) fn format_uptime_days(boot_secs: u64, now_secs: u64) -> String {
    if now_secs < boot_secs {
        // Clock skew: never invent an uptime.
        return "uptime unknown".to_string();
    }
    let days = (now_secs - boot_secs) / 86_400;
    match days {
        0 => "uptime less than 1 day".to_string(),
        1 => "uptime 1 day".to_string(),
        n => format!("uptime {n} days"),
    }
}

impl Cleaner for VmSwapCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Manual
    }

    fn name(&self) -> &'static str {
        "vm-swap"
    }

    fn is_available(&self) -> bool {
        // The swap directory exists on every macOS install.
        self.vm_dir.exists()
    }

    fn detect(&self) -> ScanResult {
        let entries = match std::fs::read_dir(&self.vm_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // Listed by permission only: report unknown, never a lying 0.
                return ScanResult::new(self.name(), ScanStatus::Unknown);
            }
            Err(_) => return ScanResult::new(self.name(), ScanStatus::NotFound),
        };
        let total: u64 = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_string_lossy();
                // Metadata lookups only; contents are never read. Apparent
                // size is used (swap files may be sparse; noted in advisory).
                name.starts_with("swapfile") || name == "sleepimage"
            })
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();

        // Uptime is computed here (scan is allowed to run commands during
        // detect) and cached so the advisory stays command-free.
        let uptime = self
            .runner
            .run("sysctl", &["-n", "kern.boottime"])
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .and_then(|s| parse_boottime_secs(&s))
            .map(|boot| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format_uptime_days(boot, now)
            })
            .unwrap_or_else(|| "uptime unknown".to_string());
        let _ = self.note.set(Some(uptime));

        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.vm_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        // No-op: swap files are owned by the kernel and re-initialized on
        // reboot. Deleting them from a running system is not possible without
        // disabling swap.
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![],
            deleted_paths: vec![],
        })
    }

    fn advisory(&self) -> Option<String> {
        let uptime = self.note.get().cloned().flatten()?;
        Some(format!(
            "vm-swap: {} holds swap files (apparent size; sparse files may differ). {}. \
             sasurahime cannot delete them: they are re-initialized on reboot.",
            self.vm_dir.display(),
            uptime
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::Risk;
    use crate::test_helpers::MockRunner;

    #[test]
    fn parse_boottime_secs_reads_sec_pair() {
        assert_eq!(
            parse_boottime_secs("{ sec = 1729600000, usec = 123456 } Thu Jan  1 00:00:00 2026"),
            Some(1_729_600_000)
        );
        assert_eq!(
            parse_boottime_secs("kern.boottime: { sec = 100, usec = 0 }"),
            Some(100)
        );
    }

    #[test]
    fn parse_boottime_secs_rejects_invalid_output() {
        assert_eq!(parse_boottime_secs(""), None);
        assert_eq!(parse_boottime_secs("no numbers here"), None);
        assert_eq!(parse_boottime_secs("{ sec = }"), None);
    }

    #[test]
    fn format_uptime_days_formats_zero_one_and_many() {
        // 2026-01-01 00:00:00 UTC
        let boot: u64 = 1_767_225_600;
        assert_eq!(format_uptime_days(boot, boot), "uptime less than 1 day");
        assert_eq!(format_uptime_days(boot, boot + 86_400), "uptime 1 day");
        assert_eq!(
            format_uptime_days(boot, boot + 86_400 * 12 + 3600),
            "uptime 12 days"
        );
        assert_eq!(format_uptime_days(boot + 100, boot), "uptime unknown");
    }

    #[test]
    fn clean_is_a_no_op() {
        let tmp = tempfile::TempDir::new().unwrap();
        let swap = tmp.path().join("swapfile0");
        std::fs::write(&swap, b"x".repeat(64)).unwrap();
        let cleaner = VmSwapCleaner::new(tmp.path().to_path_buf(), Box::new(MockRunner::new()));
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(swap.exists(), "no-op must not delete the swap file");
    }

    #[test]
    fn risk_is_manual_and_never_auto_cleaned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = VmSwapCleaner::new(tmp.path().to_path_buf(), Box::new(MockRunner::new()));
        assert_eq!(cleaner.risk(), Risk::Manual);
        assert!(
            !cleaner.auto_clean_allowed(),
            "vm-swap never runs under --yes"
        );
    }

    #[test]
    fn detect_sums_apparent_sizes_of_swap_and_sleepimage() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("swapfile0"), b"x".repeat(100)).unwrap();
        std::fs::write(tmp.path().join("sleepimage"), b"x".repeat(50)).unwrap();
        std::fs::write(tmp.path().join("unrelated"), b"x".repeat(10_000)).unwrap();
        let cleaner = VmSwapCleaner::new(tmp.path().to_path_buf(), Box::new(MockRunner::new()));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Pruneable(150)),
            "expected Pruneable(150), got {:?}",
            result.status
        );
    }

    #[test]
    fn detect_reports_unknown_when_listing_is_denied() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vm = tmp.path().join("vm");
        std::fs::create_dir_all(&vm).unwrap();
        std::fs::write(vm.join("swapfile0"), b"x".repeat(64)).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&vm, std::fs::Permissions::from_mode(0o000)).unwrap();
        }
        let cleaner = VmSwapCleaner::new(vm.clone(), Box::new(MockRunner::new()));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Unknown),
            "denied listing must be Unknown, got {:?}",
            result.status
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&vm, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn advisory_covers_uptime_and_reboot_guidance_without_running_commands() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("swapfile0"), b"x".repeat(64)).unwrap();
        // A runner that fails every command: the note must not depend on it
        // after detect, and detect must tolerate the failure.
        let cleaner = VmSwapCleaner::new(tmp.path().to_path_buf(), Box::new(MockRunner::new()));
        let _ = cleaner.detect();
        let note = cleaner.advisory().expect("note after detect");
        assert!(
            note.contains(tmp.path().to_str().unwrap()),
            "note must show the vm dir, got: {note}"
        );
        assert!(note.contains("uptime"), "note: {note}");
        assert!(note.contains("reboot"), "note: {note}");
        assert!(note.contains("apparent size"), "note: {note}");
    }
}
