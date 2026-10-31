use crate::cleaner::{CleanByName, CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::format::format_bytes;
use crate::scanner::{hints_overlap_kind, HintsOverlap};
use anyhow::Result;
use std::io::{BufRead, IsTerminal, Write};

/// Build display items and their mapping to (cleaner_index, optional_sub_target).
/// Pure function — no I/O, fully testable.
/// One selection entry: the cleaner's index plus, when the selection is a
/// sub-target, the sub's CLI spelling and its scanned bytes (the size rides
/// in the mapping so the sum re-binds by value, not by string re-match).
pub(crate) type SelectionEntry = (usize, Option<(&'static str, u64)>);

pub(crate) fn build_selection_items(
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
    pruneable_indices: &[usize],
) -> (Vec<SelectionEntry>, Vec<String>) {
    // The sub's scanned bytes ride in the mapping: the sum re-binds by the
    // carried size instead of re-matching the sub name string (PBI 59).
    let mut selection_mapping: Vec<SelectionEntry> = vec![];
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
                        selection_mapping.push((i, Some((sub_name, sub_size))));
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
/// The hints row overlaps the app-support / library-caches rows (the same
/// dirs are counted in both, [`crate::scanner::hints_overlap_kind`]), so the
/// hints selection's size is subtracted once when an overlapping sibling is
/// selected at the same time — dedup, not exclusion: a counts_toward_total
/// style name filter here would report "Will free approximately 0 B" for a
/// hints-only selection, while hints really deletes the app-support /
/// library-caches dirs (PBI 94). With no overlap this stays the plain sum.
pub(crate) fn compute_selected_total(
    selected: &[usize],
    selection_mapping: &[SelectionEntry],
    results: &[ScanResult],
) -> u64 {
    // First pass: one (kind, size) per selected row. Size is the carried
    // sub_size when the row is a sub-target, else the cleaner's Pruneable
    // bytes — the same reference the plain sum used.
    let mut total: u64 = 0;
    let mut hints_size: u64 = 0;
    let mut hints_selected = false;
    let mut sibling_selected = false;
    for &si in selected {
        let (cleaner_idx, sub) = &selection_mapping[si];
        let size = match sub {
            // The sub's size rode in the mapping: no string re-match.
            Some((_, sub_size)) => *sub_size,
            None => match &results[*cleaner_idx].status {
                ScanStatus::Pruneable(b) => *b,
                _ => 0,
            },
        };
        match hints_overlap_kind(results[*cleaner_idx].name) {
            Some(HintsOverlap::HintsRow) => {
                hints_selected = true;
                hints_size += size;
            }
            Some(HintsOverlap::OverlappingSibling) => sibling_selected = true,
            None => {}
        }
        total += size;
    }
    // Subtract the hints selection once — `hints_size` is a subset of the
    // addends above, so this cannot underflow.
    if hints_selected && sibling_selected {
        total - hints_size
    } else {
        total
    }
}

/// Names of the Pruneable Safe targets whose clean() gates on an interactive
/// prompt (`Cleaner::interactive_gated`) and therefore skips when the run is
/// not interactive. Pure function — no I/O, fully testable. `interactive`
/// carries the stdin-is-a-terminal fact so run_auto reads the same
/// `prompt::is_interactive` the cleaners' own gate reads: on a TTY the gate
/// passes and the cleaner prompts, so the notice is for non-interactive runs
/// only (PBI 60).
pub(crate) fn gate_skipped_names(
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
    interactive: bool,
) -> Vec<String> {
    if interactive {
        return vec![];
    }
    results
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            matches!(r.status, ScanStatus::Pruneable(b) if b > 0)
                && cleaners[*i].auto_clean_allowed()
                && cleaners[*i].interactive_gated()
        })
        .map(|(i, _)| cleaners[i].name().to_string())
        .collect()
}

/// The one y/N decision, shared by [`ask_yn`] and the hint prompt
/// (hint/cleaner.rs): true only for a bare y/Y answer with optional
/// surrounding whitespace — n, any other word, an empty line and EOF all
/// decline. Extracted from ask_yn's decision line so the hint flow cannot
/// drift from the entry-point prompts (PBI 80).
pub(crate) fn is_yes_answer(answer: &str) -> bool {
    answer.trim().eq_ignore_ascii_case("y")
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
    Ok(is_yes_answer(&line))
}

/// stdin-backed `ask_yn` used by the interactive entries.
pub(crate) fn confirm(prompt: &str, out: &mut dyn Write) -> std::io::Result<bool> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    ask_yn(prompt, out, &mut input)
}

/// RAII bracket for the secondary-confirmation suppression inside cleaners:
/// sets `prompt::set_confirm_suppressed(true)` on creation and restores
/// `false` on drop, so both interactive entry points share one bracket
/// semantics. Bind it to a named local (`let _skip_confirm = ...`);
/// `let _ = ...` would drop it at once.
pub(crate) struct SkipConfirmGuard;

impl SkipConfirmGuard {
    pub(crate) fn new() -> Self {
        crate::prompt::set_confirm_suppressed(true);
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
        crate::prompt::set_confirm_suppressed(false);
    }
}

/// Line shapes for the post-batch report. Every batch entry point prints
/// the same failure and skip facts; the `auto` batch (CLI arm and TUI)
/// names the cleaner in every line, while the TUI whole-target branch runs
/// one batch call per target and omits the name (the selection context
/// already named it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReportShape {
    /// `Error cleaning {name}: {e}` and `  - {name}: {detail}`.
    Named,
    /// `Error: {e}` and `  - {detail}`.
    Bare,
}

/// Print one batch run's per-target failures and skipped files to `out`.
/// Failure output is deliberately not a run_batch callback (PBI-33 recorded
/// decision, option b): run_batch only collects, and every caller reports
/// after the batch — one shared path wins over per-failure immediacy.
pub fn report_batch_failures(
    out: &mut dyn Write,
    summary: &crate::batch::BatchSummary,
    shape: ReportShape,
) -> std::io::Result<()> {
    for f in &summary.failures {
        match shape {
            ReportShape::Named => writeln!(out, "Error cleaning {}: {}", f.name, f.error)?,
            ReportShape::Bare => writeln!(out, "Error: {}", f.error)?,
        }
    }
    // Cancelled targets come after the failures and stay neutral: a decline
    // is the user's run-level choice, not an error, so the line never says
    // `Error cleaning` (PBI 78).
    for name in &summary.cancelled {
        match shape {
            ReportShape::Named => writeln!(out, "{name}: cancelled by user")?,
            ReportShape::Bare => writeln!(out, "cancelled by user")?,
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
pub fn report_total_freed(out: &mut dyn Write, total_freed: u64) -> std::io::Result<()> {
    writeln!(out, "\nTotal freed: {}", format_bytes(total_freed))
}

/// Clean one sub-target selection through the in-process clean seam
/// ([`CleanByName`], PBI-50): `clean_fn` resolves `name` + sub-target to the
/// same dispatch the CLI uses and returns the REAL CleanResult, so
/// `Total freed` reflects actual bytes — the old self re-exec
/// (`sasurahime clean <name> --sub <sub>`) fabricated `bytes_freed: 0` and
/// under-counted the total. dry_run is always false: the interactive TUI
/// always deletes (it asked "Proceed?" above). The reporter is DeepSuppress:
/// the TUI owns its output here (`with_spinner_result` draws the one line),
/// so the seam must not start a second competing spinner.
pub(crate) fn clean_sub_target(
    name: &str,
    sub_name: &str,
    clean_fn: &CleanByName,
) -> Result<CleanResult> {
    crate::progress::with_spinner_result(&format!("Cleaning {name} {sub_name}..."), || {
        clean_fn(
            name,
            Some(sub_name),
            false, // dry_run: interactive always deletes
            &crate::progress::DeepSuppressReporter,
        )
    })
}

/// Non-interactive: clean every pruneable cleaner without prompting.
/// Used by `--yes` flag. `mode` is the run's resolved deletion mode,
/// resolved once in `main` and passed down as an argument.
pub fn run_auto(cleaners: &[Box<dyn Cleaner>], mode: crate::trash::TrashMode) -> Result<()> {
    let (results, _available) = crate::scanner::detect_all(cleaners);

    // PBI 60: Safe targets whose clean() gates on an interactive prompt skip
    // on a non-TTY stdin even though `--yes` selected them (the per-cleaner
    // selection cannot run without a terminal). Name them here so an
    // unattended run never looks like a no-op success. On a TTY the gate
    // passes and the cleaner prompts, so this prints in non-interactive
    // runs only. Read before the scan results move into the policy: the TTY
    // fact is a per-run observation, not a selection input (PBI 74).
    let gate_skipped = gate_skipped_names(&results, cleaners, crate::prompt::is_interactive());

    let opts = crate::cleaner::CleanOptions {
        trash_mode: mode,
        ..Default::default()
    };
    // PBI 74: the run/skip classification is owned by
    // UnattendedPolicy::Yes — the scan results ride in the policy, so this
    // scan-driven flow cannot grow the registry-driven flows' filters. The
    // wording below stays at the call site: it is specific to `--yes`.
    let selection = crate::unattended::UnattendedPolicy::Yes { results }.select(cleaners, &opts);

    // Non-Safe targets (Moderate, Risky) are only reachable by an explicit
    // selection, so tell the user they were left out and why.
    let skipped: Vec<String> = selection
        .skipped
        .iter()
        .map(|(i, reason)| format!("{} ({})", cleaners[*i].name(), reason.risk().label()))
        .collect();
    if !skipped.is_empty() {
        println!(
            "Skipped {} target(s) that need an explicit selection: {}",
            skipped.len(),
            skipped.join(", ")
        );
    }

    if !gate_skipped.is_empty() {
        println!(
            "Skipped {} target(s) that need an interactive terminal: {}",
            gate_skipped.len(),
            gate_skipped.join(", ")
        );
    }

    if selection.run.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    if mode == crate::trash::TrashMode::Permanent {
        // The scanned pruneable bytes ride in the selection (PBI 74), so
        // the confirmation reports the same sum the old scan-index path did.
        let total_reclaimable: u64 = selection.run.iter().map(|&(_, bytes)| bytes).sum();
        println!(
            "Scan complete. Found {} item(s), ~{} will be permanently deleted.",
            selection.run.len(),
            format_bytes(total_reclaimable),
        );
        if !confirm("Are you sure? [y/N] ", &mut std::io::stdout())? {
            println!("Aborted.");
            // PBI 69: the abort must not look like success to a script. An
            // unattended `--yes --permanent` run that reaches EOF (or answers
            // n) at this confirmation now exits non-zero, so a caller can
            // tell an aborted run from a completed one. The TUI's own decline
            // in run_interactive stays Ok: an interactive user chose to
            // cancel, which is not a script failure.
            anyhow::bail!("aborted before permanent deletion");
        }
    }

    // Suppress secondary confirmation prompts (TUI already asked above).
    let _skip_confirm = SkipConfirmGuard::new();

    // The one batch path: animated spinner per cleaner, accumulation, the
    // history guard and per-target error collection all live in
    // batch::run_batch. with_notes=false: this flow never printed notes, and
    // passing the run's options is behavior-identical to the old bare
    // `clean(false, rep)` calls (every reachable cleaner computes the same
    // work when the options carry nothing). dry_run is always false here.
    let selected: Vec<&dyn Cleaner> = selection
        .run
        .iter()
        .map(|&(i, _)| cleaners[i].as_ref())
        .collect();
    let summary = crate::batch::run_batch(
        &selected,
        &opts,
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
/// Exits with an error message if stdin is not a terminal. `clean_fn` is the
/// in-process clean seam ([`CleanByName`], PBI-50) wired by the binary:
/// sub-target selections dispatch through it instead of re-exec'ing this
/// binary, and its REAL CleanResult is what `Total freed` reports.
pub fn run_interactive(
    cleaners: &[Box<dyn Cleaner>],
    clean_fn: &CleanByName,
    mode: crate::trash::TrashMode,
) -> Result<()> {
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

    // MultiSelect::interact() writes to and reads from the process terminal.
    let selected = dialoguer::MultiSelect::new()
        .with_prompt("Select caches to clean  [space to toggle, enter to confirm]")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("Nothing selected. Exiting.");
        return Ok(());
    }

    let total = compute_selected_total(&selected, &selection_mapping, &results);

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
        let (cleaner_idx, sub) = &selection_mapping[si];
        if let Some((sub_name, _)) = sub {
            let name = cleaners[*cleaner_idx].name();
            // PBI-50: the sub-target clean runs in-process through the seam
            // (wired by the binary) instead of re-exec'ing this binary with
            // `clean <name> --sub <sub>`. The seam returns the REAL
            // CleanResult, so Total freed reflects actual bytes — the old
            // self re-exec fabricated `bytes_freed: 0` and under-counted.
            match clean_sub_target(name, sub_name, clean_fn) {
                Ok(r) => freed += r.bytes_freed,
                Err(e) => eprintln!("Error cleaning {name} {sub_name}: {e}"),
            }
        } else {
            // Whole-target selection through the one batch path: the same
            // animated spinner, history guard and error collection as every
            // other batch loop. One target per call keeps the selection order
            // interleaved with the sub-target seam branch above.
            // with_notes=false: this flow never printed notes.
            let batch_targets = [cleaners[*cleaner_idx].as_ref()];
            let opts = crate::cleaner::CleanOptions {
                trash_mode: mode,
                ..Default::default()
            };
            let summary = crate::batch::run_batch(
                &batch_targets,
                &opts,
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
        fn clean(
            &self,
            _dry_run: bool,
            _reporter: &dyn ProgressReporter,
            _opts: &crate::cleaner::CleanOptions,
        ) -> Result<CleanResult> {
            Ok(CleanResult {
                name: self.name,
                bytes_freed: self.size,
                uses_trash: false,
                skipped: vec![],
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
        // The sub's size rides in the mapping (PBI 59): the sum re-binds by
        // the carried size, no string re-match.
        assert_eq!(mapping[0], (0, Some(("DerivedData", 500))));
        assert_eq!(mapping[1], (0, Some(("Archives", 300))));
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
        let total = compute_selected_total(&[0], &mapping, &results);
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
        let total = compute_selected_total(&[0, 1], &mapping, &results);
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
        let total = compute_selected_total(&[0, 1], &mapping, &results);
        assert_eq!(total, 500 + 2000);
    }

    // --- PBI 98: the TUI estimate nets the hints/sibling overlap out ---

    #[test]
    fn compute_selected_total_subtracts_hints_overlap_when_sibling_selected() {
        // The hints row overlaps the app-support / library-caches rows (the
        // same dirs are counted in both, scanner::hints_overlap_kind): when
        // both sides are selected the hints selection's size must not be
        // summed a second time — the confirm-line estimate would otherwise
        // overshoot the real free. The plain sum reports 1550 here.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new(
                "hints",
                550,
                vec![("slack", 250), ("xcode", 300)],
            )),
            Box::new(TestCleaner::new("app-support", 1000, vec![])),
            Box::new(TestCleaner::new("library-caches", 400, vec![])),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        let (mapping, _items) = build_selection_items(&results, &cleaners, &[0, 1, 2]);

        // Select both hint rows (idx 0, 1 = 250 + 300) and app-support: the
        // hints side is already counted in the app-support row, so only the
        // sibling's 1000 remain.
        let total = compute_selected_total(&[0, 1, 2], &mapping, &results);
        assert_eq!(
            total, 1000,
            "the hints selection must not double count against a selected sibling"
        );

        // + library-caches: still one subtraction, not one per sibling row.
        let total = compute_selected_total(&[0, 1, 2, 3], &mapping, &results);
        assert_eq!(
            total, 1400,
            "the overlap must be subtracted exactly once across 3 selections"
        );
    }

    #[test]
    fn compute_selected_total_keeps_hints_when_selected_alone() {
        // Dedup, not exclusion: hints really deletes the app-support /
        // library-caches dirs (PBI 94), so a hints-only selection must keep
        // its size. A counts_toward_total-style name filter here would
        // report "Will free approximately 0 B" and go red on this pin.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(TestCleaner::new(
            "hints",
            550,
            vec![("slack", 250), ("xcode", 300)],
        ))];
        let results = vec![cleaners[0].detect()];
        let (mapping, _items) = build_selection_items(&results, &cleaners, &[0]);
        // Select both hint rows (idx 0, 1 = 250 + 300).
        let total = compute_selected_total(&[0, 1], &mapping, &results);
        assert_eq!(
            total, 550,
            "a hints-only selection must keep its size, not report 0 B"
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
                _opts: &crate::cleaner::CleanOptions,
            ) -> Result<CleanResult> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(CleanResult {
                    name: "counting",
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                })
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(ManualOnly::new(Box::new(Counting(
            Arc::clone(&calls),
        ))))];

        run_auto(&cleaners, crate::trash::TrashMode::Trash).unwrap();

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

    // --- PBI-80: the one y/N decision, shared with the hint prompt ---

    #[test]
    fn is_yes_answer_judges_the_y_n_table() {
        // The unit table: a bare y/Y with optional surrounding whitespace is
        // a yes; n, any other word, an empty line and whitespace-only
        // decline (EOF declines at the call sites).
        for yes in ["y", "Y", " y ", "\ty\t", "y\n"] {
            assert!(is_yes_answer(yes), "{yes:?} must be a yes");
        }
        for no in ["n", "N", "", " ", "\n", "yes", "no", "Y y"] {
            assert!(!is_yes_answer(no), "{no:?} must be a no");
        }
    }

    #[test]
    fn ask_yn_decides_through_the_shared_is_yes_answer() {
        // The sharing pin: ask_yn's decision line is is_yes_answer, so the
        // entry-point prompts and the hint prompt cannot drift apart.
        let src = include_str!("interactive.rs");
        let start = src
            .find("pub(crate) fn ask_yn(")
            .expect("ask_yn source present");
        let end = src[start..]
            .find("pub(crate) fn confirm(")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            body.contains("is_yes_answer(&line)"),
            "ask_yn must decide through the shared is_yes_answer"
        );
    }

    #[test]
    fn report_batch_failures_named_shape_names_the_cleaner() {
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![crate::batch::BatchFailure {
                name: "brew".to_string(),
                error: anyhow::anyhow!("boom"),
            }],
            cancelled: vec![],
            skipped: vec![("a.log".to_string(), "locked".to_string())],
            results: vec![],
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
            failures: vec![crate::batch::BatchFailure {
                name: "brew".to_string(),
                error: anyhow::anyhow!("boom"),
            }],
            cancelled: vec![],
            skipped: vec![("a.log".to_string(), "locked".to_string())],
            results: vec![],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Bare).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Error: boom\n1 file(s) skipped due to errors (permission/lock):\n  - locked\n"
        );
    }

    #[test]
    fn report_batch_failures_reports_cancelled_after_failures() {
        // A cancel is a run-level decision, not an error: the cancelled
        // line is neutral (`{name}: cancelled by user`), comes after the
        // failures, and never says `Error cleaning` (PBI 78).
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![crate::batch::BatchFailure {
                name: "brew".to_string(),
                error: anyhow::anyhow!("boom"),
            }],
            cancelled: vec!["colima".to_string()],
            skipped: vec![],
            results: vec![],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Named).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Error cleaning brew: boom\ncolima: cancelled by user\n"
        );
    }

    #[test]
    fn report_batch_failures_cancelled_bare_shape_omits_the_name() {
        // Bare shape (the TUI whole-target branch) names no cleaner: the
        // selection context already did, so the cancelled line stays bare.
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![],
            cancelled: vec!["colima".to_string()],
            skipped: vec![],
            results: vec![],
        };
        let mut out: Vec<u8> = Vec::new();
        report_batch_failures(&mut out, &summary, ReportShape::Bare).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "cancelled by user\n");
    }

    #[test]
    fn report_batch_failures_silent_when_nothing_failed() {
        let summary = crate::batch::BatchSummary {
            total_freed: 0,
            failures: vec![],
            cancelled: vec![],
            skipped: vec![],
            results: vec![],
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

    // --- PBI-50: the in-process clean seam ---

    #[test]
    fn clean_sub_target_calls_the_seam_and_propagates_the_real_result() {
        // The sub-target clean must go through the seam callback and return
        // its REAL CleanResult. The old self re-exec fabricated
        // `bytes_freed: 0` for sub-target cleans, so the TUI's `Total freed`
        // under-counted every sub-target selection.
        let calls: std::sync::Mutex<Vec<(String, Option<String>, bool)>> =
            std::sync::Mutex::new(Vec::new());
        let clean_fn = |name: &str, sub: Option<&str>, dry: bool, _rep: &dyn ProgressReporter| {
            calls
                .lock()
                .unwrap()
                .push((name.to_string(), sub.map(str::to_string), dry));
            Ok(CleanResult::freed("xcode", 4096, false))
        };
        let result = clean_sub_target("xcode", "DerivedData", &clean_fn).unwrap();
        assert_eq!(result.name, "xcode");
        assert_eq!(
            result.bytes_freed, 4096,
            "the REAL bytes freed must flow through the seam"
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![("xcode".to_string(), Some("DerivedData".to_string()), false)],
            "the seam must be called once with (name, Some(sub), dry_run=false)"
        );
    }

    #[test]
    fn clean_sub_target_propagates_a_seam_error() {
        let clean_fn =
            |_name: &str, _sub: Option<&str>, _dry: bool, _rep: &dyn ProgressReporter| {
                Err(anyhow::anyhow!("unknown clean target: nope"))
            };
        let err = clean_sub_target("nope", "DerivedData", &clean_fn).unwrap_err();
        assert!(
            err.to_string().contains("unknown clean target"),
            "the seam error must reach the caller, got: {err}"
        );
    }

    // --- PBI 60: run_auto's gate-skipped notice ---

    /// A cleaner that gates on an interactive prompt (the ollama shape):
    /// configurable size and risk, `interactive_gated()` always true.
    struct GatedCleaner {
        name: &'static str,
        size: u64,
        risk: crate::cleaner::Risk,
    }

    impl GatedCleaner {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                size: 500,
                risk: crate::cleaner::Risk::Safe,
            }
        }
        fn with_size(mut self, size: u64) -> Self {
            self.size = size;
            self
        }
        fn with_risk(mut self, risk: crate::cleaner::Risk) -> Self {
            self.risk = risk;
            self
        }
    }

    impl Cleaner for GatedCleaner {
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
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &crate::cleaner::CleanOptions,
        ) -> Result<CleanResult> {
            // The interactive gate skips before any deletion on a non-TTY
            // stdin, so a gated clean can only ever return nothing.
            Ok(CleanResult::empty(self.name))
        }
        fn risk(&self) -> crate::cleaner::Risk {
            self.risk
        }
        fn interactive_gated(&self) -> bool {
            true
        }
    }

    #[test]
    fn gate_skipped_names_includes_gated_safe_targets_when_not_interactive() {
        // A Pruneable Safe cleaner that gates on interactive is selected by
        // `--yes` but skips on a non-TTY stdin: it must be named so the
        // unattended run never looks like a no-op success (PBI 60).
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("brew", 2000, vec![])),
            Box::new(GatedCleaner::new("ollama")),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert_eq!(
            gate_skipped_names(&results, &cleaners, false),
            vec!["ollama"],
            "the gate-skipped Safe target must be named, ungated ones must not"
        );
    }

    #[test]
    fn gate_skipped_names_is_empty_when_interactive() {
        // On a TTY the gate passes and the cleaner prompts, so the notice
        // must not fire (PBI 60's TTY scenario).
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(GatedCleaner::new("ollama"))];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert!(
            gate_skipped_names(&results, &cleaners, true).is_empty(),
            "an interactive run must not warn about gate skips"
        );
    }

    #[test]
    fn gate_skipped_names_excludes_nothing_to_skip_and_non_safe() {
        // A gated cleaner with nothing to reclaim skips nothing visible, and
        // a non-Safe gated target is already covered by the
        // explicit-selection notice: neither belongs in this list.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(GatedCleaner::new("hollow").with_size(0)),
            Box::new(GatedCleaner::new("risky").with_risk(crate::cleaner::Risk::Risky)),
            Box::new(GatedCleaner::new("ollama")),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert_eq!(
            gate_skipped_names(&results, &cleaners, false),
            vec!["ollama"],
            "only the Pruneable Safe gated targets are named"
        );
    }
}
