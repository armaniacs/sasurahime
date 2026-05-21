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
