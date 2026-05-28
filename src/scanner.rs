use crate::cleaner::{Cleaner, ScanStatus};
use crate::format::format_bytes;
use comfy_table::presets::{ASCII_FULL, UTF8_FULL};
use comfy_table::Table;
use rayon::prelude::*;

pub fn run_scan(cleaners: &[Box<dyn Cleaner>]) {
    // Pre-filter: skip cleaners whose binary is not installed
    let available: Vec<&Box<dyn Cleaner>> = cleaners.iter().filter(|c| c.is_available()).collect();
    let total_available = available.len();

    let results: Vec<_> = crate::progress::with_parallel_scan(total_available, |pb| {
        available
            .par_iter()
            .map(|c| {
                let r = c.detect();
                pb.inc(1);
                r
            })
            .collect()
    });

    let mut table = Table::new();
    if crate::history::USE_UNICODE.load(std::sync::atomic::Ordering::Relaxed) {
        table.load_preset(UTF8_FULL);
    } else {
        table.load_preset(ASCII_FULL);
    }
    table.set_header(vec!["Category", "Size", "Status", "Target"]);

    let mut total: u64 = 0;

    for r in &results {
        let (size, status) = match &r.status {
            ScanStatus::Pruneable(b) => {
                total += b;
                (format_bytes(*b), "pruneable")
            }
            ScanStatus::Clean => ("-".to_string(), "clean"),
            ScanStatus::NotFound => ("-".to_string(), "not found"),
        };
        let target = r.primary_target.as_deref().unwrap_or("-");
        table.add_row(vec![r.name, &size, status, target]);
    }

    println!("{table}");

    if total > 0 {
        println!("\nTotal reclaimable: {}", format_bytes(total));
    }
}
