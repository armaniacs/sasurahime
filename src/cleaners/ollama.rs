use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
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
                let size = parse_model_size(parts.get(2).unwrap_or(&"0B"));
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

fn parse_model_size(s: &str) -> u64 {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("GB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_073_741_824.0) as u64
    } else if let Some(n) = s.strip_suffix("MB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_048_576.0) as u64
    } else if let Some(n) = s.strip_suffix("KB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_024.0) as u64
    } else {
        0
    }
}

impl Cleaner for OllamaCleaner {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn detect(&self) -> ScanResult {
        let bytes = self.total_size();
        if bytes == 0 {
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        ScanResult {
            name: self.name(),
            status: ScanStatus::Pruneable(bytes),
        }
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
                });
            }

            let items: Vec<String> = models
                .iter()
                .map(|m| format!("{:<24}  {}", m.name, crate::format::format_bytes(m.size)))
                .collect();
            let defaults: Vec<bool> = vec![true; models.len()];

            println!("\nOllama models in ~/.ollama/models/:\n");
            let selections = dialoguer::MultiSelect::new()
                .items(&items)
                .defaults(&defaults)
                .interact()?;

            if selections.is_empty() {
                println!("[ollama] nothing selected");
                return Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                });
            }

            let mut total: u64 = 0;
            if !selections.is_empty() {
                reporter.progress_init(self.name(), selections.len());
            }
            for (j, &i) in selections.iter().enumerate() {
                let m = &models[i];
                reporter.progress_tick(Path::new(&m.name), j + 1, m.size);
                self.runner.run("ollama", &["rm", &m.name])?;
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
            });
        }

        self.clean_fallback(dry_run)
    }
}

impl OllamaCleaner {
    fn clean_fallback(&self, dry_run: bool) -> Result<CleanResult> {
        let dir = &self.models_dir;
        if !dir.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
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
            });
        }
        let path_str = dir.to_string_lossy();
        let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
        crate::trash::delete_path(dir)?;
        println!("[ollama] removed: {}", dir.display());
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: size,
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

    #[test]
    fn parse_model_size_gb() {
        assert_eq!(
            parse_model_size("4.7GB"),
            (4.7_f64 * 1_073_741_824.0) as u64
        );
    }

    #[test]
    fn parse_model_size_mb() {
        assert_eq!(parse_model_size("234MB"), (234.0_f64 * 1_048_576.0) as u64);
    }
}
