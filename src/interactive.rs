use crate::cleaner::{Cleaner, ScanResult, ScanStatus};
use crate::format::format_bytes;
use anyhow::Result;
use rayon::prelude::*;
use std::io::IsTerminal;

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

/// Non-interactive: clean every pruneable cleaner without prompting.
/// Used by `--yes` flag.
pub fn run_auto(cleaners: &[Box<dyn Cleaner>]) -> Result<()> {
    let total = cleaners.len();
    let mut results: Vec<ScanResult> = (0..total)
        .map(|i| ScanResult::new(cleaners[i].name(), ScanStatus::NotFound))
        .collect();

    let scan_indices: Vec<usize> = (0..total).filter(|&i| cleaners[i].is_available()).collect();

    if !scan_indices.is_empty() {
        let scanned: Vec<(usize, ScanResult)> =
            crate::progress::with_parallel_scan(scan_indices.len(), |pb| {
                scan_indices
                    .par_iter()
                    .map(|&i| {
                        let r = cleaners[i].detect();
                        pb.inc(1);
                        (i, r)
                    })
                    .collect()
            });
        for (i, r) in scanned {
            results[i] = r;
        }
    }

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
        print!("Are you sure? [y/N] ");
        {
            use std::io::Write;
            std::io::stdout().flush()?;
        }
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Suppress secondary confirmation prompts (TUI already asked above).
    crate::cleaners::generic::set_skip_confirm(true);

    let reporter = crate::progress::VerboseProgress::new();
    let mut total_freed: u64 = 0;
    for i in pruneable_indices {
        let name = cleaners[i].name();
        let result = crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
            cleaners[i].clean(false, &reporter)
        });
        match result {
            Ok(r) => {
                total_freed += r.bytes_freed;
                if r.bytes_freed > 0 {
                    crate::history::write_history_entry(name, r.bytes_freed, r.skipped.len());
                }
            }
            Err(e) => eprintln!("Error cleaning {name}: {e}"),
        }
    }

    crate::cleaners::generic::set_skip_confirm(false);

    println!("\nTotal freed: {}", format_bytes(total_freed));
    Ok(())
}

/// Interactive TUI: scan, let user select with checkboxes, then clean.
/// Exits with an error message if stdin is not a terminal.
pub fn run_interactive(cleaners: &[Box<dyn Cleaner>]) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        eprintln!("sasurahime: not a terminal. Use --yes for non-interactive mode.");
        std::process::exit(1);
    }

    let total = cleaners.len();
    let mut results: Vec<ScanResult> = (0..total)
        .map(|i| ScanResult::new(cleaners[i].name(), ScanStatus::NotFound))
        .collect();

    let scan_indices: Vec<usize> = (0..total).filter(|&i| cleaners[i].is_available()).collect();

    if !scan_indices.is_empty() {
        let scanned: Vec<(usize, ScanResult)> =
            crate::progress::with_parallel_scan(scan_indices.len(), |pb| {
                scan_indices
                    .par_iter()
                    .map(|&i| {
                        let r = cleaners[i].detect();
                        pb.inc(1);
                        (i, r)
                    })
                    .collect()
            });
        for (i, r) in scanned {
            results[i] = r;
        }
    }

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
    print!("Proceed? [y/N] ");
    {
        use std::io::Write;
        std::io::stdout().flush()?;
    }
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Aborted.");
        return Ok(());
    }

    // Suppress secondary confirmation prompts inside cleaners (TUI already
    // asked "Proceed?" above; re-confirming would be confusing).
    crate::cleaners::generic::set_skip_confirm(true);

    let reporter = crate::progress::VerboseProgress::new();
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
                    })
                },
            );
            match result {
                Ok(r) => freed += r.bytes_freed,
                Err(e) => eprintln!("Error cleaning {} {}: {e}", name, sub_name),
            }
        } else {
            let name = cleaners[*cleaner_idx].name();
            let result =
                crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
                    cleaners[*cleaner_idx].clean(false, &reporter)
                });
            match result {
                Ok(r) => {
                    freed += r.bytes_freed;
                    if r.bytes_freed > 0 {
                        crate::history::write_history_entry(name, r.bytes_freed, r.skipped.len());
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
    }

    crate::cleaners::generic::set_skip_confirm(false);

    println!("\nTotal freed: {}", format_bytes(freed));
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
            })
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
}
