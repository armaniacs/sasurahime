mod cleaner;
mod cleaners;
mod command;
mod config;
mod format;
mod interactive;
mod progress;
mod scanner;

use clap::{Parser, Subcommand};
use cleaner::Cleaner;
use command::SystemCommandRunner;
use dirs::home_dir;
use std::path::PathBuf;

const SUPPORTED_TARGETS: &[(&str, &str)] = &[
    ("uv", "Stale simple-vN index directories + uv cache prune"),
    ("brew", "Homebrew download cache"),
    ("mise", "Unused runtime versions"),
    ("browsers", "Old Puppeteer / Playwright builds"),
    ("bun", "Bun package cache"),
    ("go", "Go build cache"),
    ("pip", "pip package cache"),
    ("node-gyp", "node-gyp build cache directories"),
    ("npm", "npm package cache"),
    ("yarn", "yarn cache"),
    ("pnpm", "pnpm store"),
    (
        "caches",
        "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)",
    ),
    ("logs", "Log files older than N days"),
    ("xcode", "Xcode DerivedData project directories"),
];

#[derive(Parser)]
#[command(
    name = "sasurahime",
    version = env!("CARGO_PKG_VERSION"),
    about = "macOS developer cache cleaner"
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
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
                let cleaner = cleaners::uv::UvCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Brew { dry_run } => {
                let cleaner =
                    cleaners::brew::BrewCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Mise { dry_run } => {
                let cleaner =
                    cleaners::mise::MiseCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Browsers { dry_run } => {
                let cleaner =
                    cleaners::browser::BrowserCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Bun { dry_run } => {
                let result = cleaners::generic::GenericCleaner::bun(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Go { dry_run } => {
                let result = cleaners::generic::GenericCleaner::go(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Pip { dry_run } => {
                let result = cleaners::generic::GenericCleaner::pip(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::NodeGyp { dry_run } => {
                let result = cleaners::generic::GenericCleaner::node_gyp(
                    &home,
                    Box::new(SystemCommandRunner),
                )
                .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Npm { dry_run } => {
                let result = cleaners::generic::GenericCleaner::npm(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Yarn { dry_run } => {
                let result = cleaners::generic::GenericCleaner::yarn(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Pnpm { dry_run } => {
                let result = cleaners::generic::GenericCleaner::pnpm(Box::new(SystemCommandRunner))
                    .clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
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
                    match c.clean(dry_run) {
                        Ok(r) => total += r.bytes_freed,
                        Err(e) => eprintln!("Error cleaning {}: {e}", c.name()),
                    }
                }
                println!("Total freed: {}", format::format_bytes(total));
            }
            CleanTarget::Logs { dry_run, keep_days } => {
                let days = keep_days.unwrap_or(config.logs_keep_days);
                // Convert config::ExtraLogTarget → cleaners::log::OwnedLogTarget and append to defaults.
                let extra: Vec<cleaners::log::OwnedLogTarget> = config
                    .logs_extra_targets
                    .iter()
                    .map(|t| cleaners::log::OwnedLogTarget {
                        name: t.name.clone(),
                        path: config::Config::expand_tilde(&t.path, &home),
                        exclude: t.exclude.clone(),
                    })
                    .collect();
                let cleaner = cleaners::log::LogCleaner::new_with_extra(&home, days, extra);
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Xcode { dry_run } => {
                let cleaner =
                    cleaners::xcode::XcodeCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
        },
    }

    Ok(())
}
