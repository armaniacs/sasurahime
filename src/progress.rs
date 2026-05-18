use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

pub fn with_spinner<R>(msg: &str, f: impl FnOnce() -> R) -> R {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f();
    pb.finish_and_clear();
    eprintln!("{msg} ✓");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_spinner_returns_value() {
        let result = with_spinner("test", || 42);
        assert_eq!(result, 42);
    }
}
