// PBI 102: the test injection seam for the select port. These tests walk the
// interactive select path (select → delete → freed display) through the port
// seam in-process:
//
// - the binary cannot be spawned with a mock port (the seam is in-process
//   state), and the TUI loop itself (`interactive::run_interactive`) is
//   TTY-gated and drives dialoguer directly — the pseudo-TTY harness that
//   could walk it is PBI-103's territory;
// - so the Outside-In entry is the top-level `Cleaner::clean()` of an
//   interactive-gated cleaner, called with a tempdir as HOME (the
//   tests/AGENTS.md "call the top-level function with a tempdir as HOME"
//   shape). `CleanPrompt::with_port` replaces the ambient TTY resolution so
//   the select resolves through a mock port that never asks for a real TTY;
// - the growth-variant mock grows the fixture INSIDE `multi_select` (the
//   user is reading the screen while the logs grow), which pins the freed
//   contract on the interactive path itself: freed must be the delete-time
//   on-disk measurement the select re-walks right before the deletion, never
//   the collect-time size (round 8, class C2).
//
// The mock ports speak the port contract only (Vec<usize> / bool): no
// dialoguer type crosses the boundary. Each test owns its mock and its
// statics, and the seam override is per-thread, so the tests stay parallel
// safe.

use anyhow::{bail, Result};
use sasurahime::cleaner::{CleanOptions, Cleaner};
use sasurahime::cleaners::library_logs::LibraryLogsCleaner;
use sasurahime::command::CommandRunner;
use sasurahime::format::dir_size;
use sasurahime::progress::ProgressReporter;
use sasurahime::prompt::{CleanPrompt, PromptPort};
use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::sync::Mutex;
use tempfile::TempDir;

// ── Fixtures ────────────────────────────────────────────────────────────────

/// A runner whose external calls (only `chflags`, under Permanent mode)
/// succeed; the deletion itself is std::fs under Permanent.
struct TestRunner;

impl CommandRunner for TestRunner {
    fn run(&self, program: &str, _args: &[&str]) -> Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: format!("{program} ok\n").into_bytes(),
            stderr: vec![],
        })
    }

    fn exists(&self, _program: &str) -> bool {
        true
    }
}

/// A reporter without live rendering: the mock port never asks for a screen.
struct QuietReporter;

impl ProgressReporter for QuietReporter {
    fn show_spinner(&self) -> bool {
        false
    }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _n: usize, _size: u64) {}
    fn progress_finish(&self) {}
}

/// One oversized log dir under the tempdir HOME, the same fixture shape the
/// library-logs drift pins use: `~/Library/Logs/<app>/logs/<file>`.
fn make_log_fixture(home: &Path) -> std::path::PathBuf {
    let app = home.join("Library/Logs/com.example.app");
    fs::create_dir_all(app.join("logs")).unwrap();
    fs::write(app.join("logs/a.log"), vec![1u8; 20_000]).unwrap();
    app
}

/// A library-logs cleaner over the tempdir HOME with the size threshold at
/// 1 byte, so the fresh 20 KB fixture is oversized (fresh mtimes are never
/// stale) and the interactive select is the only prompt the clean has.
fn make_cleaner(home: &Path) -> LibraryLogsCleaner {
    let mut cleaner = LibraryLogsCleaner::new(home, Box::new(TestRunner));
    cleaner.size_threshold = 1;
    cleaner
}

fn permanent_opts() -> CleanOptions {
    CleanOptions {
        trash_mode: sasurahime::trash::TrashMode::Permanent,
        ..Default::default()
    }
}

// ── Mock ports ──────────────────────────────────────────────────────────────

/// The growth variant (BDD scenario 1): the port records the display items
/// it received, grows the fixture INSIDE the select (the user is reading the
/// screen while the logs grow), measures the on-disk size after the growth,
/// then hands back the picked indices. The measurement it records is the
/// post-growth on-disk 実測 the freed number must equal.
struct GrowingSelectPort;

static GROW_TARGET: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);
static GROWTH_MEASURED: Mutex<Option<u64>> = Mutex::new(None);
static GROWTH_ITEMS: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());

impl PromptPort for GrowingSelectPort {
    fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
        bail!("the confirm path is not part of the select-port seam")
    }

    fn multi_select(
        &self,
        _name: &str,
        _reporter: &dyn ProgressReporter,
        _prompt: Option<&str>,
        items: &[String],
        _defaults: &[bool],
    ) -> Result<Option<Vec<usize>>> {
        GROWTH_ITEMS.lock().unwrap().push(items.to_vec());
        let app = GROW_TARGET
            .lock()
            .unwrap()
            .clone()
            .expect("fixture path installed before the clean");
        // The user is reading the screen: the log dir grows inside a
        // subdirectory (the top-level mtime is untouched, the PBI 93/97
        // drift shape), then the port measures what is on disk NOW.
        fs::write(app.join("logs/b.log"), vec![1u8; 4096])?;
        *GROWTH_MEASURED.lock().unwrap() = Some(dir_size(&app));
        Ok(Some(vec![0]))
    }
}

/// The empty-selection variant (BDD scenario 3): the port answers an empty
/// selection, which is the shared routing's other skip exit.
struct EmptySelectPort;

static EMPTY_ITEMS: Mutex<Vec<Vec<String>>> = Mutex::new(Vec::new());

impl PromptPort for EmptySelectPort {
    fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
        bail!("the confirm path is not part of the select-port seam")
    }

    fn multi_select(
        &self,
        _name: &str,
        _reporter: &dyn ProgressReporter,
        _prompt: Option<&str>,
        items: &[String],
        _defaults: &[bool],
    ) -> Result<Option<Vec<usize>>> {
        EMPTY_ITEMS.lock().unwrap().push(items.to_vec());
        Ok(Some(vec![]))
    }
}

/// The shared-routing variant: answers an empty selection and records the
/// picks it hands back in its own static, so the `multi_select_or_skip`
/// routing test below can tell the injected port from the ambient
/// NonInteractive fallback - both reach the same Ok(None) after the same
/// routing - and stays parallel-safe against the empty-selection cleaner
/// test (each mock owns its static).
struct EmptyRoutingPort;

static ROUTING_HANDBACK: Mutex<Option<Option<Vec<usize>>>> = Mutex::new(None);

impl PromptPort for EmptyRoutingPort {
    fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
        bail!("the confirm path is not part of the select-port seam")
    }

    fn multi_select(
        &self,
        _name: &str,
        _reporter: &dyn ProgressReporter,
        _prompt: Option<&str>,
        _items: &[String],
        _defaults: &[bool],
    ) -> Result<Option<Vec<usize>>> {
        let picks = Some(Vec::new());
        // The shared routing turns this handback and a NonInteractive None
        // into the same Ok(None), so the record is the only observable that
        // says the injected port was consulted: a broken seam (with_port
        // installing nothing) leaves the record None.
        *ROUTING_HANDBACK.lock().unwrap() = Some(picks.clone());
        Ok(picks)
    }
}

// ── E2E: the interactive select path with a mock port ──────────────────────

#[test]
fn interactive_select_freed_is_the_delete_time_measurement_not_the_collect_time_size() {
    let tmp = TempDir::new().unwrap();
    let app = make_log_fixture(tmp.path());
    let collect_size = dir_size(&app);

    *GROW_TARGET.lock().unwrap() = Some(app.clone());
    let _port = CleanPrompt::with_port(&GrowingSelectPort);

    let result = make_cleaner(tmp.path())
        .clean(false, &QuietReporter, &permanent_opts())
        .expect("the interactive-select clean succeeds");

    // The select went through the port seam: the mock saw the display items
    // the cleaner built for the select screen.
    let items = GROWTH_ITEMS.lock().unwrap();
    assert_eq!(items.len(), 1, "the port must be consulted exactly once");
    assert!(
        items[0].iter().any(|i| i.contains("com.example.app")),
        "the display items must reach the port: {:?}",
        items[0]
    );
    drop(items);
    *GROW_TARGET.lock().unwrap() = None;

    // The picked selection was deleted on disk.
    assert!(!app.exists(), "the selected entry must be deleted");

    // freed == 成長後の on-disk 実測: the number the select reports must be
    // the delete-time re-measurement, never the collect-time size.
    let grown = GROWTH_MEASURED
        .lock()
        .unwrap()
        .expect("the mock measured after the growth");
    assert!(
        grown > collect_size,
        "fixture: the dir must have grown inside the select ({grown} <= {collect_size})"
    );
    assert_eq!(
        result.bytes_freed, grown,
        "freed must be the delete-time on-disk measurement, not the \
         collect-time size ({collect_size})"
    );
    assert!(
        !result.uses_trash,
        "the pinned Permanent mode must never claim trash"
    );
    assert!(
        result.skipped.is_empty(),
        "every deletion must succeed: {:?}",
        result.skipped
    );
}

#[test]
fn interactive_select_empty_selection_takes_the_shared_skip_routing() {
    let tmp = TempDir::new().unwrap();
    let app = make_log_fixture(tmp.path());

    let _port = CleanPrompt::with_port(&EmptySelectPort);

    let result = make_cleaner(tmp.path())
        .clean(false, &QuietReporter, &permanent_opts())
        .expect("the empty-selection clean succeeds");

    // The port was consulted and answered an empty selection.
    assert_eq!(
        EMPTY_ITEMS.lock().unwrap().len(),
        1,
        "the port must be consulted exactly once"
    );

    // The shared routing: the empty-selection exit hands Ok(None) back, so
    // the clean skips and deletes nothing.
    assert_eq!(
        result.bytes_freed, 0,
        "an empty selection must delete nothing"
    );
    assert!(app.exists(), "an empty selection must not delete the entry");
}

#[test]
fn empty_selection_through_the_shared_select_path_hands_back_none() {
    // The routing pin at the shared ritual itself: the port's empty
    // selection is the skip exit (`[{name}] nothing selected` + Ok(None)),
    // not an error and not a picked list. The one-line stdout shape is
    // pinned by value in src/prompt.rs (an in-process test cannot capture
    // the process stdout this line prints to).
    //
    // The Ok(None) return alone is indistinguishable from the ambient
    // fallback: with no TTY the NonInteractive port answers the same
    // Ok(None) after the same routing, so a broken seam (with_port
    // installing nothing) would pass silently. The mock records the picks
    // it hands back, and the assertion below pins that the mock's
    // `Some(vec![])` - not a NonInteractive None - is what reached the
    // shared ritual.
    let _port = CleanPrompt::with_port(&EmptyRoutingPort);
    let items = vec!["com.example.app  (2.5 KB)  [large 2.5 KB]".to_string()];
    let picks = sasurahime::prompt::multi_select_or_skip(
        "library-logs",
        &QuietReporter,
        None,
        &items,
        &[false],
    )
    .expect("an empty selection is a None, not an error");
    assert!(
        picks.is_none(),
        "the shared routing must hand Ok(None) back to the cleaner"
    );
    // Outer Some = the injected port was consulted at all; inner
    // Some(vec![]) = the empty selection it handed the ritual. A None
    // record means the seam resolved the ambient NonInteractive instead -
    // the same Ok(None) from a port that never saw the display items.
    let handback = ROUTING_HANDBACK.lock().unwrap().clone();
    assert_eq!(
        handback,
        Some(Some(Vec::new())),
        "the shared ritual must have consumed the mock's Some(vec![]); a \
         None record means the seam installed nothing and the ambient \
         NonInteractive resolved instead"
    );
}
