//! Owns everything the cleaners know about installed apps: `.app`
//! enumeration, `Info.plist` identity reads, the installed set, and the
//! canonical install roots. `orphan-containers` consumes the installed set
//! to spare owned containers; `app-support` consumes the identities to tell
//! leftovers of uninstalled apps from the data of installed apps. A cleaner
//! that needs installed apps must go through this module, not around it.

use crate::command::CommandRunner;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The only values taken from an app's `Info.plist`. README promises that
/// nothing else is read out of it, so do not add fields here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub bundle_id: String,
    pub name: Option<String>,
    pub display_name: Option<String>,
}

const PLIST_KEY_BUNDLE_ID: &str = "CFBundleIdentifier";
const PLIST_KEY_NAME: &str = "CFBundleName";
const PLIST_KEY_DISPLAY_NAME: &str = "CFBundleDisplayName";

/// Value of `<key>{key}</key>` when it is directly followed by a non-blank
/// `<string>`. A different `<key>` in between means the value is not a string.
fn xml_string_value(xml: &str, key: &str) -> Option<String> {
    const OPEN: &str = "<string>";
    const CLOSE: &str = "</string>";
    let key_tag = format!("<key>{key}</key>");
    let after = &xml[xml.find(&key_tag)? + key_tag.len()..];
    let open_at = after.find(OPEN)?;
    if after.find("<key>").is_some_and(|k| k < open_at) {
        return None;
    }
    let rest = &after[open_at + OPEN.len()..];
    let value = rest[..rest.find(CLOSE)?].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// `None` when `CFBundleIdentifier` is missing; the two names are optional.
pub fn parse_app_identity_from_xml(xml: &str) -> Option<AppIdentity> {
    Some(AppIdentity {
        bundle_id: xml_string_value(xml, PLIST_KEY_BUNDLE_ID)?,
        name: xml_string_value(xml, PLIST_KEY_NAME),
        display_name: xml_string_value(xml, PLIST_KEY_DISPLAY_NAME),
    })
}

/// Reads an `Info.plist`. Binary plists (magic `bplist00`) are turned into XML
/// on stdout by `plutil`, which ships with macOS, so no plist crate is needed.
/// `-o -` matters: without it `plutil -convert` rewrites the file in place.
pub fn read_app_identity(plist: &Path, runner: &dyn CommandRunner) -> Option<AppIdentity> {
    let bytes = fs::read(plist).ok()?;
    if bytes.starts_with(b"bplist00") {
        let path = plist.to_str()?;
        let out = runner
            .run("plutil", &["-convert", "xml1", "-o", "-", path])
            .ok()?;
        if !out.status.success() {
            return None;
        }
        return parse_app_identity_from_xml(std::str::from_utf8(&out.stdout).ok()?);
    }
    parse_app_identity_from_xml(std::str::from_utf8(&bytes).ok()?)
}

/// An `.app` bundle probe shared with the orphan-containers cleaner:
/// case-insensitive `.app` extension plus a directory check, so a plain file
/// named `Foo.app` is never treated as a bundle.
pub(crate) fn is_app_bundle(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
        && p.is_dir()
}

/// `.app` bundles directly under each root, plus one level of subfolders
/// (for example `/Applications/Setapp/*.app`). Symlinked folders are not
/// followed: an unreadable file type is treated as a symlink. Overlapping
/// roots list each distinct bundle path once, so every bundle is read once
/// (the canonical root list makes a Utilities app visible through two roots).
pub(crate) fn list_app_bundles(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(root) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if is_app_bundle(&path) {
                if seen.insert(path.clone()) {
                    found.push(path);
                }
                continue;
            }
            let is_real_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_real_dir {
                continue;
            }
            if let Ok(sub) = fs::read_dir(&path) {
                for s in sub.flatten() {
                    let sub_path = s.path();
                    if is_app_bundle(&sub_path) && seen.insert(sub_path.clone()) {
                        found.push(sub_path);
                    }
                }
            }
        }
    }
    found
}

/// Bundle file names still identify an app whose `Info.plist` cannot be read.
pub(crate) fn app_bundle_stems(roots: &[PathBuf]) -> Vec<String> {
    list_app_bundles(roots)
        .iter()
        .filter_map(|app| app.file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect()
}

/// Identities of the apps found under `app_dirs`, in directory order.
/// Bundles whose `Info.plist` cannot be read are skipped.
pub fn collect_app_identities(
    app_dirs: &[PathBuf],
    runner: &dyn CommandRunner,
) -> Vec<AppIdentity> {
    list_app_bundles(app_dirs)
        .iter()
        .filter_map(|app| read_app_identity(&app.join("Contents/Info.plist"), runner))
        .collect()
}

/// Lowercase bundle IDs of the apps found under `app_dirs`.
pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String> {
    collect_app_identities(app_dirs, runner)
        .into_iter()
        .map(|app| app.bundle_id.to_ascii_lowercase())
        .collect()
}

/// The canonical install roots, shared by `orphan-containers` and
/// `app-support`, so the README read-scope promise and the installed set
/// agree. The real system roots (`/Applications`, `/System/Applications`,
/// ...) cost minutes of cold metadata I/O on a fresh CI runner, so there the
/// default collection limits itself to the HOME-derived root, where fixtures
/// put their apps. An injected root set (tests, config) is never trimmed.
pub fn default_app_roots(home: &Path) -> Vec<PathBuf> {
    if crate::config::is_ci() {
        vec![home.join("Applications")]
    } else {
        vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/Applications/Utilities"),
            PathBuf::from("/System/Applications"),
            PathBuf::from("/System/Applications/Utilities"),
            home.join("Applications"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{EnvGuard, MockRunner};
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex;
    use tempfile::TempDir;

    const SECRET_URL_SCHEME: &str = "sasurahime-secret-url-scheme-7f3a";
    const SECRET_VALUE: &str = "SasurahimeSecretValue7f3a";

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// Answers every `plutil` call with the same stdout and records the arguments.
    struct PlutilStub {
        stdout: String,
        exit_code: i32,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl PlutilStub {
        fn new(stdout: &str, exit_code: i32) -> Self {
            Self {
                stdout: stdout.to_string(),
                exit_code,
                calls: Mutex::new(vec![]),
            }
        }
    }

    impl CommandRunner for PlutilStub {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            if program != "plutil" {
                anyhow::bail!("unexpected program {program}");
            }
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.exit_code << 8),
                stdout: self.stdout.as_bytes().to_vec(),
                stderr: vec![],
            })
        }

        fn exists(&self, program: &str) -> bool {
            program == "plutil"
        }
    }

    fn plist_xml(bundle_id: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleName</key>\n\t<string>X</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>{bundle_id}</string>\n</dict>\n</plist>\n"
        )
    }

    /// A plist whose other keys hold values that must never come out of the
    /// reader. They are placed before the three keys on purpose.
    fn plist_xml_with_other_keys(bundle_id: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>{SECRET_URL_SCHEME}</string>
            </array>
        </dict>
    </array>
    <key>SasurahimeDummyKey</key>
    <string>{SECRET_VALUE}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleName</key>
    <string>Keeper</string>
    <key>CFBundleDisplayName</key>
    <string>Keeper Display</string>
</dict>
</plist>
"#
        )
    }

    fn write_app(apps_dir: &Path, rel: &str, bundle_id: &str) {
        let contents = apps_dir.join(rel).join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("Info.plist"), plist_xml(bundle_id)).unwrap();
    }

    /// An app whose `Info.plist` is binary, so every identity read goes
    /// through `plutil`.
    fn write_binary_app(apps_dir: &Path, rel: &str) {
        let contents = apps_dir.join(rel).join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("Info.plist"), b"bplist00\x00\x01\x02").unwrap();
    }

    #[test]
    fn parse_app_identity_from_xml_reads_three_keys() {
        assert_eq!(
            parse_app_identity_from_xml(&plist_xml("com.foo.App")),
            Some(AppIdentity {
                bundle_id: "com.foo.App".to_string(),
                name: Some("X".to_string()),
                display_name: None,
            })
        );
    }

    #[test]
    fn parse_app_identity_from_xml_missing_bundle_id_is_none() {
        let xml = "<dict><key>CFBundleName</key><string>X</string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_key_without_string_is_none() {
        let xml =
            "<dict><key>CFBundleIdentifier</key><true/><key>Other</key><string>y</string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_empty_bundle_id_is_none() {
        let xml = "<dict><key>CFBundleIdentifier</key><string>  </string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_ignores_other_keys() {
        let identity =
            parse_app_identity_from_xml(&plist_xml_with_other_keys("com.foo.Keeper")).unwrap();
        assert_eq!(
            identity,
            AppIdentity {
                bundle_id: "com.foo.Keeper".to_string(),
                name: Some("Keeper".to_string()),
                display_name: Some("Keeper Display".to_string()),
            }
        );
        let shown = format!("{identity:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
    }

    #[test]
    fn app_identity_has_exactly_three_fields() {
        // No `..` in the pattern: adding a field to AppIdentity breaks this
        // test on purpose, because README promises only these three values.
        let AppIdentity {
            bundle_id,
            name,
            display_name,
        } = AppIdentity {
            bundle_id: "com.foo.App".to_string(),
            name: None,
            display_name: None,
        };
        assert_eq!(bundle_id, "com.foo.App");
        assert!(name.is_none() && display_name.is_none());
    }

    #[test]
    fn read_app_identity_reads_xml_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, plist_xml("com.foo.App")).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(
            read_app_identity(&p, &runner).map(|a| a.bundle_id),
            Some("com.foo.App".to_string())
        );
    }

    #[test]
    fn read_app_identity_garbage_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(read_app_identity(&p, &runner), None);
    }

    #[test]
    fn read_app_identity_missing_file_is_none() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(
            read_app_identity(&tmp.path().join("nope.plist"), &runner),
            None
        );
    }

    #[test]
    fn read_app_identity_binary_plist_converts_with_plutil() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let runner = PlutilStub::new(&plist_xml_with_other_keys("com.example.bin"), 0);
        let identity = read_app_identity(&p, &runner).unwrap();
        assert_eq!(identity.bundle_id, "com.example.bin");
        assert_eq!(identity.name.as_deref(), Some("Keeper"));
        let shown = format!("{identity:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
        let path = p.to_str().unwrap().to_string();
        assert_eq!(
            *runner.calls.lock().unwrap(),
            vec![vec![
                "-convert".to_string(),
                "xml1".to_string(),
                "-o".to_string(),
                "-".to_string(),
                path,
            ]]
        );
    }

    #[test]
    fn read_app_identity_binary_plist_plutil_failure_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let failing = PlutilStub::new("", 1);
        assert_eq!(read_app_identity(&p, &failing), None);
        let not_found = MockRunner::new().with_not_found();
        assert_eq!(read_app_identity(&p, &not_found), None);
    }

    #[test]
    fn collect_installed_ids_scans_top_level_and_one_subfolder() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        write_app(&apps, "Foo.app", "com.foo.App");
        write_app(&apps, "Utilities/Bar.app", "com.bar.Tool");
        write_app(&apps, "Utilities/Deep/Baz.app", "com.baz.TooDeep");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.foo.app", "com.bar.tool"]));
    }

    #[test]
    fn collect_installed_ids_ignores_broken_and_non_app_entries() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Broken.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp/Contents")).unwrap();
        fs::write(
            apps.join("NotAnApp/Contents/Info.plist"),
            plist_xml("com.no.App"),
        )
        .unwrap();
        fs::write(apps.join("file.app"), b"plain file").unwrap();
        write_app(&apps, "Good.app", "com.good.App");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.good.app"]));
    }

    #[test]
    fn collect_installed_ids_handles_missing_dirs_and_multiple_roots() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("A");
        let b = tmp.path().join("B");
        write_app(&a, "One.app", "com.one.App");
        write_app(&b, "Two.app", "com.two.App");
        let missing = tmp.path().join("missing");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[a, missing, b], &runner);
        assert_eq!(ids, set(&["com.one.app", "com.two.app"]));
    }

    #[test]
    fn collect_app_identities_returns_only_the_three_values() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        let contents = apps.join("Keeper.app/Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(
            contents.join("Info.plist"),
            plist_xml_with_other_keys("com.foo.Keeper"),
        )
        .unwrap();
        let runner = MockRunner::new().with_not_found();
        let found = collect_app_identities(&[apps], &runner);
        assert_eq!(
            found,
            vec![AppIdentity {
                bundle_id: "com.foo.Keeper".to_string(),
                name: Some("Keeper".to_string()),
                display_name: Some("Keeper Display".to_string()),
            }]
        );
        let shown = format!("{found:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
    }

    #[test]
    fn list_app_bundles_finds_top_level_and_one_level_nested() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Foo Bar.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("Setapp/Baz.app")).unwrap();
        fs::create_dir_all(apps.join("Setapp/deep/Qux.app")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp")).unwrap();
        let mut found: Vec<String> = list_app_bundles(&[apps.clone(), tmp.path().join("missing")])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        found.sort();
        assert_eq!(found, vec!["Baz.app", "Foo Bar.app"]);
    }

    #[test]
    fn default_app_roots_lists_the_canonical_five_roots() {
        // The production default. On CI the gate trims it to the
        // HOME-derived root, so pin CI=false for this assertion. Exhaustive
        // on purpose: README promises this read scope and the orphan
        // cleaner consumes the same list, so a drift must fail here.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = EnvGuard::set("CI", "false");
        let dirs = default_app_roots(Path::new("/Users/x"));
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/Applications"),
                PathBuf::from("/Applications/Utilities"),
                PathBuf::from("/System/Applications"),
                PathBuf::from("/System/Applications/Utilities"),
                PathBuf::from("/Users/x/Applications"),
            ]
        );
    }

    #[test]
    fn default_app_roots_trims_to_home_on_ci() {
        // The CI gate: fresh runners cannot afford the cold metadata I/O of
        // the real system roots, so the default collection limits itself to
        // the HOME-derived root. An injected set is never trimmed.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = EnvGuard::set("CI", "true");
        let dirs = default_app_roots(Path::new("/Users/x"));
        assert_eq!(dirs, vec![PathBuf::from("/Users/x/Applications")]);
    }

    #[test]
    fn list_app_bundles_lists_a_shared_bundle_path_once() {
        // The canonical root list makes a Utilities app visible twice: top
        // level under the Utilities root and one sub-folder level under the
        // parent root. The path dedup keeps one entry per bundle path, so
        // nothing downstream reads the same bundle twice.
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        let utilities = apps.join("Utilities");
        write_app(&apps, "Utilities/Foo.app", "com.foo.util");
        write_app(&utilities, "Bar.app", "com.bar.tool");
        let mut found = list_app_bundles(&[apps, utilities]);
        found.sort();
        assert_eq!(
            found,
            vec![
                tmp.path().join("Applications/Utilities/Bar.app"),
                tmp.path().join("Applications/Utilities/Foo.app"),
            ]
        );
    }

    #[test]
    fn collect_app_identities_spawns_plutil_once_per_bundle_path() {
        // BDD: with the same bundle path visible through two roots, plutil
        // runs once per bundle path. Binary plists make every read go
        // through plutil, so the call count is the proof.
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        let utilities = apps.join("Utilities");
        write_binary_app(&apps, "Utilities/Foo.app");
        write_binary_app(&utilities, "Bar.app");
        let runner = PlutilStub::new(&plist_xml("com.count.check"), 0);
        let identities = collect_app_identities(&[apps, utilities], &runner);
        assert_eq!(identities.len(), 2);
        let calls = runner.calls.lock().unwrap().len();
        assert_eq!(calls, 2, "plutil must run once per distinct bundle path");
    }

    #[test]
    fn canonical_roots_enumerate_second_level_apps_but_not_third_level() {
        // Traversal boundary under the canonical list: an app two levels
        // under /Applications (/Applications/Utilities/<Folder>/X.app) is
        // top level under the Utilities root and is collected; an app three
        // levels under /Applications stays invisible, because each root
        // walks one sub-folder level.
        let tmp = TempDir::new().unwrap();
        let utilities = tmp.path().join("Applications/Utilities");
        write_app(&utilities, "Adobe Installers/X.app", "com.adobe.x");
        write_app(&utilities, "Adobe Installers/Deep/Y.app", "com.adobe.y");
        let roots = vec![
            tmp.path().join("Applications"),
            utilities,
            tmp.path().join("System/Applications"),
            tmp.path().join("System/Applications/Utilities"),
            tmp.path().join("home/Applications"),
        ];
        let ids = collect_installed_ids(&roots, &MockRunner::new().with_not_found());
        assert!(ids.contains("com.adobe.x"), "{ids:?}");
        assert!(!ids.contains("com.adobe.y"), "{ids:?}");
    }
}
