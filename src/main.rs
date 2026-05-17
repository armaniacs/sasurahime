mod cleaner;
mod cleaners;
mod command;
mod format;
mod scanner;

use clap::{Parser, Subcommand};
use cleaner::Cleaner;
use command::SystemCommandRunner;
use dirs::home_dir;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sasurahime", about = "macOS developer cache cleaner")]
struct Cli {
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
}

fn home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().expect("cannot determine HOME directory"))
}

fn all_cleaners(home: &std::path::Path) -> Vec<Box<dyn cleaner::Cleaner>> {
    vec![
        Box::new(cleaners::uv::UvCleaner::new(home, Box::new(SystemCommandRunner))),
        Box::new(cleaners::brew::BrewCleaner::new(home, Box::new(SystemCommandRunner))),
    ]
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let home = home();

    match cli.command {
        Some(Commands::Scan) | None => {
            let cleaners = all_cleaners(&home);
            scanner::run_scan(&cleaners);
        }
        Some(Commands::Clean { target }) => match target {
            CleanTarget::Uv { dry_run } => {
                let cleaner = cleaners::uv::UvCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
            CleanTarget::Brew { dry_run } => {
                let cleaner = cleaners::brew::BrewCleaner::new(&home, Box::new(SystemCommandRunner));
                let result = cleaner.clean(dry_run)?;
                println!("Freed: {}", format::format_bytes(result.bytes_freed));
            }
        },
    }

    Ok(())
}
