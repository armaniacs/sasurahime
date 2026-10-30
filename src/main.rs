use sasurahime::{
    batch, cleaner, cleaners, command, config, doctor, explorer, format, hint, history,
    interactive, progress, safety, scan_cache, scanner, unattended,
};

use clap::{Parser, Subcommand};
use cleaner::{CleanCancelled, CleanOptions, CleanResult, Cleaner};
use command::SystemCommandRunner;
use config::Config;
use progress::{build_reporter_from_flags, merge_suppress_flags, ProgressReporter};
use std::path::{Path, PathBuf};

/// Macro that generates the CleanTarget enum, SUPPORTED_TARGETS,
/// SPECIAL_TARGETS, ALL_TARGETS, the identity methods (command_name /
/// dry_run / is_special), the dispatch helper, standard_cleaners() and
/// caches_group_cleaners() from a single definition table.
/// all_cleaners() composes standard_cleaners() with special_cleaners().
///
/// Usage:
/// ```ignore
/// define_cleaners! {
///     // Standard cleaners (simple { dry_run: bool } variant). An optional
///     // `@group caches ;` marker joins the row to the caches batch:
///     Act : "act" => "description" ; |home, _config| { factory_expr }
///     Bun : "bun" => "description" ; @group caches ; { factory_expr }
///     ...
///     ;
///     // Special rows: the clap doc, the CLI name and the `dry_run`
///     // marker live in the row header; the fields stay verbatim; the
///     // second desc is the `sasurahime targets` listing text:
///     Caches : "caches" => "Clean all generic caches (...)" ; special ;
///         { #[arg(long)] dry_run: bool, } ; "All generic caches (...)" ;
///         dry_run,
///     ...
///     ;
///     // The external catch-all (exactly one):
///     External : "external" => "Catch-all for external plugin targets" ;
///         external ; (Vec<String>),
/// }
/// ```
///
/// The identity methods are exhaustive without a `_` arm: a variant added
/// to the table gets its `command_name` / `dry_run` / `is_special` arm
/// automatically, so a missing run arm or a removed cli-name drops the
/// build instead of panicking at runtime. The special rows' enum variants
/// are generated from their row headers, so `#[doc]` / `#[command(name)]`
/// live only there (a duplicate clap derive attribute would not compile).
///
/// The group membership is compile-time checked: a `@group <name>` row
/// resolves `<name>::DEFINED_GROUP_MARKER`, and only the `caches` module is
/// generated below, so an unknown group ident fails to compile instead of
/// silently joining the caches batch.
macro_rules! define_cleaners {
    ($(
        $(#[$variant_meta:meta])*
        $variant:ident : $cli_name:literal => $desc:expr ;
        $( @group $group:ident ; )?
        ($factory:expr)
    ),+ $(,)?
    ;
        $(
            $svariant:ident : $scli_name:literal => $sdoc:expr ;
            special ;
            $sfields:tt ;
            $stargets_desc:expr ;
            dry_run
        ),+ $(,)?
    ;
        $evariant:ident : $ecli_name:literal => $edoc:expr ;
        external ;
        $efields:tt $(,)?
    ) => {
        #[derive(Subcommand)]
        enum CleanTarget {
            $(
                $(#[$variant_meta])*
                #[command(name = $cli_name)]
                $variant { #[arg(long)] dry_run: bool },
            )*
            $(
                #[doc = $sdoc]
                #[command(name = $scli_name)]
                $svariant $sfields,
            )+
            #[doc = $edoc]
            #[command(external_subcommand)]
            $evariant $efields,
        }

        /// The standard rows of the table, in table order. Production
        /// consumers read ALL_TARGETS (the generated union); this section
        /// view backs the registry pins (table order, label match).
        #[cfg(test)]
        const SUPPORTED_TARGETS: &[(&str, &str)] = &[
            $( ($cli_name, $desc) ),*
        ];

        /// The special rows of the table. Production consumers read
        /// ALL_TARGETS; this section view backs the registry pins (name-set
        /// union, non-overlap).
        #[cfg(test)]
        const SPECIAL_TARGETS: &[(&str, &str)] = &[
            $( ($scli_name, $stargets_desc) ),*
        ];

        const ALL_TARGETS: &[(&str, &str)] = &[
            $( ($cli_name, $desc), )*
            $( ($scli_name, $stargets_desc), )*
        ];

        fn standard_cleaners(
            home: &std::path::Path,
            config: &config::Config,
        ) -> Vec<Box<dyn cleaner::Cleaner>> {
            vec![
                $( Box::new(($factory)(home, config)) as Box<dyn cleaner::Cleaner>, )*
            ]
        }

        /// The group namespace a `@group <name>` row resolves at compile
        /// time. Only the caches group is defined today: adding a group
        /// needs a new module here (and a generated factory fn), which
        /// keeps an unknown group ident a compile error rather than silent
        /// membership in the caches batch.
        #[doc(hidden)]
        pub mod caches {
            /// The marker every `@group <name>` row must resolve.
            pub const DEFINED_GROUP_MARKER: () = ();
        }

        /// The `@group caches` rows of the table, in table order (PBI-82
        /// Phase 2): the `clean caches` arm composes the batch from here,
        /// so a new cache row marked `@group caches` joins the batch
        /// without touching the arm.
        fn caches_group_cleaners(
            home: &std::path::Path,
            config: &config::Config,
        ) -> Vec<Box<dyn cleaner::Cleaner>> {
            vec![
                $(
                    $(
                        {
                            const _: () = $group::DEFINED_GROUP_MARKER;
                            Box::new(($factory)(home, config)) as Box<dyn cleaner::Cleaner>
                        },
                    )?
                )*
            ]
        }

        fn dispatch_clean(
            home: &std::path::Path,
            config: &config::Config,
            target: &CleanTarget,
            dry_run: bool,
            reporter: &dyn ProgressReporter,
            opts: &CleanOptions,
        ) -> anyhow::Result<CleanResult> {
            match target {
                $(
                    CleanTarget::$variant { .. } => ($factory)(home, config).clean(dry_run, reporter, opts),
                )*
                _ => unreachable!("dispatch_clean: unexpected special variant"),
            }
        }

        impl CleanTarget {
            /// The CLI command name, generated from the definition table's
            /// row headers. The match is exhaustive without a `_` arm: a
            /// variant added to the table gets its arm automatically, and
            /// the external catch-all resolves the static marker.
            fn command_name(&self) -> &'static str {
                match self {
                    $( CleanTarget::$variant { .. } => $cli_name, )*
                    $( CleanTarget::$svariant { .. } => $scli_name, )*
                    CleanTarget::$evariant(..) => $ecli_name,
                }
            }

            /// The run's dry-run flag. Standard variants carry exactly one
            /// `dry_run: bool` field; every special row ends with the
            /// `dry_run` marker (its `dry_run` field is destructured, the
            /// rest is ignored); the external catch-all carries raw args
            /// and no field, so the panic-free answer is false — that arm
            /// resolves its own dry-run two-position from the raw args.
            fn dry_run(&self) -> bool {
                match self {
                    $( CleanTarget::$variant { dry_run } => *dry_run, )*
                    $( CleanTarget::$svariant { dry_run, .. } => *dry_run, )*
                    CleanTarget::$evariant(..) => false,
                }
            }

            /// True when the variant is handled by the special dispatch
            /// block in run (the special rows plus the external catch-all).
            /// Generated from the same table: a special row without a run
            /// arm makes the run arm's match non-exhaustive at compile time.
            fn is_special(&self) -> bool {
                match self {
                    $( CleanTarget::$variant { .. } => false, )*
                    $( CleanTarget::$svariant { .. } => true, )*
                    CleanTarget::$evariant(..) => true,
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

    Bun : "bun" => "Bun package cache" ; @group caches ;
    (|_home, _config| cleaners::generic::CliCleaner::bun(Box::new(SystemCommandRunner))),

    Go : "go" => "Go build cache" ; @group caches ;
    (|_home, _config| cleaners::generic::CliCleaner::go(Box::new(SystemCommandRunner))),

    Pip : "pip" => "pip package cache" ; @group caches ;
    (|_home, _config| cleaners::generic::CliCleaner::pip(Box::new(SystemCommandRunner))),

    NodeGyp : "node-gyp" => "node-gyp build cache directories" ; @group caches ;
    (|home, config| cleaners::generic::PathCleaner::node_gyp(home, Box::new(SystemCommandRunner)).with_config(config)),

    Npm : "npm" => "npm package cache" ; @group caches ;
    (|_home, _config| cleaners::generic::CliCleaner::npm(Box::new(SystemCommandRunner))),

    Yarn : "yarn" => "yarn cache" ; @group caches ;
    (|_home, _config| cleaners::generic::CliCleaner::yarn(Box::new(SystemCommandRunner))),

    Pnpm : "pnpm" => "pnpm store" ; @group caches ;
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
    Caches : "caches"
        => "Clean all generic caches (bun, go, pip, node-gyp, npm, yarn, pnpm)" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)" ;
    dry_run,

    Logs : "logs"
        => "Remove log files older than N days" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Days to keep (default: value from config file, fallback 7)
        #[arg(long)]
        keep_days: Option<u32>,
    } ;
    "Log files older than N days" ;
    dry_run,

    Xcode : "xcode"
        => "Clean Xcode caches (DerivedData, Archives)" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Subcategories to clean: derived-data, archives (default: derived-data only)
        #[arg(long, value_delimiter = ',')]
        sub: Option<Vec<String>>,
    } ;
    "Xcode DerivedData project directories" ;
    dry_run,

    Trash : "trash"
        => "Report Trash size" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "~/.Trash size (scan only)" ;
    dry_run,

    Ollama : "ollama"
        => "Analyze and clean Ollama model cache" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "Ollama model cache" ;
    dry_run,

    LibraryLogs : "library-logs"
        => "Analyze and clean ~/Library/Logs/ with heuristic recommendations" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Skip prompt — delete all suggested entries
        #[arg(long, short = 'a')]
        all: bool,
    } ;
    "Analyze and clean ~/Library/Logs/ with heuristic recommendations" ;
    dry_run,

    DeviceSupport : "device-support"
        => "Remove old Xcode DeviceSupport directories, keeping recent N versions" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Number of recent versions to keep (default: 2)
        #[arg(long, default_value = "2")]
        keep: u32,
    } ;
    "Xcode DeviceSupport old version cleanup" ;
    dry_run,

    IosBackup : "ios-backup"
        => "Clean iOS device backups from ~/Library/Application Support/MobileSync/Backup/" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "iOS device backups (irreversible \u{2014} backed up to Trash)" ;
    dry_run,

    ApfsSnapshot : "apfs-snapshot"
        => "Delete APFS local Time Machine snapshots" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Delete every local snapshot without prompting (mutually exclusive
        /// with --thin)
        #[arg(long)]
        all: bool,
        /// Thin local snapshots toward this free-space target (e.g. 10GB,
        /// decimal bytes as tmutil expects). Mutually exclusive with --all
        #[arg(long)]
        thin: Option<String>,
        /// Thinning urgency 1..=4 (default 2; 4 interrupts running backups
        /// and needs an extra confirmation). Requires --thin
        #[arg(long)]
        urgency: Option<u32>,
    } ;
    "APFS local Time Machine snapshots (tmutil deletelocalsnapshot)" ;
    dry_run,

    VmSwap : "vm-swap"
        => "Report /private/var/vm swap files (no-op; re-initialized on reboot)" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "/private/var/vm swap files (report only; re-initialized on reboot)" ;
    dry_run,

    All : "all"
        => "Explicit full clean: Safe targets by default, everything with --include-manual. Honors global --older-than / --larger-than filters." ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Also clean Moderate / Risky / Manual targets
        #[arg(long)]
        include_manual: bool,
    } ;
    "Explicit full clean (Safe targets; --include-manual for the rest)" ;
    dry_run,

    ProjectArtifacts : "project-artifacts"
        => "Purge stale project artifacts (node_modules / target / .build)" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
    } ;
    "Stale project artifacts (node_modules / target / .build with manifests)" ;
    dry_run,

    Hints : "hints"
        => "Quit app, clean known app hint caches, relaunch app" ;
    special ;
    {
        #[arg(long)]
        dry_run: bool,
        /// Sub-targets to clean: display names from the scan tip (e.g.
        /// "Claude logs"). Default: every hint above its threshold
        #[arg(long, value_delimiter = ',')]
        sub: Option<Vec<String>>,
    } ;
    "Known app hint caches (quit app / clean / relaunch)" ;
    dry_run,
    ;
    External : "external"
        => "Catch-all for external plugin targets (sasurahime-* on PATH)" ;
    external ;
    (Vec<String>),
}

#[derive(Parser)]
#[command(
    name = "sasurahime",
    version = env!("CARGO_PKG_VERSION"),
    about = "macOS developer cache cleaner",
    help_template = "\n{name} {version}\n{about-with-newline}\n{usage-heading} {usage}\n\n{all-args}\n"
)]
struct Cli {
    /// Non-interactive: clean all pruneable caches without prompting.
    /// With --permanent, one final "Are you sure?" confirmation still runs;
    /// declining it (or a closed stdin) exits non-zero.
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

fn home() -> PathBuf {
    // An EMPTY HOME is filtered inside the crate-wide resolver
    // (config::resolved_home_dir) exactly like the tripwire's empty filter
    // (is_safe_delete_target): PathBuf::from("") would silently resolve
    // every cache/history path against the process CWD.
    config::resolved_home_dir().expect("cannot determine HOME directory")
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
        // PBI-83: the KNOWN_ENTRIES hint table as ONE registry entry, so
        // hint cleaning shows in scan, the TUI and `targets` like every
        // other cleaner. Moderate: never runs unattended.
        Box::new(hint::KnownAppHintsCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
    ]
}

/// Built-in target names a plugin must never impersonate. ALL_TARGETS is
/// the generated union of the two table sections, so a table addition is
/// protected here without touching this fn.
fn builtin_target_names() -> Vec<&'static str> {
    ALL_TARGETS.iter().map(|(name, _)| *name).collect()
}

/// The core registry: the macro table's standard rows plus the special
/// cleaners, in table order. all_cleaners / clean_by_name / filter_support
/// compose from here, so their registries cannot drift from each other
/// (the drift that produced the caches TrashMode bug is unrepresentable).
fn core_cleaners(
    home: &std::path::Path,
    config: &config::Config,
) -> Vec<Box<dyn cleaner::Cleaner>> {
    standard_cleaners(home, config)
        .into_iter()
        .chain(special_cleaners(home, config))
        .collect()
}

fn all_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>> {
    let mut cleaners: Vec<Box<dyn cleaner::Cleaner>> = core_cleaners(home, config)
        .into_iter()
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
///
/// Plugin cleaners are deliberately excluded (only the core cleaners plus
/// user-defined `[[custom]]` targets are listed): a `[cleaner.<plugin>]`
/// block therefore reports `unknown cleaner name` instead of being silently
/// applied to a plugin that may not honor the field. Widening this to
/// plugins is a semantic decision left to a separate PBI (PBI-82 Phase 4
/// records the exclusion on purpose, it is not an oversight).
fn filter_support(
    home: &std::path::Path,
    config: &config::Config,
) -> Vec<(String, cleaner::PerCleanerFilters)> {
    let mut support: Vec<(String, cleaner::PerCleanerFilters)> = core_cleaners(home, config)
        .into_iter()
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
    opts: &CleanOptions,
) -> anyhow::Result<CleanResult>
where
    F: FnOnce(bool, &dyn ProgressReporter, &CleanOptions) -> anyhow::Result<CleanResult>,
{
    let msg = format!("Cleaning {label}...");
    let result = if reporter.show_spinner() {
        // One shared spinner: each deletion updates its message with the
        // folder being removed, so the user sees what is happening. No
        // steady tick: an in-cleaner confirmation prompt must not compete
        // with animation, but ticks still redraw the line.
        let spinner = sasurahime::progress::SharedSpinner::start(&msg, false);
        let r = cleaner_fn(dry_run, &spinner.reporter(), opts);
        spinner.finish();
        match r {
            Ok(v) => {
                eprintln!("{msg} [OK]");
                v
            }
            Err(e) if e.is::<CleanCancelled>() => {
                // User cancelled — clean shutdown, no [FAILED], no Freed line.
                // The hint was already printed in GenericCleaner.
                return Ok(CleanResult::cancelled());
            }
            Err(e) => {
                eprintln!("{msg} [FAILED]");
                return Err(e);
            }
        }
    } else {
        let r = cleaner_fn(dry_run, reporter, opts);
        match r {
            Ok(v) => v,
            Err(e) if e.is::<CleanCancelled>() => return Ok(CleanResult::cancelled()),
            Err(e) => return Err(e),
        }
    };

    if reporter.show_spinner() {
        println!(
            "{}",
            freed_message(
                result.bytes_freed,
                result.uses_trash,
                opts.trash_mode == sasurahime::trash::TrashMode::Trash,
            )
        );
        if let Some(note) = batch::post_clean_note(
            result.bytes_freed,
            result.uses_trash,
            opts.trash_mode == sasurahime::trash::TrashMode::Trash,
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

    // Aligned contract (PBI 78): both engines treat CleanCancelled as a
    // run-level decision, not an error — this path returns Ok(cancelled)
    // (exit 0), run_batch collects the target name into
    // BatchSummary.cancelled and the caller reports a neutral
    // `cancelled by user` line. The [OK]/[FAILED] status line is owned by
    // the reporter kind on both engines: it prints only when
    // reporter.show_spinner(), and `animated` stays the caller's flag
    // (no-tick-while-prompting). What stays single-target here is only the
    // CleanResult-shaped report (per-target freed_message and the skip
    // lines above — PBI 73's shared report_batch_failures takes a
    // BatchSummary); the history write funnels through the one guard in
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
    opts: &CleanOptions,
) -> anyhow::Result<CleanResult>
where
    F: FnOnce(bool, &dyn ProgressReporter, &CleanOptions) -> anyhow::Result<CleanResult>,
{
    let result = run_clean_target(label, cleaner_fn, dry_run, reporter, opts)?;
    if result.exit_code() != 0 {
        std::process::exit(1);
    }
    Ok(result)
}

/// Runs the `clean caches` batch and reports through the shared helpers
/// (PBI 73). The cleaners, the run's resolved opts, the reporter and
/// `dry_run` all arrive as arguments, so the arm owns no mode: the
/// TrashMode contract (the one mode resolution in `main`) holds here too,
/// and a test can inject stubs for every dependency.
fn run_caches_batch(
    cleaners: &[&dyn cleaner::Cleaner],
    opts: &CleanOptions,
    reporter: &dyn ProgressReporter,
    dry_run: bool,
) -> anyhow::Result<()> {
    // Bare batch: no post-clean notes (this entry point never printed them).
    let summary = batch::run_batch(cleaners, opts, reporter, dry_run, false, false);
    interactive::report_batch_failures(
        &mut std::io::stderr(),
        &summary,
        interactive::ReportShape::Named,
    )?;
    interactive::report_total_freed(&mut std::io::stdout(), summary.total_freed)?;
    Ok(())
}

/// Maps a sub-target name to the Xcode subcategory: one parse handles both
/// the CLI's `--sub` spellings (`derived-data`, `deriveddata`, `archives`)
/// and the TUI display names (`DerivedData`, `Archives`) — the second
/// display-name re-match this used to carry is gone (PBI 59).
fn xcode_subcategory(sub: &str) -> Option<cleaners::xcode::XcodeSubcategory> {
    cleaners::xcode::XcodeSubcategory::parse(sub)
}

/// The one xcode clean runner (PBI-82): parses the requested subcategories
/// through `xcode_subcategory`, rebuilds the cleaner with them and runs it
/// through `run_clean_target`. Shared by the `clean xcode` CLI arm and the
/// `clean_by_name` seam, so the parse and the unknown-spelling error have
/// one owner. The error is the CLI arm's wording: it lists every accepted
/// spelling (the seam's shorter variant is gone — PBI 63 makes an unknown
/// spelling an error, not a silent drop). `subs = None` builds the bare
/// cleaner, whose `clean()` defaults to DerivedData.
fn run_xcode_clean(
    home: &std::path::Path,
    subs: Option<&[String]>,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
    opts: &CleanOptions,
) -> anyhow::Result<CleanResult> {
    let mut xcode_cleaner = cleaners::xcode::XcodeCleaner::new(home, Box::new(SystemCommandRunner));
    if let Some(subs) = subs {
        // PBI 63: an unknown spelling is an error, not a silent drop — the
        // old filter_map narrowed the run to DerivedData and left Archives
        // behind.
        let parsed: Vec<_> = subs
            .iter()
            .map(|s| {
                xcode_subcategory(s).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown xcode subcategory: {s} \
                         (expected derived-data, deriveddata, \
                         archives, DerivedData, Archives)"
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        xcode_cleaner = xcode_cleaner.with_subcategories(parsed);
    }
    run_clean_target(
        "xcode",
        |dry, rep, opts| xcode_cleaner.clean(dry, rep, opts),
        dry_run,
        reporter,
        opts,
    )
}

/// The one hints clean runner (PBI-83): validates the requested sub-targets
/// against the KNOWN_ENTRIES display names, rebuilds the cleaner with them
/// and runs it through `run_clean_target`. Shared by the `clean hints` CLI
/// arm and the `clean_by_name` seam, so the validation and the unknown-name
/// error have one owner. An unknown spelling is an error, not a silent
/// no-op clean (the PBI 63 shape `run_xcode_clean` pins for xcode). `subs =
/// None` builds the bare cleaner, whose `clean()` cleans every hint above
/// its threshold.
fn run_hints_clean(
    home: &std::path::Path,
    subs: Option<&[String]>,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
    opts: &CleanOptions,
) -> anyhow::Result<CleanResult> {
    let mut hints_cleaner = hint::KnownAppHintsCleaner::new(home, Box::new(SystemCommandRunner));
    if let Some(subs) = subs {
        let unknown: Vec<&String> = subs
            .iter()
            .filter(|s| !hint::is_known_hint_name(s))
            .collect();
        if !unknown.is_empty() {
            anyhow::bail!(
                "unknown hints sub-target: {} (run `sasurahime scan` for the hint names)",
                unknown
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        hints_cleaner = hints_cleaner.with_sub_names(subs.to_vec());
    }
    run_clean_target(
        "hints",
        |dry, rep, opts| hints_cleaner.clean(dry, rep, opts),
        dry_run,
        reporter,
        opts,
    )
}

/// PBI-50 in-process clean seam: resolves a clean target name (plus an
/// optional sub-target) to the same dispatch the `clean <target>` CLI arm
/// uses, and returns the REAL CleanResult instead of a fabricated zero.
/// Wired into `run_interactive` and `run_explore` so the TUI and the explorer
/// never re-exec this binary — the old self re-exec lost every sub-target
/// byte in `Total freed`, resolved its own exe through PATH, and re-loaded
/// config without the caller's flags.
///
/// Dispatch parity with the CLI:
/// - standard targets resolve to the same macro-table factories
///   `dispatch_clean` builds, and run through `run_clean_target` (the shared
///   spinner, CleanCancelled handling, skip report and history guard), not
///   around it — so `CleanOptions`/trash-mode resolution is untouched;
/// - `xcode` + sub-target rebuilds the cleaner with the requested
///   subcategory, exactly like the `CleanTarget::Xcode` arm, accepting both
///   the CLI spellings (`derived-data`, `archives`) and the TUI display names
///   (`DerivedData`, `Archives`; the old `--sub <display name>` re-exec
///   silently failed the parse and cleaned DerivedData regardless);
/// - custom/plugin targets are deliberately NOT resolvable here — they never
///   reach this seam (the TUI cleans them whole via `run_batch`, and the
///   explorer only lists built-in targets), so the `clean <name>` External
///   arm keeps the PBI-39 unsafe-path refusal untouched.
///
/// Scan cache: the old explorer child flushed the scan cache on exit
/// (main.rs's trailing flush); the per-clean flush is deliberately dropped —
/// deletion paths never consult the cache and the parent process still
/// flushes once at the end of main.
fn clean_by_name(
    home: &std::path::Path,
    config: &config::Config,
    name: &str,
    sub: Option<&str>,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
    opts: &CleanOptions,
) -> anyhow::Result<CleanResult> {
    if name == "xcode" {
        if let Some(sub) = sub {
            // The seam shares the CLI arm's runner: the parse, the unknown
            // spelling error and the cleaner construction have one owner.
            let subs = vec![sub.to_string()];
            return run_xcode_clean(home, Some(&subs), dry_run, reporter, opts);
        }
        // sub = None falls through to the registry resolution below, which
        // builds the same `XcodeCleaner::new` the `CleanTarget::Xcode` arm
        // builds for a no-`--sub` run (clean() then defaults to DerivedData).
    }
    if name == "hints" {
        // The seam shares the CLI arm's runner: the sub-target validation,
        // the unknown-name error and the cleaner construction have one
        // owner (run_hints_clean). `sub = Some(display_name)` is the TUI's
        // selection from `sub_targets()`; `sub = None` is the whole
        // hints target, exactly like the `CleanTarget::Hints` arm.
        let subs = sub.map(|s| vec![s.to_string()]);
        return run_hints_clean(home, subs.as_deref(), dry_run, reporter, opts);
    }
    let registry = core_cleaners(home, config);
    // Identity boundary (PBI 59): the standard targets' typed identity is the
    // `CleanTarget` enum the CLI dispatches through. A parallel `CleanerId`
    // on the `Cleaner` trait was considered and rejected: it would duplicate
    // `CleanTarget` 1:1 for the 26 struct cleaners and still need a string
    // arm for plugins (dynamic discovery), i.e. two sources of truth for the
    // same identity. This seam is the one place a runtime string enters, and
    // both sides of the equality are produced by the same code (the TUI's
    // selection list is built from `cleaner.name()`).
    let cleaner = registry
        .into_iter()
        .find(|c| c.name() == name)
        .ok_or_else(|| anyhow::anyhow!("unknown clean target: {name}"))?;
    run_clean_target(
        cleaner.name(),
        |dry, rep, opts| cleaner.clean(dry, rep, opts),
        dry_run,
        reporter,
        opts,
    )
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

/// The one hint-flow entry point, shared by `scan` and the interactive arm:
/// collect, print, then offer (selection collection). `dry_run` is the
/// run's global `--dry-run` — the offer never prompts and never deletes
/// when it is set (PBI 80).
///
/// PBI-83: the offer is reduced to selection collection — the accepted
/// entries are executed through the same CleanByName seam the TUI uses
/// (`clean_by_name("hints", Some(display_name), ...)`), so the deletion and
/// the history guard follow the Cleaner-trait flow contract. Each seam call
/// writes its own history row (label = "hints", 1 cleaner = 1 name), so no
/// aggregate row is written here, and an all-skipped offer — every
/// attempted deletion failed with a skippable error — reports failure
/// through the exit code (PBI 70 contract). A quit timeout frees zero bytes
/// and records no skip, so it stays exit 0.
fn run_hint_flow(
    home: &Path,
    config: &Config,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
    clean_opts: &CleanOptions,
) -> anyhow::Result<()> {
    let runner = SystemCommandRunner;
    let hints = hint::collect_hints(home, &runner);
    hint::print_hints(&hints);
    let selection = hint::offer_auto_clean(&hints, dry_run, &hint::StdinPrompt);
    let mut aggregate = cleaner::CleanResult::empty("hints");
    for entry in &selection {
        let result = clean_by_name(
            home,
            config,
            "hints",
            Some(entry.display_name),
            dry_run,
            reporter,
            clean_opts,
        )?;
        aggregate.bytes_freed += result.bytes_freed;
        aggregate.uses_trash |= result.uses_trash;
        aggregate.skipped.extend(result.skipped);
    }
    if aggregate.exit_code() == 1 {
        std::process::exit(1);
    }
    Ok(())
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

    // The one mode resolution (PBI 71): --permanent or config.trash_mode =
    // false pins permanent deletion, everything else moves to the Trash.
    // The AND expression and its evaluation order are unchanged; the
    // resolved value flows down as an argument from here on, and nothing
    // re-reads a process-global mode.
    let trash_mode = if !cli.permanent && config.trash_mode {
        sasurahime::trash::TrashMode::Trash
    } else {
        sasurahime::trash::TrashMode::Permanent
    };
    let clean_opts = CleanOptions {
        trash_mode,
        ..Default::default()
    };

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
            // No [policy] section: refuse to delete anything unattended. The
            // entry gate stays at the arm (PBI 74): it is not a selection
            // input.
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
                trash_mode,
            };
            let cleaners_list = all_cleaners(&home, &config);
            // PBI 74: the run/skip classification is owned by
            // UnattendedPolicy::Auto. The wording stays at the call site: it
            // is specific to `auto`. Risky (destructive/irreversible) and
            // Manual (review-first) targets never run unattended, even with
            // include_manual: whole-dir removals must never happen without a
            // human.
            let selection = unattended::UnattendedPolicy::Auto.select(&cleaners_list, &opts);
            let skipped: Vec<String> = selection
                .skipped
                .iter()
                .map(|(i, reason)| {
                    let c = &cleaners_list[*i];
                    match reason {
                        unattended::SkipReason::NeverUnattended(risk) => {
                            format!("{} ({}): never runs unattended", c.name(), risk.label())
                        }
                        unattended::SkipReason::NeedsOptIn(risk) => format!(
                            "{} ({}): set [policy] include_manual = true to include",
                            c.name(),
                            risk.label()
                        ),
                        // Auto never classifies a target as scan-gated (it
                        // scans nothing); the arm renders it truthfully if
                        // wiring ever changes.
                        unattended::SkipReason::NeedsExplicitSelection(risk) => format!(
                            "{} ({}): needs an explicit selection",
                            c.name(),
                            risk.label()
                        ),
                    }
                })
                .collect();
            let batch_cleaners: Vec<&dyn cleaner::Cleaner> = selection
                .run
                .iter()
                .map(|&(i, _)| cleaners_list[i].as_ref())
                .collect();
            let summary = batch::run_batch(
                &batch_cleaners,
                &opts,
                reporter.as_ref(),
                dry_run,
                false,
                true,
            );
            interactive::report_batch_failures(
                &mut std::io::stderr(),
                &summary,
                interactive::ReportShape::Named,
            )?;
            // The target-skip guidance stays at the call site: the wording is
            // specific to `auto`.
            if !skipped.is_empty() {
                eprintln!("Skipped {} target(s):", skipped.len());
                for s in &skipped {
                    eprintln!("  - {s}");
                }
                eprintln!("Run `sasurahime clean <target>` explicitly for those.");
            }
            interactive::report_total_freed(&mut std::io::stdout(), summary.total_freed)?;
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
            run_hint_flow(&home, &config, cli.dry_run, reporter.as_ref(), &clean_opts)?;
        }
        Some(Commands::Targets) => {
            let mut targets: Vec<&(&str, &str)> = ALL_TARGETS.iter().collect();
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
            // PBI-50: the TUI's sub-target branch cleans in-process through
            // this seam — the real CleanResult fixes the `Total freed`
            // under-count of the old self re-exec, and the TUI process's own
            // flags (trash mode, config path) now apply to the clean.
            if cli.yes {
                interactive::run_auto(&cleaners, trash_mode)?;
            } else {
                interactive::run_interactive(
                    &cleaners,
                    &|name, sub, dry, rep| {
                        clean_by_name(&home, &config, name, sub, dry, rep, &clean_opts)
                    },
                    trash_mode,
                )?;
            }
            run_hint_flow(&home, &config, cli.dry_run, reporter.as_ref(), &clean_opts)?;
        }
        Some(Commands::Explore { top, all, dirs }) => {
            let roots = if dirs.is_empty() {
                explorer::default_roots(&home)
            } else {
                dirs
            };
            let top_n = if all { None } else { Some(top) };
            // PBI-50: managed-entry cleans run in-process through the seam
            // (the CLI reporter keeps the "Cleaning ... [OK] / Freed:" output
            // the old re-exec'd child printed) instead of spawning this
            // binary resolved through current_exe / PATH.
            explorer::run_explore(
                &home,
                explorer::ExploreOptions {
                    roots,
                    top: top_n,
                    dry_run: cli.dry_run,
                },
                reporter.as_ref(),
                &|name, sub, dry, rep| {
                    clean_by_name(&home, &config, name, sub, dry, rep, &clean_opts)
                },
            )?;
        }
        Some(Commands::Clean { target }) => {
            if target.is_special() {
                // --- Special targets (custom dispatch logic) ---
                match target {
                    CleanTarget::Caches { dry_run } => {
                        // The batch is the @group caches rows of the macro
                        // table: membership lives in the table, not here
                        // (PBI-82 Phase 2). The run's resolved opts flow
                        // down as an argument (the one mode resolution), and
                        // the bare batch keeps no post-clean notes (this
                        // entry point never printed them).
                        let caches = caches_group_cleaners(&home, &config);
                        let batch_cleaners: Vec<&dyn cleaner::Cleaner> =
                            caches.iter().map(|c| c.as_ref()).collect();
                        run_caches_batch(&batch_cleaners, &clean_opts, reporter.as_ref(), dry_run)?;
                    }
                    CleanTarget::Logs { dry_run, keep_days } => {
                        run_and_exit(
                            "logs",
                            |dry, rep, opts| {
                                log_cleaner_from_config(&home, &config, keep_days)
                                    .clean(dry, rep, opts)
                            },
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::Xcode { dry_run, sub } => {
                        // The --yes note stays at the call site: it is
                        // specific to the CLI arm's flags (the seam never
                        // prints it). is_xcode_running is a process check
                        // (pgrep -x Xcode), so the arm checks it on a bare
                        // cleaner built exactly like the one
                        // run_xcode_clean builds.
                        if cli.yes
                            && cleaners::xcode::XcodeCleaner::new(
                                &home,
                                Box::new(SystemCommandRunner),
                            )
                            .is_xcode_running()
                        {
                            eprintln!("Note: Xcode is running. Proceeding with --yes anyway.");
                        }
                        run_xcode_clean(
                            &home,
                            sub.as_deref(),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
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
                        let result =
                            cleaner.clean(true, reporter.as_ref(), &CleanOptions::default())?;
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
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::LibraryLogs { dry_run, all } => {
                        let cleaner = cleaners::library_logs::LibraryLogsCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        let opts = sasurahime::cleaner::CleanOptions {
                            all,
                            trash_mode,
                            ..Default::default()
                        };
                        run_and_exit(
                            "library-logs",
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &opts,
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
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::IosBackup { dry_run } => {
                        let cleaner = cleaners::ios_backup::IosCleaner::new(
                            &home,
                            Box::new(SystemCommandRunner),
                        );
                        run_and_exit(
                            "ios-backup",
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::ApfsSnapshot {
                        dry_run,
                        all,
                        thin,
                        urgency,
                    } => {
                        // Flags -> SnapshotMode in one place: --all and --thin
                        // are mutually exclusive and --urgency only applies to
                        // --thin, so conflicts and dangling flags fail loudly
                        // (exit 1) instead of being silently dropped.
                        let mode = if all && thin.is_some() {
                            eprintln!("Error: --all and --thin are mutually exclusive; pick one.");
                            std::process::exit(1);
                        } else if let Some(spec) = thin {
                            let Some(bytes) = cleaners::apfs_snapshot::parse_size_decimal(&spec)
                            else {
                                eprintln!("Error: cannot parse size: {spec} (expected e.g. 10GB)");
                                std::process::exit(1);
                            };
                            cleaners::apfs_snapshot::SnapshotMode::Thin {
                                target_bytes: bytes,
                                urgency: urgency.unwrap_or(2),
                            }
                        } else if urgency.is_some() {
                            eprintln!("Error: --urgency requires --thin.");
                            std::process::exit(1);
                        } else {
                            cleaners::apfs_snapshot::SnapshotMode::Delete { all }
                        };
                        let cleaner = cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
                            SystemCommandRunner,
                        ))
                        .with_mode(mode);
                        run_and_exit(
                            "apfs-snapshot",
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
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
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::All {
                        dry_run,
                        include_manual,
                    } => {
                        // Explicit full clean: the user picked every target on
                        // purpose. Safe targets run without --include-manual;
                        // Moderate/Risky/Manual need it. No skip-confirm guard
                        // here — skip policies stay at the call sites
                        // (batch.rs): confirm_suppressed() must stay false so
                        // a cleaner that gates on a prompt (colima's fallback
                        // full delete, generic.rs) still asks before deleting;
                        // moving the guard into run_batch would delete
                        // without asking.
                        let opts = cleaner::CleanOptions {
                            all: true,
                            include_manual,
                            older_than_days: cli.older_than,
                            larger_than_mb: cli.larger_than,
                            trash_mode,
                        };
                        let cleaners_list = all_cleaners(&home, &config);
                        // PBI 74: the classification is owned by
                        // UnattendedPolicy::All — the opt-in unlocks EVERY
                        // non-Safe risk here (unlike auto, where Risky and
                        // Manual stay locked), except quit-based cleaners,
                        // which stay skipped even with --include-manual
                        // (PBI 87). The wording stays at the call site: it
                        // is specific to `clean all`.
                        let selection =
                            unattended::UnattendedPolicy::All.select(&cleaners_list, &opts);
                        for (i, reason) in &selection.skipped {
                            let c = &cleaners_list[*i];
                            match reason {
                                // PBI 87: a quit-based cleaner is skipped even
                                // with --include-manual; the wording matches
                                // the auto arm's NeverUnattended line.
                                unattended::SkipReason::NeverUnattended(risk) => {
                                    println!(
                                        "Skipped {} ({}): never runs unattended",
                                        c.name(),
                                        risk.label()
                                    );
                                }
                                unattended::SkipReason::NeedsOptIn(risk) => println!(
                                    "Skipped {} ({}): needs --include-manual",
                                    c.name(),
                                    risk.label()
                                ),
                                // All is registry-driven and never classifies
                                // a target as explicit-selection-only; the
                                // arm renders it truthfully if wiring ever
                                // changes.
                                unattended::SkipReason::NeedsExplicitSelection(risk) => println!(
                                    "Skipped {} ({}): needs an explicit selection",
                                    c.name(),
                                    risk.label()
                                ),
                            }
                        }
                        let batch_cleaners: Vec<&dyn cleaner::Cleaner> = selection
                            .run
                            .iter()
                            .map(|&(i, _)| cleaners_list[i].as_ref())
                            .collect();
                        let summary = batch::run_batch(
                            &batch_cleaners,
                            &opts,
                            reporter.as_ref(),
                            dry_run,
                            false,
                            true,
                        );
                        interactive::report_batch_failures(
                            &mut std::io::stderr(),
                            &summary,
                            interactive::ReportShape::Named,
                        )?;
                        if !selection.skipped.is_empty() {
                            eprintln!(
                                "Skipped {} target(s) that need --include-manual.",
                                selection.skipped.len()
                            );
                        }
                        interactive::report_total_freed(
                            &mut std::io::stdout(),
                            summary.total_freed,
                        )?;
                    }
                    CleanTarget::ProjectArtifacts { dry_run } => {
                        let roots = project_artifact_roots(&home, &config);
                        let cleaner =
                            cleaners::project_artifacts::ProjectArtifactsCleaner::new(roots)
                                .with_max_depth(config.project_artifacts.max_depth)
                                .with_recency_days(config.project_artifacts.recency_days);
                        run_and_exit(
                            "project-artifacts",
                            move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                    }
                    CleanTarget::Hints { dry_run, sub } => {
                        // PBI-83: the hints target is an explicit selection —
                        // the cleaner carries no prompt (interactive_gated()
                        // is false, the prompt is the scan tip's offer). The
                        // shared runner validates the --sub names and runs
                        // the quit → ledger → relaunch flow through
                        // run_clean_target, so dry_run and the history guard
                        // follow the Cleaner-trait flow contract. The arm
                        // calls it directly (like the Xcode arm): the runner
                        // already wraps run_clean_target, so a run_and_exit
                        // wrap would write the history row twice.
                        let result = run_hints_clean(
                            &home,
                            sub.as_deref(),
                            dry_run,
                            reporter.as_ref(),
                            &clean_opts,
                        )?;
                        if result.exit_code() != 0 {
                            std::process::exit(1);
                        }
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
                            if !safety::is_safe_delete_target(&path) {
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
                                move |dry, rep, opts| cleaner.clean(dry, rep, opts),
                                dry_run,
                                reporter.as_ref(),
                                &clean_opts,
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
                                move |dry, rep, opts| plugin.clean(dry, rep, opts),
                                dry_run,
                                reporter.as_ref(),
                                &clean_opts,
                            )?;
                        }
                    }
                    _ => unreachable!(),
                }
            } else {
                // --- Standard targets: dispatch through macro-generated handler ---
                run_and_exit(
                    target.command_name(),
                    |dry, rep, opts| dispatch_clean(&home, &config, &target, dry, rep, opts),
                    target.dry_run(),
                    reporter.as_ref(),
                    &clean_opts,
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
            |_dry_run, _reporter, _opts| Err(anyhow::Error::from(CleanCancelled)),
            false,
            &reporter,
            &CleanOptions::default(),
        );
        assert!(result.is_ok(), "CleanCancelled should be converted to Ok");
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 0);
        assert!(clean_result.skipped.is_empty());
    }

    #[test]
    fn clean_all_arm_leaves_the_confirm_prompts_live() {
        // The skip-confirm guard stays at the call sites (batch.rs doc:
        // skip policies stay at the call sites): the All arm must never
        // install it, so confirm_suppressed() stays false and a cleaner
        // that gates on a prompt (colima's fallback full delete,
        // generic.rs) still asks before deleting. Hoisting the guard into
        // run_batch would make `clean all --include-manual` delete without
        // confirmation — the colima no-unconfirmed-delete safety regression.
        let src = include_str!("main.rs");
        // The All ARM (not the macro-generated dispatch helpers): the only
        // "CleanTarget::All {" head that carries `include_manual`, and the
        // first such occurrence in the file.
        let start = src
            .match_indices("CleanTarget::All {")
            .find(|(i, _)| src[*i..src.len().min(i + 160)].contains("include_manual"))
            .map(|(i, _)| i)
            .expect("the All arm must be present");
        let end = src[start..]
            .find("CleanTarget::ProjectArtifacts")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let arm = &src[start..end];
        assert!(
            !arm.contains("SkipConfirmGuard::new()")
                && !arm.contains("set_confirm_suppressed(true)"),
            "the All arm must not suppress the confirm prompts: confirm_suppressed() must stay false so colima's fallback prompt still fires"
        );
        assert!(
            !sasurahime::prompt::confirm_suppressed(),
            "outside any guard the confirm prompts must be live"
        );
    }

    #[test]
    fn run_clean_target_skipped_entries_do_not_panic() {
        let reporter = sasurahime::progress::VerboseProgress::new();
        let result = run_clean_target(
            "skipped-test",
            |_dry_run, _reporter, _opts| {
                Ok(CleanResult {
                    name: "mock-cleaner",
                    bytes_freed: 100,
                    uses_trash: false,
                    skipped: vec![sasurahime::cleaner::SkippedEntry {
                        path: std::path::PathBuf::from("/tmp/skipped-file"),
                        reason: sasurahime::cleaner::SkipCause::Reason(
                            "Permission denied".to_string(),
                        ),
                    }],
                })
            },
            false,
            &reporter,
            &CleanOptions::default(),
        );
        assert!(result.is_ok());
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 100);
        assert_eq!(clean_result.skipped.len(), 1);
    }

    #[test]
    fn all_targets_is_the_supported_special_name_set() {
        // Name-set pin (PBI-82 Phase 3): the 55-variant correspondence list
        // is gone. command_name()/dry_run()/is_special() are generated with
        // `_`-free exhaustive matches from the same definition table, so a
        // variant added to the table can never fall into a catch-all at
        // compile time (and removing a row's cli-name drops the build). What
        // stays here is the NAME set: the generated ALL_TARGETS is exactly
        // SUPPORTED_TARGETS + SPECIAL_TARGETS, with no overlap and no
        // duplicates.
        let names: Vec<&str> = ALL_TARGETS.iter().map(|(n, _)| *n).collect();
        let supported: std::collections::HashSet<&str> =
            SUPPORTED_TARGETS.iter().map(|(n, _)| *n).collect();
        let special: std::collections::HashSet<&str> =
            SPECIAL_TARGETS.iter().map(|(n, _)| *n).collect();
        let n = names.len();
        let mut sorted = names;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), n, "duplicate target name in ALL_TARGETS");
        assert_eq!(
            ALL_TARGETS.len(),
            SUPPORTED_TARGETS.len() + SPECIAL_TARGETS.len(),
            "ALL_TARGETS must be the union of the two table sections"
        );
        for name in &supported {
            assert!(!special.contains(name), "registered twice: {name}");
        }
    }

    #[test]
    fn special_arm_labels_match_the_cleaner_names() {
        // Correspondence pin: every special arm passes a history-label
        // literal to run_and_exit; the corresponding Cleaner::name() must
        // equal it, or `clean <target>` and the batch paths would write
        // history rows under two different names. Rename the cleaner side
        // only and this pin fails (each literal below mirrors the matching
        // special arm's run_and_exit argument).
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        let cfg = config::Config::default();
        let pairs: Vec<(&str, Box<dyn cleaner::Cleaner>)> = vec![
            ("logs", Box::new(log_cleaner_from_config(home, &cfg, None))),
            (
                "xcode",
                Box::new(cleaners::xcode::XcodeCleaner::new(
                    home,
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "ollama",
                Box::new(cleaners::ollama::OllamaCleaner::new(
                    home,
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "library-logs",
                Box::new(cleaners::library_logs::LibraryLogsCleaner::new(
                    home,
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "device-support",
                Box::new(cleaners::device_support::DeviceSupportCleaner::new(
                    home,
                    2,
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "ios-backup",
                Box::new(cleaners::ios_backup::IosCleaner::new(
                    home,
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "apfs-snapshot",
                Box::new(
                    cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
                        SystemCommandRunner,
                    ))
                    .with_mode(cleaners::apfs_snapshot::SnapshotMode::Delete { all: false }),
                ),
            ),
            (
                "vm-swap",
                Box::new(cleaners::vm_swap::VmSwapCleaner::new(
                    std::path::PathBuf::from("/private/var/vm"),
                    Box::new(SystemCommandRunner),
                )),
            ),
            (
                "project-artifacts",
                Box::new(cleaners::project_artifacts::ProjectArtifactsCleaner::new(
                    Vec::new(),
                )),
            ),
            (
                "hints",
                Box::new(hint::KnownAppHintsCleaner::new(
                    home,
                    Box::new(SystemCommandRunner),
                )),
            ),
        ];
        for (label, cleaner) in pairs {
            assert_eq!(
                cleaner.name(),
                label,
                "history label {label} must match the cleaner's own name(): the history rows would silently fork"
            );
        }
    }

    #[test]
    fn standard_arm_labels_match_the_cleaner_names() {
        // Correspondence pin: the standard arm passes target.command_name()
        // (the macro table's cli name) to run_and_exit; the dispatched
        // cleaner's own name() must equal it, or `clean <target>` and
        // `clean all` would write history rows under two names. Rename the
        // cleaner side only and this pin fails. SUPPORTED_TARGETS is
        // deliberately not chained with SPECIAL_TARGETS: the special
        // targets carry no cleaner in standard_cleaners().
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = standard_cleaners(tmp.path(), &config::Config::default());
        let names: std::collections::HashSet<&str> = cleaners.iter().map(|c| c.name()).collect();
        for (cli, _) in SUPPORTED_TARGETS.iter() {
            assert!(
                names.contains(cli),
                "standard target {cli} has no cleaner with the same name(): the history label would silently fork"
            );
        }
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
        // PBI-83: the KNOWN_ENTRIES hint table as one target. Moderate on
        // purpose (never Safe): hint cleaning quits running apps and removes
        // their caches, so it never runs unattended.
        ("hints", cleaner::Risk::Moderate),
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
        let missing: Vec<&str> = ALL_TARGETS
            .iter()
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
        for (cli, _) in ALL_TARGETS.iter() {
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
    fn core_cleaners_is_the_standard_special_union() {
        // Registry pin (PBI-82 Phase 4): the shared core registry is exactly
        // standard_cleaners + special_cleaners in table order, so
        // all_cleaners / clean_by_name / filter_support cannot drift from
        // each other (the drift that produced the caches TrashMode bug).
        let tmp = tempfile::TempDir::new().unwrap();
        let core = core_cleaners(tmp.path(), &config::Config::default());
        let mut expected = standard_cleaners(tmp.path(), &config::Config::default());
        expected.extend(special_cleaners(tmp.path(), &config::Config::default()));
        let core_names: Vec<&str> = core.iter().map(|c| c.name()).collect();
        let expected_names: Vec<&str> = expected.iter().map(|c| c.name()).collect();
        assert_eq!(core_names, expected_names);
    }

    #[test]
    fn caches_group_cleaners_match_the_marked_table_rows() {
        // PBI-82 Phase 2 pin: the `@group caches` markers on the
        // define_cleaners! table generate this membership. The `clean caches`
        // arm no longer hand-lists the 7 cleaners: renaming or unmarking a
        // row changes this set, and a new cache row marked `@group caches`
        // joins the batch automatically.
        let tmp = tempfile::TempDir::new().unwrap();
        let names = names_of(&caches_group_cleaners(
            tmp.path(),
            &config::Config::default(),
        ));
        assert_eq!(
            names,
            vec!["bun", "go", "pip", "node-gyp", "npm", "yarn", "pnpm"]
        );
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
        let names: Vec<&str> = ALL_TARGETS.iter().map(|(n, _)| *n).collect();
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
        let registered: std::collections::HashSet<&str> =
            ALL_TARGETS.iter().map(|(n, _)| *n).collect();
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
        let registered: std::collections::HashSet<&str> =
            ALL_TARGETS.iter().map(|(n, _)| *n).collect();
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

    // ── PBI 73: the batch arms ride the shared reporters ───────────────────

    #[test]
    fn batch_arms_report_through_the_shared_helpers() {
        // The three batch arms (auto, caches, all) adopt the shared
        // report_batch_failures + report_total_freed instead of hand-writing
        // the failure, skipped, and "Total freed" lines. The single-target
        // run_clean_target path keeps its own report (its input type is
        // CleanResult, not the BatchSummary the shared reporter takes), so
        // the hand-written forms below must stay gone.
        let src = include_str!("main.rs");
        assert!(
            src.contains("interactive::report_batch_failures(")
                && src.contains("interactive::report_total_freed("),
            "the batch arms must route failures, skips, and the Total freed line through the shared reporters"
        );
        assert!(
            !src.contains("eprintln!(\"Error cleaning {name}: {e}\");"),
            "a hand-written batch failure report is back in main.rs"
        );
        assert!(
            !src.contains(
                "println!(\"Total freed: {}\", format::format_bytes(summary.total_freed));"
            ),
            "a hand-written Total freed line is back in main.rs"
        );
    }

    // ── PBI-50: the in-process clean seam ──────────────────────────────────

    /// Serializes the tests that touch process-global state: HOME (history
    /// writes resolve HOME inside `history::write_history_entry`) and the
    /// trash mode flag. Same pattern as `home_filters_an_empty_home_var`.
    static SEAM_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn seam_maps_the_tui_display_name_to_the_xcode_subcategory() {
        // The TUI passes display names from `sub_targets()`; the old
        // `--sub <display name>` re-exec silently failed
        // `XcodeSubcategory::parse` and cleaned DerivedData regardless of the
        // selection. The seam must accept both the CLI spellings and the
        // display names.
        assert_eq!(
            xcode_subcategory("DerivedData"),
            Some(cleaners::xcode::XcodeSubcategory::DerivedData)
        );
        assert_eq!(
            xcode_subcategory("Archives"),
            Some(cleaners::xcode::XcodeSubcategory::Archives)
        );
        assert_eq!(
            xcode_subcategory("derived-data"),
            Some(cleaners::xcode::XcodeSubcategory::DerivedData)
        );
        assert_eq!(
            xcode_subcategory("archives"),
            Some(cleaners::xcode::XcodeSubcategory::Archives)
        );
        assert_eq!(xcode_subcategory("no-such-sub"), None);
    }

    #[test]
    fn seam_returns_the_real_clean_result_for_a_sub_target_clean() {
        // The old TUI self re-exec fabricated `bytes_freed: 0` for sub-target
        // cleans and under-counted `Total freed`. The seam must run the real
        // clean in-process and return the cleaner's real result. Archives is
        // used so the mid-clean Xcode-running prompt (DerivedData-only check)
        // can never block on the test's stdin.
        let tmp = tempfile::TempDir::new().unwrap();
        let archives = tmp.path().join("Library/Developer/Xcode/Archives");
        std::fs::create_dir_all(archives.join("MyApp-2026")).unwrap();
        std::fs::write(archives.join("MyApp-2026/f"), vec![0u8; 4096]).unwrap();

        let _lock = SEAM_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // The trash backend is TCC-blocked in this sandbox: pin permanent
        // deletion so the seam's real clean actually deletes. History writes
        // resolve HOME internally: point it at the tempdir so the row lands
        // there instead of the developer HOME.
        let opts = CleanOptions {
            trash_mode: sasurahime::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let previous_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmp.path());
        let result = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "xcode",
            Some("Archives"),
            false,
            &sasurahime::progress::DeepSuppressReporter,
            &opts,
        );
        match previous_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let result = result.expect("the seam must resolve xcode + Archives");
        assert_eq!(result.name, "xcode");
        assert!(
            result.bytes_freed > 0,
            "the seam must return the REAL bytes freed, got {}",
            result.bytes_freed
        );
        assert!(
            !archives.join("MyApp-2026").exists(),
            "the in-process clean must have deleted the content"
        );
    }

    #[test]
    fn seam_honors_dry_run_and_never_deletes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let archives = tmp.path().join("Library/Developer/Xcode/Archives");
        std::fs::create_dir_all(archives.join("MyApp-2026")).unwrap();
        std::fs::write(archives.join("MyApp-2026/f"), vec![0u8; 4096]).unwrap();

        let result = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "xcode",
            Some("Archives"),
            true,
            &sasurahime::progress::DeepSuppressReporter,
            &CleanOptions::default(),
        )
        .expect("dry-run through the seam must succeed");
        assert_eq!(result.bytes_freed, 0, "dry run must not report freed bytes");
        assert!(
            archives.join("MyApp-2026").exists(),
            "dry run must not delete anything"
        );
    }

    #[test]
    fn seam_resolves_the_hints_sub_target_through_the_shared_runner() {
        // PBI-83: the TUI's sub-target selection (display names from
        // `sub_targets()`) rides the CleanByName seam. The seam shares the
        // `clean hints` CLI arm's runner (run_hints_clean), so the
        // validation and the cleaner construction have one owner.
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("Library/Logs/DiagnosticReports");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("crash-report.ips"), vec![0u8; 2_097_152]).unwrap();

        let _lock = SEAM_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // The trash backend is TCC-blocked in this sandbox: pin permanent
        // deletion so the seam's real clean actually deletes. History
        // writes resolve HOME internally: point it at the tempdir so the
        // row lands there instead of the developer HOME.
        let opts = CleanOptions {
            trash_mode: sasurahime::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let previous_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tmp.path());
        let result = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "hints",
            Some("Diagnostic Reports"),
            false,
            &sasurahime::progress::DeepSuppressReporter,
            &opts,
        );
        match previous_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        let result = result.expect("the seam must resolve hints + Diagnostic Reports");
        assert_eq!(result.name, "hints");
        assert!(
            result.bytes_freed > 0,
            "the seam must return the REAL bytes freed, got {}",
            result.bytes_freed
        );
        assert!(
            !dir.exists(),
            "the in-process clean must have deleted the content"
        );
    }

    #[test]
    fn seam_rejects_an_unknown_hints_sub_target() {
        // PBI 63 shape: an unknown sub-target spelling is an error, not a
        // silent no-op clean. The validation lives in the shared runner,
        // so the CLI arm and the seam refuse identically.
        let tmp = tempfile::TempDir::new().unwrap();
        let err = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "hints",
            Some("No Such App"),
            true,
            &sasurahime::progress::DeepSuppressReporter,
            &CleanOptions::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("unknown hints sub-target"),
            "got: {err}"
        );
    }

    #[test]
    fn seam_resolves_whole_targets_through_the_registry() {
        // sub = None goes through the same registry resolution the standard
        // dispatch uses. An empty tempdir home makes `clean xcode` a no-op
        // ("not found, skipping") that returns Ok with 0 freed.
        let tmp = tempfile::TempDir::new().unwrap();
        let result = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "xcode",
            None,
            true,
            &sasurahime::progress::DeepSuppressReporter,
            &CleanOptions::default(),
        )
        .expect("the seam must resolve the whole-target xcode entry");
        assert_eq!(result.name, "xcode");
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn seam_unknown_target_and_unknown_sub_are_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "no-such-target",
            None,
            false,
            &sasurahime::progress::DeepSuppressReporter,
            &CleanOptions::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("unknown clean target"),
            "got: {err}"
        );
        let err = clean_by_name(
            tmp.path(),
            &config::Config::default(),
            "xcode",
            Some("no-such-sub"),
            false,
            &sasurahime::progress::DeepSuppressReporter,
            &CleanOptions::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("unknown xcode subcategory"),
            "got: {err}"
        );
    }
}
