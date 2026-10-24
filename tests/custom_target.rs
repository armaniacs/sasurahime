// PBI-39: a [[custom]] config name must be reachable via `clean <name>`.
//
// The external_subcommand catch-all used to shadow user-defined targets:
// `sasurahime clean my-workspace` with `[[custom]] name = "my-workspace"` in
// config.toml fell through to the plugin lookup and failed with
// "no plugin named 'my-workspace'" (exit 1). These E2E tests pin the
// resolution order (custom names before the catch-all, built-in plugin names
// unchanged) and the binary-level refusal of an unsafe [[custom]] path.
//
// Deletion determinism: every real-clean run passes `--permanent` so the
// deletion goes through `remove_dir_all` instead of the Finder trash, whose
// backends can be TCC-blocked in this environment (tests/dry_run.rs control
// pattern).

use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn write_config(home: &Path, body: &str) {
    let dir = home.join(".config/sasurahime");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.toml"), body).unwrap();
}

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn clean_custom_target_reaches_the_configured_path() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/my-workspace");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("data.bin"), b"content").unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"my-workspace\"\npath = \"~/.cache/my-workspace\"\n",
    );

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "my-workspace"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "a [[custom]] target must be reachable via clean <name>, got:\n{combined}"
    );
    assert!(
        combined.contains("Removed: data.bin"),
        "the deletion path must run, got:\n{combined}"
    );
    assert!(
        !cache.join("data.bin").exists(),
        "the target's contents must be deleted, got:\n{combined}"
    );
    assert!(
        cache.exists(),
        "the target root must survive (contents-only deletion), got:\n{combined}"
    );
}

#[test]
fn clean_custom_target_on_unsafe_path_is_refused() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"system-logs\"\npath = \"/private/var/log\"\n",
    );

    let output = sasurahime(tmp.path())
        .args(["clean", "system-logs"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        !output.status.success(),
        "an unsafe [[custom]] path must be refused with a non-zero exit, got:\n{combined}"
    );
    assert!(
        combined.contains("not safe to delete"),
        "the refusal must name the unsafe path instead of the plugin miss, got:\n{combined}"
    );
    assert!(
        Path::new("/private/var/log").exists(),
        "the unsafe path must survive"
    );
}

#[test]
fn clean_custom_target_respects_dry_run() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/my-workspace");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("data.bin"), b"content").unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"my-workspace\"\npath = \"~/.cache/my-workspace\"\n",
    );

    let output = sasurahime(tmp.path())
        .args(["clean", "my-workspace", "--dry-run"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "dry-run must be accepted at the local position, got:\n{combined}"
    );
    assert!(
        combined.contains("[dry-run] would remove:"),
        "the custom dispatch must honor the raw --dry-run, got:\n{combined}"
    );
    assert!(
        cache.join("data.bin").exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn plugin_names_keep_current_priority_alongside_custom_targets() {
    // Control for the resolution-order change: a known plugin name must still
    // dispatch through the external catch-all when a [[custom]] entry with a
    // different name is configured.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let plugin = bin_dir.join("sasurahime-foo");
    fs::write(
        &plugin,
        "#!/bin/sh\n\
         if [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\n\
         printf '%s\\n' \"$@\" > \"$HOME/plugin-args.txt\"\n\
         rm -rf \"$HOME/.cache/plugin-foo\"\n\
         exit 0\n",
    )
    .unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
    let plugin_cache = tmp.path().join(".cache/plugin-foo");
    fs::create_dir_all(&plugin_cache).unwrap();
    fs::write(plugin_cache.join("x"), "x").unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"my-workspace\"\npath = \"~/.cache/my-workspace\"\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = Command::new(env!("CARGO_BIN_EXE_sasurahime"))
        .env("HOME", tmp.path())
        .env("PATH", &path)
        .args(["--permanent", "clean", "foo"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "a plugin name must keep resolving through the catch-all, got:\n{combined}"
    );
    let args = fs::read_to_string(tmp.path().join("plugin-args.txt"))
        .expect("the plugin must have been started");
    assert!(
        !args.contains("--dry-run"),
        "the control must run the plugin's real clean, got: {args}"
    );
    assert!(
        !plugin_cache.exists(),
        "the plugin must have cleaned its cache, got:\n{combined}"
    );
}
