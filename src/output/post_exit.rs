// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-exit verbose reporting — extracted from `main.rs` to keep that
//! file under the 800-LOC cap, lives under `output/` per src/RULES.md
//! single-file policy (only `main.rs` at `src/` root).
//!
//! Owns the "post-exit verbose dump" path: when the user ran with
//! `--verbose` / `-v` and the interactive loop returned `Ok(())`, this
//! module prints two things AFTER `Terminal::drop` restored the main
//! screen:
//!
//! 1. The startup ambient info line (captured by the event loop via
//!    `interactive::set_startup_ambient_info()` — printed here because
//!    the event loop's stderr writes are invisible during the alternate
//!    screen).
//! 2. The "final runtime state" section via
//!    `interactive::print_final_runtime_state()` — includes the
//!    `exit_time:` + `duration:` line (v50.0.0-rc.1) + any live-reload
//!    field changes + the always-printed `ambient_snapback_secs:` +
//!    `ambient_entries:` lines (v50.0.0-beta.7 LTS audit).
//!
//! The startup snapshot is built from the startup CloudConfig (v50.0.0
//! beta baseline); the final values are read from the `OnceLock` statics
//! populated by `interactive::set_final_state()` during
//! `event_loop_finalize`.

use std::time::Instant;

use crate::runtime::ColorScheme;
use crate::CloudConfig;

/// Print the post-exit verbose dump (startup ambient info + final runtime
/// state section).
///
/// Only fires when `args.verbose == true` AND `result.is_ok()` (the
/// caller gates both). On error paths, the caller prints the error
/// directly and skips this section. The verbose flag itself is not
/// needed here — the section is only reachable under `--verbose`.
///
/// Parameters:
/// - `cloud_cfg`: the startup CloudConfig — the source the
///   [`crate::interactive::SessionState::from_startup`] snapshot reads
///   (scene name included — `cloud_cfg.scene_name` is the same
///   resolution the event loop launched with, replacing the old
///   re-derivation from `args.scene`; NIGHT-hunter-22 dropped the
///   now-redundant `args` parameter).
/// - `color_scheme`: the startup `ColorScheme` enum value (used to label
///   the `color_scheme:` change-tracking line; the final value comes from
///   `cloud.color_scheme()` captured at session end).
/// - `start_time`: the program-start `Instant` captured at the top of
///   `main()`. Used by `print_final_runtime_state()` to compute
///   `duration: Xm Ys` (monotonic — NTP-safe).
pub(crate) fn print_post_exit_verbose(
    cloud_cfg: &CloudConfig,
    color_scheme: ColorScheme,
    start_time: Instant,
) {
    // 1. Startup ambient info (captured during the loop, printed here
    //    because the alternate screen discards stderr).
    if let Some(info) = crate::interactive::startup_ambient_info() {
        crate::output::eprintln_verbose_purple(&info);
    }

    // 2. Final runtime state section.
    //
    // v50.0.0-alpha.7: tracks ALL live-reload fields (msg_mode, message,
    // power_dragon, crystal_dragon, async_mode, intro_color, ambient_*) —
    // not just color/scene/charset/speed/density.
    //
    // v50.0.0-rc.1: section now ALWAYS prints (even if nothing changed
    // during the session) so the user sees how long cosmostrix ran. The
    // first content line is `exit_time: <UTC-datetime> | duration: <Xm Ys>`.
    //
    // v50.0.0-beta.7 LTS: ambient_snapback_secs + ambient_entries are
    // always-printed so the user can verify the effective ambient config
    // at session end (owner audit: previously missing entirely).
    //
    // NIGHT-hunter-22: the 26-param printer call is now a two-argument
    // diff — one startup snapshot (built from the same CloudConfig the
    // event loop launched with) plus the program-start Instant.
    let startup = crate::interactive::SessionState::from_startup(cloud_cfg, color_scheme);
    crate::interactive::print_final_runtime_state(&startup, start_time);
}

/// Handle post-exit error reporting + warning drain.
///
/// Called after `interactive::run_interactive` returns. Handles:
/// 1. Live-reload fatal exit (bug #15): if the watcher set
///    LIVE_RELOAD_EXIT_CODE=2, print the error after Terminal::drop
///    (no alt-screen leak) and exit(2).
/// 2. AB-10: drain buffered runtime warnings + debug traces post-exit.
/// 3. v80.0.0-alpha.1 (S-master-HUNT-3): drain verbose-only runtime diagnostics (the
///    self-heal family) ONLY when the session ran with `--verbose`/`-v`
///    (owner bug: "[self-heal v2] predictive throttle …" exposed after
///    every non-verbose run). Actionable warnings still always drain.
///
/// On fatal live-reload error, calls `std::process::exit(2)` — does
/// NOT return. Otherwise returns normally and the caller returns
/// `result`.
///
/// `verbose` mirrors `args.verbose` from main.rs (the post-exit side
/// has no other access to the parsed CLI).
pub(crate) fn handle_post_exit_errors(verbose: bool) {
    // Live-reload fatal exit (bug #15): watcher panics + validation
    // errors set LIVE_RELOAD_EXIT_CODE=2, break the rain loop, print here
    // after Terminal::drop (no alt-screen leak).
    if crate::live_config::LIVE_RELOAD_EXIT_CODE.load(std::sync::atomic::Ordering::Acquire) != 0 {
        if let Ok(guard) = crate::live_config::LIVE_RELOAD_ERROR.lock() {
            if let Some(ref msg) = *guard {
                crate::output::eprintln_safe!(
                    "{} [live-reload] ERROR: {}{}",
                    crate::output::error_bold_open(),
                    msg,
                    crate::output::reset()
                );
                crate::output::eprintln_safe!(
                    "{}  Config NOT applied. Fix the error and restart cosmostrix.{}",
                    crate::output::error_open(),
                    crate::output::reset()
                );
            }
        }
        use std::io::Write;
        let _ = std::io::stderr().flush();
        std::process::exit(2);
    }

    // AB-10: drain buffered runtime warnings + debug traces post-exit.
    for w in crate::live_config::drain_runtime_warnings() {
        crate::output::eprintln_warn_labeled(&w);
    }
    // v80.0.0-alpha.1 (S-master-HUNT-3): verbose-only diagnostics — the self-heal family.
    // Drained (and printed) only under --verbose: the messages report
    // automatic engine behavior with no user action required, so a
    // non-verbose exit must stay clean (owner contract).
    if verbose {
        for d in crate::live_config::drain_runtime_diags() {
            crate::output::eprintln_warn_labeled(&d);
        }
    } else {
        // Non-verbose: discard (the buffer is session-scoped; the process
        // exits right after, so there is nothing to carry over).
        let _ = crate::live_config::drain_runtime_diags();
    }
    for t in crate::live_config_trace::drain_debug_traces() {
        crate::output::eprintln_safe!("{t}");
    }
}
