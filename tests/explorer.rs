// Source pin for MANAGED_PATTERNS (PBI 2026-11-20-04): every entry must be
// home-relative. After the private explorer expand_tilde was retired in
// favour of the pinned Config::expand_tilde, a non-`~/` entry can no longer
// surface as behaviour: an absolute path passes through by join-replacement
// luck and a relative path resolves under home by accident, and the
// mis-solved path just silently drops the entry from the managed
// classification (no crash, no output change). The array is hand-grown one
// line per new cleaner, so the shape contract is guarded at the source
// (tests/AGENTS.md: a source pin is the last resort for drift that cannot
// surface as behaviour).

#[test]
fn managed_patterns_are_all_home_relative() {
    let src = include_str!("../src/explorer.rs");
    // The needles are assembled from pieces so this pin's own source never
    // carries the contiguous shapes it greps for (tests/AGENTS.md; the
    // round 8 vacuous-pin lesson). The anchor is the rustfmt-stable
    // definition-line shape `path_pattern: "..."`, which skips the struct
    // field declaration (`pub path_pattern: &'static str,`).
    let anchor = concat!("path_", "pattern: ", "\"");
    let home_relative = concat!("~", "/");
    let mut count = 0;
    for (idx, line) in src.lines().enumerate() {
        let Some(pos) = line.find(anchor) else {
            continue;
        };
        count += 1;
        let literal = line[pos + anchor.len()..].split('"').next().unwrap_or("");
        assert!(
            literal.starts_with(home_relative),
            "MANAGED_PATTERNS entry {literal:?} (line {}) must start with \
             {home_relative:?}: a non-home-relative pattern resolves by accident \
             and silently drops the entry from the managed classification",
            idx + 1
        );
    }
    assert!(
        count > 0,
        "the anchor must match at least one MANAGED_PATTERNS entry line \
         (a vacuous pin is always green)"
    );
}
