use crate::command::CommandRunner;
use crate::format::format_bytes;
use std::path::Path;

use super::{base_path, PromptReader, ProcessHint};

const QUIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
const QUIT_TIMEOUT_SECS: usize = 10;

/// Quit the app, delete caches, optionally relaunch. Returns Err if quit timed out.
pub fn auto_clean_hint(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    let quit_app = match entry.quit_app {
        Some(a) => a,
        None => anyhow::bail!("{} cannot be auto-quit", entry.display_name),
    };

    // Ask the app to quit via osascript.
    eprintln!("  Quitting {}...", entry.display_name);
    runner.run("osascript", &["-e", &format!("quit app \"{quit_app}\"")])?;

    // Poll until the process is gone.
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
    // Final check.
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

    // Delete all path_suffixes.
    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            let path_str = path.to_string_lossy();
            runner.run("rm", &["-rf", &path_str])?;
        }
    }
    eprintln!("  [OK]");

    // Optionally relaunch.
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

/// Delete cache directories for a hint without quitting or relaunching the app.
fn clean_hint_dirs(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            runner.run("rm", &["-rf", &path.to_string_lossy()])?;
        }
    }
    eprintln!("  [OK]");
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

/// Print an overview of reclaimable hints to stderr.
pub fn print_hints(hints: &[ProcessHint]) {
    if hints.is_empty() {
        return;
    }
    let sep = if crate::history::USE_UNICODE.load(std::sync::atomic::Ordering::Relaxed) {
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
