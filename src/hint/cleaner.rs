use crate::cleaner::{CleanOptions, CleanResult, Cleaner, Risk, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::format_bytes;
use crate::interactive::is_yes_answer;
use crate::progress::ProgressReporter;
use std::path::{Path, PathBuf};

use super::detector::collect_hints;
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
    let output = runner.run("osascript", &["-e", &quit_app_script(quit_app)])?;
    if !output.status.success() {
        // PBI 53: a failed osascript is the real reason the app will not
        // quit; bail with it instead of waiting for the 10s poll to time out.
        anyhow::bail!(
            "osascript failed to quit {} (exit {})",
            entry.display_name,
            output.status.code().unwrap_or(-1)
        );
    }

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
        let output = runner.run("open", &["-a", app])?;
        // PBI 53: a failed relaunch must not print "[OK]".
        if output.status.success() {
            eprintln!("  [OK]");
        } else {
            log::warn!(
                "failed to relaunch {} (exit {})",
                app,
                output.status.code().unwrap_or(-1)
            );
        }
    } else {
        eprintln!(
            "  ({} will not be restarted — relaunch manually if needed)",
            entry.display_name
        );
    }
    Ok(())
}

/// Quit the app (only when the hint is running and configures a quit_app),
/// delete the hint's cache directories, optionally relaunch the running app.
/// Returns the cleaned result (so the caller can ride the shared history
/// guard) or Err if quit timed out. `mode` is the run's resolved deletion
/// mode, passed down from `main` through `offer_auto_clean`.
///
/// This is the ONE hint deletion loop: the quit/relaunch steps are gated on
/// the hint's running flag, so a non-running hint is cleaned directly with
/// neither step (the former clean_hint_dirs twin is gone — one loop, no
/// drift). The per-entry deletions ride the same delete ledger every
/// cleaner uses (PBI 70 contract): a fatal error is a prose skip and the
/// loop continues, skipped entries are recorded, freed only sums successes,
/// and the seam's trash truth is OR-in. A dry run never reaches the ledger
/// — the offer returns before the loop.
pub fn auto_clean_hint(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
    mode: crate::trash::TrashMode,
) -> anyhow::Result<crate::cleaner::CleanResult> {
    let entry = hint.entry;
    // The quit branch fires only for a running hint that configures
    // quit_app: an entry without quit_app cannot be auto-quit (PBI 53 pins
    // the bail inside quit_app_and_wait), and the offer flow never selects
    // a running hint without one.
    if hint.running && entry.quit_app.is_some() {
        quit_app_and_wait(entry, runner)?;
    }

    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    // Only entries that exist join the ledger: the seam is not asked to
    // delete a path that is not there (the pre-ledger loop filtered on
    // exists()), and each item carries the size `format::dir_size` reports
    // for it — the same metric `collect_hints` measured, so the freed sum
    // equals what the tip displayed instead of repeating the aggregate per
    // suffix on multi-suffix entries.
    let items: Vec<(std::path::PathBuf, u64)> = entry
        .path_suffixes
        .iter()
        .map(|suffix| base.join(suffix))
        .filter(|path| path.exists())
        .map(|path| {
            let size = crate::format::dir_size(&path);
            (path, size)
        })
        .collect();
    let result = crate::trash::delete_entries(
        "hints",
        runner,
        &crate::progress::DeepSuppressReporter,
        mode,
        &items,
    );
    let skip_reasons: Vec<String> = result
        .skipped
        .iter()
        .map(|s| s.reason.to_string())
        .collect();
    eprintln!("{}", completion_line(&skip_reasons));

    // Only a running app is relaunched: the non-running path never had a
    // quit or relaunch step.
    if hint.running {
        relaunch_app(entry, runner)?;
    }
    Ok(result)
}

/// True when `name` is a KNOWN_ENTRIES display name a `--sub` may select.
/// Skip entries are excluded: they are never cleaned (detector.rs), so
/// naming one is an unknown sub-target. One owner for the sub-target
/// validation shared by the `clean hints` CLI arm and the seam.
pub fn is_known_hint_name(name: &str) -> bool {
    super::apps::KNOWN_ENTRIES
        .iter()
        .any(|e| !e.skip && e.display_name == name)
}

/// The KNOWN_ENTRIES hints cleaner (PBI-83): the whole hint table as ONE
/// registry entry, so hint cleaning rides the same Cleaner-trait flow as
/// every other target. `detect()` is the `collect_hints` aggregate, `clean()`
/// is the quit → ledger → relaunch integration (PBI 80's `auto_clean_hint`)
/// per selected hint, and `sub_targets()` lists the per-app display names
/// the TUI and `--sub` both name. One cleaner, one name ("hints"): the
/// registry pin stays intact, and the per-app quit/relaunch metadata never
/// leaves the compile-time KNOWN_ENTRIES table the `|home, config|` factory
/// signature cannot carry.
pub struct KnownAppHintsCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    /// Sub-target filter: display names from `sub_targets()`. `None` cleans
    /// every hint above its threshold; `Some` cleans only the named entries.
    subs: Option<Vec<String>>,
}

impl KnownAppHintsCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            subs: None,
        }
    }

    /// Scopes the clean to the named entries (`clean hints --sub` and the
    /// TUI's sub-target selection). Names are validated by the caller
    /// (`is_known_hint_name`) before they reach here: an unknown name is an
    /// error at the arm/seam, never a silent no-op clean (PBI 63 shape).
    pub fn with_sub_names(mut self, subs: Vec<String>) -> Self {
        self.subs = Some(subs);
        self
    }

    /// The hints this cleaner would act on: the `collect_hints` aggregate
    /// (above-threshold, skip entries excluded, largest first, top 5).
    fn collect(&self) -> Vec<ProcessHint> {
        collect_hints(&self.home, &*self.runner)
    }
}

impl Cleaner for KnownAppHintsCleaner {
    fn name(&self) -> &'static str {
        "hints"
    }

    /// Moderate (never Safe): hint cleaning quits running apps and removes
    /// their caches. It therefore never runs unattended — `--yes` excludes it
    /// as a Moderate scan-driven leftover, and `auto` / `clean all` exclude
    /// it even when `[policy] include_manual` or `--include-manual` is set
    /// (PBI 87, via [`Cleaner::quits_running_apps`]). Only an explicit
    /// selection runs it (`clean hints`, `--sub`, or the TUI); the
    /// interactive offer keeps its `Quit ... and clear cache? [y/N]` prompt.
    fn risk(&self) -> Risk {
        Risk::Moderate
    }

    /// PBI 87: clean() sends osascript `quit app "..."` to running apps with
    /// a configured `quit_app` and relaunches them with `open -a` — it
    /// interferes with the user's live desktop whether or not the run has a
    /// TTY. Reporting true keeps this cleaner out of the registry-driven
    /// unattended runs (`auto`, `clean all`) even with their opt-ins;
    /// `--yes` is scan-driven and already excludes it as Moderate.
    fn quits_running_apps(&self) -> bool {
        true
    }

    /// No prompt inside the cleaner: the prompt is the entry point's job
    /// (the scan tip's offer). `clean hints` is itself an explicit selection.
    fn interactive_gated(&self) -> bool {
        false
    }

    fn detect(&self) -> ScanResult {
        let hints = self.collect();
        let total: u64 = hints.iter().map(|h| h.size_bytes).sum();
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::NotFound
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(
                "Library/Caches, Library/Application Support, Library/Logs (known app hints)",
            );
        }
        r
    }

    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        // The xcode precedent: display name + scanned bytes, the list the
        // TUI selection and `clean hints --sub` both name.
        self.collect()
            .into_iter()
            .map(|h| (h.entry.display_name, h.size_bytes))
            .collect()
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> anyhow::Result<CleanResult> {
        let hints = self.collect();
        let selected: Vec<&ProcessHint> = match &self.subs {
            Some(subs) => hints
                .iter()
                .filter(|h| subs.iter().any(|s| *s == h.entry.display_name))
                .collect(),
            None => hints.iter().collect(),
        };

        let mut summary = CleanResult::empty(self.name());

        // The top safety contract inside the cleaner too: a dry run never
        // reaches the quit/delete/relaunch seam — it lists what would be
        // cleaned and returns a zero result.
        if dry_run {
            for hint in &selected {
                let size_str = format_bytes(hint.size_bytes);
                if hint.running {
                    eprintln!(
                        "[dry-run] would quit and clear {} ({size_str})",
                        hint.entry.display_name
                    );
                } else {
                    eprintln!(
                        "[dry-run] would clear {} ({size_str})",
                        hint.entry.display_name
                    );
                }
            }
            return Ok(summary);
        }

        for hint in selected {
            match auto_clean_hint(hint, &self.home, &*self.runner, opts.trash_mode) {
                Ok(result) => {
                    summary.bytes_freed += result.bytes_freed;
                    summary.uses_trash |= result.uses_trash;
                    summary.skipped.extend(result.skipped);
                }
                // A quit timeout is a prose error, not a ledger skip: the
                // loop continues (the offer's former warn-and-continue
                // policy), frees nothing for this hint, and never marks it
                // skipped — the all-skipped exit-code contract only counts
                // ledger skips.
                Err(e) => eprintln!("  Error: {e}"),
            }
        }
        Ok(summary)
    }
}

/// The scan-tip offer, reduced to selection collection (PBI-83): ask the
/// user per actionable hint and return the accepted entries. The execution
/// is the caller's job — it rides the CleanByName seam through the
/// [`KnownAppHintsCleaner`], so the deletion, dry_run and the history guard
/// follow the same Cleaner-trait flow contract as every other cleaner.
///
/// A dry run is the top safety contract: it never prompts (a `y` sitting in
/// stdin is never read) and collects nothing — it prints one dry-run line
/// per hint and returns an empty selection (PBI 80).
///
/// For running apps with a configured `quit_app`, and for non-running apps
/// alike, the accepted entries come back as the selection; the quit →
/// delete → relaunch steps live in the cleaner's clean() (one loop, no
/// drift).
pub fn offer_auto_clean(
    hints: &[ProcessHint],
    dry_run: bool,
    prompt: &dyn PromptReader,
) -> Vec<&'static HintEntry> {
    let mut accepted: Vec<&'static HintEntry> = Vec::new();

    let actionable: Vec<_> = hints
        .iter()
        .filter(|h| !h.running || h.entry.quit_app.is_some())
        .collect();

    if actionable.is_empty() {
        return accepted;
    }

    if dry_run {
        for hint in actionable {
            let size_str = format_bytes(hint.size_bytes);
            if hint.running {
                eprintln!(
                    "[dry-run] would quit and clear {} ({size_str})",
                    hint.entry.display_name
                );
            } else {
                eprintln!(
                    "[dry-run] would clear {} ({size_str})",
                    hint.entry.display_name
                );
            }
        }
        return accepted;
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
        if let Some(input) = prompt.read_line() {
            if is_yes_answer(&input) {
                accepted.push(hint.entry);
            }
        }
    }
    accepted
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
