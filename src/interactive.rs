use crate::cleaner::{Cleaner, ScanResult, ScanStatus};
use crate::format::format_bytes;
use anyhow::Result;
use std::io::{BufRead, IsTerminal, Write};

/// Build display items and their mapping to (cleaner_index, optional_sub_target).
/// Pure function — no I/O, fully testable.
pub(crate) fn build_selection_items(
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
    pruneable_indices: &[usize],
) -> (Vec<(usize, Option<&'static str>)>, Vec<String>) {
    let mut selection_mapping: Vec<(usize, Option<&'static str>)> = vec![];
    let items: Vec<String> = pruneable_indices
        .iter()
        .flat_map(|&i| {
            let bytes = match &results[i].status {
                ScanStatus::Pruneable(b) => *b,
                _ => 0,
            };
            let subs = cleaners[i].sub_targets();
            if subs.is_empty() {
                selection_mapping.push((i, None));
                vec![format!("{}  ({})", cleaners[i].name(), format_bytes(bytes))]
            } else {
                subs.iter()
                    .map(|&(sub_name, sub_size)| {
                        selection_mapping.push((i, Some(sub_name)));
                        format!(
                            "  {} > {}  ({})",
                            cleaners[i].name(),
                            sub_name,
                            format_bytes(sub_size)
                        )
                    })
                    .collect()
            }
        })
        .collect();
    (selection_mapping, items)
}

/// Compute the total freed bytes for selected items.
/// Pure function — no I/O, fully testable.
pub(crate) fn compute_selected_total(
    selected: &[usize],
    selection_mapping: &[(usize, Option<&'static str>)],
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
) -> u64 {
    selected
        .iter()
        .map(|&si| {
            let (cleaner_idx, sub_name) = &selection_mapping[si];
            if let Some(sub_name) = sub_name {
                cleaners[*cleaner_idx]
                    .sub_targets()
                    .iter()
                    .find(|(n, _)| n == sub_name)
                    .map(|(_, s)| *s)
                    .unwrap_or(0)
            } else if let ScanStatus::Pruneable(b) = &results[*cleaner_idx].status {
                *b
            } else {
                0
            }
        })
        .sum()
}

/// Indices that `--yes` may clean: Pruneable and allowed for unattended runs.
pub(crate) fn auto_clean_indices(
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
) -> Vec<usize> {
    results
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            matches!(r.status, ScanStatus::Pruneable(_)) && cleaners[*i].auto_clean_allowed()
        })
        .map(|(i, _)| i)
        .collect()
}

/// Ask a y/N question: write `prompt` verbatim to `out`, flush it, then read
/// one line from `input`. Returns true only for a bare y/Y answer — n, any
/// other word, an empty line and EOF all decline. The decline branch (print
/// "Aborted.", return early) stays at the call sites because each entry
/// point aborts differently. Reader and writer are parameters so the y/N
/// skeleton is unit-testable and the xcode mid-clean check can prompt on
/// stderr instead of stdout.
pub(crate) fn ask_yn(
    prompt: &str,
    out: &mut dyn Write,
    input: &mut dyn BufRead,
) -> std::io::Result<bool> {
    write!(out, "{prompt}")?;
    out.flush()?;
    let mut line = String::new();
    input.read_line(&mut line)?;
    Ok(line.trim().eq_ignore_ascii_case("y"))
}

/// stdin-backed `ask_yn` used by the interactive entries.
pub(crate) fn confirm(prompt: &str, out: &mut dyn Write) -> std::io::Result<bool> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    ask_yn(prompt, out, &mut input)
}

/// RAII bracket for the secondary-confirmation suppression inside cleaners:
/// sets `generic::set_skip_confirm(true)` on creation and restores `false`
/// on drop, so both interactive entry points share one bracket semantics.
/// Bind it to a named local (`let _skip_confirm = ...`); `let _ = ...`
/// would drop it at once.
pub(crate) struct SkipConfirmGuard;

impl SkipConfirmGuard {
    pub(crate) fn new() -> Self {
        crate::cleaners::generic::set_skip_confirm(true);
        Self
    }
}

impl Default for SkipConfirmGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SkipConfirmGuard {
    fn drop(&mut self) {
        crate::cleaners::generic::set_skip_confirm(false);
    }
}

/// Line shapes for the post-batch report. Both interactive entry points
/// print the same failure and skip facts; run_auto's multi-target batch
/// names the cleaner in every line, while the TUI whole-target branch runs
/// one batch call per target and omits the name (the selection context
/// already named it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ReportShape {
    /// `Error cleaning {name}: {e}` and `  - {name}: {detail}`.
    Named,
    /// `Error: {e}` and `  - {detail}`.
    Bare,
}

/// Print one batch run's per-target failures and skipped files to `out`.
/// Failure output is deliberately not a run_batch callback (PBI-33 recorded
/// decision, option b): run_batch only collects, and every caller reports
/// after the batch — one shared path wins over per-failure immediacy.
pub(crate) fn report_batch_failures(
    out: &mut dyn Write,
    summary: &crate::batch::BatchSummary,
    shape: ReportShape,
) -> std::io::Result<()> {
    for (name, e) in &summary.failures {
        match shape {
            ReportShape::Named => writeln!(out, "Error cleaning {name}: {e}")?,
            ReportShape::Bare => writeln!(out, "Error: {e}")?,
        }
    }
    if !summary.skipped.is_empty() {
        writeln!(
            out,
            "{} file(s) skipped due to errors (permission/lock):",
            summary.skipped.len()
        )?;
        for (name, detail) in &summary.skipped {
            match shape {
                ReportShape::Named => writeln!(out, "  - {name}: {detail}")?,
                ReportShape::Bare => writeln!(out, "  - {detail}")?,
            }
        }
    }
    Ok(())
}

/// Print the closing "Total freed" line after a clean run.
pub(crate) fn report_total_freed(out: &mut dyn Write, total_freed: u64) -> std::io::Result<()> {
    writeln!(out, "\nTotal freed: {}", format_bytes(total_freed))
}

/// Non-interactive: clean every pruneable cleaner without prompting.
/// Used by `--yes` flag.
pub fn run_auto(cleaners: &[Box<dyn Cleaner>]) -> Result<()> {
    let (results, _available) = crate::scanner::detect_all(cleaners);

    let pruneable_indices = auto_clean_indices(&results, cleaners);

    // Non-Safe targets (Moderate, Risky) are only reachable by an explicit
    // selection, so tell the user they were left out and why.
    let skipped: Vec<String> = results
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            matches!(r.status, ScanStatus::Pruneable(b) if b > 0)
                && !cleaners[*i].auto_clean_allowed()
        })
        .map(|(i, _)| format!("{} ({})", cleaners[i].name(), cleaners[i].risk().label()))
        .collect();
    if !skipped.is_empty() {
        println!(
            "Skipped {} target(s) that need an explicit selection: {}",
            skipped.len(),
            skipped.join(", ")
        );
    }

    if pruneable_indices.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    if !crate::trash::is_trash_mode() {
        let total_reclaimable: u64 = pruneable_indices
            .iter()
            .filter_map(|&i| {
                if let ScanStatus::Pruneable(b) = &results[i].status {
                    Some(*b)
                } else {
                    None
                }
            })
            .sum();
        println!(
            "Scan complete. Found {} item(s), ~{} will be permanently deleted.",
            pruneable_indices.len(),
            format_bytes(total_reclaimable),
        );
        if !confirm("Are you sure? [y/N] ", &mut std::io::stdout())? {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Suppress secondary confirmation prompts (TUI already asked above).
    let _skip_confirm = SkipConfirmGuard::new();

    // The one batch path: animated spinner per cleaner, accumulation, the
    // history guard and per-target error collection all live in
    // batch::run_batch. with_notes=false: this flow never printed notes, and
    // passing the default options is behavior-identical to the old bare
    // `clean(false, rep)` calls (every reachable cleaner computes the same
    // work when the options carry nothing). dry_run is always false here.
    let selected: Vec<&dyn Cleaner> = pruneable_indices
        .iter()
        .map(|&i| cleaners[i].as_ref())
        .collect();
    let summary = crate::batch::run_batch(
        &selected,
        &crate::cleaner::CleanOptions::default(),
        &crate::progress::SuppressReporter,
        false, // dry_run: interactive always deletes
        true,  // animated
        false, // with_notes
    );
    // Failure output (PBI-33 recorded decision, option b): run_batch collects
    // per-target failures and every caller prints them after the batch. The
    // old per-cleaner immediacy (`Error cleaning {name}: {e}` inside the
    // loop) is deliberately not restored: a per-failure callback would fork
    // run_batch's signature for one entry point while clean all, `auto`,
    // `caches` and the TUI whole-target branch already print collected
    // failures after the batch — one shared path wins over immediacy.
    report_batch_failures(&mut std::io::stderr(), &summary, ReportShape::Named)?;

    // Restore the secondary confirmation prompts before the total line —
    // the same effect order as the old explicit pair of calls.
    drop(_skip_confirm);

    report_total_freed(&mut std::io::stdout(), summary.total_freed)?;
    Ok(())
}

/// Interactive TUI: scan, let user select with checkboxes, then clean.
/// Exits with an error message if stdin is not a terminal.
pub fn run_interactive(cleaners: &[Box<dyn Cleaner>]) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        eprintln!("sasurahime: not a terminal. Use --yes for non-interactive mode.");
        std::process::exit(1);
    }

    let (results, _available) = crate::scanner::detect_all(cleaners);

    // Collect indices of pruneable cleaners only — nothing to select otherwise.
    let pruneable_indices: Vec<usize> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r.status, ScanStatus::Pruneable(_)))
        .map(|(i, _)| i)
        .collect();

    if pruneable_indices.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    let (selection_mapping, items) = build_selection_items(&results, cleaners, &pruneable_indices);

    // dialoguer 0.11: MultiSelect::interact() writes to and reads from the process terminal.
    let selected = dialoguer::MultiSelect::new()
        .with_prompt("Select caches to clean  [space to toggle, enter to confirm]")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("Nothing selected. Exiting.");
        return Ok(());
    }

    let total = compute_selected_total(&selected, &selection_mapping, &results, cleaners);

    println!("\nWill free approximately {}.", format_bytes(total));
    if !confirm("Proceed? [y/N] ", &mut std::io::stdout())? {
        println!("Aborted.");
        return Ok(());
    }

    // Suppress secondary confirmation prompts inside cleaners (TUI already
    // asked "Proceed?" above; re-confirming would be confusing).
    let _skip_confirm = SkipConfirmGuard::new();

    let mut freed: u64 = 0;
    for &si in &selected {
        let (cleaner_idx, sub_name) = &selection_mapping[si];
        if let Some(sub_name) = sub_name {
            let name = cleaners[*cleaner_idx].name();
            let result = crate::progress::with_spinner_result(
                &format!("Cleaning {} {}...", name, sub_name),
                || {
                    let status = std::process::Command::new(std::env::current_exe()?)
                        .args(["clean", name, "--sub", sub_name])
                        .status()?;
                    if !status.success() {
                        anyhow::bail!("`{} clean {} --sub {}` failed", name, name, sub_name);
                    }
                    Ok(crate::cleaner::CleanResult {
                        name,
                        bytes_freed: 0,
                        uses_trash: false,
                        skipped: vec![],
                        deleted_paths: vec![],
                    })
                },
            );
            match result {
                Ok(r) => freed += r.bytes_freed,
                Err(e) => eprintln!("Error cleaning {} {}: {e}", name, sub_name),
            }
        } else {
            // Whole-target selection through the one batch path: the same
            // animated spinner, history guard and error collection as every
            // other batch loop. One target per call keeps the selection order
            // interleaved with the sub-target subprocess branch above.
            // with_notes=false: this flow never printed notes.
            let batch_targets = [cleaners[*cleaner_idx].as_ref()];
            let summary = crate::batch::run_batch(
                &batch_targets,
                &crate::cleaner::CleanOptions::default(),
                &crate::progress::SuppressReporter,
                false, // dry_run: interactive always deletes
                true,  // animated
                false, // with_notes
            );
            freed += summary.total_freed;
            // This flow's error line names no cleaner; the skip report keeps
            // the single-target "path: reason" shape for the same reason.
            report_batch_failures(&mut std::io::stderr(), &summary, ReportShape::Bare)?;
        }
    }

    // Restore the secondary confirmation prompts before the total line —
    // the same effect order as the old explicit pair of calls.
    drop(_skip_confirm);

    report_total_freed(&mut std::io::stdout(), freed)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::CleanResult;
    use crate::progress::ProgressReporter;
    use anyhow::Result;

    /// A test cleaner with a fixed name, size, and sub-targets.
    struct TestCleaner {
        name: &'static str,
        size: u64,
        subs: Vec<(&'static str, u64)>,
    }

    impl TestCleaner {
        fn new(name: &'static str, size: u64, subs: Vec<(&'static str, u64)>) -> Self {
            Self { name, size, subs }
        }
    }

    impl Cleaner for TestCleaner {
        fn name(&self) -> &'static str {
            self.name
        }
        fn detect(&self) -> ScanResult {
            if self.size > 0 {
                ScanResult::new(self.name, ScanStatus::Pruneable(self.size))
            } else {
                ScanResult::new(self.name, ScanStatus::NotFound)
            }
        }
        fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
            Ok(CleanResult {
                name: self.name,
                bytes_freed: self.size,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            })
        }
        // Test cleaners represent regenerable caches so the auto-clean
        // selection logic can be exercised.
        fn risk(&self) -> crate::cleaner::Risk {
            crate::cleaner::Risk::Safe
        }
        fn sub_targets(&self) -> Vec<(&'static str, u64)> {
            self.subs.clone()
        }
    }

    #[test]
    fn build_selection_items_single_cleaner_no_sub() {
        let cleaners: Vec<Box<dyn Cleaner>> =
            vec![Box::new(TestCleaner::new("test", 1000, vec![]))];
        let results = vec![cleaners[0].detect()];
        let pruneable = vec![0];
        let (mapping, items) = build_selection_items(&results, &cleaners, &pruneable);
        assert_eq!(mapping.len(), 1);
        assert_eq!(mapping[0], (0, None));
        assert_eq!(items.len(), 1);
        assert!(items[0].contains("test"));
        assert!(items[0].contains("1000 B"));
    }

    #[test]
    fn build_selection_items_with_sub_targets() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(TestCleaner::new(
            "xcode",
            0,
            vec![("DerivedData", 500), ("Archives", 300)],
        ))];
        let results = vec![cleaners[0].detect()];
        let pruneable = vec![0];
        let (mapping, items) = build_selection_items(&results, &cleaners, &pruneable);
        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping[0], (0, Some("DerivedData")));
        assert_eq!(mapping[1], (0, Some("Archives")));
        assert_eq!(items.len(), 2);
        assert!(items[0].contains("DerivedData"));
        assert!(items[0].contains("500"));
        assert!(items[1].contains("Archives"));
        assert!(items[1].contains("300"));
    }

    #[test]
    fn build_selection_items_mixed_cleaners() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("xcode", 0, vec![("DerivedData", 500)])),
            Box::new(TestCleaner::new("brew", 2000, vec![])),
        ];
        let results = vec![cleaners[0].detect(), cleaners[1].detect()];
        // Both are not-find/clean (xcode sub-target has size but the cleaner itself is 0).
        // pruneable_indices only includes cleaners with Pruneable status.
        let pruneable = vec![1]; // only brew is Pruneable
        let (mapping, items) = build_selection_items(&results, &cleaners, &pruneable);
        assert_eq!(mapping.len(), 1);
        assert_eq!(mapping[0], (1, None));
        assert!(items[0].contains("brew"));
    }

    #[test]
    fn compute_selected_total_cleaner_without_sub() {
        let cleaners: Vec<Box<dyn Cleaner>> =
            vec![Box::new(TestCleaner::new("brew", 2000, vec![]))];
        let results = vec![cleaners[0].detect()];
        let (mapping, _items) = build_selection_items(&results, &cleaners, &[0]);
        let total = compute_selected_total(&[0], &mapping, &results, &cleaners);
        assert_eq!(total, 2000);
    }

    #[test]
    fn compute_selected_total_with_sub_category() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(TestCleaner::new(
            "xcode",
            0,
            vec![("DerivedData", 500), ("Archives", 300)],
        ))];
        let results = vec![cleaners[0].detect()];
        let (mapping, _items) = build_selection_items(&results, &cleaners, &[0]);
        // Select both sub-targets (indices 0 and 1 in the flattened items)
        let total = compute_selected_total(&[0, 1], &mapping, &results, &cleaners);
        assert_eq!(total, 800);
    }

    #[test]
    fn compute_selected_total_mixed_selection() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("xcode", 0, vec![("DerivedData", 500)])),
            Box::new(TestCleaner::new("brew", 2000, vec![])),
        ];
        let results = vec![cleaners[0].detect(), cleaners[1].detect()];
        let (mapping, _items) = build_selection_items(&results, &cleaners, &[0, 1]);
        // Select xcode > DerivedData (idx 0) and brew (idx 1)
        let total = compute_selected_total(&[0, 1], &mapping, &results, &cleaners);
        assert_eq!(total, 500 + 2000);
    }

    #[test]
    fn auto_clean_indices_skips_manual_only() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("brew", 2000, vec![])),
            Box::new(crate::cleaners::manual_only::ManualOnly::new(Box::new(
                TestCleaner::new("docker", 3000, vec![]),
            ))),
            Box::new(TestCleaner::new("empty", 0, vec![])),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert_eq!(auto_clean_indices(&results, &cleaners), vec![0]);
    }

    #[test]
    fn auto_clean_indices_excludes_unavailable() {
        struct UnavailableCleaner(&'static str);
        impl Cleaner for UnavailableCleaner {
            fn name(&self) -> &'static str {
                self.0
            }
            fn detect(&self) -> ScanResult {
                ScanResult::new(self.0, ScanStatus::Unavailable)
            }
            fn clean(
                &self,
                _dry_run: bool,
                _reporter: &dyn ProgressReporter,
            ) -> Result<CleanResult> {
                panic!("unavailable cleaner must never be cleaned by --yes")
            }
        }
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("brew", 2000, vec![])),
            Box::new(UnavailableCleaner("mise")),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert_eq!(
            auto_clean_indices(&results, &cleaners),
            vec![0],
            "unavailable targets must never be picked by --yes"
        );
    }

    #[test]
    fn run_auto_does_not_clean_manual_only_targets() {
        use crate::cleaners::manual_only::ManualOnly;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct Counting(Arc<AtomicUsize>);

        impl Cleaner for Counting {
            fn name(&self) -> &'static str {
                "counting"
            }
            fn detect(&self) -> ScanResult {
                ScanResult::new("counting", ScanStatus::Pruneable(10))
            }
            fn clean(
                &self,
                _dry_run: bool,
                _reporter: &dyn ProgressReporter,
            ) -> Result<CleanResult> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(CleanResult {
                    name: "counting",
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                })
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(ManualOnly::new(Box::new(Counting(
            Arc::clone(&calls),
        ))))];

        run_auto(&cleaners).unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    // --- PBI-49: shared confirm / skip-confirm bracket / batch report ---

    #[test]
    fn ask_yn_y_confirms_and_writes_the_prompt_verbatim() {
        let mut out: Vec<u8> = Vec::new();
        let mut input = std::io::Cursor::new(b"y\n".as_slice());
        let confirmed = ask_yn("Are you sure? [y/N] ", &mut out, &mut input).unwrap();
        assert!(confirmed);
        assert_eq!(out, b"Are you sure? [y/N] ");
    }

    #[test]
    fn ask_yn_n_declines_so_the_call_site_aborts() {
        let mut out: Vec<u8> = Vec::new();
        let mut input = std::io::Cursor::new(b"n\n".as_slice());
        let confirmed = ask_yn("Proceed? [y/N] ", &mut out, &mut input).unwrap();
        assert!(!confirmed, "n must decline: both entry points abort on it");
        assert_eq!(out, b"Proceed? [y/N] ");
    }

    #[test]
    fn ask_yn_other_answers_and_eof_decline() {
        for answer in ["\n", "no\n", "N\n", "yes\n", ""] {
            let mut out: Vec<u8> = Vec::new();
            let mut input = std::io::Cursor::new(answer.as_bytes());
            assert!(
                !ask_yn("?", &mut out, &mut input).unwrap(),
                "{answer:?} must decline"
            );
        }
    }

    #[test]
    fn report_batch_failures_named_shape_names_the_cleaner() {
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![("brew".to_string(), "boom".to_string())],
            skipped: vec![("a.log".to_string(), "locked".to_string())],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Named).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Error cleaning brew: boom\n1 file(s) skipped due to errors (permission/lock):\n  - a.log: locked\n"
        );
    }

    #[test]
    fn report_batch_failures_bare_shape_omits_the_name() {
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![("brew".to_string(), "boom".to_string())],
            skipped: vec![("a.log".to_string(), "locked".to_string())],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Bare).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Error: boom\n1 file(s) skipped due to errors (permission/lock):\n  - locked\n"
        );
    }

    #[test]
    fn report_batch_failures_silent_when_nothing_failed() {
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![],
            skipped: vec![],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Named).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn report_total_freed_prints_the_closing_line() {
        let mut out: Vec<u8> = Vec::new();
        report_total_freed(&mut out, 2048).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\nTotal freed: 2.0 KB\n");
    }
}
