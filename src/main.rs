mod cleaner;
mod cleaners;
mod command;
mod config;
mod context;
mod explorer;
mod format;
mod hint;
mod history;
mod interactive;
mod progress;
mod scanner;
mod trash;

#[cfg(test)]
pub(crate) mod test_helpers;

use clap::{Parser, Subcommand};
use cleaner::{CleanCancelled, CleanResult, Cleaner};
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
                    $( CleanTarget::$variant { .. } => $cli_name, )*
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
    (|home, config| cleaners::generic::GenericCleaner::act(home, Box::new(SystemCommandRunner)).with_config(config)),

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
    (|home, config| cleaners::generic::GenericCleaner::node_gyp(home, Box::new(SystemCommandRunner)).with_config(config)),

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
    (|home, config| cleaners::generic::GenericCleaner::spm_cache(home, Box::new(SystemCommandRunner)).with_config(config)),

    Conda : "conda" => "Conda clean --all";
    (|_home, _config| cleaners::generic::GenericCleaner::conda(Box::new(SystemCommandRunner))),

    Poetry : "poetry" => "Poetry cache clear --all";
    (|_home, _config| cleaners::generic::GenericCleaner::poetry(Box::new(SystemCommandRunner))),

    Pipx : "pipx" => "pipx cache and unused packages";
    (|_home, _config| cleaners::generic::GenericCleaner::pipx(Box::new(SystemCommandRunner))),

    Deno : "deno" => "Deno cache reload";
    (|_home, _config| cleaners::generic::GenericCleaner::deno(Box::new(SystemCommandRunner))),

    Dotnet : "dotnet" => ".NET NuGet cache clear";
    (|_home, _config| cleaners::generic::GenericCleaner::dotnet(Box::new(SystemCommandRunner))),

    Gem : "gem" => "RubyGems old gem versions cleanup";
    (|_home, _config| cleaners::generic::GenericCleaner::gem(Box::new(SystemCommandRunner))),

    Bundle : "bundle" => "Bundler cache clean";
    (|_home, _config| cleaners::generic::GenericCleaner::bundle(Box::new(SystemCommandRunner))),

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
    (|home, config| cleaners::generic::GenericCleaner::downloads(home, Box::new(SystemCommandRunner)).with_config(config)),

    VscodeExtensions : "vscode-extensions" => "VS Code extensions cache";
    (|home, config| cleaners::generic::GenericCleaner::vscode_extensions(home, Box::new(SystemCommandRunner)).with_config(config)),

    Maven : "maven" => "Maven local repository (mvn dependency:purge-local-repository)";
    (|home, _config| cleaners::generic::GenericCleaner::maven(home, Box::new(SystemCommandRunner))),

    Terraform : "terraform" => "Terraform provider plugin cache";
    (|home, config| cleaners::generic::GenericCleaner::terraform(home, Box::new(SystemCommandRunner)).with_config(config)),

    Flutter : "flutter" => "Flutter/Dart pub cache (dart pub cache clean)";
    (|home, _config| cleaners::generic::GenericCleaner::flutter(home, Box::new(SystemCommandRunner))),

    Volta : "volta" => "Volta Node.js manager cache";
    (|home, config| cleaners::generic::GenericCleaner::volta(home, Box::new(SystemCommandRunner)).with_config(config)),

    Sbt : "sbt" => "Scala/sbt build cache and Ivy cache";
    (|home, config| cleaners::generic::GenericCleaner::sbt(home, Box::new(SystemCommandRunner)).with_config(config)),

    TreeSitter : "tree-sitter" => "tree-sitter parser compilation cache";
    (|home, config| cleaners::generic::GenericCleaner::tree_sitter(home, Box::new(SystemCommandRunner)).with_config(config)),

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
    /// Clean Xcode caches (DerivedData, Archives)
    Xcode {
        #[arg(long)]
        dry_run: bool,
        /// Subcategories to clean: derived-data, archives (default: all)
        #[arg(long, value_delimiter = ',')]
        sub: Option<Vec<String>>,
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
    /// Permanently delete files instead of moving to Trash.
    /// By default, files are moved to macOS Trash for safety.
    #[arg(long)]
    permanent: bool,

    /// Suppress per-entry progress output (spinner only)
    #[arg(long)]
    suppress: bool,

    /// Suppress all output including spinner
    #[arg(long)]
    deep_suppress: bool,

    /// Print detailed file/dir and command output for each operation
    #[arg(long, global = true)]
    verbose: bool,

    /// Dry run: show what would be cleaned without deleting anything
    #[arg(long, global = true)]
    dry_run: bool,

    /// Path to config file (default: ~/.config/sasurahime/config.toml)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Disable Unicode box-drawing characters in stats output
    #[arg(long, global = true)]
    no_unicode: bool,

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
    /// Explore disk usage by app — discover and act (OmniDiskSweeper-style)
    Explore {
        /// Show top N largest entries per section (default: 20)
        #[arg(long, default_value = "20")]
        top: usize,
        /// Show all entries (overrides --top)
        #[arg(long)]
        all: bool,
        /// Scan this root directory instead of defaults (repeatable)
        #[arg(long = "dir", value_name = "PATH")]
        dirs: Vec<PathBuf>,
    },
    /// Show deletion history and statistics
    Stats {
        /// Show only the last N entries
        #[arg(long, default_value = "10")]
        last: usize,
    },
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
            | CleanTarget::Xcode { dry_run, .. }
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
    // Apply per-cleaner `older_than_days` for logs if set; fall back to
    // `config.logs_keep_days` (which itself defaults to 7).
    let logs_keep_days = config.effective_logs_keep_days();

    let mut cleaners: Vec<Box<dyn cleaner::Cleaner>> = vec![
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
            logs_keep_days,
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
        Box::new(
            cleaners::generic::GenericCleaner::act(home, Box::new(SystemCommandRunner))
                .with_config(config),
        ),
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
        Box::new(
            cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner))
                .with_config(config),
        ),
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
    ];
    // Apply exclude filter
    cleaners.retain(|c| !config.exclude.iter().any(|e| e == c.name()));
    // Add custom cleaners (always included since user defined them)
    for ct in &config.custom {
        let path = config::Config::expand_tilde(&ct.path, home);
        cleaners.push(Box::new(cleaners::custom::CustomPathCleaner::new(
            ct.name.clone(),
            path,
        )));
    }
    cleaners
}

/// Runs a single-target clean and prints freed bytes.
fn run_clean_target<F>(
    label: &str,
    cleaner_fn: F,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
) -> anyhow::Result<CleanResult>
where
    F: FnOnce(bool, &dyn ProgressReporter) -> anyhow::Result<CleanResult>,
{
    let msg = format!("Cleaning {label}...");
    let result = if reporter.show_spinner() {
        // Static message (no spinner animation) so the confirmation prompt
        // inside the cleaner is not competing with a spinner tick.
        eprint!("{msg}");
        let r = cleaner_fn(dry_run, reporter);
        match r {
            Ok(v) => {
                eprintln!(" [OK]");
                v
            }
            Err(e) if e.is::<CleanCancelled>() => {
                // User cancelled — clean shutdown, no [FAILED], no Freed line.
                // The hint was already printed in GenericCleaner.
                return Ok(CleanResult {
                    name: "",
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                });
            }
            Err(e) => {
                eprintln!(" [FAILED]");
                return Err(e);
            }
        }
    } else {
        let r = cleaner_fn(dry_run, reporter);
        match r {
            Ok(v) => v,
            Err(e) if e.is::<CleanCancelled>() => {
                return Ok(CleanResult {
                    name: "",
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                })
            }
            Err(e) => return Err(e),
        }
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

    if !result.skipped.is_empty() {
        eprintln!(
            "{} file(s) skipped due to errors (permission/lock):",
            result.skipped.len()
        );
        for entry in &result.skipped {
            eprintln!("  - {}: {}", entry.path.display(), entry.reason);
        }
    }

    if !dry_run && result.bytes_freed > 0 {
        history::write_history_entry(label, result.bytes_freed, result.skipped.len());
    }

    Ok(result)
}

/// Wraps `run_clean_target` and exits with code 1 if `CleanResult.exit_code() != 0`.
fn run_and_exit<F>(
    label: &str,
    cleaner_fn: F,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
) -> anyhow::Result<CleanResult>
where
    F: FnOnce(bool, &dyn ProgressReporter) -> anyhow::Result<CleanResult>,
{
    let result = run_clean_target(label, cleaner_fn, dry_run, reporter)?;
    if result.exit_code() != 0 {
        std::process::exit(1);
    }
    Ok(result)
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
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp(None)
        .init();

    let cli = Cli::parse();
    log::info!("sasurahime v{} starting", env!("CARGO_PKG_VERSION"));
    eprintln!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
    let home = home();

    let config = match &cli.config {
        Some(path) => config::Config::load_from_path(path),
        None => {
            let config_dir = home.join(".config/sasurahime");
            config::Config::load(&config_dir)
        }
    };
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading config: {e}");
            std::process::exit(1);
        }
    };
    let reporter = build_reporter(&cli, &config);

    // Set global context flags before any subcommand runs.
    crate::context::set_verbose(cli.verbose);
    crate::context::set_dry_run(cli.dry_run);

    // Default: trash mode enabled.
    // --permanent flag overrides to permanent deletion.
    // Config trash_mode = false also overrides to permanent deletion.
    let trash_mode = !cli.permanent && config.trash_mode;
    crate::trash::set_trash_mode(trash_mode);

    if cli.no_unicode {
        crate::history::USE_UNICODE.store(false, std::sync::atomic::Ordering::Relaxed);
    }

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
        Some(Commands::Explore { top, all, dirs }) => {
            let roots = if dirs.is_empty() {
                explorer::default_roots(&home)
            } else {
                dirs
            };
            let top_n = if all { None } else { Some(top) };
            explorer::run_explore(
                &home,
                explorer::ExploreOptions {
                    roots,
                    top: top_n,
                    dry_run: cli.dry_run,
                },
            )?;
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
                            Box::new(
                                cleaners::generic::GenericCleaner::node_gyp(
                                    &home,
                                    Box::new(SystemCommandRunner),
                                )
                                .with_config(&config),
                            ),
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
                                Ok(r) => {
                                    total += r.bytes_freed;
                                    if !dry_run && r.bytes_freed > 0 {
                                        history::write_history_entry(
                                            c.name(),
                                            r.bytes_freed,
                                            r.skipped.len(),
                                        );
                                    }
                                }
                                Err(e) => eprintln!("Error cleaning {}: {e}", c.name()),
                            }
                        }
                        println!("Total freed: {}", format::format_bytes(total));
                    }
                    CleanTarget::Logs { dry_run, keep_days } => {
                        let days = keep_days.unwrap_or_else(|| config.effective_logs_keep_days());
                        let extra: Vec<cleaners::log::OwnedLogTarget> = config
                            .logs_extra_targets
                            .iter()
                            .map(|t| cleaners::log::OwnedLogTarget {
                                name: t.name.clone(),
                                path: config::Config::expand_tilde(&t.path, &home),
                                exclude: t.exclude.clone(),
                            })
                            .collect();
                        run_and_exit(
                            "logs",
                            |dry, rep| {
                                cleaners::log::LogCleaner::new_with_extra(&home, days, extra)
                                    .clean(dry, rep)
                            },
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::Xcode { dry_run, sub } => {
                        let mut xcode_cleaner = cleaners::xcode::XcodeCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        if let Some(ref subs) = sub {
                            let parsed: Vec<_> = subs
                                .iter()
                                .filter_map(|s| cleaners::xcode::XcodeSubcategory::from_str(s))
                                .collect();
                            if !parsed.is_empty() {
                                xcode_cleaner = xcode_cleaner.with_subcategories(parsed);
                            }
                        }
                        if cli.yes && xcode_cleaner.is_xcode_running() {
                            eprintln!("Note: Xcode is running. Proceeding with --yes anyway.");
                        }
                        run_and_exit(
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
                        run_and_exit(
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
                        let opts = crate::cleaner::CleanOptions { all };
                        run_and_exit(
                            "library-logs",
                            move |dry, rep| cleaner.clean_with_opts(dry, rep, &opts),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::DeviceSupport { dry_run, keep } => {
                        let cleaner = cleaners::device_support::DeviceSupportCleaner::new(
                            &home,
                            keep,
                            Box::new(SystemCommandRunner),
                        );
                        run_and_exit(
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
                        run_and_exit(
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
                        run_and_exit(
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
                run_and_exit(
                    target.command_name(),
                    |dry, rep| dispatch_clean(&home, &config, &target, dry, rep),
                    target.dry_run(),
                    reporter.as_ref(),
                )?;
            }
        }
        Some(Commands::Stats { last }) => {
            let history_dir = history::default_history_dir(&home);
            let entries = history::load_history(&history_dir.join("history.json"));
            if entries.is_empty() {
                println!("No history yet. Run 'sasurahime clean' to get started.");
            } else {
                let summary = history::compute_stats(&entries);
                if last > 0 && last < entries.len() {
                    println!("{}", history::format_stats_last(&summary, last));
                } else {
                    println!("{}", history::format_stats(&summary));
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::CleanCancelled;

    #[test]
    fn run_clean_target_cancelled_returns_ok_zero() {
        let reporter = crate::progress::VerboseProgress::new();
        let result = run_clean_target(
            "cancelled-test",
            |_dry_run, _reporter| Err(anyhow::Error::from(CleanCancelled)),
            false,
            &reporter,
        );
        assert!(result.is_ok(), "CleanCancelled should be converted to Ok");
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 0);
        assert!(clean_result.skipped.is_empty());
    }

    #[test]
    fn run_clean_target_skipped_entries_do_not_panic() {
        let reporter = crate::progress::VerboseProgress::new();
        let result = run_clean_target(
            "skipped-test",
            |_dry_run, _reporter| {
                Ok(CleanResult {
                    name: "mock-cleaner",
                    bytes_freed: 100,
                    uses_trash: false,
                    skipped: vec![crate::cleaner::SkippedEntry {
                        path: std::path::PathBuf::from("/tmp/skipped-file"),
                        reason: "Permission denied".to_string(),
                    }],
                    deleted_paths: vec![],
                })
            },
            false,
            &reporter,
        );
        assert!(result.is_ok());
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 100);
        assert_eq!(clean_result.skipped.len(), 1);
    }
}
