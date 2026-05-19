use assert_cmd::Command;
use std::collections::HashSet;

fn sasurahime() -> Command {
    Command::cargo_bin("sasurahime").unwrap()
}

/// Verify every target listed in `targets` output has a matching CLI subcommand.
#[test]
fn all_targets_are_valid_cli_subcommands() {
    let output = sasurahime().args(["targets"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let targets: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();

    assert!(!targets.is_empty(), "targets output must not be empty");

    // All targets should be recognized as clean subcommands
    for target in &targets {
        let help_out = sasurahime()
            .args(["clean", target, "--help"])
            .output()
            .unwrap();
        assert!(
            help_out.status.success(),
            "target `{target}` is in targets list but not a valid subcommand"
        );
    }
}

/// Verify no duplicate names between standard and extra targets.
#[test]
fn no_duplicate_targets() {
    let output = sasurahime().args(["targets"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let names: Vec<&str> = stdout
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();

    let mut seen = HashSet::new();
    for name in &names {
        assert!(
            seen.insert(name),
            "duplicate target name in targets output: {name}"
        );
    }
}
