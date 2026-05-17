use crate::cleaner::{Cleaner, ScanStatus};
use crate::format::format_bytes;
use comfy_table::{presets::UTF8_FULL, Table};

pub fn run_scan(cleaners: &[Box<dyn Cleaner>]) {
    let results: Vec<_> = cleaners.iter().map(|c| c.detect()).collect();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Category", "Size", "Status"]);

    let mut total: u64 = 0;

    for r in &results {
        let (size, status) = match &r.status {
            ScanStatus::Pruneable(b) => {
                total += b;
                (format_bytes(*b), "pruneable")
            }
            ScanStatus::Clean => ("-".to_string(), "clean"),
            ScanStatus::NotFound => ("-".to_string(), "not found"),
            ScanStatus::PermissionDenied => ("-".to_string(), "permission denied"),
        };
        table.add_row(vec![r.name, &size, status]);
    }

    println!("{table}");

    if total > 0 {
        println!("\nTotal reclaimable: {}", format_bytes(total));
    }
}
