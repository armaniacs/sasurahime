use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

pub trait ProgressReporter: Send + Sync {
    fn show_spinner(&self) -> bool;
    fn progress_init(&self, label: &str, total: usize);
    fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64);
    fn progress_finish(&self);
}

fn spinner_style() -> &'static ProgressStyle {
    static STYLE: OnceLock<ProgressStyle> = OnceLock::new();
    STYLE.get_or_init(|| {
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("valid indicatif template")
    })
}

/// Message shown while a cleaner deletes a path: names the folder/file so
/// the user sees what is being removed, with progress and size. `total` of 0
/// means the count is unknown and is omitted.
pub fn tick_message(
    prefix: &str,
    path: &Path,
    current: usize,
    total: usize,
    size_bytes: u64,
) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    let size = crate::format::format_bytes(size_bytes);
    if total > 0 {
        format!("{prefix}: {name} ({current}/{total}, {size})")
    } else {
        format!("{prefix}: {name} ({size})")
    }
}

/// A single shared spinner whose message updates as the cleaner deletes each
/// path: `progress_tick` names the folder being removed, so the user sees
/// what is happening instead of a bare rotating character. `animated` enables
/// the steady tick; keep it off while the cleaner may still prompt.
pub struct SharedSpinner {
    pb: ProgressBar,
    prefix: String,
}

impl SharedSpinner {
    pub fn start(msg: &str, animated: bool) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(spinner_style().clone());
        pb.set_message(msg.to_string());
        if animated {
            pb.enable_steady_tick(Duration::from_millis(100));
        }
        Self {
            pb,
            prefix: msg.trim_end_matches("...").to_string(),
        }
    }

    pub fn reporter(&self) -> CleaningProgress {
        CleaningProgress {
            pb: self.pb.clone(),
            prefix: self.prefix.clone(),
        }
    }

    #[cfg_attr(not(test), expect(dead_code))]
    pub fn message(&self) -> String {
        self.pb.message()
    }

    pub fn finish(&self) {
        self.pb.finish_and_clear();
    }
}

/// [`ProgressReporter`] bound to a [`SharedSpinner`]: ticks update the shared
/// spinner's message instead of creating a second competing bar.
pub struct CleaningProgress {
    pb: ProgressBar,
    prefix: String,
}

impl ProgressReporter for CleaningProgress {
    fn show_spinner(&self) -> bool {
        true
    }

    fn progress_init(&self, _label: &str, total: usize) {
        if total > 0 {
            self.pb.set_length(total as u64);
        }
    }

    fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64) {
        let total = self.pb.length().unwrap_or(0) as usize;
        let msg = tick_message(&self.prefix, path, current, total, size_bytes);
        // No hidden-target print: the cleaners print one line per deletion
        // themselves, and a tick line here would double the piped output.
        // The live spinner (TTY) carries the per-path progress.
        self.pb.set_message(msg);
        if total > 0 {
            self.pb.set_position(current as u64);
        }
    }

    fn progress_finish(&self) {
        // The caller clears the bar right after; nothing to render here.
    }
}

#[cfg_attr(not(test), expect(dead_code))]
pub fn with_spinner<R>(msg: &str, f: impl FnOnce() -> R) -> R {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style().clone());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f();
    pb.finish_and_clear();
    eprintln!("{msg} [OK]");
    result
}

/// Spinner whose message updates with the path being deleted. `animated`
/// enables the steady tick (safe only when the cleaner will not prompt).
pub fn with_spinner_result_progress<T, E: std::fmt::Display>(
    msg: &str,
    animated: bool,
    f: impl FnOnce(&dyn ProgressReporter) -> Result<T, E>,
) -> Result<T, E> {
    let spinner = SharedSpinner::start(msg, animated);
    let result = f(&spinner.reporter());
    spinner.finish();
    if result.is_ok() {
        eprintln!("{msg} [OK]");
    } else {
        eprintln!("{msg} [FAILED]");
    }
    result
}

pub fn with_spinner_result<T, E: std::fmt::Display>(
    msg: &str,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style().clone());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f();
    pb.finish_and_clear();
    if result.is_ok() {
        eprintln!("{msg} [OK]");
    } else {
        eprintln!("{msg} [FAILED]");
    }
    result
}

/// Show a consolidated progress bar for parallel operations.
/// The progress bar is hidden in non-TTY environments (e.g. tests).
/// `f` receives a `&ProgressBar` that `par_iter()` threads can call
/// `pb.inc(1)` / `pb.set_message(...)` on.
pub fn with_parallel_scan<R>(total: usize, f: impl FnOnce(&ProgressBar) -> R) -> R {
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:20}] {msg}")
            .expect("valid template")
            .progress_chars("=> "),
    );
    pb.set_message(format!("Scanning {total} cleaners..."));
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f(&pb);
    pb.finish_and_clear();
    eprintln!("Scan complete [OK]");
    result
}

pub struct VerboseProgress {
    pb: Mutex<Option<ProgressBar>>,
    last_tick: Mutex<Option<Instant>>,
}

impl VerboseProgress {
    pub fn new() -> Self {
        Self {
            pb: Mutex::new(None),
            last_tick: Mutex::new(None),
        }
    }
}

impl ProgressReporter for VerboseProgress {
    fn show_spinner(&self) -> bool {
        true
    }

    fn progress_init(&self, label: &str, total: usize) {
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:30.cyan/blue} {pos}/{len} ETA {eta}",
                )
                .expect("valid indicatif template")
                .progress_chars("=> "),
        );
        pb.set_message(format!("Cleaning {label}..."));
        pb.enable_steady_tick(Duration::from_millis(100));
        *self.pb.lock().unwrap() = Some(pb);
    }

    fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64) {
        if let Some(ref pb) = *self.pb.lock().unwrap() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");

            let speed_str = self
                .last_tick
                .lock()
                .unwrap()
                .map(|start| {
                    let elapsed = start.elapsed();
                    let secs = elapsed.as_secs_f64().max(0.001);
                    format_speed(size_bytes, secs)
                })
                .unwrap_or_default();

            *self.last_tick.lock().unwrap() = Some(Instant::now());

            pb.set_message(format!(
                "{name}{speed_str} ({}/{})",
                current,
                pb.length().unwrap_or(0)
            ));
            pb.set_position(current as u64);
        }
    }

    fn progress_finish(&self) {
        if let Some(pb) = self.pb.lock().unwrap().take() {
            pb.finish_and_clear();
        }
    }
}

pub struct DeepSuppressReporter;

impl ProgressReporter for DeepSuppressReporter {
    fn show_spinner(&self) -> bool {
        false
    }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
    fn progress_finish(&self) {}
}

pub struct SuppressReporter;

impl ProgressReporter for SuppressReporter {
    fn show_spinner(&self) -> bool {
        true
    }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
    fn progress_finish(&self) {}
}

pub fn build_reporter_from_flags(suppress: bool, deep_suppress: bool) -> Box<dyn ProgressReporter> {
    if deep_suppress {
        Box::new(DeepSuppressReporter)
    } else if suppress {
        Box::new(SuppressReporter)
    } else {
        Box::new(VerboseProgress::new())
    }
}

pub fn merge_suppress_flags(
    cli_suppress: bool,
    cli_deep_suppress: bool,
    cfg_suppress: bool,
    cfg_deep_suppress: bool,
) -> (bool, bool) {
    let suppress = cli_suppress || cfg_suppress;
    let deep_suppress = cli_deep_suppress || cfg_deep_suppress;
    if deep_suppress {
        (false, true)
    } else {
        (suppress, false)
    }
}

fn format_speed(size_bytes: u64, elapsed_secs: f64) -> String {
    if size_bytes == 0 || elapsed_secs <= 0.0 {
        return String::new();
    }
    let mb = size_bytes as f64 / 1_048_576.0;
    let secs = elapsed_secs.max(0.001);
    format!(", {:.1} MB/s", mb / secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn with_spinner_returns_value() {
        let result = with_spinner("test", || 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn verbose_progress_shows_spinner() {
        let reporter = VerboseProgress::new();
        assert!(reporter.show_spinner());
    }

    #[test]
    fn deep_suppress_reporter_hides_spinner() {
        let reporter = DeepSuppressReporter;
        assert!(!reporter.show_spinner());
    }

    #[test]
    fn suppress_reporter_shows_spinner() {
        let reporter = SuppressReporter;
        assert!(reporter.show_spinner());
    }

    #[test]
    fn verbose_progress_lifecycle() {
        let reporter = VerboseProgress::new();
        let path = Path::new("/tmp/test.log");
        reporter.progress_init("test", 5);
        reporter.progress_tick(path, 1, 1024);
        reporter.progress_tick(path, 2, 2048);
        reporter.progress_finish();
        assert!(reporter.show_spinner());
    }

    #[test]
    fn suppress_reporter_progress_is_noop() {
        let reporter = SuppressReporter;
        reporter.progress_init("test", 5);
        reporter.progress_tick(Path::new("/x"), 1, 512);
        reporter.progress_finish();
    }

    #[test]
    fn deep_suppress_reporter_progress_is_noop() {
        let reporter = DeepSuppressReporter;
        reporter.progress_init("test", 5);
        reporter.progress_tick(Path::new("/x"), 1, 512);
        reporter.progress_finish();
    }

    #[test]
    fn tick_message_shows_the_path_being_deleted() {
        let msg = tick_message(
            "Cleaning app-support",
            Path::new("/Users/u/Library/Application Support/Google/Chrome"),
            2,
            5,
            4096,
        );
        assert!(
            msg.contains("Chrome"),
            "the tick must name the folder being deleted, got: {msg}"
        );
        assert!(
            msg.contains("2/5"),
            "the tick must show progress, got: {msg}"
        );
        assert!(
            msg.contains("4.0 KB"),
            "the tick must show the size, got: {msg}"
        );
        assert!(msg.starts_with("Cleaning app-support"));
        // Without a total (single-item cleans) the message still names the path.
        let msg = tick_message(
            "Cleaning caches",
            Path::new("/Users/u/.cache/act"),
            1,
            0,
            1024,
        );
        assert!(msg.contains("act"), "got: {msg}");
        assert!(msg.contains("1.0 KB"), "got: {msg}");
    }

    #[test]
    fn cleaning_progress_updates_the_shared_spinner_message() {
        // One shared spinner: the cleaner's progress_tick updates its message
        // so the user sees the folder being deleted while it runs.
        let spinner = SharedSpinner::start("Cleaning app-support...", false);
        let rep = spinner.reporter();
        rep.progress_init("app-support", 5);
        rep.progress_tick(
            Path::new("/Users/u/Library/Application Support/Google/Chrome"),
            2,
            4096,
        );
        let msg = spinner.message();
        assert!(
            msg.contains("Chrome"),
            "the shared spinner must show the path being deleted, got: {msg}"
        );
        assert!(msg.contains("2/5"), "got: {msg}");
        assert!(spinner.reporter().show_spinner());
        spinner.finish();
    }

    #[test]
    fn build_reporter_default_verbose() {
        let r = build_reporter_from_flags(false, false);
        assert!(r.show_spinner());
    }

    #[test]
    fn build_reporter_deep_suppress_wins_over_suppress() {
        let r = build_reporter_from_flags(true, true);
        assert!(!r.show_spinner());
    }

    #[test]
    fn build_reporter_suppress_shows_spinner() {
        let r = build_reporter_from_flags(true, false);
        assert!(r.show_spinner());
    }

    #[test]
    fn merge_flags_cli_suppress_or_config() {
        let (s, d) = merge_suppress_flags(true, false, false, false);
        assert!(s);
        assert!(!d);
    }

    #[test]
    fn merge_flags_config_suppress_applied() {
        let (s, _d) = merge_suppress_flags(false, false, true, false);
        assert!(s);
    }

    #[test]
    fn merge_flags_deep_wins_over_suppress() {
        let (_s, d) = merge_suppress_flags(true, true, false, false);
        assert!(d);
    }

    #[test]
    fn format_speed_shows_mb_per_sec() {
        let s = format_speed(10_485_760, 2.0);
        assert_eq!(s, ", 5.0 MB/s");
    }

    #[test]
    fn format_speed_zero_bytes_returns_empty() {
        let s = format_speed(0, 2.0);
        assert_eq!(s, "");
    }

    #[test]
    fn format_speed_zero_elapsed_returns_empty() {
        let s = format_speed(1024, 0.0);
        assert_eq!(s, "");
    }

    #[test]
    fn format_speed_small_values() {
        let s = format_speed(1_048_576, 10.0);
        assert_eq!(s, ", 0.1 MB/s");
    }

    #[test]
    fn with_spinner_result_returns_ok() {
        let result: Result<i32, String> = with_spinner_result("test", || Ok(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn with_spinner_result_returns_error() {
        let result: Result<i32, &str> = with_spinner_result("test", || Err("boom"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "boom");
    }
}
