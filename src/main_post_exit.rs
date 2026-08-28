// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-exit verbose reporting — extracted from `main.rs` to keep that
//! file under the 1500-LOC cap.
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
        cloud_cfg.power_dragon,
        cloud_cfg.crystal_dragon,
        cloud_cfg.async_mode,
        cloud_cfg.intro_color.as_deref(),
        start_time,
        cloud_cfg.ambient_snapback_secs,
        cloud_cfg.ambient_schedule.entries.len(),
    );
}
