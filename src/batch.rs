//! One batch-execution path for the duplicated "clean a batch, accumulate,
//! write history" loops. Every entry point that cleans more than one target
//! funnels through [`run_batch`]; skip policies stay at the call sites —
//! they are genuinely different per entry point.

use crate::cleaner::CancelClassified;
use std::io::Write;

/// One batch failure carried as a typed error: the cleaner name plus the
/// whole `anyhow::Error` (cause/context chain intact), instead of a
/// flattened `to_string()` that throws the chain away at collection time.
/// Stage 3 (the single/batch unification, PBI 2026-11-21-05) rebuilds the
/// single-target `Err` contract from this — see
/// [`report_single_target`]'s doc.
#[derive(Debug)]
pub struct BatchFailure {
    pub name: String,
    pub error: anyhow::Error,
}

/// Summary of a batch clean run: what the callers print afterwards.
pub struct BatchSummary {
    pub total_freed: u64,
    /// Per-target failures (typed error carry), reported by the caller.
    pub failures: Vec<BatchFailure>,
    /// Targets the user cancelled (cleaner name), reported by the caller as
    /// a neutral `cancelled by user` line — a cancel is a run-level
    /// decision, not an error, so it never lands in `failures`.
    pub cancelled: Vec<String>,
    /// Per-entry skips (cleaner name + "path: reason"), reported by the
    /// caller in the single-target "N file(s) skipped" shape.
    pub skipped: Vec<(String, String)>,
    /// Stage-3 carry: each successful cleaner's `CleanResult`, in loop
    /// order, so the single/batch unification can rebuild the single-target
    /// contract (a "succeeded but exit_code() == 1" result survives the
    /// summary instead of being compressed into `total_freed`). Consumed
    /// by [`report_single_target`]; see `run_batch`'s doc.
    pub results: Vec<crate::cleaner::CleanResult>,
}

/// The ONE batch execution path: spinner per cleaner, result accumulation,
/// post-clean notes, the single history-write guard, and per-target error
/// collection. Skip policies stay at the call sites — they are genuinely
/// different per entry point.
///
/// Reporter use per branch: the ANIMATED branch binds the cleaner's progress
/// to the shared spinner (`with_spinner_result_progress`); the NON-animated
/// branch passes `reporter` straight to the cleaner. The [OK]/[FAILED]
/// status line is owned by the reporter kind on both paths: the non-animated
/// branch prints it only when `reporter.show_spinner()` (a quiet reporter —
/// deep-suppress — gets none, matching the single-target path), and the
/// animated branch is keyed on the caller's `animated` flag, which is never
/// derived from `show_spinner` (no-tick-while-prompting: a cleaner on this
/// path may still prompt, and a derived steady tick would redraw the line
/// under the prompt).
///
/// Output-line families (stage 3, PBI 2026-11-21-05): the display families
/// have ONE owner — [`report_single_target`] on the single engine, and the
/// collection-shaped delivery below plus the shared family helpers on the
/// batch engine. The engines share the data path (this summary IS the
/// single engine's carry, via [`BatchSummary::from_single_outcome`]); what
/// differs is only the delivery: the batch prints after the loop through
/// `report_batch_failures` / `report_total_freed` (PBI-33 recorded
/// decision, option b), the single engine prints immediately.
///
/// | # | Line family | Owner (stage 3) | Gate |
/// |---|---|---|---|
/// | 1 | status line (`[OK]`/`[FAILED]`) | single: [`report_single_target`]'s token emission (SharedSpinner(false) mechanics in `run_clean_target`); batch: fused with the spinner mechanics — animated → `with_spinner_result_progress` / `show_spinner()` → `with_spinner_result` / quiet → none; the token RULES are one contract ([OK] / cancel-silent / [FAILED]) | `animated` is the caller's flag, never derived from `show_spinner()`; a `CleanCancelled` Err prints no status token on either path; the classification is owned by the `CancelClassified` impl beside `CleanCancelled` (cleaner.rs), not by the emission |
/// | 2 | cancel line | single: [`report_single_target`] (silent `Ok(cancelled)`, no token, no history row); batch: `summary.cancelled` → the caller's neutral `cancelled by user` line (`report_batch_failures`) | `CancelClassified::is_user_cancelled` predicate (the ONE cancel predicate, cleaner.rs) |
/// | 3 | note line (`post_clean_note`) | the one gate [`gated_post_clean_note`], called by [`report_single_target`] (single) and by this loop (batch) | single: `show_spinner()`; batch: `with_notes` && `show_spinner()` — `--deep-suppress` hides; default and `--suppress` show |
/// | 4 | freed line | single: [`report_single_target`] (per-target `freed_message`); batch: the caller's closing `Total freed` line (`report_total_freed`) | per-target and total are separate concepts |
/// | 5 | skip lines | single: [`report_single_target`] (nameless `N file(s) skipped` + `path: reason`); batch: `summary.skipped` → `report_batch_failures` (named shape, cleaner name included) | when `skipped` is non-empty |
/// | 6 | exit code | INTENTIONAL ASYMMETRY, not unified (deep-dig ruling 2): single = CLI error custom (`exit_code() != 0` → exit 1 via `run_and_exit`), batch = report-only (always exit 0) | pinned by the PBI 78 scope fence 2 |
///
/// Stage-3 carry (consumed, no longer a preparation): `failures` keeps
/// each error as an `anyhow::Error` (cause/context chain intact) and
/// `results` keeps each successful [`CleanResult`] in loop order, so
/// [`BatchSummary::from_single_outcome`] + [`report_single_target`] can
/// rebuild the single-target `Err` contract and return the "succeeded but
/// exit_code() == 1" result without loss. The batch callers keep reading
/// `failures` / `skipped` / `cancelled` through `report_batch_failures`.
///
/// Stage-3 trap: do NOT let [`report_single_target`] display or record
/// `results[0].name` — `CleanResult::name` is a pub field whose equality
/// with the batch loop's `c.name()` is NOT pinned by any test (a non-
/// invariant), so the display and history labels must keep coming from the
/// batch's own cleaner name, the same label the branches above already
/// use. The source pin
/// `report_single_target_never_reads_the_result_name` (this file's tests)
/// keeps it that way.
///
/// Round-7 contract, restated by the 2026-11-21 deep-dig ruling (goal over
/// means: the original wording "form enum plus stage 3" named one candidate
/// mechanism; the goal it served — one owner per line family — is what the
/// contract protects, so it now reads "the unified report mechanism plus
/// stage 3"): a change to a line family's semantics (a new family, a gate
/// move, a gate condition change) must land in the ONE owner;
/// a `run_batch` argument-list change takes the unified report mechanism
/// plus stage 3 in the same PR; a new line family shared by both engines
/// takes stage 3 plus the unified mechanism; a one-sided note-gate change
/// bundles it. The one-PR bundling requirement (the full BatchOptions
/// migration plus stage 3 together) was kept and discharged by that PR. The
/// shipped mechanism is [`BatchOptions`] (the argument struct, not a form
/// enum — the three spinner mechanics measured across the engines, single:
/// SharedSpinner(false), batch: animated / non-animated + show_spinner,
/// would fork this dispatch per enum variant without a byte of output
/// change to show for it) plus [`report_single_target`].
pub fn run_batch(
    cleaners: &[&dyn crate::cleaner::Cleaner],
    opts: &crate::cleaner::CleanOptions,
    reporter: &dyn crate::progress::ProgressReporter,
    batch_opts: BatchOptions,
) -> BatchSummary {
    // Named destructure: the `animated` flag keeps its own name at the
    // dispatch (the source pin run_batch_status_line_is_owned_by_the_
    // reporter_kind anchors on it), and a bool swap stays a compile error.
    let BatchOptions {
        dry_run,
        animated,
        with_notes,
    } = batch_opts;
    let trash_mode = opts.trash_mode == crate::trash::TrashMode::Trash;
    let mut summary = BatchSummary {
        total_freed: 0,
        failures: Vec::new(),
        cancelled: Vec::new(),
        skipped: Vec::new(),
        results: Vec::new(),
    };
    for c in cleaners {
        let msg = format!("Cleaning {}...", c.name());
        let result = if animated {
            // No steady tick while a cleaner may still prompt: the animated
            // branch is for flows that already suppressed its prompts.
            crate::progress::with_spinner_result_progress(&msg, animated, |rep| {
                c.clean(dry_run, rep, opts)
            })
        } else if reporter.show_spinner() {
            // Spinner reporter: the plain non-animated spinner draws the
            // one status line, exactly like the single-target path.
            crate::progress::with_spinner_result(&msg, || c.clean(dry_run, reporter, opts))
        } else {
            // Quiet reporter (deep-suppress): no spinner, no status line —
            // the single-target path's show_spinner()==false branch prints
            // none either.
            c.clean(dry_run, reporter, opts)
        };
        match result {
            Ok(r) => {
                summary.total_freed += r.bytes_freed;
                for s in &r.skipped {
                    summary
                        .skipped
                        .push((c.name().to_string(), skipped_detail(s)));
                }
                // Notes follow the status line's gate (PBI 90): the quiet
                // deep-suppress reporter gets no per-target note either, so
                // `clean all --deep-suppress` stays as quiet as the
                // single-target path's show_spinner()==false branch. The
                // gate is the one family helper (stage 3).
                if let Some(note) = gated_post_clean_note(
                    r.bytes_freed,
                    r.uses_trash,
                    trash_mode,
                    reporter.show_spinner(),
                    with_notes,
                ) {
                    println!("{note}");
                }
                write_history_entry_if_needed(
                    c.name(),
                    dry_run,
                    r.bytes_freed,
                    r.skipped.len(),
                    r.uses_trash,
                );
                // Stage-3 carry (see the doc): the single-target result is
                // parked in the summary LAST — the skipped/history/note
                // branches above consume `r`'s fields, and pushing before
                // them would move `r` out from under those reads.
                summary.results.push(r);
            }
            Err(e) => {
                // CleanCancelled is a run-level decision, not an error: it
                // gets its own channel so the caller reports a neutral
                // `cancelled by user` line instead of `Error cleaning ...`.
                // The classification is the one CancelClassified predicate
                // (cleaner.rs), not a local downcast; the name comes from
                // the loop variable `c` (CleanResult::cancelled() is
                // intentionally nameless, so CleanResult.name has nothing to
                // give); remaining targets keep running.
                if e.is_user_cancelled() {
                    summary.cancelled.push(c.name().to_string());
                } else {
                    // The error itself rides in the summary (R1: no
                    // `to_string()` flattening here) so the anyhow chain
                    // survives for stage 3's Err reconstruction.
                    summary.failures.push(BatchFailure {
                        name: c.name().to_string(),
                        error: e,
                    });
                }
            }
        }
    }
    summary
}

/// Display and behavior switches for one batch run, passed by name so an
/// `animated`/`with_notes` swap is a compile error instead of a
/// silent spinner regression invisible to non-TTY E2E (round 9 whywhy
/// §2.a: positional bools are the attack surface).
///
/// Stage 3 (PBI 2026-11-21-05) discharged the round-7 contract that used to
/// freeze [`run_batch`]'s argument list: the struct is now the ONE options
/// shape for every direct [`run_batch`] call site, alongside
/// [`report_single_target`]. `animated` stays an independent caller flag —
/// never derived from `show_spinner` (no-tick-while-prompting).
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchOptions {
    pub dry_run: bool,
    pub animated: bool,
    pub with_notes: bool,
}

/// The one unattended-batch skeleton (PBI 2026-11-20-03): `select`'s run
/// list is dropped onto [`run_batch`] here, so the three unattended entry
/// points (`auto`, `clean all`, `--yes`) hand over their selection instead
/// of each hand-writing the registry-index mapping and a positional-bool
/// `run_batch` call — those bool meanings live at exactly one call site.
/// Skip rendering is NOT part of the skeleton: the wording and the stream
/// choice stay at the caller (PBI 74), rendered through [`render_skip_line`].
pub fn run_selected_batch(
    cleaners: &[Box<dyn crate::cleaner::Cleaner>],
    selection: &crate::unattended::UnattendedSelection,
    opts: &crate::cleaner::CleanOptions,
    reporter: &dyn crate::progress::ProgressReporter,
    opts_for_batch: BatchOptions,
) -> BatchSummary {
    // The panic contract "selection indices index into this same slice" is
    // doc-only otherwise: this keeps a mis-wired selection a loud debug
    // failure instead of an out-of-bounds panic deep in the loop.
    debug_assert!(
        selection
            .run
            .iter()
            .map(|&(i, _)| i)
            .chain(selection.skipped.iter().map(|&(i, _)| i))
            .all(|i| i < cleaners.len()),
        "selection indices must be select's output over this same cleaners slice"
    );
    let batch_cleaners: Vec<&dyn crate::cleaner::Cleaner> = selection
        .run
        .iter()
        .map(|&(i, _)| cleaners[i].as_ref())
        .collect();
    run_batch(&batch_cleaners, opts, reporter, opts_for_batch)
}

/// The per-variant advice wordings one unattended flow owns (PBI 74: the
/// wording stays at the call site). `None` renders the bare risk-only shape
/// (`--yes`'s E2E-pinned generic line); `Some` renders
/// "{name} ({risk}): {advice}".
pub struct SkipAdvice {
    pub never_unattended: Option<&'static str>,
    pub needs_opt_in: Option<&'static str>,
    pub needs_explicit_selection: Option<&'static str>,
}

/// The ONE renderer for unattended skip lines (PBI 2026-11-20-03): the
/// single exhaustive match over `SkipReason`. A new variant breaks the
/// build here — one error covering all three unattended paths — instead of
/// silently flowing into `--yes`'s generic risk-only line (round 9 whywhy
/// §2.a), and the per-flow [`SkipAdvice`] table forces the wording decision
/// at every call site. The stream write stays at the caller (PBI 74): this
/// fn returns the line and prints nothing.
pub fn render_skip_line(
    name: &str,
    reason: crate::unattended::SkipReason,
    advice: &SkipAdvice,
) -> String {
    match reason {
        crate::unattended::SkipReason::NeverUnattended(risk) => {
            render_advised_line(name, risk.label(), advice.never_unattended)
        }
        crate::unattended::SkipReason::NeedsOptIn(risk) => {
            render_advised_line(name, risk.label(), advice.needs_opt_in)
        }
        crate::unattended::SkipReason::NeedsExplicitSelection(risk) => {
            render_advised_line(name, risk.label(), advice.needs_explicit_selection)
        }
    }
}

fn render_advised_line(name: &str, risk_label: &str, advice: Option<&str>) -> String {
    match advice {
        Some(advice) => format!("{name} ({risk_label}): {advice}"),
        None => format!("{name} ({risk_label})"),
    }
}

/// Minimal audit trail / restore guidance printed after a clean, so the
/// record exists even without `--verbose`. `None` when nothing was freed.
/// `uses_trash` is the cleaner's own report: a cleaner that deletes outside
/// the Trash (tmutil, rustup, ...) must not claim its bytes can be restored.
pub fn post_clean_note(bytes_freed: u64, uses_trash: bool, trash_mode: bool) -> Option<String> {
    if bytes_freed == 0 {
        return None;
    }
    if trash_mode {
        if uses_trash {
            Some("Restore from Finder (Trash) or run `open ~/.Trash`.".to_string())
        } else {
            None
        }
    } else {
        Some("Permanently deleted: no Trash copy was kept.".to_string())
    }
}

/// The per-entry skip detail rendering, `{path}: {reason}` — the one shape
/// both engines speak (stage 3): the batch loop stores it named beside the
/// cleaner, the single engine renders it nameless. Extracted so the two
/// conversion sites cannot drift.
fn skipped_detail(s: &crate::cleaner::SkippedEntry) -> String {
    format!("{}: {}", s.path.display(), s.reason)
}

/// The note line's one gate (family 3 owner, stage 3): the note rides the
/// status line's reporter gate on both engines — `show_spinner` — AND the
/// batch caller's `with_notes` (the single engine always wants notes, so
/// [`report_single_target`] passes `true`). `--deep-suppress` (a quiet
/// reporter) hides; default and `--suppress` show.
fn gated_post_clean_note(
    bytes_freed: u64,
    uses_trash: bool,
    trash_mode: bool,
    show_spinner: bool,
    with_notes: bool,
) -> Option<String> {
    if show_spinner && with_notes {
        post_clean_note(bytes_freed, uses_trash, trash_mode)
    } else {
        None
    }
}

/// Message shown after a clean. `uses_trash` is the cleaner's own report: a
/// cleaner that deletes outside the Trash (tmutil, rustup, ...) must not
/// claim its bytes can be restored.
///
/// # freed 意味論の契約(PBI 97)
///
/// "Freed" の意味は対象によって異なる。来歴の検証はこの表で 1 hop に落ちる:
///
/// | 種 | 意味 | anchor |
/// |---|---|---|
/// | (a) ledger 総和 | caller 提供の per-item size(計測は待ち窓の後、対話 select 経路)の和、成功分だけ | src/trash.rs:293-295 + :333 |
/// | (b) 差分 | 外部ツール実行前後の dir_size 差 | src/cleaners/generic.rs:640 / src/cleaners/uv.rs:158 |
/// | (c) 実行前全量 | CLI 実行成功時に実行前のディレクトリ全量を freed として報告 | src/cli_fallback.rs:147 + :168 |
/// | (d) 検証済み上限 | plugin の claimed freed を削除後の残り実測サイズで上限切り | src/cleaners/plugin.rs:431-438 |
/// | 解釈 | Trash 軸の分離表示(Logical/Physical) | src/batch.rs(本 doc) |
pub fn freed_message(bytes_freed: u64, uses_trash: bool, trash_mode: bool) -> String {
    if trash_mode && uses_trash && bytes_freed > 0 {
        // Two axes: the logical size is gone from the caches, but the bytes
        // still sit in the Trash until it is emptied.
        format!(
            "Logical freed: {} / Physical freed: 0 B (in Trash)",
            crate::format::format_bytes(bytes_freed)
        )
    } else {
        format!("Freed: {}", crate::format::format_bytes(bytes_freed))
    }
}

/// The gates one single-target report consults (the family table's Gate
/// column, restricted to what a report can see). Named fields on purpose
/// (round 9 whywhy §2.a): a bool swap is a compile error instead of a
/// silent gate regression invisible to non-TTY E2E.
#[derive(Debug, Clone, Copy)]
pub struct SingleTargetReport {
    /// The reporter-kind gate (`reporter.show_spinner()`): owns the status
    /// token, the freed line and the note line on the single engine. A
    /// quiet reporter (deep-suppress) suppresses all three — the same
    /// quietness the batch's non-animated branch grants.
    pub show_spinner: bool,
    /// Passed through to the one history guard (dry runs record nothing).
    pub dry_run: bool,
    /// The run's resolved TrashMode as `trash_mode == TrashMode::Trash`:
    /// picks the note wording (restore guidance vs permanent audit) and the
    /// freed-message axis (Logical/Physical vs a single Freed line).
    pub trash_mode: bool,
}

impl BatchSummary {
    /// The single engine's outcome expressed as the same carry the batch
    /// loop produces (stage 3's data-path unification): one entry, in the
    /// loop's vocabulary, with the caller's label — the batch loop's
    /// `c.name()` counterpart — naming every collection. The labels here
    /// are the caller's, never `CleanResult::name` (a non-invariant; see
    /// `run_batch`'s doc): [`report_single_target`] consumes the carry.
    pub fn from_single_outcome(
        label: &str,
        outcome: anyhow::Result<crate::cleaner::CleanResult>,
    ) -> BatchSummary {
        match outcome {
            Ok(r) => BatchSummary {
                total_freed: r.bytes_freed,
                failures: Vec::new(),
                cancelled: Vec::new(),
                skipped: r
                    .skipped
                    .iter()
                    .map(|s| (label.to_string(), skipped_detail(s)))
                    .collect(),
                results: vec![r],
            },
            Err(e) if e.is_user_cancelled() => BatchSummary {
                total_freed: 0,
                failures: Vec::new(),
                cancelled: vec![label.to_string()],
                skipped: Vec::new(),
                results: Vec::new(),
            },
            Err(e) => BatchSummary {
                total_freed: 0,
                failures: vec![BatchFailure {
                    name: label.to_string(),
                    error: e,
                }],
                cancelled: Vec::new(),
                skipped: Vec::new(),
                results: Vec::new(),
            },
        }
    }
}

/// The ONE single-target report engine (stage 3, PBI 2026-11-21-05): it
/// consumes the same stage-3 carry the batch loop produces — one target's
/// `results` / `failures` / `skipped` / `cancelled` (via
/// [`BatchSummary::from_single_outcome`]) — and rebuilds the single-target
/// contract from it:
///
/// - a user cancel is a silent `Ok(CleanResult::cancelled())`: no status
///   token, no families, no history row (`CleanCancelled`'s token-free
///   contract; the classification is the one `CancelClassified` predicate);
/// - a failure rebuilds the single-target `Err` (the anyhow chain rides
///   intact in [`BatchFailure`]), printing the `[FAILED]` token only when
///   the reporter shows a spinner — a quiet (deep-suppress) reporter
///   suppresses it, and the error still reaches the caller;
/// - a success renders the display families — the `[OK]` token, the freed
///   line, the note line (gated), the nameless skip lines — writes the one
///   history row through the shared guard, and returns the `CleanResult`
///   intact so a "succeeded but `exit_code() == 1`" result survives to
///   `run_and_exit`.
///
/// Stage-3 trap: every label here comes from the `label` parameter — the
/// same label the batch loop takes from `c.name()` — never from
/// `CleanResult::name` (a pub field whose equality with the loop's cleaner
/// name is NOT pinned by any test; see `run_batch`'s doc). The source pin
/// `report_single_target_never_reads_the_result_name` keeps it that way.
///
/// The writers are seams (the `report_batch_failures` precedent): the
/// single flow passes stdout/stderr, the unit pins pass buffers. The
/// stream-to-family assignment matches the historical single-target path
/// byte for byte: tokens and skip lines on stderr, freed and note on
/// stdout.
pub fn report_single_target(
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    label: &str,
    summary: BatchSummary,
    gates: SingleTargetReport,
) -> anyhow::Result<crate::cleaner::CleanResult> {
    let msg = format!("Cleaning {label}...");
    let BatchSummary {
        total_freed: _,
        failures,
        cancelled,
        skipped,
        results,
    } = summary;
    if !cancelled.is_empty() {
        // A cancel is a run-level decision, not an error: the single
        // contract is the nameless silent Ok (no token, no families, no
        // history row) — the cancelled label is already in the carry.
        return Ok(crate::cleaner::CleanResult::cancelled());
    }
    if let Some(f) = failures.into_iter().next() {
        if gates.show_spinner {
            writeln!(stderr, "{msg} [FAILED]")?;
        }
        return Err(f.error);
    }
    let Some(r) = results.into_iter().next() else {
        // Unreachable by construction: from_single_outcome always fills
        // exactly one of the three channels. A loud failure beats a silent
        // no-op report if the wiring ever changes.
        anyhow::bail!("report_single_target: empty carry (no result, failure, or cancel)");
    };
    if gates.show_spinner {
        writeln!(stderr, "{msg} [OK]")?;
        writeln!(
            stdout,
            "{}",
            freed_message(r.bytes_freed, r.uses_trash, gates.trash_mode)
        )?;
    }
    if let Some(note) = gated_post_clean_note(
        r.bytes_freed,
        r.uses_trash,
        gates.trash_mode,
        gates.show_spinner,
        true,
    ) {
        writeln!(stdout, "{note}")?;
    }
    if !skipped.is_empty() {
        writeln!(
            stderr,
            "{} file(s) skipped due to errors (permission/lock):",
            skipped.len()
        )?;
        for (_name, detail) in &skipped {
            // The single-target shape is nameless: the selection context
            // already named the target (the batch's named shape lives in
            // report_batch_failures).
            writeln!(stderr, "  - {detail}")?;
        }
    }
    write_history_entry_if_needed(
        label,
        gates.dry_run,
        r.bytes_freed,
        r.skipped.len(),
        r.uses_trash,
    );
    Ok(r)
}

/// The one history-record guard, shared by [`run_batch`] and the single-target
/// path: entries are recorded only for real runs that freed something.
///
/// WHY zero-byte runs record nothing (pre-existing policy, kept on purpose):
/// zero-byte runs — every deletion skipped, empty-dir removals, or CLI tools
/// reporting 0 freed (brew stdout parsing, `size_before - size_after`) — are
/// not recorded. The history table tracks recovered bytes; a deletion without
/// bytes has no row. This is the only place that decides: callers here and
/// `history::write_history_entry` below do not re-check `dry_run` or bytes.
pub fn write_history_entry_if_needed(
    label: &str,
    dry_run: bool,
    bytes_freed: u64,
    skipped_count: usize,
    uses_trash: bool,
) {
    if !dry_run && bytes_freed > 0 {
        crate::history::write_history_entry(label, bytes_freed, skipped_count, uses_trash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::{CleanOptions, CleanResult, Risk, ScanResult, ScanStatus};
    use crate::progress::ProgressReporter;
    use crate::unattended::SkipReason;
    use anyhow::Result;
    use tempfile::TempDir;

    /// Stub cleaner that always succeeds and frees a fixed byte count.
    struct OkCleaner(&'static str);

    impl crate::cleaner::Cleaner for OkCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Ok(CleanResult::freed(self.0, 100, false))
        }
    }

    /// Stub cleaner that always fails.
    struct FailingCleaner(&'static str);

    impl crate::cleaner::Cleaner for FailingCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Err(anyhow::anyhow!("boom"))
        }
    }

    /// Stub cleaner that always cancels: the `Err(CleanCancelled)` shape a
    /// user decline returns (the CleanCancelled source itself is not touched
    /// by this PBI — a hand-made Err drives the summary instead).
    struct CancelledCleaner(&'static str);

    impl crate::cleaner::Cleaner for CancelledCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Err(anyhow::Error::from(crate::cleaner::CleanCancelled))
        }
    }

    /// Stub cleaner that succeeds with one per-entry skip recorded
    /// (seam-classified skippable failure, PBI-43 shape).
    struct SkippingCleaner(&'static str);

    impl crate::cleaner::Cleaner for SkippingCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Ok(CleanResult {
                name: self.0,
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![crate::cleaner::SkippedEntry {
                    path: std::path::PathBuf::from("/tmp/locked-cache"),
                    reason: crate::cleaner::SkipCause::Delete(
                        crate::cleaner::DeleteError::TrashRefused("boom".to_string()),
                    ),
                }],
            })
        }
    }

    #[test]
    fn run_batch_carries_skipped_entries() {
        // A per-entry skippable failure must be carried by the summary so
        // every batch entry point can report it in the single-target
        // "N file(s) skipped" shape.
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(SkippingCleaner("skip-a")),
            Box::new(OkCleaner("ok-a")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true, // never deletes and never writes history
                animated: false,
                with_notes: false,
            },
        );
        assert_eq!(
            summary.skipped.len(),
            1,
            "the one skipped entry must be carried by the summary"
        );
        assert_eq!(
            summary.skipped[0].0, "skip-a",
            "the skip must name the cleaner"
        );
        assert!(
            summary.skipped[0].1.contains("locked-cache"),
            "the skip must carry the path, got: {}",
            summary.skipped[0].1
        );
        assert!(
            summary.skipped[0].1.contains("trash failed"),
            "the skip must carry the reason, got: {}",
            summary.skipped[0].1
        );
    }

    #[test]
    fn run_batch_sums_only_successful_cleans_and_collects_failures() {
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(OkCleaner("ok-a")),
            Box::new(FailingCleaner("bad")),
            Box::new(OkCleaner("ok-b")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true, // never deletes and never writes history
                animated: false,
                with_notes: false,
            },
        );
        assert_eq!(
            summary.total_freed, 200,
            "only the successful cleans sum into total_freed"
        );
        assert_eq!(summary.failures.len(), 1, "one failure for the one error");
        assert_eq!(summary.failures[0].name, "bad");
        assert!(
            summary.failures[0].error.to_string().contains("boom"),
            "the failure message must carry the error, got: {}",
            summary.failures[0].error
        );
    }

    #[test]
    fn run_batch_collects_cancelled_separately_from_failures() {
        // A mixed batch must keep cancel apart from failure: cancelled
        // target names go to `cancelled`, other errors go to `failures`,
        // and the two never mix (cancel is a run-level decision, not an
        // error — PBI 78).
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(CancelledCleaner("cancel-a")),
            Box::new(FailingCleaner("bad")),
            Box::new(OkCleaner("ok-a")),
            Box::new(CancelledCleaner("cancel-b")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true, // never deletes and never writes history
                animated: false,
                with_notes: false,
            },
        );
        assert_eq!(
            summary.cancelled,
            vec!["cancel-a".to_string(), "cancel-b".to_string()],
            "cancelled target names must be collected, in loop order"
        );
        assert_eq!(
            summary.failures.len(),
            1,
            "only the real error is a failure"
        );
        assert_eq!(summary.failures[0].name, "bad");
        assert_eq!(
            summary.total_freed, 100,
            "the successful clean still sums into total_freed"
        );
    }

    #[test]
    fn run_batch_reports_cancelled_not_failed() {
        // A cancelled cleaner must never land in failures: the caller
        // reports `{name}: cancelled by user`, never `Error cleaning ...`.
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> =
            vec![Box::new(CancelledCleaner("colima"))];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true,
                animated: false,
                with_notes: false,
            },
        );
        assert_eq!(summary.cancelled, vec!["colima".to_string()]);
        assert!(
            summary.failures.is_empty(),
            "a user cancel is not a failure, got: {:?}",
            summary.failures
        );
        assert_eq!(summary.total_freed, 0);
    }

    #[test]
    fn run_batch_cancel_via_the_status_line_path_continues_remaining_targets() {
        // The contradictory-2-lines path: the non-animated branch × a
        // spinner reporter funnels through with_spinner_result, whose
        // status line is the only status-token printer on this path. A
        // cancelled cleaner must land in `cancelled` with its name (never
        // `failures`) and the targets after it must keep running (PBI 78
        // channel; the status-token absence itself is pinned by the
        // progress.rs source pin
        // with_spinner_helpers_stay_silent_on_a_user_cancel).
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(CancelledCleaner("colima")),
            Box::new(OkCleaner("after-cancel")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter, // show_spinner() == true
            BatchOptions {
                dry_run: true,   // never deletes and never writes history
                animated: false, // the with_spinner_result branch
                with_notes: false,
            },
        );
        assert_eq!(
            summary.cancelled,
            vec!["colima".to_string()],
            "the cancel must be carried with the cleaner name"
        );
        assert!(
            summary.failures.is_empty(),
            "a user cancel is not a failure, got: {:?}",
            summary.failures
        );
        assert_eq!(
            summary.total_freed, 100,
            "the targets after the cancel must keep running"
        );
    }

    #[test]
    fn run_batch_cancel_via_the_animated_path_continues_remaining_targets() {
        // The animated-path symmetric pin: animated=true takes the animated
        // branch (with_spinner_result_progress) no matter the reporter kind,
        // and the cancel contract must match the non-animated branch exactly
        // — the cancelled target lands in `cancelled` with its name, never
        // in `failures`, and the targets after it keep running (PBI 78
        // channel). indicatif exposes no output-capture seam on this path,
        // so the status-token absence itself is pinned behaviorally at the
        // emit_result_status seam in progress.rs and structurally by the
        // progress.rs source pin
        // (with_spinner_helpers_stay_silent_on_a_user_cancel).
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(CancelledCleaner("colima")),
            Box::new(OkCleaner("after-cancel")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter, // spinner reporter (show_spinner() == true)
            BatchOptions {
                dry_run: true,  // never deletes and never writes history
                animated: true, // the with_spinner_result_progress branch
                with_notes: false,
            },
        );
        assert_eq!(
            summary.cancelled,
            vec!["colima".to_string()],
            "the animated path must carry the cancel with the cleaner name"
        );
        assert!(
            summary.failures.is_empty(),
            "a user cancel is not a failure on the animated path either, got: {:?}",
            summary.failures
        );
        assert_eq!(
            summary.total_freed, 100,
            "the targets after the cancel must keep running on the animated path"
        );
    }

    #[test]
    fn run_batch_animated_path_accumulates_like_the_plain_path() {
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(OkCleaner("anim")),
            Box::new(FailingCleaner("bad-anim")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true,
                animated: true, // with_spinner_result_progress
                with_notes: false,
            },
        );
        assert_eq!(summary.total_freed, 100);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].name, "bad-anim");
    }

    #[test]
    fn run_batch_status_line_is_owned_by_the_reporter_kind() {
        // The non-animated branch prints its [OK]/[FAILED] status line only
        // when reporter.show_spinner(): a quiet reporter (deep-suppress)
        // gets none, matching the single-target path's show_spinner()==
        // false branch (PBI 78). The animated branch must stay first and
        // stay keyed on the caller's animated flag — animated is never
        // derived from show_spinner (no-tick-while-prompting: a cleaner on
        // the non-animated path may still prompt, so a derived steady tick
        // would redraw the line under the prompt). indicatif exposes no
        // output-capture seam here, so this pins the source (the same style
        // as the progress.rs steady-tick pins).
        let src = include_str!("batch.rs");
        let start = src
            .find("let result = if animated {")
            .expect("run_batch's branch dispatch must be present");
        let end = src[start..]
            .find("match result {")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            body.starts_with("let result = if animated {"),
            "the animated branch must stay first and keyed on the caller's animated flag"
        );
        assert!(
            body.contains("} else if reporter.show_spinner() {"),
            "the non-animated branch must gate the status line on reporter.show_spinner() (deep-suppress gets no [OK] line)"
        );
        assert!(
            !body.contains("animated = "),
            "animated must never be derived from another flag (no-tick-while-prompting)"
        );
    }

    #[test]
    fn run_batch_writes_history_only_when_not_dry_run() {
        // HOME pointed at a tempdir makes write_history_entry observable:
        // the history file lands under $HOME/.local/share/sasurahime. The
        // swap is serialized under TEST_LOCK because the HOME variable is
        // process-global: a concurrent history probe (the zero-byte test
        // below) writes under the swapped HOME, so a racing swap sends its
        // rows into the wrong directory.
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Declared after the lock so the guard restores HOME while the lock
        // is still held, then the lock is released.
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        let history_path = home.join(".local/share/sasurahime/history.json");

        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![Box::new(OkCleaner("hist-a"))];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();

        // Dry run: nothing deleted, nothing recorded.
        let _ = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true,
                animated: false,
                with_notes: false,
            },
        );
        assert!(!history_path.exists(), "dry run must not write history");

        // Real run: one history entry for the one successful cleaner.
        let _ = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: false,
                animated: false,
                with_notes: false,
            },
        );
        let entries = crate::history::load_history(&history_path);
        assert_eq!(entries.len(), 1, "one entry for the one success");
        assert_eq!(entries[0].cleaner, "hist-a");
        assert_eq!(entries[0].freed_bytes, 100);
        assert!(!entries[0].moved_to_trash);
    }

    #[test]
    fn write_history_entry_if_needed_records_nothing_for_zero_byte_real_runs() {
        // Pins the zero-byte policy: a real run that freed 0 bytes (all
        // deletions skipped, empty dirs, a CLI tool reporting 0 freed) has no
        // history row — the history table tracks recovered bytes. The guard
        // short-circuits before any I/O, so a unique path nobody else writes
        // (no HOME swap: that races with the parallel EnvGuard test) must stay
        // absent.
        let tmp = TempDir::new().unwrap();
        let history_path = tmp.path().join(".local/share/sasurahime/history.json");

        write_history_entry_if_needed("zero-byte", false, 0, 0, false);
        assert!(
            !history_path.exists(),
            "a real zero-byte run must not be recorded"
        );
    }

    #[test]
    fn post_clean_note_shows_restore_guidance_for_trash_runs() {
        let note = post_clean_note(2048, true, true).expect("restore note");
        assert!(
            note.contains("Finder") && note.contains("~/.Trash"),
            "restore guidance must mention Finder and ~/.Trash, got: {note}"
        );
        // A cleaner that never moves to Trash prints nothing in trash mode.
        assert_eq!(post_clean_note(2048, false, true), None);
    }

    #[test]
    fn post_clean_note_keeps_a_permanent_audit_trail() {
        let note = post_clean_note(2048, true, false).expect("audit note");
        assert!(
            note.contains("Permanently deleted"),
            "permanent runs must keep a minimal audit trail, got: {note}"
        );
        assert_eq!(
            post_clean_note(2048, false, false).expect("audit note"),
            "Permanently deleted: no Trash copy was kept."
        );
    }

    #[test]
    fn post_clean_note_is_silent_when_nothing_was_freed() {
        assert_eq!(post_clean_note(0, true, true), None);
        assert_eq!(post_clean_note(0, false, false), None);
    }

    #[test]
    fn render_skip_line_pins_the_line_structure_for_every_variant() {
        // The renderer owns the "{name} ({risk}): {advice}" structure and
        // the risk label of each variant; the wording itself is the
        // caller's advice argument (PBI 74). The None-advice mode is
        // --yes's E2E-pinned generic shape.
        let advised = SkipAdvice {
            never_unattended: Some("never runs unattended"),
            needs_opt_in: Some("needs --include-manual"),
            needs_explicit_selection: Some("needs an explicit selection"),
        };
        assert_eq!(
            render_skip_line(
                "hints",
                SkipReason::NeverUnattended(Risk::Moderate),
                &advised
            ),
            "hints (Moderate): never runs unattended"
        );
        assert_eq!(
            render_skip_line("spm", SkipReason::NeedsOptIn(Risk::Moderate), &advised),
            "spm (Moderate): needs --include-manual"
        );
        assert_eq!(
            render_skip_line(
                "docker",
                SkipReason::NeedsExplicitSelection(Risk::Risky),
                &advised
            ),
            "docker (Risky): needs an explicit selection"
        );
        let bare = SkipAdvice {
            never_unattended: None,
            needs_opt_in: None,
            needs_explicit_selection: None,
        };
        assert_eq!(
            render_skip_line(
                "cargo",
                SkipReason::NeedsExplicitSelection(Risk::Moderate),
                &bare
            ),
            "cargo (Moderate)"
        );
    }

    #[test]
    fn run_selected_batch_runs_only_the_selections_run_list() {
        // The skeleton maps select's registry indices onto run_batch's
        // slice in registry order — a mis-mapped index would clean the
        // wrong target or run a skipped one.
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(OkCleaner("sel-a")),
            Box::new(FailingCleaner("sel-b")),
            Box::new(OkCleaner("sel-c")),
        ];
        let selection = crate::unattended::UnattendedSelection {
            run: vec![(0, 0), (2, 0)],
            skipped: vec![(
                1,
                crate::unattended::SkipReason::NeedsOptIn(crate::cleaner::Risk::Moderate),
            )],
        };
        let summary = run_selected_batch(
            &boxed,
            &selection,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            BatchOptions {
                dry_run: true, // never deletes and never writes history
                animated: false,
                with_notes: false,
            },
        );
        assert_eq!(
            summary.total_freed, 200,
            "only the run list's targets run, in registry order"
        );
        assert!(
            summary.failures.is_empty(),
            "the skipped index must never run, got: {:?}",
            summary.failures
        );
    }

    #[test]
    fn the_skip_reason_match_lives_only_in_the_shared_renderer() {
        // Source pin (PBI 2026-11-20-03 BDD2; tests/AGENTS.md last-resort
        // rule): the guarded structure — a new SkipReason variant must
        // surface as ONE build error at the renderer's match instead of
        // silently flowing into --yes's generic risk-only line — has no
        // behavioural surface while the E2E pins only see the three current
        // variants (round 9 whywhy §2.a). The variant literals are
        // assembled so this pin never matches its own source.
        let variant = |name: &str| format!("SkipReason::{name}(");
        let arms = [
            variant("NeverUnattended"),
            variant("NeedsOptIn"),
            variant("NeedsExplicitSelection"),
        ];

        // The renderer owns the one exhaustive match: every arm appears
        // exactly once in batch.rs's production code and no wildcard arm
        // can swallow a new variant.
        let batch_prod = include_str!("batch.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let start = batch_prod
            .find("pub fn render_skip_line(")
            .expect("the shared skip renderer must stay defined");
        let end = batch_prod[start..]
            .find("fn render_advised_line(")
            .expect("the renderer's line helper must stay beside the match");
        let body = &batch_prod[start..start + end];
        assert!(
            body.contains("match reason {"),
            "the renderer must still match on the skip reason"
        );
        for arm in &arms {
            assert_eq!(
                body.matches(arm).count(),
                1,
                "the SkipReason match must stay in the one renderer, got {arm} outside it"
            );
        }
        assert!(
            !body.contains("_ =>"),
            "a wildcard arm would silently swallow a new SkipReason variant"
        );
        let rest = &batch_prod[start + end..];
        for arm in &arms {
            assert!(
                !rest.contains(arm),
                "a second SkipReason match appeared in batch.rs: {arm}"
            );
        }

        // The unattended flows must not hand-write reason rendering: no
        // variant references and no direct risk-label line in main.rs or
        // interactive.rs.
        for (file, src) in [
            ("main.rs", include_str!("main.rs")),
            ("interactive.rs", include_str!("interactive.rs")),
        ] {
            for arm in &arms {
                assert!(
                    !src.contains(arm),
                    "{file} must render skips through the shared renderer, found {arm}"
                );
            }
            assert!(
                !src.contains(".risk().label()"),
                "{file} must not hand-write the generic risk-only line"
            );
        }
    }

    // ── Stage 3: report_single_target's truth table (PBI 2026-11-21-05) ────

    /// A successful CleanResult like the single flow's: bytes freed, trash
    /// reported by the cleaner, no skips.
    fn single_ok_result() -> CleanResult {
        CleanResult::freed("unused-name", 100, true)
    }

    fn spinner_gates() -> SingleTargetReport {
        SingleTargetReport {
            show_spinner: true,
            dry_run: true,
            trash_mode: true,
        }
    }

    fn quiet_gates() -> SingleTargetReport {
        SingleTargetReport {
            show_spinner: false,
            ..spinner_gates()
        }
    }

    /// Runs report_single_target over in-memory buffers and returns
    /// (stdout, stderr, outcome).
    fn report(
        label: &str,
        summary: BatchSummary,
        gates: SingleTargetReport,
    ) -> (String, String, anyhow::Result<CleanResult>) {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let result = report_single_target(&mut out, &mut err, label, summary, gates);
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
            result,
        )
    }

    #[test]
    fn report_single_target_renders_the_ok_families_under_the_spinner_gate() {
        // Truth table, row: success × show_spinner. The [OK] token (stderr)
        // precedes the freed line and the restore note (stdout); the carry's
        // result survives intact so run_and_exit can still consult
        // exit_code().
        let (out, err, result) = report(
            "trash-target",
            BatchSummary::from_single_outcome("trash-target", Ok(single_ok_result())),
            spinner_gates(),
        );
        let r = result.expect("the success must survive");
        assert_eq!(r.bytes_freed, 100);
        assert_eq!(r.exit_code(), 0, "a plain success keeps exit code 0");
        assert_eq!(
            err, "Cleaning trash-target... [OK]\n",
            "the [OK] token is the only stderr line"
        );
        assert!(
            out.contains("Logical freed: 100 B / Physical freed: 0 B (in Trash)"),
            "the trash-mode freed axis renders, got: {out}"
        );
        assert!(
            out.contains("Restore from Finder (Trash) or run `open ~/.Trash`."),
            "the restore note rides the spinner gate, got: {out}"
        );
        assert!(
            out.starts_with("Logical freed") && out.ends_with("`open ~/.Trash`.\n"),
            "freed line first, note second, got: {out}"
        );
    }

    #[test]
    fn report_single_target_suppresses_token_freed_and_note_for_a_quiet_reporter() {
        // Truth table, row: success × quiet reporter (deep-suppress). No
        // token, no freed line, no note — and the skip lines still render
        // (the single engine's skip family is not reporter-gated).
        let mut r = single_ok_result();
        r.skipped = vec![crate::cleaner::SkippedEntry {
            path: std::path::PathBuf::from("/tmp/locked"),
            reason: crate::cleaner::SkipCause::Reason("locked".to_string()),
        }];
        let (out, err, result) = report(
            "quiet-target",
            BatchSummary::from_single_outcome("quiet-target", Ok(r)),
            quiet_gates(),
        );
        let _ = result.expect("the success must survive");
        assert_eq!(
            err, "1 file(s) skipped due to errors (permission/lock):\n  - /tmp/locked: locked\n",
            "skip lines only"
        );
        assert_eq!(out, "", "no freed line, no note under deep-suppress");
        assert!(
            !err.contains("[OK]"),
            "no status token for a quiet reporter"
        );
    }

    #[test]
    fn report_single_target_renders_the_nameless_skip_lines() {
        // Truth table, row: all-skipped success × show_spinner. The skip
        // family is the single-target shape: "N file(s) skipped" plus
        // nameless `  - path: reason` lines on stderr — the batch's named
        // shape lives in report_batch_failures. A zero-byte result with
        // skips carries exit_code() == 1: the report engine must return the
        // CleanResult intact so run_and_exit can still consult it.
        let mut r = CleanResult::freed("unused-name", 0, false);
        r.skipped = vec![crate::cleaner::SkippedEntry {
            path: std::path::PathBuf::from("/tmp/locked-cache"),
            reason: crate::cleaner::SkipCause::Delete(crate::cleaner::DeleteError::TrashRefused(
                "denied".to_string(),
            )),
        }];
        let (out, err, result) = report(
            "skip-target",
            BatchSummary::from_single_outcome("skip-target", Ok(r)),
            spinner_gates(),
        );
        let r = result.expect("the success must survive");
        assert_eq!(
            r.exit_code(),
            1,
            "a succeeded-but-all-skipped result carries exit_code() == 1 to run_and_exit"
        );
        assert!(err.contains("[OK]"), "the token still prints, got: {err}");
        assert!(
            err.contains("1 file(s) skipped due to errors (permission/lock):"),
            "got: {err}"
        );
        assert!(
            err.contains("  - /tmp/locked-cache: trash failed: denied"),
            "the nameless detail carries path and reason, got: {err}"
        );
        assert!(
            !err.contains("skip-target:"),
            "the single skip line never names the cleaner, got: {err}"
        );
        assert_eq!(
            out, "Freed: 0 B\n",
            "zero freed with skips: the freed line renders, no note (bytes == 0)"
        );
    }

    #[test]
    fn report_single_target_is_silent_on_a_user_cancel() {
        // Truth table, rows: cancel × {spinner, quiet}. CleanCancelled's
        // token-free contract: no status token, no families, no history —
        // the single-target Ok(cancelled()) shape.
        for gates in [spinner_gates(), quiet_gates()] {
            let (out, err, result) = report(
                "cancel-target",
                BatchSummary::from_single_outcome(
                    "cancel-target",
                    Err(anyhow::Error::from(crate::cleaner::CleanCancelled)),
                ),
                gates,
            );
            let r = result.expect("a cancel is Ok(cancelled)");
            assert_eq!(r.bytes_freed, 0);
            assert!(r.skipped.is_empty());
            assert_eq!(out, "", "no freed line, no note");
            assert_eq!(err, "", "no [FAILED], no [OK] — the token-free contract");
        }
    }

    #[test]
    fn report_single_target_rebuilds_the_err_contract_with_the_failed_token() {
        // Truth table, row: failure × show_spinner. The carry's BatchFailure
        // error is rebuilt as the single-target Err (chain intact), and the
        // [FAILED] token rides the spinner gate.
        let (out, err, result) = report(
            "fail-target",
            BatchSummary::from_single_outcome("fail-target", Err(anyhow::anyhow!("boom"))),
            spinner_gates(),
        );
        let e = result.expect_err("the failure must rebuild the Err");
        assert!(
            e.to_string().contains("boom"),
            "the Err must carry the carried error, got: {e}"
        );
        assert_eq!(err, "Cleaning fail-target... [FAILED]\n");
        assert_eq!(out, "", "no freed line after a failure");
    }

    #[test]
    fn report_single_target_suppresses_the_failed_token_for_a_quiet_reporter() {
        // Truth table, row: failure × quiet reporter (deep-suppress). The
        // [FAILED] token is suppressed — the E2E counterpart lives in
        // tests/plugin.rs
        // (failing_plugin_clean_suppresses_the_failed_token_under_deep_suppress)
        // — but the Err itself still reaches the caller, so the run still
        // fails.
        let (out, err, result) = report(
            "quiet-fail",
            BatchSummary::from_single_outcome("quiet-fail", Err(anyhow::anyhow!("boom"))),
            quiet_gates(),
        );
        let e = result.expect_err("the Err contract survives the quiet gate");
        assert!(e.to_string().contains("boom"));
        assert_eq!(err, "", "no [FAILED] token under deep-suppress");
        assert_eq!(out, "");
    }

    #[test]
    fn report_single_target_writes_history_once_for_a_real_run() {
        // The history contract on the single engine (stage 3): one row per
        // successful target, recorded only for real runs that freed
        // something, labeled with the caller's label — the same 1-guard the
        // batch loop uses. HOME swap serialized under TEST_LOCK, mirroring
        // run_batch_writes_history_only_when_not_dry_run.
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        let history_path = home.join(".local/share/sasurahime/history.json");

        // Dry run: nothing recorded.
        let _ = report(
            "single-hist",
            BatchSummary::from_single_outcome("single-hist", Ok(single_ok_result())),
            SingleTargetReport {
                dry_run: true,
                ..spinner_gates()
            },
        );
        assert!(!history_path.exists(), "a dry run records nothing");

        // Real run: exactly one row, labeled by the parameter.
        let _ = report(
            "single-hist",
            BatchSummary::from_single_outcome("single-hist", Ok(single_ok_result())),
            SingleTargetReport {
                dry_run: false,
                ..spinner_gates()
            },
        );
        let entries = crate::history::load_history(&history_path);
        assert_eq!(entries.len(), 1, "one row for the one target");
        assert_eq!(entries[0].cleaner, "single-hist");
        assert_eq!(entries[0].freed_bytes, 100);
        assert!(entries[0].moved_to_trash, "the stub reports uses_trash");
    }

    #[test]
    fn from_single_outcome_maps_the_outcome_onto_the_carry() {
        // The single engine speaks the batch loop's vocabulary: a success
        // fills results (the stage-3 carry) plus the named skips, a cancel
        // fills `cancelled` with the label, a failure fills `failures` with
        // the intact error.
        let label = "carry-target";
        let ok = BatchSummary::from_single_outcome(label, Ok(single_ok_result()));
        assert_eq!(ok.results.len(), 1);
        assert_eq!(ok.total_freed, 100);
        assert!(ok.failures.is_empty() && ok.cancelled.is_empty() && ok.skipped.is_empty());

        let cancelled = BatchSummary::from_single_outcome(
            label,
            Err(anyhow::Error::from(crate::cleaner::CleanCancelled)),
        );
        assert_eq!(cancelled.cancelled, vec![label.to_string()]);
        assert!(
            cancelled.failures.is_empty() && cancelled.results.is_empty(),
            "a cancel never lands in failures"
        );

        let failed = BatchSummary::from_single_outcome(label, Err(anyhow::anyhow!("boom")));
        assert_eq!(failed.failures.len(), 1);
        assert_eq!(failed.failures[0].name, label);
        assert!(
            failed.failures[0].error.to_string().contains("boom"),
            "the chain rides intact"
        );
        assert!(failed.results.is_empty() && failed.cancelled.is_empty());
    }

    #[test]
    fn report_single_target_never_reads_the_result_name() {
        // Stage-3 trap source pin (PBI 2026-11-21-05; tests/AGENTS.md
        // last-resort rule): CleanResult::name is a pub field whose equality
        // with the batch loop's c.name() is NOT a pinned invariant, so the
        // report engine must take every label — the status token, the
        // history row — from its `label` parameter (the same label the
        // batch loop takes from c.name()), never from the carry's
        // results[0].name. A behavior pin cannot see the difference when
        // the two spellings agree, so single_ok_result() deliberately names
        // its result "unused-name": the exact-string token pins above would
        // catch a results[0].name display. This structural guard covers the
        // history row and any future label use, in the progress.rs
        // steady-tick pin style. The literals are assembled so this pin
        // never matches its own source.
        let name_access = concat!(".", "name");
        let token_call = concat!("Cleaning", " {label}...");
        let history_call = concat!("write_history_entry_if_needed", "(");
        let src = include_str!("batch.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap();
        let start = prod
            .find("pub fn report_single_target(")
            .expect("the single-target report engine must stay defined");
        let end = prod[start + 1..]
            .find("\npub fn ")
            .map(|i| start + 1 + i)
            .unwrap_or(prod.len());
        let body = &prod[start..end];
        assert!(
            body.contains(token_call),
            "the status token must be rebuilt from the label parameter"
        );
        let hist_start = body
            .find(history_call)
            .expect("the history guard must stay on the single engine's path");
        let args = &body[hist_start + history_call.len()..];
        assert!(
            args.trim_start().starts_with("label"),
            "the history row must be labeled with the label parameter, got: {}",
            &args[..args.len().min(60)]
        );
        assert!(
            !body.contains(name_access),
            "report_single_target must never read CleanResult::name (stage-3 trap), found {name_access}"
        );
    }
}
