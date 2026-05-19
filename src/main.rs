mod cleaner;
mod cleaners;
mod command;
mod config;
mod format;
mod interactive;
mod progress;
mod scanner;

use clap::{Parser, Subcommand};
use cleaner::{CleanResult, Cleaner};
use command::SystemCommandRunner;
use dirs::home_dir;
use std::path::PathBuf;

const SUPPORTED_TARGETS: &[(&str, &str)] = &[
    ("brew", "Homebrew download cache"),
    ("browsers", "Old Puppeteer / Playwright builds"),
    ("bun", "Bun package cache"),
    ("caches", "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)"),
    ("cargo", "Cargo registry cache + target/ directories"),
    ("cocoapods", "CocoaPods cache clean --all"),
    ("conda", "Conda clean --all"),
    ("deno", "Deno cache reload"),
    ("docker", "Docker system prune (images, containers, build cache)"),
    ("downloads", "~/Downloads old files"),
    ("go", "Go build cache"),
    ("gradle", "Gradle old version caches"),
    ("jetbrains", "JetBrains IDE caches (old versions)"),
    ("logs", "Log files older than N days"),
    ("mise", "Unused runtime versions"),
    ("node-gyp", "node-gyp build cache directories"),
    ("npm", "npm package cache"),
    ("orbstack", "Orbstack prune"),
    ("pip", "pip package cache"),
    ("pipx", "pipx cache and unused packages"),
    ("pnpm", "pnpm store"),
    ("poetry", "Poetry cache clear --all"),
    ("rustup", "Unused Rust toolchains"),
    ("spm", "SwiftPM cache directory"),
    ("trash", "~/.Trash size (scan only)"),
    ("uv", "Stale simple-vN index directories + uv cache prune"),
    ("xcode", "Xcode DerivedData project directories"),
    ("yarn", "yarn cache"),
];

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
    /// List supported cache targets
    Targets,
}

#[derive(Subcommand)]
enum CleanTarget {
    /// Clean uv package cache
    Uv {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Homebrew download cache
    Brew {
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove unused mise runtime versions
    Mise {
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove old Playwright / Puppeteer browser binaries
    Browsers {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean bun package cache
    Bun {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Go build cache
    Go {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean pip package cache
    Pip {
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove node-gyp build cache directories
    #[command(name = "node-gyp")]
    NodeGyp {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean npm package cache
    Npm {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean yarn package cache
    Yarn {
        #[arg(long)]
        dry_run: bool,
    },
    /// Prune pnpm store
    Pnpm {
        #[arg(long)]
        dry_run: bool,
    },
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
    /// Clean Cargo build cache
    Cargo {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Docker system cache
    Docker {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Orbstack cache
    Orbstack {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean CocoaPods cache
    CocoaPods {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean SwiftPM cache
    #[command(name = "spm")]
    SwiftPM {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Conda package cache
    Conda {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Poetry cache
    Poetry {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean pipx caches
    Pipx {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Deno cache
    Deno {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Rustup toolchains
    Rustup {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean Gradle caches
    Gradle {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean JetBrains IDE caches
    #[command(name = "jetbrains")]
    JetBrains {
        #[arg(long)]
        dry_run: bool,
    },
    /// Report Trash size
    Trash {
        #[arg(long)]
        dry_run: bool,
    },
    /// Clean old Downloads
    Downloads {
        #[arg(long)]
        dry_run: bool,
    },
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
    ]
}

/// Runs a single-target clean with spinner and prints freed bytes.
fn run_clean_target<F>(label: &str, cleaner_fn: F, dry_run: bool) -> anyhow::Result<()>
where
    F: FnOnce(bool) -> anyhow::Result<CleanResult>,
{
    let result =
        crate::progress::with_spinner(&format!("Cleaning {label}..."), || cleaner_fn(dry_run))?;
    println!("Freed: {}", format::format_bytes(result.bytes_freed));
    Ok(())
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

    match cli.command {
        Some(Commands::Scan) => {
            let cleaners = all_cleaners(&home, &config);
            scanner::run_scan(&cleaners);
        }
        Some(Commands::Targets) => {
            for (name, desc) in SUPPORTED_TARGETS {
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
        }
        Some(Commands::Clean { target }) => match target {
            CleanTarget::Uv { dry_run } => {
                run_clean_target(
                    "uv",
                    |dry| {
                        cleaners::uv::UvCleaner::new(&home, Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Brew { dry_run } => {
                run_clean_target(
                    "brew",
                    |dry| {
                        cleaners::brew::BrewCleaner::new(&home, Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Mise { dry_run } => {
                run_clean_target(
                    "mise",
                    |dry| {
                        cleaners::mise::MiseCleaner::new(&home, Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Browsers { dry_run } => {
                run_clean_target(
                    "browsers",
                    |dry| {
                        cleaners::browser::BrowserCleaner::new(&home, Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Bun { dry_run } => {
                run_clean_target(
                    "bun",
                    |dry| {
                        cleaners::generic::GenericCleaner::bun(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Go { dry_run } => {
                run_clean_target(
                    "go",
                    |dry| {
                        cleaners::generic::GenericCleaner::go(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Pip { dry_run } => {
                run_clean_target(
                    "pip",
                    |dry| {
                        cleaners::generic::GenericCleaner::pip(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::NodeGyp { dry_run } => {
                run_clean_target(
                    "node-gyp",
                    |dry| {
                        cleaners::generic::GenericCleaner::node_gyp(
                            &home,
                            Box::new(SystemCommandRunner),
                        )
                        .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Npm { dry_run } => {
                run_clean_target(
                    "npm",
                    |dry| {
                        cleaners::generic::GenericCleaner::npm(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Yarn { dry_run } => {
                run_clean_target(
                    "yarn",
                    |dry| {
                        cleaners::generic::GenericCleaner::yarn(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
            CleanTarget::Pnpm { dry_run } => {
                run_clean_target(
                    "pnpm",
                    |dry| {
                        cleaners::generic::GenericCleaner::pnpm(Box::new(SystemCommandRunner))
                            .clean(dry)
                    },
                    dry_run,
                )?;
            }
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
                    match crate::progress::with_spinner(
                        &format!("Cleaning {}...", c.name()),
                        || c.clean(dry_run),
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
                    |dry| cleaners::log::LogCleaner::new_with_extra(&home, days, extra).clean(dry),
                    dry_run,
                )?;
            }
            CleanTarget::Xcode { dry_run } => {
                let cleaner =
                    cleaners::xcode::XcodeCleaner::new(&home, Box::new(SystemCommandRunner));
                if cli.yes && cleaner.is_xcode_running() {
                    eprintln!("Note: Xcode is running. Proceeding with --yes anyway.");
                }
                run_clean_target("xcode", |dry| cleaner.clean(dry), dry_run)?;
            }
            CleanTarget::Cargo { dry_run } => {
                run_clean_target("cargo", |dry| {
                    cleaners::cargo::CargoCleaner::new(&home, Box::new(SystemCommandRunner))
                        .clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Docker { dry_run } => {
                run_clean_target("docker", |dry| {
                    cleaners::generic::GenericCleaner::docker(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Orbstack { dry_run } => {
                run_clean_target("orbstack", |dry| {
                    cleaners::generic::GenericCleaner::orbstack(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::CocoaPods { dry_run } => {
                run_clean_target("cocoapods", |dry| {
                    cleaners::generic::GenericCleaner::cocoapods(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::SwiftPM { dry_run } => {
                run_clean_target("spm", |dry| {
                    cleaners::generic::GenericCleaner::spm_cache(
                        &home,
                        Box::new(SystemCommandRunner),
                    )
                    .clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Conda { dry_run } => {
                run_clean_target("conda", |dry| {
                    cleaners::generic::GenericCleaner::conda(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Poetry { dry_run } => {
                run_clean_target("poetry", |dry| {
                    cleaners::generic::GenericCleaner::poetry(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Pipx { dry_run } => {
                run_clean_target("pipx", |dry| {
                    cleaners::generic::GenericCleaner::pipx(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Deno { dry_run } => {
                run_clean_target("deno", |dry| {
                    cleaners::generic::GenericCleaner::deno(Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Rustup { dry_run } => {
                run_clean_target("rustup", |dry| {
                    cleaners::rustup::RustupCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Gradle { dry_run } => {
                run_clean_target("gradle", |dry| {
                    cleaners::gradle::GradleCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::JetBrains { dry_run } => {
                run_clean_target("jetbrains", |dry| {
                    cleaners::gradle::JetBrainsCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
                }, dry_run)?;
            }
            CleanTarget::Trash { dry_run } => {
                run_clean_target("trash", |_dry| { todo!("TrashCleaner") }, dry_run)?;
            }
            CleanTarget::Downloads { dry_run } => {
                run_clean_target("downloads", |_dry| { todo!("DownloadsCleaner") }, dry_run)?;
            }
        },
    }

    Ok(())
}
