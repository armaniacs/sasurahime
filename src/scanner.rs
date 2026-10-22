use crate::cleaner::{Cleaner, Risk, ScanResult, ScanStatus};
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

/// Human-readable status label, shared by the table and the JSON output.
pub(crate) fn status_label(status: &ScanStatus) -> &'static str {
    match status {
        ScanStatus::Pruneable(_) => "pruneable",
        ScanStatus::Clean => "clean",
        ScanStatus::NotFound => "not found",
        ScanStatus::Unavailable => "unavailable",
        ScanStatus::Unknown => "unknown",
    }
}

/// (size column, status column, bytes counted toward the total).
/// `Unavailable` and `Unknown` rows display their own status and are excluded
/// from the total.
fn status_columns(status: &ScanStatus) -> (String, &'static str, Option<u64>) {
    match status {
        ScanStatus::Pruneable(b) => (format_bytes(*b), status_label(status), Some(*b)),
        ScanStatus::Clean => ("-".to_string(), status_label(status), None),
        ScanStatus::NotFound => ("-".to_string(), status_label(status), None),
        ScanStatus::Unavailable => ("-".to_string(), status_label(status), None),
        ScanStatus::Unknown => ("-".to_string(), status_label(status), None),
    }
}

/// One machine-readable entry of `scan --json`.
#[derive(serde::Serialize)]
pub(crate) struct ScanJsonEntry {
    pub(crate) name: &'static str,
    /// Measured bytes, or `null` when the size is not measurable.
    pub(crate) size_bytes: Option<u64>,
    pub(crate) status: &'static str,
    /// Risk classification (Safe / Moderate / Risky / Manual).
    pub(crate) risk: &'static str,
    /// Whether a byte estimate is available for this target.
    pub(crate) estimable: bool,
}

/// Serializes the scan document: entries plus the total reclaimable bytes.
pub(crate) fn scan_json_document(
    results: &[ScanResult],
    total: u64,
    risks: &std::collections::HashMap<&str, Risk>,
) -> String {
    let targets: Vec<ScanJsonEntry> = results
        .iter()
        .map(|r| ScanJsonEntry {
            name: r.name,
            size_bytes: match r.status {
                ScanStatus::Pruneable(b) => Some(b),
                _ => None,
            },
            status: status_label(&r.status),
            risk: risks.get(r.name).map(|risk| risk.label()).unwrap_or("-"),
            estimable: matches!(r.status, ScanStatus::Pruneable(_)),
        })
        .collect();
    let doc = serde_json::json!({
        "targets": targets,
        "total_reclaimable_bytes": total,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

/// Runs detect on every available cleaner, sorted by size.
fn scan_results(cleaners: &[Box<dyn Cleaner>]) -> Vec<ScanResult> {
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
    results
}

pub fn run_scan(cleaners: &[Box<dyn Cleaner>]) {
    let results = scan_results(cleaners);
    let available: Vec<&Box<dyn Cleaner>> = cleaners.iter().filter(|c| c.is_available()).collect();
    // Same classification as `--yes` and the TUI, shown per row.
    let risks: std::collections::HashMap<&str, Risk> =
        available.iter().map(|c| (c.name(), c.risk())).collect();

    let mut table = Table::new();
    if crate::history::USE_UNICODE.load(std::sync::atomic::Ordering::Relaxed) {
        table.load_preset(UTF8_FULL);
    } else {
        table.load_preset(ASCII_FULL);
    }
    table.set_header(vec!["Category", "Size", "Status", "Risk", "Target"]);

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
        let risk = risks.get(r.name).map(|risk| risk.label()).unwrap_or("-");
        let target = r.primary_target.as_deref().unwrap_or("-");
        table.add_row(vec![r.name, &size, status, risk, target]);
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

/// Machine-readable scan: prints one JSON document, no table, no hints and no
/// prompt. Used by `scan --json` for scripts and CI.
pub fn run_scan_json(cleaners: &[Box<dyn Cleaner>]) {
    let results = scan_results(cleaners);
    let available: Vec<&Box<dyn Cleaner>> = cleaners.iter().filter(|c| c.is_available()).collect();
    let risks: std::collections::HashMap<&str, Risk> =
        available.iter().map(|c| (c.name(), c.risk())).collect();
    let total: u64 = results
        .iter()
        .filter_map(|r| match r.status {
            ScanStatus::Pruneable(b) => Some(b),
            _ => None,
        })
        .sum();
    println!("{}", scan_json_document(&results, total, &risks));
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
    fn status_columns_unknown_is_excluded_from_total() {
        let (size, status, counted) = status_columns(&ScanStatus::Unknown);
        assert_eq!(status, "unknown");
        assert_eq!(size, "-");
        assert_eq!(counted, None, "unknown must not count toward the total");
    }

    #[test]
    fn scan_json_entries_shape_and_estimable_flag() {
        let results = vec![
            r("uv", ScanStatus::Pruneable(1234)),
            r("docker", ScanStatus::Unknown),
            r("gone", ScanStatus::NotFound),
            r("failed", ScanStatus::Unavailable),
        ];
        let mut risks = std::collections::HashMap::new();
        risks.insert("uv", Risk::Safe);
        risks.insert("docker", Risk::Moderate);
        let doc = scan_json_document(&results, 1234, &risks);
        let parsed: serde_json::Value = serde_json::from_str(&doc).expect("valid JSON");
        let entries = parsed["targets"].as_array().unwrap();
        assert_eq!(entries.len(), 4);
        let uv = &entries[0];
        assert_eq!(uv["name"], "uv");
        assert_eq!(uv["size_bytes"], 1234);
        assert_eq!(uv["status"], "pruneable");
        assert_eq!(uv["risk"], "Safe");
        assert_eq!(uv["estimable"], true);
        let docker = &entries[1];
        assert!(docker["size_bytes"].is_null());
        assert_eq!(docker["status"], "unknown");
        assert_eq!(docker["risk"], "Moderate");
        assert_eq!(docker["estimable"], false);
        assert_eq!(entries[2]["status"], "not found");
        assert_eq!(entries[3]["status"], "unavailable");
    }

    #[test]
    fn scan_json_document_is_valid_and_counts_total() {
        let results = vec![
            r("uv", ScanStatus::Pruneable(100)),
            r("docker", ScanStatus::Unknown),
        ];
        let total: u64 = results
            .iter()
            .filter_map(|r| match r.status {
                ScanStatus::Pruneable(b) => Some(b),
                _ => None,
            })
            .sum();
        let mut risks = std::collections::HashMap::new();
        risks.insert("uv", Risk::Safe);
        risks.insert("docker", Risk::Moderate);
        let doc = scan_json_document(&results, total, &risks);
        let parsed: serde_json::Value = serde_json::from_str(&doc).expect("valid JSON");
        assert_eq!(parsed["total_reclaimable_bytes"], 100);
        assert_eq!(parsed["targets"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["targets"][1]["status"], "unknown");
        assert_eq!(parsed["targets"][0]["risk"], "Safe");
        assert_eq!(parsed["targets"][1]["risk"], "Moderate");
    }

    #[test]
    fn status_columns_pruneable_counts_toward_total() {
        let (size, status, counted) = status_columns(&ScanStatus::Pruneable(1234));
        assert_eq!(status, "pruneable");
        assert_eq!(counted, Some(1234));
        assert!(!size.is_empty());
    }
}
