//! The one deletion-safety module: which paths may be deleted at all, and
//! how an env-var override resolves to a cache directory (PBI 57).
//!
//! Before this module `is_safe_delete_target` and `resolve_env_dir` lived in
//! the cleaner layer (`generic.rs`), and the platform-side delete seam
//! (`trash.rs::safe_delete`) called back into it — the platform layer
//! depending on the cleaner layer. Extracting the pair here removes the
//! inversion: both functions change for the same reason (deletion safety),
//! so they share one module below the seam.
//!
//! PBI 89 adds the third member for the same reason: the per-component
//! symlink scope check (`symlink_scope`), cut out of the check
//! `app_support.rs::resolve_allowed` already carried, so every delete path
//! vetted by the seam rejects an intermediate symlink below its trusted
//! base with one implementation.

use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn is_safe_delete_target(path: &Path) -> bool {
    let check = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = check.to_string_lossy();
    if path_str.is_empty() {
        return false;
    }
    if path_str.contains("..") {
        return false;
    }
    // Block HOME itself and every ancestor of HOME: an env-var redirect
    // (ACT_CACHE_DIR=$HOME, TF_PLUGIN_CACHE_DIR=/Users) must never be
    // deletable, and subdirectories of HOME stay allowed. HOME comes from
    // the crate-wide resolver (crate::config::resolved_home_dir: a non-empty
    // HOME env var first, then dirs::home_dir()), the same chain
    // `main::home` uses. This check stays stricter than `main::home`: the
    // empty filter inside the resolver keeps an empty HOME from leaking
    // PathBuf::from("") into the guard, and a `None` resolution skips the
    // HOME/ancestor guard instead of disabling it silently.
    let home = crate::config::resolved_home_dir();
    if let Some(home) = home {
        // HOME must be canonicalized with the target: a symlinked HOME would
        // otherwise defeat this check (canonical(HOME) != raw HOME).
        let home_path = fs::canonicalize(&home).unwrap_or_else(|_| home.clone());
        if check == home_path || home_path.starts_with(&check) {
            return false;
        }
    }
    // A relative path resolves against the process CWD: env vars like
    // XDG_CACHE_HOME="" must never yield a deletable target.
    if !check.is_absolute() {
        return false;
    }
    // Block exact sensitive roots and system paths.
    // Allow macOS tempdirs (/var/folders/...) while blocking /var itself
    // and known system subdirectories. Mounted and shared roots (/Volumes,
    // /net, /Users/Shared, /Library, /opt) are blocked whole: nothing under
    // them is this tool's business, and the component-wise prefix match
    // keeps HOME-scoped ~/Library paths deletable.
    let is_exact = |p: &str| check == Path::new(p);
    let is_sys = |p: &str| check.starts_with(p);
    !is_exact("/")
        && !is_exact("/private")
        && !is_sys("/System")
        && !is_sys("/etc")
        && !is_sys("/dev")
        && !is_sys("/proc")
        && !is_sys("/Applications")
        && !is_sys("/usr")
        && !is_sys("/tmp")
        && !is_sys("/private/etc")
        && !is_sys("/private/tmp")
        && !is_sys("/private/var/db")
        && !is_sys("/private/var/log")
        && !is_sys("/private/var/run")
        && !is_sys("/private/var/tmp")
        && !is_sys("/private/var/root")
        && !is_sys("/private/var/vm")
        && !is_sys("/var/log")
        && !is_sys("/var/db")
        && !is_sys("/var/root")
        && !is_sys("/var/run")
        && !is_exact("/var")
        && !is_sys("/Volumes")
        && !is_sys("/net")
        && !is_sys("/Users/Shared")
        && !is_sys("/Library")
        && !is_sys("/opt")
}

/// Outcome of the per-component symlink scope check for one delete
/// candidate ([`symlink_scope`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkScope {
    /// Every component below the trusted base exists and none is a symbolic
    /// link: the path may proceed to the deletion sink.
    Clean,
    /// Some component below the base does not exist: not a symlink, and the
    /// downstream delete produces its own not-found classification.
    Missing,
    /// Some component below the base is a symbolic link, a component could
    /// not be inspected, or the path carries a component this check cannot
    /// inspect: fail closed, the path must be refused.
    Rejected,
}

/// The per-component symlink scope check (PBI 89): no component strictly
/// below the trusted `base` may be a symbolic link.
///
/// [`is_safe_delete_target`] canonicalizes and vets the FINAL target, so a
/// cache path whose INTERMEDIATE component is a symlink into HOME (e.g.
/// `~/Library/Application Support/Slack` → `~/Documents/重要`) resolves to an
/// in-HOME destination and deletes the real data through the link. This
/// check inspects the components below `base` one by one with
/// `fs::symlink_metadata` and rejects the path at the first symlink, the
/// same shape as `app_support.rs::resolve_allowed`. Inspection is impossible
/// (a component's metadata cannot be read) or the path carries a `..`
/// component: fail closed to [`SymlinkScope::Rejected`].
///
/// The scope is limited to components below `base` on purpose: on macOS
/// `/var`, `/tmp` and `/etc` are themselves symlinks to `/private/*`, so
/// walking every component from the filesystem root would reject every
/// tempdir (`/var/folders/...`) and kill the whole test suite. A path that
/// is not below `base` at all is not inspected here — it relies on
/// [`is_safe_delete_target`]'s canonicalize + blocklist vetting.
pub fn symlink_scope(base: &Path, path: &Path) -> SymlinkScope {
    let Ok(rel) = path.strip_prefix(base) else {
        return SymlinkScope::Clean;
    };
    let rel: Vec<Component> = rel.components().collect();
    if rel.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return SymlinkScope::Rejected;
    }
    let mut cur = base.to_path_buf();
    for comp in rel {
        cur.push(comp);
        let meta = match fs::symlink_metadata(&cur) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SymlinkScope::Missing,
            Err(_) => return SymlinkScope::Rejected,
        };
        if meta.file_type().is_symlink() {
            return SymlinkScope::Rejected;
        }
    }
    SymlinkScope::Clean
}

/// The seam's scope check anchored on the trusted HOME base (PBI 89): the
/// [`safe_delete`]-side variant of [`symlink_scope`], so every deletion that
/// reaches the seam is inspected below HOME without each caller passing a
/// base. HOME comes from the crate-wide resolver, the same chain
/// [`is_safe_delete_target`] uses. With no resolvable HOME there is no
/// trusted base to anchor the component walk on: fail closed.
pub fn symlink_scope_below_home(path: &Path) -> SymlinkScope {
    match crate::config::resolved_home_dir() {
        Some(home) => symlink_scope(&home, path),
        None => SymlinkScope::Rejected,
    }
}

/// Resolves a cache directory that may be overridden by `var`.
///
/// # Side effect
///
/// Reads the process environment (`std::env::var(var)`) exactly once, at the
/// point where a cleaner is CONSTRUCTED: every env-dependent cleaner calls
/// this function from its constructor, so the resolved value is pinned at
/// construction and detect/clean never re-read the process environment.
/// PBI 88 unified the huggingface / pre-commit wrappers onto the
/// constructor-resolution shape the flutter / terraform / act cleaners
/// already used, and this revises the PBI 57 pin ("Reads the process
/// environment at call time... Injecting the value instead is a behavior
/// change and is deliberately out of scope (PBI 57)"): the read is no
/// longer per-call, and the value injection PBI 57 deferred is what PBI 88
/// shipped — the constructor passes the resolved `PathBuf` into the
/// adapter, so the lookup happens at construction by construction.
///
/// The OVERRIDE VALUE ITSELF is validated BEFORE any subpath is joined, so a
/// root-adjacent value (e.g. `HF_HOME=/`) can never yield a joined path
/// that passes the safety check. The JOIN RESULT is validated too: a
/// subpath carrying `..` — or an absolute one, which `PathBuf::join` uses
/// to replace the base — fails closed to the default. Unsafe values warn
/// and fall back. This fail-closed validation and the warn behaviour are
/// unchanged (PBI 57).
pub fn resolve_env_dir(
    var: &str,
    subpath: &str,
    default: std::path::PathBuf,
    label: &str,
) -> std::path::PathBuf {
    match std::env::var(var) {
        Ok(value) => {
            let base = PathBuf::from(&value);
            if !is_safe_delete_target(&base) {
                eprintln!(
                    "[{label}] WARNING: {var}={value} points to an unsafe path, using default"
                );
                default
            } else if subpath.is_empty() {
                base
            } else {
                let joined = base.join(subpath);
                if joined.to_string_lossy().contains("..") || !is_safe_delete_target(&joined) {
                    eprintln!(
                        "[{label}] WARNING: {var}={subpath} escapes the cache dir, using default"
                    );
                    default
                } else {
                    joined
                }
            }
        }
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_safe_delete_target_rejects_root() {
        assert!(!is_safe_delete_target(Path::new("/")));
    }

    #[test]
    fn is_safe_delete_target_rejects_system_dirs() {
        assert!(!is_safe_delete_target(Path::new("/System/Library")));
        assert!(!is_safe_delete_target(Path::new("/etc/hosts")));
        assert!(!is_safe_delete_target(Path::new("/var/log")));
        assert!(!is_safe_delete_target(Path::new("/Applications/Xcode.app")));
    }

    #[test]
    fn is_safe_delete_target_rejects_empty() {
        assert!(!is_safe_delete_target(Path::new("")));
    }

    #[test]
    fn is_safe_delete_target_rejects_home_and_ancestors() {
        // ACT_CACHE_DIR=$HOME or a /Users redirect must never be deletable:
        // an env var can point a Risk::Safe target at the whole home.
        // TEST_LOCK serializes the process-global HOME mutation against the
        // other HOME-scoped tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        assert!(
            !is_safe_delete_target(&home),
            "HOME itself is never a delete target"
        );
        assert!(
            !is_safe_delete_target(home.parent().unwrap()),
            "an ancestor of HOME is never a delete target"
        );
        // Subdirectories of HOME stay deletable.
        let cache = home.join(".cache/act");
        std::fs::create_dir_all(&cache).unwrap();
        assert!(
            is_safe_delete_target(&cache),
            "subdirs of HOME remain deletable"
        );
    }

    #[test]
    fn is_safe_delete_target_canonicalizes_home() {
        // A symlinked HOME must not defeat the guard: a target equal to
        // canonical(HOME) is still HOME. TEST_LOCK serializes the
        // process-global HOME mutation against the other HOME-scoped tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let real = tempfile::TempDir::new().unwrap();
        let link_dir = tempfile::TempDir::new().unwrap();
        let link = link_dir.path().join("linkhome");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &link.to_string_lossy());
        let canonical_home = std::fs::canonicalize(real.path()).unwrap();
        assert!(
            !is_safe_delete_target(&canonical_home),
            "canonical(HOME) is still HOME even when HOME itself is a symlink"
        );
    }

    /// Removes an env var for the test body and restores it on drop, even
    /// during a panic, so a failed test cannot unset it for parallel tests.
    /// (`test_helpers::EnvGuard` can only set a value.)
    struct UnsetEnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl UnsetEnvGuard {
        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for UnsetEnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn is_safe_delete_target_stays_active_when_home_unset() {
        // HOME unset used to silently disable the HOME/ancestor guard. The
        // dirs::home_dir fallback (the same one `main::home` uses) must keep
        // it resolved: the real home stays refused with HOME removed.
        // TEST_LOCK serializes the process-global HOME mutation against the
        // other HOME-scoped tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = UnsetEnvGuard::unset("HOME");
        let real_home = dirs::home_dir().expect("dirs fallback must resolve without HOME");
        assert!(
            !is_safe_delete_target(&real_home),
            "with HOME unset, the fallback-resolved home must still be refused"
        );
        // Unsetting HOME must not flip the whole check off: an unrelated path
        // stays deletable.
        let other = tempfile::TempDir::new().unwrap();
        let other_canon = other.path().canonicalize().unwrap();
        assert!(
            is_safe_delete_target(&other_canon),
            "unsetting HOME must not disable the check for unrelated paths"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_relative_paths() {
        // A relative env-derived target resolves against the process CWD:
        // never a safe delete target.
        assert!(!is_safe_delete_target(Path::new("relative/cache")));
        assert!(!is_safe_delete_target(Path::new("pre-commit")));
    }

    #[test]
    fn is_safe_delete_target_allows_home_cache() {
        assert!(is_safe_delete_target(Path::new("/Users/test/.cache/act")));
        assert!(is_safe_delete_target(Path::new(
            "/Users/test/Library/Caches"
        )));
    }

    #[test]
    fn is_safe_delete_target_rejects_tmp() {
        assert!(!is_safe_delete_target(Path::new("/tmp")));
        assert!(!is_safe_delete_target(Path::new("/tmp/safe-dir")));
    }

    #[test]
    fn is_safe_delete_target_rejects_private_tmp() {
        assert!(!is_safe_delete_target(Path::new("/private/tmp")));
        assert!(!is_safe_delete_target(Path::new("/dev/null")));
        assert!(!is_safe_delete_target(Path::new("/proc/self")));
    }

    #[test]
    fn is_safe_delete_target_rejects_usr() {
        assert!(!is_safe_delete_target(Path::new("/usr/local")));
        assert!(!is_safe_delete_target(Path::new("/usr/lib")));
    }

    #[test]
    fn is_safe_delete_target_rejects_dotdot_traversal() {
        assert!(!is_safe_delete_target(Path::new(
            "/Users/foo/../../etc/passwd"
        )));
        assert!(!is_safe_delete_target(Path::new("/tmp/../etc")));
    }

    #[test]
    fn is_safe_delete_target_canonicalize_follows_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let target = tmp.path().join("symlink_to_etc");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", &target).unwrap();
            assert!(
                !is_safe_delete_target(&target),
                "symlink to /etc must be rejected"
            );
        }
    }

    #[test]
    fn is_safe_delete_target_rejects_dotdot_redirection() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        let traversed = sub.join("../../etc");
        assert!(
            !is_safe_delete_target(&traversed),
            "path with .. to /etc must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_canonicalize_fallback_nonexistent() {
        let p = Path::new("/nonexistent_dir_for_test/deep/path");
        assert!(
            is_safe_delete_target(p),
            "non-existent path should fall through (not in blocklist)"
        );
    }

    #[test]
    fn is_safe_delete_target_canonicalize_fallback_rejects_dotdot() {
        let p = Path::new("/nonexistent/../../etc");
        assert!(
            !is_safe_delete_target(p),
            "path with .. must be rejected even if uncanonicalizable"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_private_var_tmp() {
        assert!(
            !is_safe_delete_target(Path::new("/private/var/tmp")),
            "/private/var/tmp must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/private/var/tmp/some-dir")),
            "/private/var/tmp/* must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_private_var_root() {
        assert!(
            !is_safe_delete_target(Path::new("/private/var/root")),
            "/private/var/root must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_volumes() {
        // /Volumes hosts mounted external and network volumes: a custom
        // target on a removable disk must never be a delete target.
        assert!(
            !is_safe_delete_target(Path::new("/Volumes")),
            "/Volumes itself must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/Volumes/USB")),
            "/Volumes/* (a mounted volume) must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_users_shared() {
        // /Users/Shared is shared with all local users: its contents must
        // never be auto-deleted. Per-user homes stay deletable.
        assert!(
            !is_safe_delete_target(Path::new("/Users/Shared")),
            "/Users/Shared must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/Users/Shared/other-users-cache")),
            "/Users/Shared/* must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_system_library() {
        // The system-wide /Library tree is refused, but HOME-scoped
        // ~/Library paths pass the HOME check before the blocklist and stay
        // deletable: the /Library entry matches whole path components only.
        assert!(
            !is_safe_delete_target(Path::new("/Library")),
            "/Library must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/Library/Logs")),
            "/Library/* must be rejected"
        );
        assert!(
            is_safe_delete_target(Path::new("/Users/test/Library/Caches")),
            "~/Library must stay deletable"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_opt() {
        // /opt holds installed software (e.g. /opt/homebrew): never a delete
        // target.
        assert!(
            !is_safe_delete_target(Path::new("/opt")),
            "/opt must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/opt/homebrew")),
            "/opt/* must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_net() {
        // /net is the autofs root for network mounts: never a delete target.
        assert!(
            !is_safe_delete_target(Path::new("/net")),
            "/net must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/net/server/share")),
            "/net/* (a network mount) must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_private_var_vm() {
        // /private/var/vm holds the kernel swapfiles: never a delete target.
        // It sits under /private/var, which is not itself blocked, so
        // /private/var/folders tempdirs stay allowed. /var/vm canonicalizes
        // to /private/var/vm and is refused through the same entry.
        assert!(
            !is_safe_delete_target(Path::new("/private/var/vm")),
            "/private/var/vm must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/private/var/vm/swapfile0")),
            "/private/var/vm/* (swapfile) must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/var/vm")),
            "/var/vm (canonical /private/var/vm) must be rejected"
        );
        assert!(
            is_safe_delete_target(Path::new("/private/var/folders/xy/T/tmpdir")),
            "/private/var/folders tempdirs must stay allowed"
        );
    }

    #[test]
    fn resolve_env_dir_falls_back_when_value_is_root() {
        // PBI 19 acceptance at the resolver level: the OVERRIDE VALUE is
        // validated BEFORE any subpath is joined, so HF_HOME=/ can never
        // yield /hub. Unsafe values fall back to the default.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default/hub");
        let _guard = crate::test_helpers::EnvGuard::set("HF_HOME", "/");
        assert_eq!(
            resolve_env_dir("HF_HOME", "hub", default.clone(), "huggingface"),
            default,
            "a root-adjacent override (HF_HOME=/) must fall back to the default"
        );
    }

    #[test]
    fn resolve_env_dir_warns_and_falls_back_when_value_is_unsafe() {
        // An unsafe override value (/etc) warns on stderr and falls back to
        // the default instead of being used as the delete target.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard = crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/etc");
        assert_eq!(
            resolve_env_dir(
                "SASURAHIME_TEST_RESOLVE_VAR",
                "hub",
                default.clone(),
                "test"
            ),
            default,
            "an unsafe override (/etc) must warn and fall back to the default"
        );
    }

    #[test]
    fn resolve_env_dir_returns_joined_path_when_value_is_safe() {
        // A safe override is used and the subpath is joined: the joined path
        // is returned unchanged.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard =
            crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir("SASURAHIME_TEST_RESOLVE_VAR", "hub", default, "test"),
            PathBuf::from("/Users/u/hf/hub"),
            "a safe override must be used with the subpath joined"
        );
    }

    #[test]
    fn resolve_env_dir_returns_base_when_subpath_is_empty() {
        // An empty subpath (terraform, act, flutter) returns the override
        // value itself, unchanged.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard =
            crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir("SASURAHIME_TEST_RESOLVE_VAR", "", default, "test"),
            PathBuf::from("/Users/u/hf"),
            "an empty subpath must return the override unchanged"
        );
    }

    #[test]
    fn resolve_env_dir_returns_default_when_var_unset() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        // The test var must be absent: we only remove a stale value and never
        // set it, so nothing can leak into parallel tests.
        std::env::remove_var("SASURAHIME_TEST_UNSET_RESOLVE_VAR");
        assert!(std::env::var("SASURAHIME_TEST_UNSET_RESOLVE_VAR").is_err());
        assert_eq!(
            resolve_env_dir(
                "SASURAHIME_TEST_UNSET_RESOLVE_VAR",
                "hub",
                default.clone(),
                "test"
            ),
            default,
            "an unset var must yield the default"
        );
    }

    #[test]
    fn resolve_env_dir_fail_closed_when_the_joined_subpath_escapes() {
        // The JOIN RESULT is validated after the join: a safe override with
        // an escaping subpath (`..`) must fail closed to the default, never
        // yield a delete target outside the cache dir.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard =
            crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir(
                "SASURAHIME_TEST_RESOLVE_VAR",
                "../../etc",
                default.clone(),
                "test"
            ),
            default,
            "an escaping subpath must fail closed to the default"
        );
    }

    #[test]
    fn resolve_env_dir_fail_closed_when_the_subpath_is_absolute() {
        // PathBuf::join with an ABSOLUTE subpath replaces the base entirely:
        // the join validation must catch that escape too.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard =
            crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir(
                "SASURAHIME_TEST_RESOLVE_VAR",
                "/etc",
                default.clone(),
                "test"
            ),
            default,
            "an absolute subpath must not replace the cache dir"
        );
    }

    #[test]
    fn resolve_env_dir_joined_safe_subpath_still_used() {
        // The hardening must not regress the normal join: a safe subpath on
        // a safe override is still used.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let default = tmp.path().join("default-cache");
        let _guard =
            crate::test_helpers::EnvGuard::set("SASURAHIME_TEST_RESOLVE_VAR", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir("SASURAHIME_TEST_RESOLVE_VAR", "hub", default, "test"),
            PathBuf::from("/Users/u/hf/hub"),
            "a safe subpath on a safe override must still be joined"
        );
    }

    // ── per-component symlink scope check (PBI 89) ──

    #[test]
    fn symlink_scope_allows_a_clean_path_below_base() {
        // Every component below the trusted base exists and none is a
        // symlink: the path proceeds.
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        let leaf = base.join("Library/Application Support/Slack/Cache");
        fs::create_dir_all(&leaf).unwrap();
        assert!(
            matches!(symlink_scope(&base, &leaf), SymlinkScope::Clean),
            "a symlink-free path below the base must be Clean, got {:?}",
            symlink_scope(&base, &leaf)
        );
    }

    #[test]
    fn symlink_scope_allows_a_file_leaf() {
        // The seam also deletes files (logs, single caches): a regular file
        // leaf is not a symlink and must stay Clean.
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        fs::create_dir_all(&base).unwrap();
        fs::write(base.join("old.log"), b"x").unwrap();
        assert!(matches!(
            symlink_scope(&base, &base.join("old.log")),
            SymlinkScope::Clean
        ));
    }

    #[test]
    fn symlink_scope_rejects_an_intermediate_symlink() {
        // The PBI-89 attack shape: ~/Library/Application Support/Slack →
        // ~/Documents/重要. The first symlinked component below the base
        // rejects the whole path, fail closed.
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        let real = tmp.path().join("real-data");
        fs::create_dir_all(real.join("Cache")).unwrap();
        fs::create_dir_all(&base).unwrap();
        std::os::unix::fs::symlink(&real, base.join("Slack")).unwrap();
        let attacked = base.join("Slack/Cache");
        assert!(
            matches!(symlink_scope(&base, &attacked), SymlinkScope::Rejected),
            "an intermediate symlink below the base must be Rejected, got {:?}",
            symlink_scope(&base, &attacked)
        );
    }

    #[test]
    fn symlink_scope_rejects_a_symlinked_leaf() {
        // The leaf itself may not be a symlink either: a leaf link into HOME
        // redirects the deletion the same way an intermediate one does.
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        let real = tmp.path().join("real-data");
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&base).unwrap();
        std::os::unix::fs::symlink(&real, base.join("Cache")).unwrap();
        assert!(matches!(
            symlink_scope(&base, &base.join("Cache")),
            SymlinkScope::Rejected
        ));
    }

    #[test]
    fn symlink_scope_reports_missing_for_a_nonexistent_path() {
        // A component below the base that does not exist is not a symlink:
        // Missing (the hint filter drops it, the seam lets the delete
        // produce its own not-found classification).
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        fs::create_dir_all(&base).unwrap();
        assert!(matches!(
            symlink_scope(&base, &base.join("nope/Cache")),
            SymlinkScope::Missing
        ));
    }

    #[test]
    fn symlink_scope_fails_closed_when_a_component_is_unreadable() {
        // 検査不能は拒否 (fail closed): a component whose metadata cannot be
        // inspected must reject the path, never skip the check.
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        fs::create_dir_all(base.join("child")).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o000)).unwrap();
        let scope = symlink_scope(&base, &base.join("child"));
        fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            matches!(scope, SymlinkScope::Rejected),
            "an uninspectable component must fail closed, got {scope:?}"
        );
    }

    #[test]
    fn symlink_scope_does_not_inspect_components_above_the_base() {
        // The macOS pitfall: /var, /tmp and /etc are symlinks to /private/*.
        // A tempdir under /var/folders must not be rejected by the /var
        // symlink, so the check stays scoped to components below the base:
        // a path that is not below the base at all is not inspected here.
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(matches!(
            symlink_scope(Path::new("/Users/other"), tmp.path()),
            SymlinkScope::Clean
        ));
        // A base deeper inside the tempdir with a clean path below it stays
        // Clean even though the path itself was reached through /var →
        // /private/var: components above the base are not inspected.
        let base = tmp.path().join("Library/Application Support");
        let leaf = base.join("Slack/Cache");
        fs::create_dir_all(&leaf).unwrap();
        assert!(matches!(symlink_scope(&base, &leaf), SymlinkScope::Clean));
    }

    #[test]
    fn symlink_scope_rejects_a_traversing_relative_component() {
        // A `..` component in the path below the base cannot be inspected
        // component-wise: fail closed.
        let tmp = tempfile::TempDir::new().unwrap();
        let base = tmp.path().join("base");
        let sub = base.join("sub");
        fs::create_dir_all(&sub).unwrap();
        assert!(matches!(
            symlink_scope(&base, &sub.join("../../etc")),
            SymlinkScope::Rejected
        ));
    }

    #[test]
    fn symlink_scope_below_home_rejects_the_intermediate_symlink_attack() {
        // The seam's HOME-anchored variant: the PBI-89 attack path below a
        // tempdir HOME is refused, while a clean cache path below the same
        // HOME stays Clean.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let real = tmp.path().join("real-data");
        fs::create_dir_all(real.join("Cache")).unwrap();
        let link = home.join("Library/Application Support/Slack");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        assert!(matches!(
            symlink_scope_below_home(&link.join("Cache")),
            SymlinkScope::Rejected
        ));
        let clean = home.join("Library/Caches/ms-playwright");
        fs::create_dir_all(&clean).unwrap();
        assert!(matches!(
            symlink_scope_below_home(&clean),
            SymlinkScope::Clean
        ));
    }
}
