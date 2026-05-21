use crate::cleaner::{Cleaner, ScanStatus};
use crate::format::format_bytes;
use anyhow::Result;
use std::io::IsTerminal;

/// Non-interactive: clean every pruneable cleaner without prompting.
/// Used by `--yes` flag.
pub fn run_auto(cleaners: &[Box<dyn Cleaner>]) -> Result<()> {
    let results: Vec<_> = cleaners
        .iter()
        .map(|c| {
            let name = c.name();
            crate::progress::with_spinner(&format!("Scanning {name}..."), || c.detect())
        })
        .collect();

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

    let reporter = crate::progress::VerboseProgress::new();
    let mut total_freed: u64 = 0;
    for i in pruneable_indices {
        let name = cleaners[i].name();
        let result = crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
            cleaners[i].clean(false, &reporter)
        });
        match result {
            Ok(r) => total_freed += r.bytes_freed,
            Err(e) => eprintln!("Error cleaning {name}: {e}"),
        }
    }

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

    let results: Vec<_> = cleaners
        .iter()
        .map(|c| {
            let name = c.name();
            crate::progress::with_spinner(&format!("Scanning {name}..."), || c.detect())
        })
        .collect();

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

    let items: Vec<String> = pruneable_indices
        .iter()
        .map(|&i| {
            let r = &results[i];
            let size_str = match &r.status {
                ScanStatus::Pruneable(b) => format_bytes(*b),
                _ => "-".to_string(),
            };
            format!("{:<20} {}", r.name, size_str)
        })
        .collect();

    // dialoguer 0.11: MultiSelect::interact() writes to and reads from the process terminal.
    let selected = dialoguer::MultiSelect::new()
        .with_prompt("Select caches to clean  [space to toggle, enter to confirm]")
        .items(&items)
        .interact()?;

    if selected.is_empty() {
        println!("Nothing selected. Exiting.");
        return Ok(());
    }

    let total: u64 = selected
        .iter()
        .filter_map(|&si| {
            if let ScanStatus::Pruneable(b) = &results[pruneable_indices[si]].status {
                Some(*b)
            } else {
                None
            }
        })
        .sum();

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

    let reporter = crate::progress::VerboseProgress::new();
    let mut freed: u64 = 0;
    for &si in &selected {
        let cleaner_idx = pruneable_indices[si];
        let name = cleaners[cleaner_idx].name();
        let result = crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
            cleaners[cleaner_idx].clean(false, &reporter)
        });
        match result {
            Ok(r) => freed += r.bytes_freed,
            Err(e) => eprintln!("Error: {e}"),
        }
    }

    println!("\nTotal freed: {}", format_bytes(freed));
    Ok(())
}
