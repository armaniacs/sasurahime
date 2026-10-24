use sasurahime::{
    batch, cleaner, cleaners, command, config, doctor, explorer, format, hint, history,
    interactive, progress, scan_cache, scanner,
};

use clap::{Parser, Subcommand};
use cleaner::{CleanCancelled, CleanResult, Cleaner};
use command::SystemCommandRunner;
use config::Config;
use progress::{build_reporter_from_flags, merge_suppress_flags, ProgressReporter};
use std::path::PathBuf;

/// Macro that generates the CleanTarget enum, SUPPORTED_TARGETS, the dispatch
/// helpers and standard_cleaners() from a single definition table.
/// all_cleaners() composes standard_cleaners() with special_cleaners().
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

        fn standard_cleaners(
            home: &std::path::Path,
            config: &config::Config,
        ) -> Vec<Box<dyn cleaner::Cleaner>> {
            vec![
                $( Box::new(($factory)(home, config)) as Box<dyn cleaner::Cleaner>, )*
            ]
        }

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
    (|home, config| cleaners::generic::PathCleaner::act(home, Box::new(SystemCommandRunner)).with_config(config)),

    Uv : "uv" => "Stale simple-vN index directories + uv cache prune";
    (|home, _config| cleaners::uv::UvCleaner::new(home, Box::new(SystemCommandRunner))),

    Brew : "brew" => "Homebrew download cache";
    (|home, _config| cleaners::brew::BrewCleaner::new(home, Box::new(SystemCommandRunner))),

    Mise : "mise" => "Unused runtime versions";
    (|home, _config| cleaners::mise::MiseCleaner::new(home, Box::new(SystemCommandRunner))),

    Browsers : "browsers" => "Old Puppeteer / Playwright builds";
    (|home, _config| cleaners::browser::BrowserCleaner::new(home, Box::new(SystemCommandRunner))),

    Bun : "bun" => "Bun package cache";
    (|_home, _config| cleaners::generic::CliCleaner::bun(Box::new(SystemCommandRunner))),

    Go : "go" => "Go build cache";
    (|_home, _config| cleaners::generic::CliCleaner::go(Box::new(SystemCommandRunner))),

    Pip : "pip" => "pip package cache";
    (|_home, _config| cleaners::generic::CliCleaner::pip(Box::new(SystemCommandRunner))),

    NodeGyp : "node-gyp" => "node-gyp build cache directories";
    (|home, config| cleaners::generic::PathCleaner::node_gyp(home, Box::new(SystemCommandRunner)).with_config(config)),

    Npm : "npm" => "npm package cache";
    (|_home, _config| cleaners::generic::CliCleaner::npm(Box::new(SystemCommandRunner))),

    Yarn : "yarn" => "yarn cache";
    (|_home, _config| cleaners::generic::CliCleaner::yarn(Box::new(SystemCommandRunner))),

    Pnpm : "pnpm" => "pnpm store";
    (|_home, _config| cleaners::generic::CliCleaner::pnpm(Box::new(SystemCommandRunner))),

    Cargo : "cargo" => "Cargo registry cache + target/ directories";
    (|home, _config| cleaners::cargo::CargoCleaner::new(home, Box::new(SystemCommandRunner))),

    Docker : "docker" => "Docker system prune (images, containers, build cache)";
    (|home, _config| cleaners::generic::CliCleaner::docker(home, Box::new(SystemCommandRunner))),

    Orbstack : "orbstack" => "Orbstack prune";
    (|_home, _config| cleaners::generic::CliCleaner::orbstack(Box::new(SystemCommandRunner))),

    CocoaPods : "cocoapods" => "CocoaPods cache clean --all";
    (|_home, _config| cleaners::generic::CliCleaner::cocoapods(Box::new(SystemCommandRunner))),

    Colima : "colima" => "Colima VM disk images (inactive) prune";
    (|home, _config| cleaners::generic::CliCleaner::colima_prune(home, Box::new(SystemCommandRunner))),

    SwiftPM : "spm" => "SwiftPM cache directory";
    (|home, config| cleaners::generic::PathCleaner::spm_cache(home, Box::new(SystemCommandRunner)).with_config(config)),

    Conda : "conda" => "Conda clean --all";
    (|_home, _config| cleaners::generic::CliCleaner::conda(Box::new(SystemCommandRunner))),

    Poetry : "poetry" => "Poetry cache clear --all";
    (|_home, _config| cleaners::generic::CliCleaner::poetry(Box::new(SystemCommandRunner))),

    Pipx : "pipx" => "pipx cache and unused packages";
    (|_home, _config| cleaners::generic::CliCleaner::pipx(Box::new(SystemCommandRunner))),

    Deno : "deno" => "Deno cache reload";
    (|_home, _config| cleaners::generic::CliCleaner::deno(Box::new(SystemCommandRunner))),

    Dotnet : "dotnet" => ".NET NuGet cache clear";
    (|_home, _config| cleaners::generic::CliCleaner::dotnet(Box::new(SystemCommandRunner))),

    Gem : "gem" => "RubyGems old gem versions cleanup";
    (|_home, _config| cleaners::generic::CliCleaner::gem(Box::new(SystemCommandRunner))),

    Bundle : "bundle" => "Bundler cache clean";
    (|_home, _config| cleaners::generic::CliCleaner::bundle(Box::new(SystemCommandRunner))),

    Rustup : "rustup" => "Unused Rust toolchains";
    (|home, _config| cleaners::rustup::RustupCleaner::new(home, Box::new(SystemCommandRunner))),

    Simulator : "simulator" => "iOS Simulator cache (xcrun simctl delete unavailable)";
    (|home, _config| cleaners::generic::CliCleaner::simulator(home, Box::new(SystemCommandRunner))),

    Gradle : "gradle" => "Gradle old version caches";
    (|home, _config| cleaners::gradle::GradleCleaner::new(home, Box::new(SystemCommandRunner))),

    Huggingface : "huggingface" => "Hugging Face model cache (hub/)";
    (|home, _config| cleaners::huggingface::HuggingFaceCleaner::new(home, Box::new(SystemCommandRunner))),

    PreCommit : "pre-commit" => "pre-commit hook environment cache";
    (|home, _config| cleaners::pre_commit::PreCommitCleaner::new(home, Box::new(SystemCommandRunner))),

    JetBrains : "jetbrains" => "JetBrains IDE caches (old versions)";
    (|home, _config| cleaners::gradle::JetBrainsCleaner::new(home, Box::new(SystemCommandRunner))),

    Downloads : "downloads" => "~/Downloads old files";
    (|home, config| cleaners::generic::PathCleaner::downloads(home, Box::new(SystemCommandRunner)).with_config(config)),

    VscodeExtensions : "vscode-extensions" => "VS Code extensions cache";
    (|home, config| cleaners::generic::PathCleaner::vscode_extensions(home, Box::new(SystemCommandRunner)).with_config(config)),

    Maven : "maven" => "Maven local repository (mvn dependency:purge-local-repository)";
    (|home, _config| cleaners::generic::CliCleaner::maven(home, Box::new(SystemCommandRunner))),

    Terraform : "terraform" => "Terraform provider plugin cache";
    (|home, config| cleaners::generic::PathCleaner::terraform(home, Box::new(SystemCommandRunner)).with_config(config)),

    Flutter : "flutter" => "Flutter/Dart pub cache (dart pub cache clean)";
    (|home, _config| cleaners::generic::CliCleaner::flutter(home, Box::new(SystemCommandRunner))),

    Volta : "volta" => "Volta Node.js manager cache";
    (|home, config| cleaners::generic::PathCleaner::volta(home, Box::new(SystemCommandRunner)).with_config(config)),

    Sbt : "sbt" => "Scala/sbt build cache and Ivy cache";
    (|home, config| cleaners::generic::PathCleaner::sbt(home, Box::new(SystemCommandRunner)).with_config(config)),

    TreeSitter : "tree-sitter" => "tree-sitter parser compilation cache";
    (|home, config| cleaners::generic::PathCleaner::tree_sitter(home, Box::new(SystemCommandRunner)).with_config(config)),

    OrphanContainers : "orphan-containers" => "Leftover Containers / Group Containers of uninstalled apps (interactive)";
    (|home, _config| cleaners::orphan_containers::OrphanContainersCleaner::new(home, Box::new(SystemCommandRunner))),

    LibraryCaches : "library-caches" => "Per-app ~/Library/Caches/ listing; known app caches removable (interactive)";
    (|home, _config| cleaners::library_caches::LibraryCachesCleaner::new(home, Box::new(SystemCommandRunner))),

    AppSupport : "app-support" => "Cache dirs of known apps in ~/Library/Application Support (interactive)";
    (|home, _config| cleaners::app_support::AppSupportCleaner::new(home, Box::new(SystemCommandRunner))),

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
        /// Delete every local snapshot without prompting
        #[arg(long)]
        all: bool,
        /// Thin local snapshots toward this free-space target (e.g. 10GB,
        /// decimal bytes as tmutil expects)
        #[arg(long)]
        thin: Option<String>,
        /// Thinning urgency 1..=4 (default 2; 4 interrupts running backups
        /// and needs an extra confirmation)
        #[arg(long)]
        urgency: Option<u32>,
    },
    /// Report /private/var/vm swap files (no-op; re-initialized on reboot)
    #[command(name = "vm-swap")]
    VmSwap {
        #[arg(long)]
        dry_run: bool,
    },
    /// Explicit full clean: Safe targets by default, everything with
    /// --include-manual. Honors global --older-than / --larger-than filters.
    All {
        #[arg(long)]
        dry_run: bool,
        /// Also clean Moderate / Risky / Manual targets
        #[arg(long)]
        include_manual: bool,
    },
    /// Purge stale project artifacts (node_modules / target / .build)
    #[command(name = "project-artifacts")]
    ProjectArtifacts {
        #[arg(long)]
        dry_run: bool,
    },
    /// Catch-all for external plugin targets (sasurahime-* on PATH)
    #[command(external_subcommand)]
    External(Vec<String>),
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

    /// Disable the persistent scan cache (always full walks)
    #[arg(long, global = true)]
    no_cache: bool,

    /// Global age filter in days (clean --all): only entries older than this
    #[arg(long, global = true)]
    older_than: Option<u32>,

    /// Global size filter in MB (clean --all): only dirs of at least this size
    #[arg(long, global = true)]
    larger_than: Option<u64>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan cache locations and report sizes
    Scan {
        /// Output machine-readable JSON instead of the table (no hints, no prompts)
        #[arg(long)]
        json: bool,
    },
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
    #[command(visible_alias = "history")]
    Stats {
        /// Show only the last N entries
        #[arg(long, default_value = "10")]
        last: usize,
    },
    /// Diagnose Full Disk Access (FDA) problems before scanning
    Doctor,
    /// Unattended policy apply: cleans only what the [policy] section allows.
    /// Irreversible (Risky) targets never run unattended.
    Auto {
        /// Show what would be cleaned without deleting anything
        #[arg(long)]
        dry_run: bool,
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
            CleanTarget::VmSwap { .. } => "vm-swap",
            CleanTarget::All { .. } => "all",
            CleanTarget::ProjectArtifacts { .. } => "project-artifacts",
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
            CleanTarget::IosBackup { dry_run } => *dry_run,
            CleanTarget::ApfsSnapshot { dry_run, .. } => *dry_run,
            CleanTarget::VmSwap { dry_run } => *dry_run,
            CleanTarget::All { dry_run, .. } => *dry_run,
            CleanTarget::ProjectArtifacts { dry_run } => *dry_run,
            _ => self.dispatch_dry_run(),
        }
    }
}

// Manual SUPPORTED_TARGETS entries for special targets not in the macro table
// (shared with the library's correspondence-table tests).
fn extra_targets() -> &'static [(&'static str, &'static str)] {
    crate::cleaner::extra_targets()
}

fn home() -> PathBuf {
    // An EMPTY HOME is filtered exactly like the tripwire's empty filter
    // (is_safe_delete_target): PathBuf::from("") would silently resolve
    // every cache/history path against the process CWD.
    std::env::var("HOME")
        .ok()
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .expect("cannot determine HOME directory")
}

/// Every cleaner whose `risk()` is not `Safe` is wrapped in `ManualOnly` so it
/// runs only when selected in the TUI or via `clean <target>`. There is no
/// central list: the classification on each cleaner decides.
/// Expands `[project-artifacts]` roots (tilde-aware). Defaults apply when the
/// config leaves roots empty.
fn project_artifact_roots(home: &std::path::Path, config: &config::Config) -> Vec<PathBuf> {
    if config.project_artifacts.roots.is_empty() {
        config::ProjectArtifactsConfig::default().roots
    } else {
        config
            .project_artifacts
            .roots
            .iter()
            .map(|r| config::Config::expand_tilde(&r.to_string_lossy(), home))
            .collect()
    }
}

/// Builds the `logs` cleaner from the config's extra log targets. This is
/// the ONE factory for the logs target: the scan registry (`special_cleaners`)
/// and the `clean logs` arm both construct through it, so the keep-days
/// fallback and the extra-target mapping live in exactly one place.
/// `keep_days` overrides the config's effective keep-days (the `--keep-days`
/// CLI flag); `None` uses the config value.
fn log_cleaner_from_config(
    home: &std::path::Path,
    config: &config::Config,
    keep_days: Option<u32>,
) -> cleaners::log::LogCleaner {
    let days = keep_days.unwrap_or_else(|| config.effective_logs_keep_days());
    let extra: Vec<cleaners::log::OwnedLogTarget> = config
        .logs_extra_targets
        .iter()
        .map(|t| cleaners::log::OwnedLogTarget {
            name: t.name.clone(),
            path: config::Config::expand_tilde(&t.path, home),
            exclude: t.exclude.clone(),
        })
        .collect();
    cleaners::log::LogCleaner::new_with_extra(home, days, extra)
}

fn special_cleaners(
    home: &std::path::Path,
    config: &config::Config,
) -> Vec<Box<dyn cleaner::Cleaner>> {
    vec![
        Box::new(cleaners::xcode::XcodeCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(log_cleaner_from_config(home, config, None)),
        Box::new(cleaners::library_logs::LibraryLogsCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::ollama::OllamaCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        // keep=2 is the shared default: the `clean device-support` CLI flag
        // carries the same `default_value = "2"`, so the registry and the
        // CLI arm build identically unless --keep is passed explicitly.
        // detect() ignores keep; only the clean path uses it.
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
        Box::new(cleaners::vm_swap::VmSwapCleaner::new(
            std::path::PathBuf::from("/private/var/vm"),
            Box::new(SystemCommandRunner),
        )),
        Box::new(
            cleaners::project_artifacts::ProjectArtifactsCleaner::new(project_artifact_roots(
                home, config,
            ))
            .with_max_depth(config.project_artifacts.max_depth)
            .with_recency_days(config.project_artifacts.recency_days),
        ),
    ]
}

/// Built-in target names a plugin must never impersonate.
fn builtin_target_names() -> Vec<&'static str> {
    SUPPORTED_TARGETS
        .iter()
        .chain(extra_targets())
        .map(|(name, _)| *name)
        .collect()
}

fn all_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>> {
    let mut cleaners: Vec<Box<dyn cleaner::Cleaner>> = standard_cleaners(home, config)
        .into_iter()
        .chain(special_cleaners(home, config))
        .chain(
            cleaners::plugin::PluginCleaner::discover_excluding(&builtin_target_names())
                .into_iter()
                .map(|p| Box::new(p) as Box<dyn cleaner::Cleaner>),
        )
        .map(|c| -> Box<dyn cleaner::Cleaner> {
            if c.auto_clean_allowed() {
                c
            } else {
                Box::new(cleaners::manual_only::ManualOnly::new(c))
            }
        })
        .collect();
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

/// Message shown after a clean. `uses_trash` is the cleaner's own report: a
/// cleaner that deletes outside the Trash (tmutil, rustup, ...) must not claim
/// its bytes can be restored.
fn freed_message(bytes_freed: u64, uses_trash: bool, trash_mode: bool) -> String {
    if trash_mode && uses_trash && bytes_freed > 0 {
        // Two axes: the logical size is gone from the caches, but the bytes
        // still sit in the Trash until it is emptied.
        format!(
            "Logical freed: {} / Physical freed: 0 B (in Trash)",
            sasurahime::format::format_bytes(bytes_freed)
        )
    } else {
        format!("Freed: {}", sasurahime::format::format_bytes(bytes_freed))
    }
}

/// Cleaners that accept `[cleaner.<name>]` filters, keyed by `Cleaner::name()`.
fn filter_support(
    home: &std::path::Path,
    config: &config::Config,
) -> Vec<(String, cleaner::PerCleanerFilters)> {
    let mut support: Vec<(String, cleaner::PerCleanerFilters)> = standard_cleaners(home, config)
        .into_iter()
        .chain(special_cleaners(home, config))
        .map(|c| (c.name().to_string(), c.per_cleaner_filters()))
        .collect();
    for ct in &config.custom {
        support.push((ct.name.clone(), cleaner::PerCleanerFilters::None));
    }
    support
}

/// Lists `[cleaner.*]` settings that no cleaner will apply. Pure so the wording
/// can be tested without capturing stderr.
fn ineffective_per_cleaner_settings(
    config: &config::Config,
    support: &[(String, cleaner::PerCleanerFilters)],
) -> Vec<String> {
    let mut keys: Vec<&String> = config.per_cleaner.keys().collect();
    keys.sort();
    let mut out = Vec::new();
    for key in keys {
        let pcc = &config.per_cleaner[key];
        let Some((_, capability)) = support.iter().find(|(n, _)| n == key.as_str()) else {
            out.push(format!("[{key}] unknown cleaner name"));
            continue;
        };
        let mut fields = Vec::new();
        if pcc.older_than_days.is_some() && *capability == cleaner::PerCleanerFilters::None {
            fields.push("older_than_days");
        }
        if pcc.larger_than_mb.is_some() && *capability != cleaner::PerCleanerFilters::Both {
            fields.push("larger_than_mb");
        }
        if fields.is_empty() {
            continue;
        }
        let why = match capability {
            cleaner::PerCleanerFilters::None => "this cleaner has no per-cleaner filters",
            cleaner::PerCleanerFilters::OlderThanDays => "only older_than_days applies",
            cleaner::PerCleanerFilters::Both => "no effect",
        };
        out.push(format!("[{key}] {}: {why}", fields.join(", ")));
    }
    out
}

fn warn_ineffective_per_cleaner_settings(home: &std::path::Path, config: &config::Config) {
    let lines = ineffective_per_cleaner_settings(config, &filter_support(home, config));
    if lines.is_empty() {
        return;
    }
    eprintln!("[config] some [cleaner.*] settings have no effect:");
    for line in lines {
        eprintln!("  - {line}");
    }
}

/// Lists [[custom]] names that collide with a built-in target name. Pure so
/// the wording can be tested without capturing stderr.
fn colliding_custom_names(config: &config::Config) -> Vec<String> {
    let builtin = builtin_target_names();
    config
        .custom
        .iter()
        .filter(|ct| builtin.iter().any(|n| *n == ct.name))
        .map(|ct| ct.name.clone())
        .collect()
}

fn warn_custom_name_collisions(config: &config::Config) {
    let names = colliding_custom_names(config);
    if names.is_empty() {
        return;
    }
    eprintln!("[config] [[custom]] names collide with built-in targets:");
    for name in &names {
        eprintln!("  - [{name}] collides with built-in target; `clean {name}` runs the built-in");
    }
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
        // One shared spinner: each deletion updates its message with the
        // folder being removed, so the user sees what is happening. No
        // steady tick: an in-cleaner confirmation prompt must not compete
        // with animation, but ticks still redraw the line.
        let spinner = sasurahime::progress::SharedSpinner::start(&msg, false);
        let r = cleaner_fn(dry_run, &spinner.reporter());
        spinner.finish();
        match r {
            Ok(v) => {
                eprintln!("{msg} [OK]");
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
                eprintln!("{msg} [FAILED]");
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
        println!(
            "{}",
            freed_message(
                result.bytes_freed,
                result.uses_trash,
                sasurahime::trash::is_trash_mode()
            )
        );
        if let Some(note) = batch::post_clean_note(
            result.bytes_freed,
            result.uses_trash,
            sasurahime::trash::is_trash_mode(),
        ) {
            println!("{note}");
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

    // The single-target path keeps its own flow (CleanCancelled, per-target
    // freed_message) but funnels the history write through the one guard in
    // batch.rs, so `!dry_run && bytes_freed > 0` has exactly one owner.
    batch::write_history_entry_if_needed(
        label,
        dry_run,
        result.bytes_freed,
        result.skipped.len(),
        result.uses_trash,
    );

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
    if !config.per_cleaner.is_empty() {
        warn_ineffective_per_cleaner_settings(&home, &config);
    }
    warn_custom_name_collisions(&config);

    let reporter = build_reporter(&cli, &config);

    // Set global context flags before any subcommand runs.
    sasurahime::context::set_verbose(cli.verbose);

    // Default: trash mode enabled.
    // --permanent flag overrides to permanent deletion.
    // Config trash_mode = false also overrides to permanent deletion.
    let trash_mode = !cli.permanent && config.trash_mode;
    sasurahime::trash::set_trash_mode(trash_mode);

    if cli.no_unicode {
        format::set_use_unicode(false);
    }

    // Persistent scan cache: enabled unless --no-cache. Loaded once per
    // process and saved back at the end; deletion paths never consult it.
    let scan_cache_path = history::default_history_dir(&home).join(scan_cache::CACHE_FILE_NAME);
    if !cli.no_cache {
        scan_cache::init(&scan_cache_path);
    }

    match cli.command {
        Some(Commands::Doctor) => {
            doctor::run(&home);
        }
        Some(Commands::Auto { dry_run }) => {
            // No [policy] section: refuse to delete anything unattended.
            if !config.policy.present {
                eprintln!(
                    "Error: no [policy] section in ~/.config/sasurahime/config.toml. Write a policy (e.g. older_than_days = 30) before running `auto`."
                );
                std::process::exit(1);
            }
            let opts = cleaner::CleanOptions {
                all: true,
                include_manual: config.policy.include_manual,
                older_than_days: config.policy.older_than_days,
                larger_than_mb: config.policy.larger_than_mb,
                trash_mode: Some(sasurahime::trash::TrashMode::Global),
            };
            let cleaners_list = all_cleaners(&home, &config);
            let mut skipped: Vec<String> = Vec::new();
            // Skip policies stay at the call site: the wording is specific to
            // `auto`. Risky (destructive/irreversible) and Manual
            // (review-first) targets never run unattended, even with
            // include_manual: whole-dir removals must never happen without a
            // human.
            let mut batch_cleaners: Vec<&dyn cleaner::Cleaner> = Vec::new();
            for c in &cleaners_list {
                if matches!(c.risk(), cleaner::Risk::Risky | cleaner::Risk::Manual) {
                    skipped.push(format!(
                        "{} ({}): never runs unattended",
                        c.name(),
                        c.risk().label()
                    ));
                    continue;
                }
                // Moderate targets still honor [policy] include_manual;
                // only Safe runs without it.
                if !c.auto_clean_allowed()
                    && !(opts.include_manual && c.risk() == cleaner::Risk::Moderate)
                {
                    skipped.push(format!(
                        "{} ({}): set [policy] include_manual = true to include",
                        c.name(),
                        c.risk().label()
                    ));
                    continue;
                }
                batch_cleaners.push(c.as_ref());
            }
            let summary = batch::run_batch(
                &batch_cleaners,
                &opts,
                reporter.as_ref(),
                dry_run,
                false,
                true,
            );
            for (name, e) in &summary.failures {
                eprintln!("Error cleaning {name}: {e}");
            }
            if !summary.skipped.is_empty() {
                eprintln!(
                    "{} file(s) skipped due to errors (permission/lock):",
                    summary.skipped.len()
                );
                for (name, detail) in &summary.skipped {
                    eprintln!("  - {name}: {detail}");
                }
            }
            if !skipped.is_empty() {
                eprintln!("Skipped {} target(s):", skipped.len());
                for s in &skipped {
                    eprintln!("  - {s}");
                }
                eprintln!("Run `sasurahime clean <target>` explicitly for those.");
            }
            println!("Total freed: {}", format::format_bytes(summary.total_freed));
        }
        Some(Commands::Scan { json }) => {
            let cleaners = all_cleaners(&home, &config);
            if json {
                // Machine-readable output: no hints, no interactive prompt.
                scanner::run_scan_json(&cleaners, &home);
                // The early return below skips the trailing flush: warm the
                // cache here too, or scripts that only ever run --json would
                // never save detect results.
                if !cli.no_cache {
                    scan_cache::flush(&scan_cache_path);
                }
                return Ok(());
            }
            scanner::run_scan(&cleaners, &home);
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
            // Plugin-derived targets are marked so they can be told apart
            // from built-ins.
            for p in cleaners::plugin::PluginCleaner::discover_excluding(&builtin_target_names()) {
                println!("{:<12} (plugin)", p.name());
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
                    | CleanTarget::VmSwap { .. }
                    | CleanTarget::All { .. }
                    | CleanTarget::ProjectArtifacts { .. }
                    | CleanTarget::External(_)
            ) {
                // --- Special targets (custom dispatch logic) ---
                match target {
                    CleanTarget::Caches { dry_run } => {
                        let caches: Vec<Box<dyn cleaner::Cleaner>> = vec![
                            Box::new(cleaners::generic::CliCleaner::bun(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::CliCleaner::go(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::CliCleaner::pip(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(
                                cleaners::generic::PathCleaner::node_gyp(
                                    &home,
                                    Box::new(SystemCommandRunner),
                                )
                                .with_config(&config),
                            ),
                            Box::new(cleaners::generic::CliCleaner::npm(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::CliCleaner::yarn(Box::new(
                                SystemCommandRunner,
                            ))),
                            Box::new(cleaners::generic::CliCleaner::pnpm(Box::new(
                                SystemCommandRunner,
                            ))),
                        ];
                        // Bare batch: no post-clean notes (this entry point
                        // never printed them). Passing the default options is
                        // behavior-identical to the old bare `clean()` calls:
                        // every caches cleaner computes the same filters from
                        // its own fields when the options carry none.
                        let batch_cleaners: Vec<&dyn cleaner::Cleaner> =
                            caches.iter().map(|c| c.as_ref()).collect();
                        let summary = batch::run_batch(
                            &batch_cleaners,
                            &cleaner::CleanOptions::default(),
                            reporter.as_ref(),
                            dry_run,
                            false,
                            false,
                        );
                        for (name, e) in &summary.failures {
                            eprintln!("Error cleaning {name}: {e}");
                        }
                        if !summary.skipped.is_empty() {
                            eprintln!(
                                "{} file(s) skipped due to errors (permission/lock):",
                                summary.skipped.len()
                            );
                            for (name, detail) in &summary.skipped {
                                eprintln!("  - {name}: {detail}");
                            }
                        }
                        println!("Total freed: {}", format::format_bytes(summary.total_freed));
                    }
                    CleanTarget::Logs { dry_run, keep_days } => {
                        run_and_exit(
                            "logs",
                            |dry, rep| {
                                log_cleaner_from_config(&home, &config, keep_days).clean(dry, rep)
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
                                .filter_map(|s| cleaners::xcode::XcodeSubcategory::parse(s))
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
                        let cleaner = cleaners::generic::PathCleaner::trash(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        // Report-only in both modes: sasurahime never empties
                        // the Trash itself.
                        let _ = dry_run;
                        let result = cleaner.clean(true, reporter.as_ref())?;
                        println!("Trash holds: {}", format::format_bytes(result.bytes_freed));
                        eprintln!(
                            "Warning: sasurahime cannot safely empty ~/.Trash. Open it in Finder (or run `open ~/.Trash`) and empty it there."
                        );
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
                        let opts = sasurahime::cleaner::CleanOptions {
                            all,
                            ..Default::default()
                        };
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
                    CleanTarget::ApfsSnapshot {
                        dry_run,
                        all,
                        thin,
                        urgency,
                    } => {
                        let mut cleaner = cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(
                            Box::new(SystemCommandRunner),
                        )
                        .with_all(all);
                        if let Some(spec) = thin {
                            let Some(bytes) = cleaners::apfs_snapshot::parse_size_decimal(&spec)
                            else {
                                eprintln!("Error: cannot parse size: {spec} (expected e.g. 10GB)");
                                std::process::exit(1);
                            };
                            cleaner = cleaner.with_thin(bytes, urgency.unwrap_or(2));
                        }
                        run_and_exit(
                            "apfs-snapshot",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::VmSwap { dry_run } => {
                        // No-op by design: swap files are re-initialized on
                        // reboot, nothing can be deleted from a running system.
                        println!(
                            "vm-swap is a no-op: swap files in /private/var/vm are re-initialized on reboot. Nothing was deleted."
                        );
                        let cleaner = cleaners::vm_swap::VmSwapCleaner::new(
                            std::path::PathBuf::from("/private/var/vm"),
                            Box::new(SystemCommandRunner),
                        );
                        run_and_exit(
                            "vm-swap",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::All {
                        dry_run,
                        include_manual,
                    } => {
                        // Explicit full clean: the user picked every target on
                        // purpose. Safe targets run without --include-manual;
                        // Moderate/Risky/Manual need it.
                        let opts = cleaner::CleanOptions {
                            all: true,
                            include_manual,
                            older_than_days: cli.older_than,
                            larger_than_mb: cli.larger_than,
                            trash_mode: Some(sasurahime::trash::TrashMode::Global),
                        };
                        let cleaners_list = all_cleaners(&home, &config);
                        let mut skipped_targets: usize = 0;
                        // Skip policy stays at the call site: the wording is
                        // specific to `clean all`.
                        let mut batch_cleaners: Vec<&dyn cleaner::Cleaner> = Vec::new();
                        for c in &cleaners_list {
                            if !opts.include_manual && !c.auto_clean_allowed() {
                                skipped_targets += 1;
                                println!(
                                    "Skipped {} ({}): needs --include-manual",
                                    c.name(),
                                    c.risk().label()
                                );
                                continue;
                            }
                            batch_cleaners.push(c.as_ref());
                        }
                        let summary = batch::run_batch(
                            &batch_cleaners,
                            &opts,
                            reporter.as_ref(),
                            dry_run,
                            false,
                            true,
                        );
                        for (name, e) in &summary.failures {
                            eprintln!("Error cleaning {name}: {e}");
                        }
                        if !summary.skipped.is_empty() {
                            eprintln!(
                                "{} file(s) skipped due to errors (permission/lock):",
                                summary.skipped.len()
                            );
                            for (name, detail) in &summary.skipped {
                                eprintln!("  - {name}: {detail}");
                            }
                        }
                        if skipped_targets > 0 {
                            eprintln!(
                                "Skipped {skipped_targets} target(s) that need --include-manual."
                            );
                        }
                        println!("Total freed: {}", format::format_bytes(summary.total_freed));
                    }
                    CleanTarget::ProjectArtifacts { dry_run } => {
                        let roots = project_artifact_roots(&home, &config);
                        let cleaner =
                            cleaners::project_artifacts::ProjectArtifactsCleaner::new(roots)
                                .with_max_depth(config.project_artifacts.max_depth)
                                .with_recency_days(config.project_artifacts.recency_days);
                        run_and_exit(
                            "project-artifacts",
                            move |dry, rep| cleaner.clean(dry, rep),
                            dry_run,
                            reporter.as_ref(),
                        )?;
                    }
                    CleanTarget::External(args) => {
                        let Some(name) = args.first().cloned() else {
                            eprintln!("Error: missing target name.");
                            std::process::exit(1);
                        };
                        // `--help`/`-h` after the target name is captured
                        // verbatim into the external_subcommand's raw args,
                        // so clap never intercepts it. Running the plugin's
                        // clean for help output would be destructive:
                        // short-circuit with an advisory naming the plugin
                        // and exit 0, like clap does for its own subcommands.
                        if args.iter().any(|a| a == "--help" || a == "-h") {
                            println!(
                                "'{name}' is an external plugin target. Run `sasurahime-{name} --help` for the plugin's own usage."
                            );
                            std::process::exit(0);
                        }
                        // PBI-39: a [[custom]] config name resolves before the
                        // plugin lookup, so a user-defined target is reachable
                        // via `clean <name>` instead of falling into this
                        // catch-all. Built-in names never reach this arm: clap
                        // parses them into their own CleanTarget variants, so
                        // their priority is unchanged.
                        if let Some(ct) = config.custom.iter().find(|c| c.name == name) {
                            let path = config::Config::expand_tilde(&ct.path, &home);
                            // Refuse an unsafe target before any deletion
                            // attempt: the cleaner-level check only skips
                            // (exit 0), an explicit `clean <name>` must fail
                            // loudly and leave the path alone.
                            if !cleaners::generic::is_safe_delete_target(&path) {
                                eprintln!(
                                    "Error: refusing to clean '{name}': {} is not safe to delete.",
                                    path.display()
                                );
                                std::process::exit(1);
                            }
                            let cleaner =
                                cleaners::custom::CustomPathCleaner::new(ct.name.clone(), path);
                            // Same two-position dry-run resolution as the
                            // plugin arm below.
                            let dry_run = cli.dry_run || args.iter().any(|a| a == "--dry-run");
                            run_and_exit(
                                cleaner.name(),
                                move |dry, rep| cleaner.clean(dry, rep),
                                dry_run,
                                reporter.as_ref(),
                            )?;
                        } else {
                            let Some(plugin) =
                                cleaners::plugin::PluginCleaner::discover_excluding(&[])
                                    .into_iter()
                                    .find(|p| p.name() == name)
                            else {
                                eprintln!("Error: no plugin named '{name}' on PATH.");
                                std::process::exit(1);
                            };
                            // Honor the global --dry-run (declared `global = true`
                            // on Cli) in addition to the local position carried by
                            // the raw external args. The External arm re-parses
                            // those args by hand, so clap's global-value flow-down
                            // never reaches it: without the OR, `--dry-run clean
                            // <plugin>` ran the plugin's real clean.
                            let dry_run = cli.dry_run || args.iter().any(|a| a == "--dry-run");
                            run_and_exit(
                                plugin.name(),
                                move |dry, rep| plugin.clean(dry, rep),
                                dry_run,
                                reporter.as_ref(),
                            )?;
                        }
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

    if !cli.no_cache {
        scan_cache::flush(&scan_cache_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sasurahime::cleaner::CleanCancelled;

    #[test]
    fn home_filters_an_empty_home_var() {
        // HOME="" must be filtered exactly like the tripwire's empty filter:
        // fall back to dirs::home_dir(), never a CWD-relative empty path
        // (cache and history would silently land in the process CWD).
        // sasurahime::test_helpers is lib-test-only, so the bin test swaps
        // and restores HOME by hand, serialized with a local lock: no other
        // bin test reads HOME, and lib tests run in a separate process.
        static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = HOME_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let expected = dirs::home_dir().expect("test host must have a home dir");
        let previous = std::env::var("HOME").ok();
        std::env::set_var("HOME", "");
        let got = home();
        // Restored while the lock is still held, then the lock is released.
        match previous {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        assert_eq!(
            got, expected,
            "an empty HOME must fall back to dirs::home_dir(), got {:?}",
            got
        );
    }

    #[test]
    fn run_clean_target_cancelled_returns_ok_zero() {
        let reporter = sasurahime::progress::VerboseProgress::new();
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
        let reporter = sasurahime::progress::VerboseProgress::new();
        let result = run_clean_target(
            "skipped-test",
            |_dry_run, _reporter| {
                Ok(CleanResult {
                    name: "mock-cleaner",
                    bytes_freed: 100,
                    uses_trash: false,
                    skipped: vec![sasurahime::cleaner::SkippedEntry {
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

    const LEGACY_SCAN_TARGETS: &[&str] = &[
        "uv",
        "brew",
        "mise",
        "browsers",
        "xcode",
        "logs",
        "act",
        "huggingface",
        "pre-commit",
        "library-logs",
        "colima",
        "ollama",
        "device-support",
        "ios-backup",
        "apfs-snapshot",
    ];

    fn names_of(cleaners: &[Box<dyn cleaner::Cleaner>]) -> Vec<&'static str> {
        cleaners.iter().map(|c| c.name()).collect()
    }

    #[test]
    fn all_cleaners_keeps_legacy_targets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        for legacy in LEGACY_SCAN_TARGETS {
            assert!(names.contains(legacy), "missing legacy target: {legacy}");
        }
    }

    #[test]
    fn all_cleaners_names_are_unique() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "duplicate cleaner names: {names:?}"
        );
    }

    const NOT_SCANNED: &[&str] = &["caches", "trash", "all"];

    // Risk classification table: every registered cleaner must match. Listing
    // every Moderate entry explicitly catches accidental defaults (未分類の検出).
    const RISK_EXPECTED: &[(&str, cleaner::Risk)] = &[
        // Safe: regenerable developer caches, safe unattended.
        ("uv", cleaner::Risk::Safe),
        ("brew", cleaner::Risk::Safe),
        ("mise", cleaner::Risk::Safe),
        ("browsers", cleaner::Risk::Safe),
        ("act", cleaner::Risk::Safe),
        ("xcode", cleaner::Risk::Safe),
        ("logs", cleaner::Risk::Safe),
        ("huggingface", cleaner::Risk::Safe),
        ("pre-commit", cleaner::Risk::Safe),
        ("library-logs", cleaner::Risk::Safe),
        ("ollama", cleaner::Risk::Safe),
        ("device-support", cleaner::Risk::Safe),
        // Manual: user data, only cleaned when explicitly selected.
        ("downloads", cleaner::Risk::Manual),
        ("app-support", cleaner::Risk::Manual),
        ("library-caches", cleaner::Risk::Manual),
        ("orphan-containers", cleaner::Risk::Manual),
        ("project-artifacts", cleaner::Risk::Manual),
        // Risky: destructive or irreversible.
        ("ios-backup", cleaner::Risk::Risky),
        ("apfs-snapshot", cleaner::Risk::Risky),
        // colima clean removes ALL stopped VM disk data (containers, images,
        // volumes) — destructive and irreversible, so it must never run
        // unattended without explicit opt-in (see PBI 42).
        ("colima", cleaner::Risk::Risky),
        // Manual: user data, only cleaned when explicitly selected.
        ("vm-swap", cleaner::Risk::Manual),
        // Moderate: large or partially user-visible data.
        ("bun", cleaner::Risk::Moderate),
        ("go", cleaner::Risk::Moderate),
        ("pip", cleaner::Risk::Moderate),
        ("node-gyp", cleaner::Risk::Moderate),
        ("npm", cleaner::Risk::Moderate),
        ("yarn", cleaner::Risk::Moderate),
        ("pnpm", cleaner::Risk::Moderate),
        ("cargo", cleaner::Risk::Moderate),
        ("docker", cleaner::Risk::Moderate),
        ("orbstack", cleaner::Risk::Moderate),
        ("cocoapods", cleaner::Risk::Moderate),
        ("spm", cleaner::Risk::Moderate),
        ("conda", cleaner::Risk::Moderate),
        ("poetry", cleaner::Risk::Moderate),
        ("pipx", cleaner::Risk::Moderate),
        ("deno", cleaner::Risk::Moderate),
        ("dotnet", cleaner::Risk::Moderate),
        ("gem", cleaner::Risk::Moderate),
        ("bundle", cleaner::Risk::Moderate),
        ("rustup", cleaner::Risk::Moderate),
        ("simulator", cleaner::Risk::Moderate),
        ("gradle", cleaner::Risk::Moderate),
        ("jetbrains", cleaner::Risk::Moderate),
        ("vscode-extensions", cleaner::Risk::Moderate),
        ("maven", cleaner::Risk::Moderate),
        ("terraform", cleaner::Risk::Moderate),
        ("flutter", cleaner::Risk::Moderate),
        ("volta", cleaner::Risk::Moderate),
        ("sbt", cleaner::Risk::Moderate),
        ("tree-sitter", cleaner::Risk::Moderate),
    ];

    #[test]
    fn every_cleaner_has_expected_risk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for (name, risk) in RISK_EXPECTED {
            let c = cleaners
                .iter()
                .find(|c| c.name() == *name)
                .unwrap_or_else(|| panic!("RISK_EXPECTED has unregistered name: {name}"));
            assert_eq!(c.risk(), *risk, "risk mismatch for {name}");
        }
    }

    #[test]
    fn auto_clean_allowed_only_for_safe() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for c in &cleaners {
            assert_eq!(
                c.auto_clean_allowed(),
                c.risk() == cleaner::Risk::Safe,
                "auto policy mismatch for {} (risk {:?})",
                c.name(),
                c.risk()
            );
        }
    }

    #[test]
    fn every_clean_target_is_registered_in_all_cleaners() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        let missing: Vec<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(cli, _)| *cli)
            .filter(|cli| !NOT_SCANNED.contains(cli))
            .filter(|cli| !names.contains(cli))
            .collect();
        assert!(
            missing.is_empty(),
            "targets missing from all_cleaners: {missing:?}"
        );
    }

    #[test]
    fn clean_target_names_need_no_translation() {
        // Every CleanTarget CLI name must equal its Cleaner::name() directly:
        // no cleaner_name_for translation map may be needed anywhere.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        for (cli, _) in SUPPORTED_TARGETS.iter().chain(extra_targets()) {
            if NOT_SCANNED.contains(cli) {
                continue;
            }
            assert!(
                names.contains(cli),
                "CLI name {cli} has no cleaner with the same name; a translation hack would be needed"
            );
        }
    }

    #[test]
    fn standard_cleaners_follow_the_macro_table_order() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = standard_cleaners(tmp.path(), &config::Config::default());
        assert_eq!(cleaners.len(), SUPPORTED_TARGETS.len());
        for ((cli, _), c) in SUPPORTED_TARGETS.iter().zip(&cleaners) {
            assert_eq!(c.name(), *cli);
        }
    }

    #[test]
    fn standard_and_special_cleaners_do_not_overlap() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config::Config::default();
        let standard = names_of(&standard_cleaners(tmp.path(), &cfg));
        let special = names_of(&special_cleaners(tmp.path(), &cfg));
        for s in &special {
            assert!(!standard.contains(s), "registered twice: {s}");
        }
    }

    #[test]
    fn only_safe_risk_targets_are_auto_clean_allowed() {
        // The single classification decides: Safe runs under --yes, everything
        // else is wrapped in ManualOnly. Irreversible cleaners (ios-backup,
        // apfs-snapshot, Risky) stay registered and scanned but never run
        // unattended.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for c in &cleaners {
            assert_eq!(
                c.auto_clean_allowed(),
                c.risk() == cleaner::Risk::Safe,
                "auto policy mismatch for {}",
                c.name()
            );
        }
        let names = names_of(&cleaners);
        for (name, risk) in RISK_EXPECTED {
            assert!(names.contains(name), "{name} must stay registered");
            let expected_allowed = *risk == cleaner::Risk::Safe;
            let c = cleaners.iter().find(|c| c.name() == *name).unwrap();
            assert_eq!(
                c.auto_clean_allowed(),
                expected_allowed,
                "auto policy for {name}"
            );
        }
    }

    #[test]
    fn destructive_targets_are_not_auto_clean_allowed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for danger in [
            "docker",
            "orbstack",
            "npm",
            "yarn",
            "pnpm",
            "gem",
            "bundle",
            "conda",
            "poetry",
            "downloads",
            "vscode-extensions",
            "cargo",
            "rustup",
            "simulator",
            "maven",
            "sbt",
            // colima destroys ALL stopped VM disk data (PBI-42): it must not
            // be reachable from --yes either.
            "colima",
        ] {
            let c = cleaners.iter().find(|c| c.name() == danger).expect(danger);
            assert!(!c.auto_clean_allowed(), "{danger} must not run under --yes");
        }
    }

    #[test]
    fn config_exclude_removes_generated_targets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config::Config {
            exclude: vec!["docker".to_string(), "cocoapods".to_string()],
            ..config::Config::default()
        };
        let names = names_of(&all_cleaners(tmp.path(), &cfg));
        assert!(!names.contains(&"docker"));
        assert!(!names.contains(&"cocoapods"));
        assert!(names.contains(&"npm"));
    }

    #[test]
    fn orphan_containers_is_registered_but_not_auto_cleaned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"orphan-containers"));
        let c = cleaners
            .iter()
            .find(|c| c.name() == "orphan-containers")
            .unwrap();
        assert!(!c.auto_clean_allowed());
    }

    #[test]
    fn library_caches_is_registered_and_distinct_from_caches() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"library-caches"));
        let c = cleaners
            .iter()
            .find(|c| c.name() == "library-caches")
            .unwrap();
        assert!(!c.auto_clean_allowed());
        let names: Vec<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(n, _)| *n)
            .collect();
        assert!(names.contains(&"library-caches"));
        assert!(names.contains(&"caches"));
        assert_eq!(
            CleanTarget::LibraryCaches { dry_run: true }.command_name(),
            "library-caches"
        );
        assert!(CleanTarget::LibraryCaches { dry_run: true }.dry_run());
    }

    #[test]
    fn app_support_is_registered_but_not_auto_cleaned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"app-support"));
        let c = cleaners.iter().find(|c| c.name() == "app-support").unwrap();
        assert!(!c.auto_clean_allowed());
    }

    #[test]
    fn freed_message_reports_trash_only_when_the_cleaner_used_it() {
        use sasurahime::format::format_bytes;
        let bytes = 2048;
        let shown = format_bytes(bytes);
        assert_eq!(freed_message(0, false, true), "Freed: 0 B");
        assert_eq!(freed_message(0, true, true), "Freed: 0 B");
        assert_eq!(
            freed_message(bytes, true, true),
            format!("Logical freed: {shown} / Physical freed: 0 B (in Trash)")
        );
        assert_eq!(freed_message(bytes, false, true), format!("Freed: {shown}"));
        assert_eq!(freed_message(bytes, true, false), format!("Freed: {shown}"));
        assert_eq!(
            freed_message(bytes, false, false),
            format!("Freed: {shown}")
        );
    }

    fn pcc(older: Option<u32>, larger: Option<u64>) -> config::PerCleanerConfig {
        config::PerCleanerConfig {
            older_than_days: older,
            larger_than_mb: larger,
        }
    }

    fn support(
        names: &[(&str, cleaner::PerCleanerFilters)],
    ) -> Vec<(String, cleaner::PerCleanerFilters)> {
        names.iter().map(|(n, s)| (n.to_string(), *s)).collect()
    }

    fn cfg_with(entries: &[(&str, config::PerCleanerConfig)]) -> config::Config {
        let mut per_cleaner = std::collections::HashMap::new();
        for (k, v) in entries {
            per_cleaner.insert(k.to_string(), v.clone());
        }
        config::Config {
            per_cleaner,
            ..config::Config::default()
        }
    }

    #[test]
    fn ineffective_settings_empty_when_no_per_cleaner_config() {
        let s = support(&[("docker", cleaner::PerCleanerFilters::None)]);
        assert!(ineffective_per_cleaner_settings(&config::Config::default(), &s).is_empty());
    }

    #[test]
    fn ineffective_settings_names_the_ignored_fields_of_a_command_cleaner() {
        let cfg = cfg_with(&[("docker", pcc(Some(30), Some(10)))]);
        let s = support(&[("docker", cleaner::PerCleanerFilters::None)]);
        assert_eq!(
            ineffective_per_cleaner_settings(&cfg, &s),
            vec![
                "[docker] older_than_days, larger_than_mb: this cleaner has no per-cleaner filters"
            ]
        );
    }

    #[test]
    fn ineffective_settings_accepted_filters_are_silent() {
        let cfg = cfg_with(&[("act", pcc(Some(30), Some(10)))]);
        let s = support(&[("act", cleaner::PerCleanerFilters::Both)]);
        assert!(ineffective_per_cleaner_settings(&cfg, &s).is_empty());
    }

    #[test]
    fn ineffective_settings_logs_reject_larger_than_only() {
        let s = support(&[("logs", cleaner::PerCleanerFilters::OlderThanDays)]);
        let ok = cfg_with(&[("logs", pcc(Some(14), None))]);
        assert!(ineffective_per_cleaner_settings(&ok, &s).is_empty());
        let bad = cfg_with(&[("logs", pcc(Some(14), Some(10)))]);
        assert_eq!(
            ineffective_per_cleaner_settings(&bad, &s),
            vec!["[logs] larger_than_mb: only older_than_days applies"]
        );
    }

    #[test]
    fn ineffective_settings_reports_unknown_cleaner_names() {
        let cfg = cfg_with(&[("no-such", pcc(None, Some(10)))]);
        let s = support(&[("docker", cleaner::PerCleanerFilters::None)]);
        assert_eq!(
            ineffective_per_cleaner_settings(&cfg, &s),
            vec!["[no-such] unknown cleaner name"]
        );
    }

    #[test]
    fn ineffective_settings_reject_the_old_cocoa_pods_alias() {
        // The cocoa-pods→cocoapods translation hack is gone: the old config
        // key is now an unknown cleaner name, "cocoapods" is the real one.
        let cfg = cfg_with(&[("cocoa-pods", pcc(Some(30), None))]);
        let s = support(&[("cocoapods", cleaner::PerCleanerFilters::None)]);
        assert_eq!(
            ineffective_per_cleaner_settings(&cfg, &s),
            vec!["[cocoa-pods] unknown cleaner name"]
        );
    }

    #[test]
    fn ineffective_settings_use_real_names_without_translation() {
        let cfg = cfg_with(&[("cocoapods", pcc(Some(30), None))]);
        let s = support(&[("cocoapods", cleaner::PerCleanerFilters::None)]);
        assert_eq!(
            ineffective_per_cleaner_settings(&cfg, &s),
            vec!["[cocoapods] older_than_days: this cleaner has no per-cleaner filters"]
        );
    }

    #[test]
    fn ineffective_settings_ignores_entries_without_filters() {
        let cfg = cfg_with(&[("docker", pcc(None, None))]);
        let s = support(&[("docker", cleaner::PerCleanerFilters::None)]);
        assert!(ineffective_per_cleaner_settings(&cfg, &s).is_empty());
    }

    #[test]
    fn ineffective_settings_use_real_cleaner_capabilities() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = cfg_with(&[
            ("logs", pcc(None, Some(10))),
            ("act", pcc(Some(30), None)),
            ("docker", pcc(Some(30), None)),
        ]);
        let lines = ineffective_per_cleaner_settings(&cfg, &filter_support(tmp.path(), &cfg));
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("[docker]"), "{lines:?}");
        assert!(lines[1].starts_with("[logs]"), "{lines:?}");
    }

    #[test]
    fn managed_patterns_clean_targets_are_registered() {
        // Correspondence table test: every path the explorer marks as managed
        // must map to a target that actually exists in the registry.
        let registered: std::collections::HashSet<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(n, _)| *n)
            .collect();
        for pat in explorer::MANAGED_PATTERNS {
            assert!(
                registered.contains(pat.clean_target),
                "explorer pattern {} maps to unregistered clean target {:?}",
                pat.path_pattern,
                pat.clean_target
            );
        }
    }
    #[test]
    fn excluded_targets_are_registered_clean_targets() {
        // Correspondence table test: every cache dir in EXCLUDED_NAMES maps to
        // a registered clean target (single source of truth: the main.rs macro
        // table plus extra_targets). EXCLUDED_PREFIXES also carries the
        // "os-managed" marker, which is never dispatched and checked separately.
        let registered: std::collections::HashSet<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(n, _)| *n)
            .collect();
        for (_, target) in cleaners::library_caches::EXCLUDED_NAMES {
            assert!(
                registered.contains(target),
                "cache dir excluded name maps to unregistered target {target:?}"
            );
        }
        for (prefix, target) in cleaners::library_caches::EXCLUDED_PREFIXES {
            if *target == "os-managed" {
                continue;
            }
            assert!(
                registered.contains(target),
                "cache dir excluded prefix {prefix} maps to unregistered target {target:?}"
            );
        }
        assert_eq!(
            cleaners::library_caches::excluded_by("CocoaPods"),
            Some("cocoapods")
        );
    }

    #[test]
    fn colliding_custom_names_pinned_to_builtin_targets() {
        // PBI-39: a [[custom]] name equal to a built-in target name must be
        // warned about; non-colliding names stay silent.
        let cfg = config::Config {
            custom: vec![
                config::CustomTarget {
                    name: "uv".to_string(),
                    path: "~/.cache/uv-custom".to_string(),
                },
                config::CustomTarget {
                    name: "my-workspace".to_string(),
                    path: "~/.cache/my-workspace".to_string(),
                },
            ],
            ..config::Config::default()
        };
        assert_eq!(colliding_custom_names(&cfg), vec!["uv".to_string()]);
    }

    #[test]
    fn colliding_custom_names_empty_without_custom_config() {
        assert!(colliding_custom_names(&config::Config::default()).is_empty());
    }
}
