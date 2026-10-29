use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::{dir_size, parse_size_str, SizeUnits};
use crate::progress::ProgressReporter;
use crate::prompt::PromptPort;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct OllamaCleaner {
    models_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

#[derive(Debug, Clone)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
}

impl OllamaCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            models_dir: home.join(".ollama/models"),
            runner,
        }
    }

    pub fn list_models(&self) -> Result<Vec<OllamaModel>> {
        if !self.runner.exists("ollama") {
            return Ok(vec![]);
        }
        let output = self.runner.run("ollama", &["list"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut models = Vec::new();
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                // PBI-51: `parse_model_size` returns `None` for unparseable
                // sizes (e.g. an "unknown" SIZE column from a future or
                // localized `ollama list`). Defaulting to 0 bytes here is
                // deliberate, not a silent fallback: `ollama list` output is
                // best-effort and must not fail the whole listing, and a 0
                // keeps `total_size()` sums and freed-bytes reports sane — a
                // zero-size row simply contributes nothing and is never
                // preferred over a parseable size.
                let size = parse_model_size(parts.get(2).unwrap_or(&"0B")).unwrap_or(0);
                models.push(OllamaModel { name, size });
            }
        }
        Ok(models)
    }

    fn total_size(&self) -> u64 {
        if let Ok(models) = self.list_models() {
            let cli_total: u64 = models.iter().map(|m| m.size).sum();
            if cli_total > 0 {
                return cli_total;
            }
        }
        if self.models_dir.exists() {
            dir_size(&self.models_dir)
        } else {
            0
        }
    }
}

/// Parses a model size string from `ollama list` output (e.g. "4.7GB") into
/// bytes using binary units. Returns `None` for unparseable input.
///
/// PBI-51: this used to be a weak local copy of brew's parser that silently
/// returned 0 for garbage via `unwrap_or(0.0)`; it now delegates to the shared
/// [`crate::format::parse_size_str`] and surfaces the failure as `None` so the
/// caller can decide the default explicitly.
fn parse_model_size(s: &str) -> Option<u64> {
    parse_size_str(s, SizeUnits::Binary)
}

impl Cleaner for OllamaCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

    fn is_available(&self) -> bool {
        self.runner.exists("ollama")
    }

    fn name(&self) -> &'static str {
        "ollama"
    }

    fn detect(&self) -> ScanResult {
        let bytes = self.total_size();
        if bytes == 0 {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let mut r = ScanResult::new(self.name(), ScanStatus::Pruneable(bytes));
        if crate::context::is_verbose() {
            r = r.with_target(self.models_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if self.runner.exists("ollama") {
            let models = self.list_models()?;
            if models.is_empty() {
                if self.models_dir.exists() && dir_size(&self.models_dir) > 0 {
                    return self.clean_fallback(dry_run);
                }
                println!("[ollama] no models found");
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                });
            }

            if dry_run {
                println!("[ollama] dry-run: {} models", models.len());
                for m in &models {
                    println!(
                        "  would remove: {} ({})",
                        m.name,
                        crate::format::format_bytes(m.size)
                    );
                }
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                });
            }

            // PBI-52: ollama is classified `Risk::Safe`, so `--yes` selects it
            // for unattended cleaning, but the `MultiSelect::interact()` below
            // fails on a non-TTY stdin with an obscure dialoguer error - a
            // Safe-classified target could never be cleaned by `--yes`. This
            // is the standardized "not a terminal" gate (PBI 55): in a
            // non-TTY, ask for no selection and skip.
            if !crate::prompt::gate_interactive("ollama") {
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                });
            }

            let items: Vec<String> = models
                .iter()
                .map(|m| format!("{:<24}  {}", m.name, crate::format::format_bytes(m.size)))
                .collect();
            let defaults: Vec<bool> = vec![true; models.len()];

            println!("\nOllama models in ~/.ollama/models/:\n");
            let selections =
                crate::prompt::TtyPrompt.multi_select(reporter, None, &items, &defaults)?;

            if selections.is_empty() {
                println!("[ollama] nothing selected");
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                });
            }

            let mut total: u64 = 0;
            let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];
            if !selections.is_empty() {
                reporter.progress_init(self.name(), selections.len());
            }
            for (j, &i) in selections.iter().enumerate() {
                let m = &models[i];
                reporter.progress_tick(Path::new(&m.name), j + 1, m.size);
                // A failed `ollama rm` must not claim the model's bytes:
                // record a SkippedEntry and continue with the remaining
                // models (PBI 53).
                if let Err(reason) = Self::remove_model(self.runner.as_ref(), &m.name) {
                    eprintln!("[ollama] failed to remove {}: {reason}", m.name);
                    skipped.push(crate::cleaner::SkippedEntry {
                        path: self.models_dir.join(&m.name),
                        reason,
                    });
                    continue;
                }
                total += m.size;
                println!(
                    "[ollama] removed: {} (freed {})",
                    m.name,
                    crate::format::format_bytes(m.size)
                );
            }
            if !selections.is_empty() {
                reporter.progress_finish();
            }
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: total,
                uses_trash: false,
                skipped,
                deleted_paths: vec![],
            });
        }

        self.clean_fallback(dry_run)
    }
}

impl OllamaCleaner {
    /// Removes a model, checking the exit code (PBI 53): a non-zero exit
    /// or a spawn error must not count as a successful removal.
    fn remove_model(runner: &dyn CommandRunner, name: &str) -> std::result::Result<(), String> {
        match runner.run("ollama", &["rm", name]) {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!(
                "ollama rm exited with {}",
                o.status.code().unwrap_or(-1)
            )),
            Err(err) => Err(format!("{err:#}")),
        }
    }

    fn clean_fallback(&self, dry_run: bool) -> Result<CleanResult> {
        let dir = &self.models_dir;
        if !dir.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }
        let size = dir_size(dir);
        if dry_run {
            println!(
                "[ollama] would remove: {} ({})",
                dir.display(),
                crate::format::format_bytes(size)
            );
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }
        let outcome =
            crate::trash::safe_delete(self.runner.as_ref(), dir, crate::trash::TrashMode::Global)?;
        let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];
        let mut freed = size;
        if let Some(reason) = outcome.skip_reason {
            skipped.push(crate::cleaner::SkippedEntry {
                path: dir.to_path_buf(),
                reason,
            });
            freed = 0;
        } else {
            println!("[ollama] removed: {}", dir.display());
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: outcome.uses_trash,
            skipped,
            deleted_paths: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;
    use std::os::unix::process::ExitStatusExt;

    struct MockOllamaRunner {
        list_output: String,
    }
    impl CommandRunner for MockOllamaRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
            assert_eq!(program, "ollama");
            if args == ["list"] {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: self.list_output.as_bytes().to_vec(),
                    stderr: vec![],
                })
            } else if args.first() == Some(&"rm") {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: vec![],
                    stderr: vec![],
                })
            } else {
                panic!("unexpected args: {args:?}");
            }
        }
        fn exists(&self, program: &str) -> bool {
            program == "ollama"
        }
    }

    #[test]
    fn list_models_parses_ollama_output() {
        let output = "NAME\tID\tSIZE\tMODIFIED\nllama3.2:3b\tabc123\t2.0GB\t2 days ago\n";
        let runner = MockOllamaRunner {
            list_output: output.to_string(),
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = OllamaCleaner::new(tmp.path(), Box::new(runner));
        let models = cleaner.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama3.2:3b");
        assert_eq!(models[0].size, (2.0_f64 * 1_073_741_824.0) as u64);
    }

    // PBI-51: `parse_model_size` now returns `Option<u64>` (explicit `None`
    // handling instead of a silent 0), so the assertions wrap the value in
    // `Some`; the parsed bytes are unchanged.
    #[test]
    fn parse_model_size_gb() {
        assert_eq!(
            parse_model_size("4.7GB"),
            Some((4.7_f64 * 1_073_741_824.0) as u64)
        );
    }

    #[test]
    fn parse_model_size_mb() {
        assert_eq!(
            parse_model_size("234MB"),
            Some((234.0_f64 * 1_048_576.0) as u64)
        );
    }

    // PBI-51: garbage input must surface as `None` (handled explicitly at the
    // `list_models` call site), not a silent 0 from `unwrap_or(0.0)`.
    #[test]
    fn parse_model_size_garbage_returns_none() {
        assert_eq!(parse_model_size("unknown"), None);
        assert_eq!(parse_model_size(""), None);
        assert_eq!(parse_model_size("12.3.4GB"), None);
    }

    #[test]
    fn list_models_garbage_size_defaults_to_zero_explicitly() {
        let output = "NAME\tID\tSIZE\tMODIFIED\nllama3.2:3b\tabc123\tunknown\t2 days ago\n";
        let runner = MockOllamaRunner {
            list_output: output.to_string(),
        };
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = OllamaCleaner::new(tmp.path(), Box::new(runner));
        let models = cleaner.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama3.2:3b");
        // Explicit call-site default (see comment in `list_models`): an
        // unparseable SIZE column contributes 0 bytes, never a mis-parse.
        assert_eq!(models[0].size, 0);
    }

    // ── PBI 53: exit-code verification for `ollama rm` ──

    #[test]
    fn remove_model_reports_nonzero_exit() {
        let runner = crate::test_helpers::MockRunner::new().with_exit_code("ollama", 1 << 8);
        let err = OllamaCleaner::remove_model(&runner, "llama3.2:3b").unwrap_err();
        assert!(
            err.contains("exited with 1"),
            "the exit code must surface in the reason, got {err}"
        );
    }

    #[test]
    fn remove_model_reports_spawn_failure() {
        let runner = crate::test_helpers::MockRunner::new().with_failure("ollama");
        let err = OllamaCleaner::remove_model(&runner, "llama3.2:3b").unwrap_err();
        assert!(
            err.contains("failed to spawn"),
            "the spawn failure must surface in the reason, got {err}"
        );
    }
}
