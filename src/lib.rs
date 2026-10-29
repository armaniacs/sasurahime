//! sasurahime core library: cache cleaners, trash handling, progress
//! reporting, and the scan/clean flow. The binary in `main.rs` is a thin
//! entry point over this library; splitting the library out lets
//! `cargo test --doc` compile and run the doc examples on pure helpers.
//!
//! # Safety rules
//!
//! * `detect()` must never delete anything.
//! * `clean(dry_run: true)` must never delete anything.
//! * mise runtime deletion cross-checks the global config and any
//!   `.mise.toml` within HOME before removing a version.
//! * macOS `uchg` immutable flags are cleared with `chflags -R nouchg`
//!   before a delete.

pub mod batch;
pub mod cleaner;
pub mod cleaners;
pub mod cli_fallback;
pub mod command;
pub mod config;
pub mod context;
pub mod disk_image;
pub mod doctor;
pub mod explorer;
pub mod format;
pub mod hint;
pub mod history;
pub mod installed_apps;
pub mod interactive;
pub mod progress;
pub mod prompt;
pub mod safety;
pub mod scan_cache;
pub mod scanner;
#[cfg(test)]
pub mod test_helpers;
pub mod trash;
pub mod unattended;
