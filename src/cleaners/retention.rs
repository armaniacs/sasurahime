//! The one owner of the keep-highest retention judgment shared by the
//! gradle, JetBrains, browser and uv cleaners.
//!
//! `keep_highest` keeps every versioned candidate except the one with the
//! highest version key (the "keep the latest N generations" rule with N = 1).
//! `is_version_dir` is the shared file-type eligibility check: only a real
//! directory may become a deletion candidate. Both moved here from
//! gradle.rs once uv joined as a fourth consumer (PBI-85 plan B). The move
//! is mechanical — identical bodies, unchanged `pub(crate)` visibility — and
//! no naming rule lives here: each consumer parses its own names and only
//! the file-type and retention decisions are shared (`is_version_dir` checks
//! the file type only, which is why it is a shared judgment rather than a
//! gradle-specific naming rule).

use std::fs;
use std::path::PathBuf;

/// A version subdirectory that may become a deletion candidate: a real
/// directory, never a symlink. Symlinked versions are stale or shared links —
/// never deletion candidates (the browsers cleaner's rule). An unreadable file
/// type is treated as a symlink: never touch what we cannot classify.
pub(crate) fn is_version_dir(e: &fs::DirEntry) -> bool {
    match e.file_type() {
        Ok(t) => t.is_dir() && !t.is_symlink(),
        Err(_) => false,
    }
}

/// The one owner of the "keep all but the highest version" retention rule
/// shared by the browsers, gradle, JetBrains and uv cleaners. Groups
/// `candidates` by their numeric version key and returns the paths whose key
/// is not the highest in the group. A candidate without a numeric key (a name
/// with no digits) is never a deletion candidate.
pub(crate) fn keep_highest(candidates: Vec<(Vec<u32>, PathBuf)>) -> Vec<PathBuf> {
    // A name without digits has no version key: never a deletion candidate.
    let versioned: Vec<(Vec<u32>, PathBuf)> = candidates
        .into_iter()
        .filter(|(k, _)| !k.is_empty())
        .collect();
    let Some(max) = versioned.iter().map(|(k, _)| k.clone()).max() else {
        return vec![];
    };
    versioned
        .into_iter()
        .filter(|(k, _)| *k != max)
        .map(|(_, p)| p)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn keep_highest_returns_all_but_highest_key() {
        let candidates = vec![
            (vec![8, 8, 0], PathBuf::from("/c/8.8.0")),
            (vec![8, 12, 0], PathBuf::from("/c/8.12.0")),
            (vec![8, 10, 0], PathBuf::from("/c/8.10.0")),
        ];
        let old = keep_highest(candidates);
        assert_eq!(
            old,
            vec![PathBuf::from("/c/8.8.0"), PathBuf::from("/c/8.10.0"),]
        );
    }

    #[test]
    fn keep_highest_skips_candidates_without_a_numeric_key() {
        // A name with no digits has no version key: never a deletion
        // candidate, even when it is the only entry in its group.
        let candidates = vec![
            (vec![], PathBuf::from("/c/nightly")),
            (vec![1], PathBuf::from("/c/journal-1")),
        ];
        let old = keep_highest(candidates);
        assert!(old.is_empty(), "empty-key candidates must never be deleted");
    }

    #[test]
    fn keep_highest_single_candidate_returns_empty() {
        let candidates = vec![(vec![8, 12, 0], PathBuf::from("/c/8.12.0"))];
        assert!(keep_highest(candidates).is_empty());
    }

    #[test]
    fn is_version_dir_rejects_symlinks() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("8.12.0");
        fs::create_dir_all(&target).unwrap();
        let link = tmp.path().join("8.10.0");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let entries: Vec<std::fs::DirEntry> = fs::read_dir(tmp.path()).unwrap().flatten().collect();
        let linked = entries.iter().find(|e| e.file_name() == "8.10.0").unwrap();
        let real = entries.iter().find(|e| e.file_name() == "8.12.0").unwrap();
        assert!(
            !is_version_dir(linked),
            "a symlinked version is never a deletion candidate"
        );
        assert!(is_version_dir(real));
    }
}
