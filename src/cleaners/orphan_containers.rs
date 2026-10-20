use std::collections::HashSet;

/// Vendors whose containers are never reported as orphans. Apple data is
/// managed by the OS, and Docker has its own cleaner.
const PROTECTED_VENDORS: &[&str] = &["com.apple", "com.docker"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Containers,
    GroupContainers,
}

impl ContainerKind {
    /// Directory name under `~/Library`, also used as the display label.
    pub fn label(self) -> &'static str {
        match self {
            ContainerKind::Containers => "Containers",
            ContainerKind::GroupContainers => "Group Containers",
        }
    }
}

/// Returns the leading team ID when `name` starts with `"<10 chars of A-Z0-9>."`.
pub fn team_id_prefix(name: &str) -> Option<&str> {
    let (head, _rest) = name.split_once('.')?;
    let is_team_id = head.len() == 10
        && head
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if is_team_id {
        Some(head)
    } else {
        None
    }
}

// `get` instead of slicing: folder names may contain multi-byte characters,
// and slicing at a non-boundary would panic.
fn strip_group_prefix(s: &str) -> &str {
    for prefix in ["groups.", "group."] {
        if let Some(head) = s.get(..prefix.len()) {
            if s.len() > prefix.len() && head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Turns a folder name into a lowercase bundle ID for matching.
/// Group Containers lose a leading team ID and then a leading `group.` /
/// `groups.`. Containers names are only lowercased.
pub fn normalize_container_id(name: &str, kind: ContainerKind) -> String {
    let stripped = match kind {
        ContainerKind::Containers => name,
        ContainerKind::GroupContainers => {
            let without_team = match team_id_prefix(name) {
                Some(team) => &name[team.len() + 1..],
                None => name,
            };
            strip_group_prefix(without_team)
        }
    };
    stripped.to_ascii_lowercase()
}

fn is_same_or_child(id: &str, parent: &str) -> bool {
    id == parent
        || (id.len() > parent.len()
            && id.starts_with(parent)
            && id.as_bytes()[parent.len()] == b'.')
}

/// `id` must already be normalized (lowercase).
pub fn is_protected_id(id: &str) -> bool {
    PROTECTED_VENDORS.iter().any(|v| is_same_or_child(id, v))
}

/// True when `id` equals an installed ID or is a dot-separated descendant of
/// one (`com.foo.app.helper` belongs to `com.foo.app`; `com.foo.appx` does not).
/// Both `id` and the set members must be lowercase.
pub fn is_owned_by_installed(id: &str, installed: &HashSet<String>) -> bool {
    if installed.contains(id) {
        return true;
    }
    id.bytes()
        .enumerate()
        .any(|(i, b)| b == b'.' && installed.contains(&id[..i]))
}

/// Returns the folder names (original spelling, input order) that look orphaned.
/// For Group Containers, a team ID that owns at least one installed app keeps
/// all of its folders, because the same team often uses unrelated group names
/// (`TEAM.slack` next to `TEAM.com.tinyspeck.slackmacgap`).
pub fn find_orphan_names(
    kind: ContainerKind,
    names: &[String],
    installed: &HashSet<String>,
) -> Vec<String> {
    let entries: Vec<(&String, String)> = names
        .iter()
        .map(|n| (n, normalize_container_id(n, kind)))
        .collect();

    let alive_teams: HashSet<&str> = if kind == ContainerKind::GroupContainers {
        entries
            .iter()
            .filter(|(_, id)| !id.is_empty() && is_owned_by_installed(id, installed))
            .filter_map(|(n, _)| team_id_prefix(n.as_str()))
            .collect()
    } else {
        HashSet::new()
    };

    entries
        .into_iter()
        .filter(|(n, id)| {
            if id.is_empty() || n.starts_with('.') {
                return false;
            }
            if is_protected_id(id) || is_owned_by_installed(id, installed) {
                return false;
            }
            match team_id_prefix(n.as_str()) {
                Some(team) => !alive_teams.contains(team),
                None => true,
            }
        })
        .map(|(n, _)| n.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn strings(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn team_id_prefix_accepts_only_ten_upper_alnum() {
        assert_eq!(team_id_prefix("S8EX82NJP6.com.foo"), Some("S8EX82NJP6"));
        assert_eq!(team_id_prefix("S8EX82NJP.com.foo"), None);
        assert_eq!(team_id_prefix("S8EX82NJP66.com.foo"), None);
        assert_eq!(team_id_prefix("s8ex82njp6.com.foo"), None);
        assert_eq!(team_id_prefix("com.foo.App"), None);
        assert_eq!(team_id_prefix("S8EX82NJP6"), None);
    }

    #[test]
    fn normalize_container_id_table() {
        use ContainerKind::{Containers as C, GroupContainers as G};
        let cases: &[(&str, ContainerKind, &str)] = &[
            ("com.foo.App", C, "com.foo.app"),
            ("S8EX82NJP6.com.foo", C, "s8ex82njp6.com.foo"),
            (
                "S8EX82NJP6.com.macpaw.CleanMyMac-mas.HealthMonitor",
                G,
                "com.macpaw.cleanmymac-mas.healthmonitor",
            ),
            (
                "243LU875E5.groups.com.apple.podcasts",
                G,
                "com.apple.podcasts",
            ),
            ("BQR82RBBHL.slack", G, "slack"),
            ("group.com.foo.App", G, "com.foo.app"),
            ("S8EX82NJP6.group.com.foo", G, "com.foo"),
            ("com.apple.Home.group", G, "com.apple.home.group"),
            ("ABCDE1234.foo", G, "abcde1234.foo"),
            ("abcde12345.foo", G, "abcde12345.foo"),
            ("S8EX82NJP6.", G, ""),
            ("group.", G, "group."),
            ("\u{3042}.com", G, "\u{3042}.com"),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(
                normalize_container_id(name, *kind),
                *expected,
                "name={name} kind={kind:?}"
            );
        }
    }

    #[test]
    fn is_protected_id_table() {
        let cases: &[(&str, bool)] = &[
            ("com.apple.safari", true),
            ("com.apple", true),
            ("com.applesauce.x", false),
            ("com.docker.docker", true),
            ("com.docker", true),
            ("com.dockerx", false),
            ("com.foo.apple", false),
        ];
        for (id, expected) in cases {
            assert_eq!(is_protected_id(id), *expected, "id={id}");
        }
    }

    #[test]
    fn is_owned_by_installed_uses_dot_boundary() {
        let installed = set(&["com.foo.app"]);
        assert!(is_owned_by_installed("com.foo.app", &installed));
        assert!(is_owned_by_installed("com.foo.app.helper", &installed));
        assert!(is_owned_by_installed("com.foo.app.a.b", &installed));
        assert!(!is_owned_by_installed("com.foo.appx", &installed));
        assert!(!is_owned_by_installed("com.foo", &installed));
        assert!(!is_owned_by_installed("com.bar.app", &installed));
    }

    #[test]
    fn find_orphan_names_containers() {
        let names = strings(&[
            "com.foo.App",
            "com.foo.App.Helper",
            "com.foo.AppX",
            "com.apple.Safari",
            "com.docker.docker",
            "com.bar.Gone",
            ".hidden",
        ]);
        let got = find_orphan_names(ContainerKind::Containers, &names, &set(&["com.foo.app"]));
        assert_eq!(got, strings(&["com.foo.AppX", "com.bar.Gone"]));
    }

    #[test]
    fn find_orphan_names_group_containers() {
        let names = strings(&[
            "BQR82RBBHL.com.tinyspeck.slackmacgap",
            "BQR82RBBHL.slack",
            "243LU875E5.groups.com.apple.podcasts",
            "ZZZZZ99999.group.com.example.gone",
            "group.com.foo.App",
            "com.apple.Home.group",
            "S8EX82NJP6.",
            "UBF8T346G9.com.microsoft.oneauth",
        ]);
        let installed = set(&["com.tinyspeck.slackmacgap", "com.foo.app"]);
        let got = find_orphan_names(ContainerKind::GroupContainers, &names, &installed);
        assert_eq!(
            got,
            strings(&[
                "ZZZZZ99999.group.com.example.gone",
                "UBF8T346G9.com.microsoft.oneauth"
            ])
        );
    }

    #[test]
    fn find_orphan_names_group_exact_id_with_team_prefix_is_kept() {
        let names = strings(&["BQR82RBBHL.com.tinyspeck.slackmacgap"]);
        let installed = set(&["com.tinyspeck.slackmacgap"]);
        assert!(find_orphan_names(ContainerKind::GroupContainers, &names, &installed).is_empty());
    }

    #[test]
    fn find_orphan_names_empty_input() {
        assert!(find_orphan_names(ContainerKind::Containers, &[], &set(&[])).is_empty());
    }
}
