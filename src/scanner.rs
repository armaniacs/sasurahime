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
    denied: &[&'static str],
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
        "unreadable_protected_paths": denied,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string())
}

/// Runs detect on every available cleaner and returns the results
/// index-aligned with `cleaners` together with the single `is_available` pass:
/// `results[i]` always describes `cleaners[i]`, with unavailable cleaners
/// keeping their `NotFound` placeholder, and `available` lists the indices
/// that were scanned so callers (run_scan / run_scan_json) reuse it for their
/// rows and risks map instead of re-filtering. A panicking detect is isolated
/// per cleaner as `NotFound`. The one detect interface; parallel/progress
/// mechanics move behind it.
pub fn detect_all(cleaners: &[Box<dyn Cleaner>]) -> (Vec<ScanResult>, Vec<usize>) {
    let total = cleaners.len();
    let mut results: Vec<ScanResult> = (0..total)
        .map(|i| ScanResult::new(cleaners[i].name(), ScanStatus::NotFound))
        .collect();

    // The one `is_available` pass per call: detect_all owns it and returns the
    // indices, so no caller ever re-filters.
    let available: Vec<usize> = (0..total).filter(|&i| cleaners[i].is_available()).collect();

    if !available.is_empty() {
        let scanned: Vec<(usize, ScanResult)> =
            crate::progress::with_parallel_scan(available.len(), |pb| {
                available
                    .par_iter()
                    .map(|&i| {
                        // WHY: one bad cleaner must not take down scan/--yes/TUI.
                        // detect() runs arbitrary per-cleaner logic; a panic here
                        // used to propagate through the rayon block and abort the
                        // whole preamble, losing every other cleaner's result.
                        // Isolate per cleaner: the panicking one degrades to
                        // NotFound, the rest still report. (Release builds set
                        // panic=abort, where unwinding — and with it this
                        // isolation — cannot apply.)
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            cleaners[i].detect()
                        }))
                        .unwrap_or_else(|_| {
                            ScanResult::new(cleaners[i].name(), ScanStatus::NotFound)
                        });
                        pb.inc(1);
                        (i, r)
                    })
                    .collect()
            });
        for (i, r) in scanned {
            results[i] = r;
        }
    }

    (results, available)
}

/// Shared preamble of `run_scan` and `run_scan_json`: the risks map and the
/// denied-paths probe. `available` comes from [`detect_all`]'s single
/// `is_available` pass — one pass per call feeds both the scan and this map.
fn scan_preamble(
    cleaners: &[Box<dyn Cleaner>],
    available: &[usize],
    home: &std::path::Path,
) -> (
    std::collections::HashMap<&'static str, Risk>,
    Vec<&'static str>,
) {
    // Same classification as `--yes` and the TUI, shown per row.
    let risks: std::collections::HashMap<&'static str, Risk> = available
        .iter()
        .map(|&i| (cleaners[i].name(), cleaners[i].risk()))
        .collect();
    // Same probe as `doctor`: protected paths blocked by missing FDA.
    let denied = crate::doctor::denied_paths(home);
    (risks, denied)
}

pub fn run_scan(cleaners: &[Box<dyn Cleaner>], home: &std::path::Path) {
    let (results, available) = detect_all(cleaners);
    let (risks, denied) = scan_preamble(cleaners, &available, home);
    // Display rows: available cleaners only, sorted by size — derived from
    // the one `available` pass, never a second availability filter.
    let mut rows: Vec<ScanResult> = available.iter().map(|&i| results[i].clone()).collect();
    sort_by_size(&mut rows);

    let mut table = Table::new();
    if crate::format::use_unicode() {
        table.load_style(UTF8_FULL);
    } else {
        table.load_style(ASCII_FULL);
    }
    table.set_header(vec!["Category", "Size", "Status", "Risk", "Target"]);

    let mut total: u64 = 0;
    let mut unavailable: usize = 0;

    for r in &rows {
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

    if !denied.is_empty() {
        println!(
            "\n{} path(s) unreadable due to missing Full Disk Access; excluded from the total.",
            denied.len()
        );
        for rel in &denied {
            println!("  - ~/{rel}");
        }
        println!("Run `sasurahime doctor` for details.");
    }

    let notes: Vec<String> = available
        .iter()
        .filter_map(|&i| cleaners[i].advisory())
        .collect();
    if !notes.is_empty() {
        println!("\nNotes:");
        for note in notes {
            println!("{note}");
        }
    }
}

/// Machine-readable scan: prints one JSON document, no table, no hints and no
/// prompt. Used by `scan --json` for scripts and CI.
pub fn run_scan_json(cleaners: &[Box<dyn Cleaner>], home: &std::path::Path) {
    let (results, available) = detect_all(cleaners);
    let (risks, denied) = scan_preamble(cleaners, &available, home);
    // Same row selection as the table: available cleaners only, sorted by
    // size, from the one `available` pass.
    let mut rows: Vec<ScanResult> = available.iter().map(|&i| results[i].clone()).collect();
    sort_by_size(&mut rows);
    let total: u64 = rows
        .iter()
        .filter_map(|r| match r.status {
            ScanStatus::Pruneable(b) => Some(b),
            _ => None,
        })
        .sum();
    println!("{}", scan_json_document(&rows, total, &risks, &denied));
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
        let doc = scan_json_document(&results, 1234, &risks, &[]);
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
        let doc = scan_json_document(&results, total, &risks, &[]);
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

    /// Tiny stub cleaner for `detect_all`: fixed name, detect status, and
    /// availability. `clean` is a no-op.
    struct StubCleaner {
        name: &'static str,
        status: ScanStatus,
        available: bool,
    }

    impl Cleaner for StubCleaner {
        fn name(&self) -> &'static str {
            self.name
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.name, self.status.clone())
        }
        fn clean(
            &self,
            _dry_run: bool,
            _reporter: &dyn crate::progress::ProgressReporter,
        ) -> anyhow::Result<crate::cleaner::CleanResult> {
            Ok(crate::cleaner::CleanResult::empty(self.name))
        }
        fn is_available(&self) -> bool {
            self.available
        }
    }

    #[test]
    fn detect_all_preserves_cleaner_order_and_names() {
        // 100 B then 300 B on purpose: not size-descending, so the assertion
        // locks registration order (detect_all must not sort).
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(StubCleaner {
                name: "alpha",
                status: ScanStatus::Pruneable(100),
                available: true,
            }),
            Box::new(StubCleaner {
                name: "beta",
                status: ScanStatus::Pruneable(300),
                available: true,
            }),
            Box::new(StubCleaner {
                name: "gamma",
                status: ScanStatus::NotFound,
                available: false,
            }),
        ];
        let (results, available) = detect_all(&cleaners);
        assert_eq!(names(&results), vec!["alpha", "beta", "gamma"]);
        assert_eq!(results[0].bytes_for_test(), 100);
        assert_eq!(results[1].bytes_for_test(), 300);
        assert!(
            matches!(results[2].status, ScanStatus::NotFound),
            "unavailable cleaner keeps its NotFound placeholder so results stay index-aligned"
        );
        assert_eq!(
            available,
            vec![0usize, 1],
            "the single is_available pass lists exactly the available cleaners"
        );
    }

    /// A cleaner whose detect() panics: the scanner must contain the blast
    /// radius per cleaner, not propagate it to the whole preamble.
    struct PanickingCleaner(&'static str);

    impl Cleaner for PanickingCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            panic!("detect exploded");
        }
        fn clean(
            &self,
            _dry_run: bool,
            _reporter: &dyn crate::progress::ProgressReporter,
        ) -> anyhow::Result<crate::cleaner::CleanResult> {
            Ok(crate::cleaner::CleanResult::empty(self.0))
        }
    }

    #[test]
    fn detect_all_isolates_a_panicking_detect() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(StubCleaner {
                name: "ok",
                status: ScanStatus::Pruneable(42),
                available: true,
            }),
            Box::new(PanickingCleaner("boom")),
        ];
        let (results, available) = detect_all(&cleaners);
        assert_eq!(names(&results), vec!["ok", "boom"]);
        assert_eq!(results[0].bytes_for_test(), 42);
        assert!(
            matches!(results[1].status, ScanStatus::NotFound),
            "a panicking detect must be isolated as NotFound, got: {:?}",
            results[1].status
        );
        assert_eq!(
            available,
            vec![0usize, 1],
            "the panicking cleaner was available; the single is_available pass must list it"
        );
    }

    /// Counts `is_available` calls so the single-pass unification is
    /// observable through run_scan: one call, one pass.
    struct CountingAvailability {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Cleaner for CountingAvailability {
        fn name(&self) -> &'static str {
            "counting"
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new("counting", ScanStatus::Pruneable(10))
        }
        fn clean(
            &self,
            _dry_run: bool,
            _reporter: &dyn crate::progress::ProgressReporter,
        ) -> anyhow::Result<crate::cleaner::CleanResult> {
            Ok(crate::cleaner::CleanResult::empty("counting"))
        }
        fn is_available(&self) -> bool {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            true
        }
    }

    #[test]
    fn run_scan_evaluates_is_available_once_per_call() {
        use std::sync::Arc;
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(CountingAvailability {
            calls: Arc::clone(&calls),
        })];
        run_scan(&cleaners, std::path::Path::new("/"));
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "is_available must be evaluated in exactly one pass per run_scan call"
        );
    }
}
