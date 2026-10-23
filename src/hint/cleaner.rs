use crate::command::CommandRunner;
use crate::format::format_bytes;
use std::path::Path;

use super::{base_path, HintEntry, ProcessHint, PromptReader};

const QUIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
const QUIT_TIMEOUT_SECS: usize = 10;

/// Quit the app via osascript and wait until its process is gone.
pub(crate) fn quit_app_and_wait(
    entry: &HintEntry,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let quit_app = match entry.quit_app {
        Some(a) => a,
        None => anyhow::bail!("{} cannot be auto-quit", entry.display_name),
    };

    eprintln!("  Quitting {}...", entry.display_name);
    runner.run("osascript", &["-e", &quit_app_script(quit_app)])?;

    let process_name = entry.process_name.unwrap_or(quit_app);
    for _ in 0..QUIT_TIMEOUT_SECS {
        std::thread::sleep(QUIT_POLL_INTERVAL);
        let still_running = runner
            .run("pgrep", &["-x", process_name])
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !still_running {
            break;
        }
    }
    let still_running = runner
        .run("pgrep", &["-x", process_name])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if still_running {
        anyhow::bail!(
            "{} did not quit within {}s — skipping cache deletion",
            entry.display_name,
            QUIT_TIMEOUT_SECS
        );
    }
    Ok(())
}

/// Builds the `quit app "..."` AppleScript for an entry's app name.
///
/// quit_app comes from the compile-time KNOWN_ENTRIES table (every current
/// value is quote- and backslash-free), but nothing in the type forbids such
/// a character in a future entry; escaping the AppleScript string delimiters
/// here means a future value can never close the `quit app "..."` literal
/// early and interpolate arbitrary AppleScript.
fn quit_app_script(quit_app: &str) -> String {
    let escaped = quit_app.replace('\\', "\\\\").replace('"', "\\\"");
    format!("quit app \"{escaped}\"")
}

/// Relaunch the app if the entry configures `relaunch_app`.
pub(crate) fn relaunch_app(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()> {
    if let Some(app) = entry.relaunch_app {
        eprintln!("  Restarting {}...", entry.display_name);
        runner.run("open", &["-a", app])?;
        eprintln!("  [OK]");
    } else {
        eprintln!(
            "  ({} will not be restarted — relaunch manually if needed)",
            entry.display_name
        );
    }
    Ok(())
}

/// Quit the app, delete caches, optionally relaunch. Returns Err if quit timed out.
pub fn auto_clean_hint(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    quit_app_and_wait(entry, runner)?;

    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    let mut skip_reasons: Vec<String> = Vec::new();
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            // The seam clears uchg flags in permanent mode and honors the
            // trash-mode decision instead of a blind `rm -rf`.
            let outcome =
                crate::trash::safe_delete(runner, &path, crate::trash::TrashMode::Global)?;
            if let Some(reason) = outcome.skip_reason {
                log::warn!("failed to delete {}: {reason}", path.display());
                skip_reasons.push(reason);
            }
        }
    }
    eprintln!("{}", completion_line(&skip_reasons));

    relaunch_app(entry, runner)
}

/// Delete cache directories for a hint without quitting or relaunching the app.
fn clean_hint_dirs(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    let mut skip_reasons: Vec<String> = Vec::new();
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            // The seam clears uchg flags in permanent mode and honors the
            // trash-mode decision instead of a blind `rm -rf`.
            let outcome =
                crate::trash::safe_delete(runner, &path, crate::trash::TrashMode::Global)?;
            if let Some(reason) = outcome.skip_reason {
                log::warn!("failed to delete {}: {reason}", path.display());
                skip_reasons.push(reason);
            }
        }
    }
    eprintln!("{}", completion_line(&skip_reasons));
    Ok(())
}

/// For each hint that can be cleaned, ask the user and clean.
///
/// For running apps with a configured `quit_app`, the app is quit first, caches
/// are deleted, and the app is optionally relaunched.  For non-running apps the
/// cache directories are deleted directly without any quit/relaunch step.
pub fn offer_auto_clean(
    hints: &[ProcessHint],
    home: &Path,
    runner: &dyn CommandRunner,
    prompt: &dyn PromptReader,
) {
    let actionable: Vec<_> = hints
        .iter()
        .filter(|h| !h.running || h.entry.quit_app.is_some())
        .collect();

    if actionable.is_empty() {
        return;
    }

    for hint in actionable {
        let size_str = format_bytes(hint.size_bytes);
        if hint.running {
            eprint!(
                "\nQuit {} and clear cache? ({size_str} will be freed) [y/N] ",
                hint.entry.display_name
            );
        } else {
            eprint!(
                "\nClear {} cache? ({size_str} will be freed) [y/N] ",
                hint.entry.display_name
            );
        }
        match prompt.read_line() {
            Some(input) if input.trim().eq_ignore_ascii_case("y") => {
                if hint.running {
                    if let Err(e) = auto_clean_hint(hint, home, runner) {
                        eprintln!("  Error: {e}");
                    }
                } else if let Err(e) = clean_hint_dirs(hint, home, runner) {
                    eprintln!("  Error: {e}");
                }
            }
            _ => {}
        }
    }
}

/// The line printed after a hint's delete loop: `  [OK]` only when every
/// attempted deletion succeeded. Any skipped deletion (the seam returned a
/// skip reason) yields a truthful `  [SKIPPED]` line instead, so a cache
/// that was not cleared never reports OK. Fatal errors propagate as `Err`
/// before this line is reached.
fn completion_line(skip_reasons: &[String]) -> String {
    if skip_reasons.is_empty() {
        "  [OK]".to_string()
    } else {
        format!("  [SKIPPED] {}", skip_reasons.join("; "))
    }
}

/// Print an overview of reclaimable hints to stderr.
pub fn print_hints(hints: &[ProcessHint]) {
    if hints.is_empty() {
        return;
    }
    let sep = if crate::format::use_unicode() {
        "─".repeat(60)
    } else {
        "-".repeat(60)
    };
    eprintln!("{sep}");
    eprintln!(" Tip: The following caches can be freed manually:");
    eprintln!("{sep}");
    for h in hints {
        let running_tag = if h.running {
            "  [running — quit first]"
        } else {
            ""
        };
        eprintln!(
            "  {:<26} {}{}",
            h.entry.display_name,
            format_bytes(h.size_bytes),
            running_tag
        );
        for cmd in h.entry.commands {
            eprintln!("    $ {cmd}");
        }
    }
    eprintln!("{sep}");
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_line_ok_when_every_delete_succeeded() {
        assert_eq!(completion_line(&[]), "  [OK]");
    }

    #[test]
    fn completion_line_never_reports_ok_after_a_skipped_delete() {
        let line = completion_line(&["trash failed: Finder denied access".to_string()]);
        assert!(
            !line.contains("[OK]"),
            "a skipped delete must not report [OK], got: {line}"
        );
        assert!(
            line.contains("[SKIPPED]"),
            "the skip must be reported truthfully, got: {line}"
        );
    }

    #[test]
    fn completion_line_reports_every_skip_reason() {
        let line = completion_line(&["first".to_string(), "second".to_string()]);
        assert!(
            line.contains("first") && line.contains("second"),
            "got: {line}"
        );
        assert!(!line.contains("[OK]"), "got: {line}");
    }

    #[test]
    fn quit_app_script_never_closes_the_string_early() {
        // A `"` in the app name must not close the `quit app "..."` literal
        // early and turn the rest into executable AppleScript.
        let script = quit_app_script("\" & do shell script \"pwned");
        assert_eq!(script, "quit app \"\\\" & do shell script \\\"pwned\"");
    }

    #[test]
    fn quit_app_script_leaves_known_entries_untouched() {
        // Every current KNOWN_ENTRIES value is quote- and backslash-free;
        // escaping must not mangle those names.
        assert_eq!(
            quit_app_script("Visual Studio Code"),
            "quit app \"Visual Studio Code\""
        );
        assert_eq!(quit_app_script("Slack"), "quit app \"Slack\"");
    }
}
