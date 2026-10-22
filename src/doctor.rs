use std::path::Path;

/// Paths that macOS Full Disk Access (FDA) gates. `read_dir` on each is the
/// probe: the result is only whether the directory can be listed, never what
/// it contains. Paths relative to HOME.
pub const PROTECTED_PATHS: &[&str] = &[
    "Library/Mail",
    "Library/Application Support/MobileSync/Backup",
    ".Trash",
];

/// Outcome of probing one protected path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    /// `read_dir` succeeded: no problem.
    Readable,
    /// `read_dir` failed with `PermissionDenied`: FDA is missing.
    Denied,
    /// The path does not exist: the user does not use it, never a problem.
    Missing,
}

impl ProbeOutcome {
    pub(crate) fn from_error_kind(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::PermissionDenied => ProbeOutcome::Denied,
            _ => ProbeOutcome::Missing,
        }
    }
}

pub(crate) struct Probe {
    /// Path relative to HOME, for display.
    pub display: &'static str,
    pub outcome: ProbeOutcome,
}

/// Probes every protected path with `read_dir` and aggregates the results.
/// Non-existent paths are `Missing`, not problems.
pub(crate) fn probe(home: &Path) -> Vec<Probe> {
    PROTECTED_PATHS
        .iter()
        .map(|rel| {
            let outcome = match std::fs::read_dir(home.join(rel)) {
                Ok(_) => ProbeOutcome::Readable,
                Err(e) => ProbeOutcome::from_error_kind(e.kind()),
            };
            Probe {
                display: rel,
                outcome,
            }
        })
        .collect()
}

/// Paths that could not be read because Full Disk Access is missing.
/// Shared by `doctor` and the scan note.
pub(crate) fn denied_paths(home: &Path) -> Vec<&'static str> {
    probe(home)
        .into_iter()
        .filter(|p| p.outcome == ProbeOutcome::Denied)
        .map(|p| p.display)
        .collect()
}

/// Runs the `doctor` subcommand: reports protected paths that Full Disk
/// Access gates, with granting instructions. Informational only: never
/// changes any configuration.
pub fn run(home: &Path) {
    println!("Checking Full Disk Access (FDA)...");

    let denied = denied_paths(home);
    if denied.is_empty() {
        println!("No problems found: all protected paths are readable.");
        return;
    }

    println!(
        "\n{} path(s) could not be read because Full Disk Access is missing:",
        denied.len()
    );
    for rel in &denied {
        println!("  - ~/{rel}");
    }
    println!("\nTo fix this, grant Full Disk Access to your terminal app");
    println!("(FDA is granted per parent process, not per command):");
    println!("  System Settings > Privacy & Security > Full Disk Access");
    println!("then restart the terminal and run `sasurahime doctor` again.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_one(home: &Path, rel: &str) -> ProbeOutcome {
        probe(home)
            .into_iter()
            .find(|p| p.display == rel)
            .map(|p| p.outcome)
            .unwrap()
    }

    #[test]
    fn missing_paths_are_not_problems() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(probe_one(tmp.path(), "Library/Mail"), ProbeOutcome::Missing);
        assert!(denied_paths(tmp.path()).is_empty());
    }

    #[test]
    fn denied_paths_are_aggregated() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mail = tmp.path().join("Library/Mail");
        std::fs::create_dir_all(&mail).unwrap();
        let backup = tmp
            .path()
            .join("Library/Application Support/MobileSync/Backup");
        std::fs::create_dir_all(&backup).unwrap();
        for p in [&mail, &backup] {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o000)).unwrap();
            }
        }
        let denied = denied_paths(tmp.path());
        assert_eq!(
            denied,
            vec![
                "Library/Mail",
                "Library/Application Support/MobileSync/Backup"
            ],
            "only denied paths are reported"
        );
        for p in [&mail, &backup] {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
    }

    #[test]
    fn error_kinds_map_to_outcomes() {
        assert_eq!(
            ProbeOutcome::from_error_kind(std::io::ErrorKind::PermissionDenied),
            ProbeOutcome::Denied
        );
        assert_eq!(
            ProbeOutcome::from_error_kind(std::io::ErrorKind::NotFound),
            ProbeOutcome::Missing
        );
        assert_eq!(
            ProbeOutcome::from_error_kind(std::io::ErrorKind::Other),
            ProbeOutcome::Missing
        );
    }

    #[test]
    fn probe_never_reads_contents() {
        // A readable directory is probed by listing only; a file in place of a
        // directory fails as Missing (not a directory), never Denied.
        let tmp = tempfile::TempDir::new().unwrap();
        let mail = tmp.path().join("Library/Mail");
        std::fs::create_dir_all(tmp.path().join("Library")).unwrap();
        std::fs::write(&mail, b"not a directory").unwrap();
        assert_eq!(probe_one(tmp.path(), "Library/Mail"), ProbeOutcome::Missing);
    }
}
