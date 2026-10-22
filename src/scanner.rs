use crate::cleaner::{Cleaner, ScanResult, ScanStatus};
use crate::format::format_bytes;
use comfy_table::presets::{ASCII_FULL, UTF8_FULL};
use comfy_table::Table;
use rayon::prelude::*;

/// Larger targets first; targets without a size (clean / not found) keep their
/// registration order at the end.
fn sort_by_size(results: &mut [ScanResult]) {
    fn size(r: &ScanResult) -> Option<u64> {
        match r.status {
            ScanStatus::Pruneable(b) => Some(b),
            _ => None,
        }
    }
    results.sort_by(|a, b| match (size(a), size(b)) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

/// (size column, status column, bytes counted toward the total).
/// `Unavailable` rows display their own status and are excluded from the total.
fn status_columns(status: &ScanStatus) -> (String, &'static str, Option<u64>) {
    match status {
        ScanStatus::Pruneable(b) => (format_bytes(*b), "pruneable", Some(*b)),
        ScanStatus::Clean => ("-".to_string(), "clean", None),
        ScanStatus::NotFound => ("-".to_string(), "not found", None),
        ScanStatus::Unavailable => ("-".to_string(), "unavailable", None),
    }
}

pub fn run_scan(cleaners: &[Box<dyn Cleaner>]) {
    // Pre-filter: skip cleaners whose binary is not installed
    let available: Vec<&Box<dyn Cleaner>> = cleaners.iter().filter(|c| c.is_available()).collect();
    let total_available = available.len();

    let mut results: Vec<_> = crate::progress::with_parallel_scan(total_available, |pb| {
        available
            .par_iter()
            .map(|c| {
                let r = c.detect();
                pb.inc(1);
                r
            })
            .collect()
    });
    sort_by_size(&mut results);

    let mut table = Table::new();
    if crate::history::USE_UNICODE.load(std::sync::atomic::Ordering::Relaxed) {
        table.load_preset(UTF8_FULL);
    } else {
        table.load_preset(ASCII_FULL);
    }
    table.set_header(vec!["Category", "Size", "Status", "Target"]);

    let mut total: u64 = 0;
    let mut unavailable: usize = 0;

    for r in &results {
        let (size, status, counted) = status_columns(&r.status);
        if let Some(b) = counted {
            total += b;
        }
        if matches!(r.status, ScanStatus::Unavailable) {
            unavailable += 1;
        }
        let target = r.primary_target.as_deref().unwrap_or("-");
        table.add_row(vec![r.name, &size, status, target]);
    }

    println!("{table}");

    if total > 0 {
        println!("\nTotal reclaimable: {}", format_bytes(total));
    }

    if unavailable > 0 {
        println!(
            "\n{unavailable} target(s) unavailable (could not be measured); excluded from the total."
        );
    }

    let notes: Vec<String> = available.iter().filter_map(|c| c.advisory()).collect();
    if !notes.is_empty() {
        println!("\nNotes:");
        for note in notes {
            println!("{note}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &'static str, status: ScanStatus) -> ScanResult {
        ScanResult::new(name, status)
    }

    fn names(v: &[ScanResult]) -> Vec<&'static str> {
        v.iter().map(|r| r.name).collect()
    }

    #[test]
    fn sort_by_size_puts_the_largest_first_and_sizeless_last() {
        let mut v = vec![
            r("small", ScanStatus::Pruneable(10)),
            r("gone", ScanStatus::NotFound),
            r("large", ScanStatus::Pruneable(1000)),
            r("clean", ScanStatus::Clean),
            r("zero", ScanStatus::Pruneable(0)),
        ];
        sort_by_size(&mut v);
        assert_eq!(names(&v), vec!["large", "small", "zero", "gone", "clean"]);
    }

    #[test]
    fn sort_by_size_keeps_registration_order_for_equal_sizes() {
        let mut v = vec![
            r("first", ScanStatus::NotFound),
            r("second", ScanStatus::Clean),
            r("third", ScanStatus::NotFound),
        ];
        sort_by_size(&mut v);
        assert_eq!(names(&v), vec!["first", "second", "third"]);
    }

    #[test]
    fn sort_by_size_treats_unavailable_as_sizeless() {
        let mut v = vec![
            r("failed", ScanStatus::Unavailable),
            r("small", ScanStatus::Pruneable(10)),
            r("clean", ScanStatus::Clean),
        ];
        sort_by_size(&mut v);
        assert_eq!(names(&v), vec!["small", "failed", "clean"]);
    }

    #[test]
    fn status_columns_unavailable_is_excluded_from_total() {
        let (size, status, counted) = status_columns(&ScanStatus::Unavailable);
        assert_eq!(status, "unavailable");
        assert_eq!(size, "-");
        assert_eq!(counted, None, "unavailable must not count toward the total");
    }

    #[test]
    fn status_columns_pruneable_counts_toward_total() {
        let (size, status, counted) = status_columns(&ScanStatus::Pruneable(1234));
        assert_eq!(status, "pruneable");
        assert_eq!(counted, Some(1234));
        assert!(!size.is_empty());
    }
}
