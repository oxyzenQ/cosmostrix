// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-exit verbose reporting — extracted from `main.rs` to keep that
//! file under the 1500-LOC cap, lives under `output/` per src/RULES.md
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
//! The startup color + scene are passed in so the section's `(was X)`
//! change-tracking suffix can compare startup vs final values.

use std::time::Instant;

use crate::config::Args;
use crate::runtime::ColorScheme;
use crate::CloudConfig;

/// Print the post-exit verbose dump (startup ambient info + final runtime
/// state section).
///
/// Only fires when `args.verbose == true` AND `result.is_ok()`. On error
/// paths, the caller prints the error directly and skips this section.
///
/// Parameters:
/// - `args`: CLI args (for `args.scene` — the startup scene name, used
///   to label the `scene:` change-tracking line).
/// - `cloud_cfg`: the startup CloudConfig (the FINAL values are read
///   from the `OnceLock` statics populated by
///   `interactive::set_final_state()` during `event_loop_finalize`).
/// - `color_scheme`: the startup `ColorScheme` enum value (used to label
///   the `color_scheme:` change-tracking line; the final value comes from
///   `cloud.color_scheme()` captured at session end).
/// - `start_time`: the program-start `Instant` captured at the top of
///   `main()`. Used by `print_final_runtime_state()` to compute
///   `duration: Xm Ys` (monotonic — NTP-safe).
pub(crate) fn print_post_exit_verbose(
    args: &Args,
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
    let startup_color = match cloud_cfg.custom_palette_name.as_deref() {
        Some(name) => format!("{name} (custom)"),
        None => format!("{color_scheme:?}"),
    };
    let startup_scene = args.scene.as_deref().unwrap_or(crate::scene::DEFAULT_SCENE);
    crate::interactive::print_final_runtime_state(
        &startup_color,
        startup_scene,
        &cloud_cfg.charset_preset,
        cloud_cfg.speed,
        cloud_cfg.density,
        cloud_cfg.msg_mode,
        cloud_cfg.message.as_deref(),
        cloud_cfg.message_border,
        // v51 msg-fill-style: startup reveal style for the (was X)
        // change-tracking suffix.
        cloud_cfg.msg_fill_style.as_str(),
        cloud_cfg.power_dragon,
        cloud_cfg.crystal_dragon,
        cloud_cfg.async_mode,
        cloud_cfg.intro_color.as_deref(),
        start_time,
        cloud_cfg.ambient_snapback_secs,
        cloud_cfg.ambient_schedule.entries.len(),
    );
}

/// Handle post-exit error reporting + warning drain.
///
/// Called after `interactive::run_interactive` returns. Handles:
/// 1. Live-reload fatal exit (bug #15): if the watcher set
///    LIVE_RELOAD_EXIT_CODE=2, print the error after Terminal::drop
///    (no alt-screen leak) and exit(2).
/// 2. AB-10: drain buffered runtime warnings + debug traces post-exit.
///
/// On fatal live-reload error, calls `std::process::exit(2)` — does
/// NOT return. Otherwise returns normally and the caller returns
/// `result`.
pub(crate) fn handle_post_exit_errors() {
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
    for t in crate::live_config_trace::drain_debug_traces() {
        crate::output::eprintln_safe!("{t}");
    }
}
