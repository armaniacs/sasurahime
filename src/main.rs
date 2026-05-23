mod cleaner;
mod cleaners;
mod command;
mod config;
mod format;
mod hint;
mod interactive;
mod progress;
mod scanner;
mod trash;

use clap::{Parser, Subcommand};
use cleaner::{CleanResult, Cleaner};
use command::SystemCommandRunner;
use config::Config;
use dirs::home_dir;
use progress::{build_reporter_from_flags, merge_suppress_flags, ProgressReporter};
use std::path::PathBuf;

/// Macro that generates CleanTarget enum, SUPPORTED_TARGETS, dispatch helpers,
/// and all_cleaners() from a single definition table.
///
/// Usage:
/// ```ignore
/// define_cleaners! {
///     // Standard cleaners (simple { dry_run: bool } variant):
///     Act : "act" => "description" ; |home, _config| { factory_expr }
///     ...
///     ;
///     // Special enum variants inserted verbatim:
///     // Caches { dry_run: bool },
///     // Logs { dry_run: bool, #[arg(long)] keep_days: Option<u32> },
/// }
/// ```
/// Helper: extract the CLI command name for a standard CleanTarget variant.
macro_rules! cmd_name {
    (Act) => {
        "act"
    };
    (Uv) => {
        "uv"
    };
    (Brew) => {
        "brew"
    };
    (Mise) => {
        "mise"
    };
    (Browsers) => {
        "browsers"
    };
    (Bun) => {
        "bun"
    };
    (Go) => {
        "go"
    };
    (Pip) => {
        "pip"
    };
    (NodeGyp) => {
        "node-gyp"
    };
    (Npm) => {
        "npm"
    };
    (Yarn) => {
        "yarn"
    };
    (Pnpm) => {
        "pnpm"
    };
    (Cargo) => {
        "cargo"
    };
    (Docker) => {
        "docker"
    };
    (Orbstack) => {
        "orbstack"
    };
    (CocoaPods) => {
        "cocoa-pods"
    };
    (Colima) => {
        "colima"
    };
    (SwiftPM) => {
        "spm"
    };
    (Conda) => {
        "conda"
    };
    (Poetry) => {
        "poetry"
    };
    (Pipx) => {
        "pipx"
    };
    (Deno) => {
        "deno"
    };
    (Rustup) => {
        "rustup"
    };
    (Simulator) => {
        "simulator"
    };
    (Gradle) => {
        "gradle"
    };
    (Huggingface) => {
        "huggingface"
    };
    (PreCommit) => {
        "pre-commit"
    };
    (JetBrains) => {
        "jetbrains"
    };
    (Downloads) => {
        "downloads"
    };
    (VscodeExtensions) => {
        "vscode-extensions"
    };
    (Maven) => {
        "maven"
    };
    (Terraform) => {
        "terraform"
    };
    (Flutter) => {
        "flutter"
    };
    (Volta) => {
        "volta"
    };
    (Sbt) => {
        "sbt"
    };
    (TreeSitter) => {
        "tree-sitter"
    };
}

/// Generate dispatch_clean and dispatch helpers from the same definition table.
macro_rules! define_cleaners {
    ($(
        $(#[$variant_meta:meta])*
        $variant:ident : $cli_name:literal => $desc:expr ;
        ($factory:expr)
    ),+ $(,)?
    ;
        $( $special_variants:tt )*
    ) => {
        #[derive(Subcommand)]
        enum CleanTarget {
            $(
                $(#[$variant_meta])*
                #[command(name = $cli_name)]
                $variant { #[arg(long)] dry_run: bool },
            )*
            $($special_variants)*
        }

        const SUPPORTED_TARGETS: &[(&str, &str)] = &[
            $( ($cli_name, $desc) ),*
        ];

        fn dispatch_clean(
            home: &std::path::Path,
            config: &config::Config,
            target: &CleanTarget,
            dry_run: bool,
            reporter: &dyn ProgressReporter,
        ) -> anyhow::Result<CleanResult> {
            match target {
                $(
                    CleanTarget::$variant { .. } => ($factory)(home, config).clean(dry_run, reporter),
                )*
                _ => unreachable!("dispatch_clean: unexpected special variant"),
            }
        }

        impl CleanTarget {
            fn dispatch_command_name(&self) -> &'static str {
                match self {
                    $( CleanTarget::$variant { .. } => cmd_name!($variant), )*
                    _ => unreachable!("dispatch_command_name: unexpected special variant"),
                }
            }

            fn dispatch_dry_run(&self) -> bool {
                match self {
                    $( CleanTarget::$variant { dry_run } => *dry_run, )*
                    _ => unreachable!("dispatch_dry_run: unexpected special variant"),
                }
            }
        }
    };
}

define_cleaners! {
    Act : "act" => "act GitHub Actions local runner cache";
    (|home, _config| cleaners::generic::GenericCleaner::act(home, Box::new(SystemCommandRunner))),

    Uv : "uv" => "Stale simple-vN index directories + uv cache prune";
    (|home, _config| cleaners::uv::UvCleaner::new(home, Box::new(SystemCommandRunner))),

    Brew : "brew" => "Homebrew download cache";
    (|home, _config| cleaners::brew::BrewCleaner::new(home, Box::new(SystemCommandRunner))),

    Mise : "mise" => "Unused runtime versions";
    (|home, _config| cleaners::mise::MiseCleaner::new(home, Box::new(SystemCommandRunner))),

    Browsers : "browsers" => "Old Puppeteer / Playwright builds";
    (|home, _config| cleaners::browser::BrowserCleaner::new(home, Box::new(SystemCommandRunner))),

    Bun : "bun" => "Bun package cache";
    (|_home, _config| cleaners::generic::GenericCleaner::bun(Box::new(SystemCommandRunner))),

    Go : "go" => "Go build cache";
    (|_home, _config| cleaners::generic::GenericCleaner::go(Box::new(SystemCommandRunner))),

    Pip : "pip" => "pip package cache";
    (|_home, _config| cleaners::generic::GenericCleaner::pip(Box::new(SystemCommandRunner))),

    NodeGyp : "node-gyp" => "node-gyp build cache directories";
    (|home, _config| cleaners::generic::GenericCleaner::node_gyp(home, Box::new(SystemCommandRunner))),

    Npm : "npm" => "npm package cache";
    (|_home, _config| cleaners::generic::GenericCleaner::npm(Box::new(SystemCommandRunner))),

    Yarn : "yarn" => "yarn cache";
    (|_home, _config| cleaners::generic::GenericCleaner::yarn(Box::new(SystemCommandRunner))),

    Pnpm : "pnpm" => "pnpm store";
    (|_home, _config| cleaners::generic::GenericCleaner::pnpm(Box::new(SystemCommandRunner))),

    Cargo : "cargo" => "Cargo registry cache + target/ directories";
    (|home, _config| cleaners::cargo::CargoCleaner::new(home, Box::new(SystemCommandRunner))),

    Docker : "docker" => "Docker system prune (images, containers, build cache)";
    (|_home, _config| cleaners::generic::GenericCleaner::docker(Box::new(SystemCommandRunner))),

    Orbstack : "orbstack" => "Orbstack prune";
    (|_home, _config| cleaners::generic::GenericCleaner::orbstack(Box::new(SystemCommandRunner))),

    CocoaPods : "cocoa-pods" => "CocoaPods cache clean --all";
    (|_home, _config| cleaners::generic::GenericCleaner::cocoapods(Box::new(SystemCommandRunner))),

    Colima : "colima" => "Colima VM disk images (inactive) prune";
    (|home, _config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner))),

    SwiftPM : "spm" => "SwiftPM cache directory";
    (|home, _config| cleaners::generic::GenericCleaner::spm_cache(home, Box::new(SystemCommandRunner))),

    Conda : "conda" => "Conda clean --all";
    (|_home, _config| cleaners::generic::GenericCleaner::conda(Box::new(SystemCommandRunner))),

    Poetry : "poetry" => "Poetry cache clear --all";
    (|_home, _config| cleaners::generic::GenericCleaner::poetry(Box::new(SystemCommandRunner))),

    Pipx : "pipx" => "pipx cache and unused packages";
    (|_home, _config| cleaners::generic::GenericCleaner::pipx(Box::new(SystemCommandRunner))),

    Deno : "deno" => "Deno cache reload";
    (|_home, _config| cleaners::generic::GenericCleaner::deno(Box::new(SystemCommandRunner))),

    Rustup : "rustup" => "Unused Rust toolchains";
    (|home, _config| cleaners::rustup::RustupCleaner::new(home, Box::new(SystemCommandRunner))),

    Simulator : "simulator" => "iOS Simulator cache (xcrun simctl delete unavailable)";
    (|home, _config| cleaners::generic::GenericCleaner::simulator(home, Box::new(SystemCommandRunner))),

    Gradle : "gradle" => "Gradle old version caches";
    (|home, _config| cleaners::gradle::GradleCleaner::new(home, Box::new(SystemCommandRunner))),

    Huggingface : "huggingface" => "Hugging Face model cache (hub/)";
    (|home, _config| cleaners::huggingface::HuggingFaceCleaner::new(home, Box::new(SystemCommandRunner))),

    PreCommit : "pre-commit" => "pre-commit hook environment cache";
    (|home, _config| cleaners::pre_commit::PreCommitCleaner::new(home, Box::new(SystemCommandRunner))),

    JetBrains : "jetbrains" => "JetBrains IDE caches (old versions)";
    (|home, _config| cleaners::gradle::JetBrainsCleaner::new(home, Box::new(SystemCommandRunner))),

    Downloads : "downloads" => "~/Downloads old files";
    (|home, _config| cleaners::generic::GenericCleaner::downloads(home, Box::new(SystemCommandRunner))),

    VscodeExtensions : "vscode-extensions" => "VS Code extensions cache";
    (|home, _config| cleaners::generic::GenericCleaner::vscode_extensions(home, Box::new(SystemCommandRunner))),

    Maven : "maven" => "Maven local repository (mvn dependency:purge-local-repository)";
    (|home, _config| cleaners::generic::GenericCleaner::maven(home, Box::new(SystemCommandRunner))),

    Terraform : "terraform" => "Terraform provider plugin cache";
    (|home, _config| cleaners::generic::GenericCleaner::terraform(home, Box::new(SystemCommandRunner))),

    Flutter : "flutter" => "Flutter/Dart pub cache (dart pub cache clean)";
    (|home, _config| cleaners::generic::GenericCleaner::flutter(home, Box::new(SystemCommandRunner))),

    Volta : "volta" => "Volta Node.js manager cache";
    (|home, _config| cleaners::generic::GenericCleaner::volta(home, Box::new(SystemCommandRunner))),

    Sbt : "sbt" => "Scala/sbt build cache and Ivy cache";
    (|home, _config| cleaners::generic::GenericCleaner::sbt(home, Box::new(SystemCommandRunner))),

    TreeSitter : "tree-sitter" => "tree-sitter parser compilation cache";
    (|home, _config| cleaners::generic::GenericCleaner::tree_sitter(home, Box::new(SystemCommandRunner))),

    ;
    /// Clean all generic caches (bun, go, pip, node-gyp, npm, yarn, pnpm)
    Caches {
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove log files older than N days
    Logs {
        #[arg(long)]
        dry_run: bool,
        /// Days to keep (default: value from config file, fallback 7)
        #[arg(long)]
        keep_days: Option<u32>,
    },
    /// Remove Xcode DerivedData build cache
    Xcode {
        #[arg(long)]
        dry_run: bool,
    },
    /// Report Trash size
    Trash {
        #[arg(long)]
        dry_run: bool,
    },
    /// Analyze and clean Ollama model cache
    Ollama {
        #[arg(long)]
        dry_run: bool,
    },
    /// Analyze and clean ~/Library/Logs/ with heuristic recommendations
    #[command(name = "library-logs")]
    LibraryLogs {
        #[arg(long)]
        dry_run: bool,
        /// Skip prompt — delete all suggested entries
        #[arg(long, short = 'a')]
        all: bool,
    },
    /// Remove old Xcode DeviceSupport directories, keeping recent N versions
    #[command(name = "device-support")]
    DeviceSupport {
        #[arg(long)]
        dry_run: bool,
        /// Number of recent versions to keep (default: 2)
        #[arg(long, default_value = "2")]
        keep: u32,
    },
    /// Clean iOS device backups from ~/Library/Application Support/MobileSync/Backup/
    #[command(name = "ios-backup")]
    IosBackup {
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete APFS local Time Machine snapshots
    #[command(name = "apfs-snapshot")]
    ApfsSnapshot {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Parser)]
#[command(
    name = "sasurahime",
    version = env!("CARGO_PKG_VERSION"),
    about = "macOS developer cache cleaner",
    help_template = "\n{name} {version}\n{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}\n"
)]
struct Cli {
    /// Non-interactive: clean all pruneable caches without prompting
    #[arg(long)]
    yes: bool,
    /// Move deleted files to Trash instead of permanent removal (default: enabled)
    #[arg(long)]
    permanent: bool,

    /// Suppress per-entry progress output (spinner only)
    #[arg(long)]
    suppress: bool,

    /// Suppress all output including spinner
    #[arg(long)]
    deep_suppress: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan cache locations and report sizes
    Scan,
    /// Clean a specific cache target
    Clean {
        #[command(subcommand)]
        target: CleanTarget,
    },
    /// List cache targets
    Targets,
}

// Manual CleanTarget impl — handles special variants not generated by the macro.
impl CleanTarget {
    fn command_name(&self) -> &'static str {
        match self {
            CleanTarget::Caches { .. } => "caches",
            CleanTarget::Logs { .. } => "logs",
            CleanTarget::Xcode { .. } => "xcode",
            CleanTarget::Trash { .. } => "trash",
            CleanTarget::Ollama { .. } => "ollama",
            CleanTarget::LibraryLogs { .. } => "library-logs",
            CleanTarget::DeviceSupport { .. } => "device-support",
            CleanTarget::IosBackup { .. } => "ios-backup",
            CleanTarget::ApfsSnapshot { .. } => "apfs-snapshot",
            _ => self.dispatch_command_name(),
        }
    }

    fn dry_run(&self) -> bool {
        match self {
            CleanTarget::Caches { dry_run }
            | CleanTarget::Xcode { dry_run }
            | CleanTarget::Trash { dry_run }
            | CleanTarget::Ollama { dry_run }
            | CleanTarget::LibraryLogs { dry_run, .. } => *dry_run,
            CleanTarget::Logs { dry_run, .. } => *dry_run,
            CleanTarget::DeviceSupport { dry_run, .. } => *dry_run,
            CleanTarget::IosBackup { dry_run } | CleanTarget::ApfsSnapshot { dry_run } => *dry_run,
            _ => self.dispatch_dry_run(),
        }
    }
}

// Manual SUPPORTED_TARGETS entries for special targets not in the macro table.
fn extra_targets() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "caches",
            "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)",
        ),
        ("logs", "Log files older than N days"),
        ("xcode", "Xcode DerivedData project directories"),
        ("trash", "~/.Trash size (scan only)"),
        ("ollama", "Ollama model cache"),
        (
            "library-logs",
            "Analyze and clean ~/Library/Logs/ with heuristic recommendations",
        ),
        ("device-support", "Xcode DeviceSupport old version cleanup"),
        (
            "ios-backup",
            "iOS device backups (irreversible — backed up to Trash)",
        ),
        (
            "apfs-snapshot",
            "APFS local Time Machine snapshots (tmutil deletelocalsnapshot)",
        ),
    ]
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().expect("cannot determine HOME directory"))
}

fn all_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>> {
    vec![
        // Sprint 1
        Box::new(cleaners::uv::UvCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::brew::BrewCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        // Sprint 2
        Box::new(cleaners::mise::MiseCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::browser::BrowserCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        // Sprint 3 — logs / xcode (added by Tasks 3 and 4)
        Box::new(cleaners::xcode::XcodeCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::log::LogCleaner::new_with_extra(
            home,
            config.logs_keep_days,
            config
                .logs_extra_targets
                .iter()
                .map(|t| cleaners::log::OwnedLogTarget {
                    name: t.name.clone(),
                    path: config::Config::expand_tilde(&t.path, home),
                    exclude: t.exclude.clone(),
                })
                .collect(),
        )),
        // Sprint 5 — act / huggingface / pre-commit
        Box::new(cleaners::generic::GenericCleaner::act(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::huggingface::HuggingFaceCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::pre_commit::PreCommitCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        // Sprint 5 — library-logs
        Box::new(cleaners::library_logs::LibraryLogsCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::generic::GenericCleaner::colima_prune(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::ollama::OllamaCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::device_support::DeviceSupportCleaner::new(
            home,
            2,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::ios_backup::IosCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
            SystemCommandRunner,
        ))),
    ]
}

/// Runs a single-target clean with spinner and prints freed bytes.
fn run_clean_target<F>(
    label: &str,
    cleaner_fn: F,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
) -> anyhow::Result<()>
where
    F: FnOnce(bool, &dyn ProgressReporter) -> anyhow::Result<CleanResult>,
{
    let result = if reporter.show_spinner() {
        crate::progress::with_spinner_result(&format!("Cleaning {label}..."), || {
            cleaner_fn(dry_run, reporter)
        })?
    } else {
        cleaner_fn(dry_run, reporter)?
    };

    if reporter.show_spinner() {
        if crate::trash::is_trash_mode() && result.bytes_freed > 0 {
            println!(
                "Freed: 0 B ({} moved to Trash)",
                crate::format::format_bytes(result.bytes_freed)
            );
        } else {
            println!("Freed: {}", crate::format::format_bytes(result.bytes_freed));
        }
    }
    Ok(())
}

fn build_reporter(cli: &Cli, config: &Config) -> Box<dyn ProgressReporter> {
    let (suppress, deep_suppress) = merge_suppress_flags(
        cli.suppress,
        cli.deep_suppress,
        config.suppress,
        config.deep_suppress,
    );
    build_reporter_from_flags(suppress, deep_suppress)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    eprintln!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
    let home = home();

    let config_dir = home.join(".config/sasurahime");
    let config = match config::Config::load(&config_dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config: {e}");
            std::process::exit(1);
        }
    };
    let reporter = build_reporter(&cli, &config);

    // Default: trash mode enabled.
    // --permanent flag overrides to permanent deletion.
    // Config trash_mode = false also overrides to permanent deletion.
    let trash_mode = !cli.permanent && config.trash_mode;
    crate::trash::set_trash_mode(trash_mode);

    match cli.command {
        Some(Commands::Scan) => {
            let cleaners = all_cleaners(&home, &config);
            scanner::run_scan(&cleaners);
            let runner = SystemCommandRunner;
            let hints = hint::collect_hints(&home, &runner);
            hint::print_hints(&hints);
            hint::offer_auto_clean(&hints, &home, &runner, &hint::StdinPrompt);
        }
        Some(Commands::Targets) => {
            let mut targets: Vec<&(&str, &str)> =
                SUPPORTED_TARGETS.iter().chain(extra_targets()).collect();
            targets.sort_by_key(|(name, _)| *name);
            for (name, desc) in targets {
                println!("{:<12} {}", name, desc);
            }
        }
        None => {
            let cleaners = all_cleaners(&home, &config);
            if cli.yes {
                interactive::run_auto(&cleaners)?;
            } else {
                interactive::run_interactive(&cleaners)?;
            }
            let runner = SystemCommandRunner;
            let hints = hint::collect_hints(&home, &runner);
            hint::print_hints(&hints);
            hint::offer_auto_clean(&hints, &home, &runner, &hint::StdinPrompt);
        }
        Some(Commands::Clean { target }) => {
            if matches!(
                target,
                CleanTarget::Caches { .. }
                    | CleanTarget::Logs { .. }
                    | CleanTarget::Xcode { .. }
                    | CleanTarget::Trash { .. }
                    | CleanTarget::Ollama { .. }
                    | CleanTarget::LibraryLogs { .. }
                    | CleanTarget::DeviceSupport { .. }
                    | CleanTarget::IosBackup { .. }
                    | CleanTarget::ApfsSnapshot { .. }
            ) {
                // --- Special targets (custom dispatch logic) ---
                match target {
                    CleanTarget::Caches { dry_run } => {
                        let caches: Vec<Box<dyn cleaner::Cleaner>> = vec![
                            Box::new(cleaners::generic::GenericCleaner::bun(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::GenericCleaner::go(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::GenericCleaner::pip(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::GenericCleaner::node_gyp(
                                &home,
                                Box::new(SystemCommandRunner),
                            )),
                            Box::new(cleaners::generic::GenericCleaner::npm(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::GenericCleaner::yarn(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::GenericCleaner::pnpm(Box::new(
                                SystemCommandRunner,
                            ))),
                        ];
                        let mut total: u64 = 0;
                        for c in &caches {
                            match crate::progress::with_spinner_result(
                                &format!("Cleaning {}...", c.name()),
                                || c.clean(dry_run, reporter.as_ref()),
                            ) {
                                Ok(r) => total += r.bytes_freed,
                                Err(e) => eprintln!("Error cleaning {}: {e}", c.name()),
                            }
                        }
                        println!("Total freed: {}", format::format_bytes(total));
                    }
                    CleanTarget::Logs { dry_run, keep_days } => {
                        let days = keep_days.unwrap_or(config.logs_keep_days);
                        let extra: Vec<cleaners::log::OwnedLogTarget> = config
                            .logs_extra_targets
                            .iter()
                            .map(|t| cleaners::log::OwnedLogTarget {
                                name: t.name.clone(),
                                path: config::Config::expand_tilde(&t.path, &home),
                                exclude: t.exclude.clone(),
                            })
                            .collect();
                        run_clean_target(
                            "logs",
                            |dry, rep| {
                                cleaners::log::LogCleaner::new_with_extra(&home, days, extra)
                                    .clean(dry, rep)
                            },
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::Xcode { dry_run } => {
                        let xcode_cleaner = cleaners::xcode::XcodeCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        if cli.yes && xcode_cleaner.is_xcode_running() {
                            eprintln!("Note: Xcode is running. Proceeding with --yes anyway.");
                        }
                        run_clean_target(
                            "xcode",
                            |dry, rep| xcode_cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::Trash { dry_run } => {
                        let cleaner = cleaners::generic::GenericCleaner::trash(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        if dry_run {
                            let result = cleaner.clean(true, reporter.as_ref())?;
                            println!("Freed: {}", format::format_bytes(result.bytes_freed));
                        } else {
                            eprintln!("Warning: Use Finder to empty Trash. sasurahime cannot safely empty ~/.Trash.");
                            println!("Freed: 0 B");
                        }
                    }
                    CleanTarget::Ollama { dry_run } => {
                        let cleaner = cleaners::ollama::OllamaCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        run_clean_target(
                            "ollama",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::LibraryLogs { dry_run, all } => {
                        let cleaner = cleaners::library_logs::LibraryLogsCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        if all {
                            run_clean_target(
                                "library-logs",
                                move |dry, rep| cleaner.clean_all(dry, rep),
                                dry_run,
                                reporter.as_ref(),
                            )?;
                        } else {
                            run_clean_target(
                                "library-logs",
                                move |dry, rep| cleaner.clean(dry, rep),
                                dry_run,
                                reporter.as_ref(),
                            )?;
                        }
                    }
                    CleanTarget::DeviceSupport { dry_run, keep } => {
                        let cleaner = cleaners::device_support::DeviceSupportCleaner::new(
                            &home,
                            keep,
                            Box::new(SystemCommandRunner),
                        );
                        run_clean_target(
                            "device-support",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::IosBackup { dry_run } => {
                        let cleaner = cleaners::ios_backup::IosCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        run_clean_target(
                            "ios-backup",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::ApfsSnapshot { dry_run } => {
                        let cleaner = cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
                            SystemCommandRunner,
                        ));
                        run_clean_target(
                            "apfs-snapshot",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    _ => unreachable!(),
                }
            } else {
                // --- Standard targets: dispatch through macro-generated handler ---
                run_clean_target(
                    target.command_name(),
                    |dry, rep| dispatch_clean(&home, &config, &target, dry, rep),
                    target.dry_run(),
                    reporter.as_ref(),
                )?;
            }
        }
    }

    Ok(())
}
