// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Interactive runtime loop for cosmostrix.
//!
//! Manages the main event loop, frame pacing, signal handling, keyboard
//! input dispatch, performance tracking, and the watchdog thread.
//!
//! ## Frame Pacing
//!
//! The pacing system uses a spin-sleep hybrid approach: the bulk of each
//! frame's idle time is spent in `poll_event()` (which also processes input),
//! while the final ~500μs uses a busy-wait spin loop for sub-millisecond
//! deadline accuracy. This eliminates OS scheduling jitter from the frame
//! cadence.
//!
//! When a frame overshoots its deadline, the next frame is scheduled from
//! `now + period` rather than `next + period`, preventing cascading stutter
//! from a single late frame.
//!
//! Under sustained performance pressure, the simulation time budget is
//! adaptively reduced (down to 30% of nominal) to prevent frame queue
//! buildup. This trades visual complexity for temporal consistency.
//!
//! ## Signal Handling
//!
//! Unix signals (SIGTERM, SIGHUP, SIGQUIT, SIGTSTP, SIGCONT) are handled via
//! a dedicated signal thread that sets an atomic `GRACEFUL_SHUTDOWN` flag.
//! SIGINT (Ctrl+C) is deprecated — only 'q' exits cosmostrix.
//! The main loop checks this flag each iteration and exits cleanly, allowing
//! `Terminal::drop()` to restore the terminal without racing on stdout.
//! A fallback force-restore fires after 1 second if the main loop is stuck.
//!
//! ## Watchdog
//!
//! A background watchdog thread monitors a global frame counter. If no frames
//! are produced for 1+ second, it restores the terminal and exits —
//! protecting against infinite loops that would leave the TTY in a broken state.
//!
//! ## Post-exit final state
//!
//! The final-state family (the `FINAL_*` OnceLocks, `set_final_state`,
//! `print_final_runtime_state`, the `last_*` accessors) lives in
//! `final_state.rs`; the facade re-exports below keep every historical
//! call path (`crate::interactive::last_*`,
//! `output::post_exit::print_final_runtime_state`) resolving unchanged.

mod activity;
mod adaptive;
mod bg_fill;
mod event_loop;
mod event_loop_adaptive;
mod event_loop_ambient;
mod event_loop_config_drain;
mod event_loop_config_rebuild;
mod event_loop_ctx;
mod event_loop_finalize;
mod event_loop_hud;
mod event_loop_intro;
mod event_loop_p5;
mod event_loop_perf_stats;
mod event_loop_post_draw;
mod event_loop_resize;
mod event_loop_scene_sync;
mod event_loop_self_heal;
mod event_loop_setup;
mod event_loop_sim_draw;
mod event_loop_stats;
mod final_state;
mod hud;
mod input;
mod signal_handlers;
mod watchdog;

#[cfg(test)]
#[path = "../../test/interactive/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../test/interactive/tests_final_state.rs"]
mod tests_final_state;
#[cfg(test)]
#[path = "../../test/interactive/tests_fmt_opt.rs"]
mod tests_fmt_opt;

// v50 LTS regression tests (first-reload scene reset crash).
#[cfg(test)]
#[path = "../../test/interactive/tests_v50_first_reload.rs"]
mod v50_first_reload;

// v50.0.0-beta.7 Z-master-1B: kitty CSI-u Shift+letter normalization (lowercase codepoint + SHIFT -> uppercase reverse-cycle arm).
#[cfg(test)]
#[path = "../../test/interactive/tests_v50_kitty_shift.rs"]
mod v50_kitty_shift;

// v80.0.0-beta.1 Z-master-1B: intro brand color (EnergyZen, immune to -c) + pause shortkey isolation ('i' rejected while paused).
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_intro_brand_pause.rs"]
mod v51_intro_brand_pause;

// v80.0.0-beta.1 Z-master-1B: exhaustive shortkey no-op lock — every key outside the active set is a complete no-op.
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_shortkey_noop.rs"]
mod v51_shortkey_noop;

// v80.0.0-beta.1 power-dragon gate: render-path pressure feed gated on power-dragon + stale aggressive release.
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_2_power_dragon_gate.rs"]
mod v51_2_power_dragon_gate;

#[cfg(test)]
#[path = "../../test/interactive/tests_v35.rs"]
mod tests_v35;

#[cfg(test)]
#[path = "../../test/interactive/tests_v35_modifier_rejection.rs"]
mod tests_v35_modifier_rejection;

// Re-export ambient_diag from crystal_dragon_engine
pub(crate) use crate::crystal_dragon_engine::ambient_diag::{
    ambient_diag_config_rebuild, ambient_diag_consistency_fix, ambient_diag_reapply,
    ambient_diag_rx, ambient_diag_scene_change, ambient_diag_schedule_empty,
    ambient_diag_schedule_reload, ambient_diag_snapback, ambient_diag_snapback_guard,
    ambient_diag_snapback_killed, ambient_diag_startup, ambient_diag_summary,
};
pub(crate) use bg_fill::fill_terminal_bg;
pub(crate) use event_loop::run_interactive;
// v52 intro_style refactor: the intro subsystem moved to crate-root
// `intro_style/`; these two items it needs from `interactive` are
// re-exported at the facade so the submodules stay private.
pub(crate) use input::is_unmodified_or_shift;
pub(crate) use watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN};
// `clear_mouse_capture_flag` is called cross-platform (terminal.rs:508).
// `request_graceful_shutdown` is only called from the Unix `recover_to_tty`
// path (terminal.rs:425) — gate the re-export so Windows doesn't warn.
pub(crate) use watchdog::clear_mouse_capture_flag;
#[cfg(unix)]
pub(crate) use watchdog::request_graceful_shutdown;

// Post-exit final-state family (final_state.rs) — re-exported at the
// facade so the historical call paths keep resolving:
// event_loop_finalize calls `super::set_final_state`,
// output/post_exit calls `crate::interactive::print_final_runtime_state`
// + `SessionState`, and the #[path] test modules reach the accessors
// through the facade (glob `super::super::*` and `super::fmt_opt_str`).
#[cfg(test)]
pub(crate) use final_state::{
    fmt_opt_str, last_ambient_entries, last_ambient_snapback_secs, last_async_mode, last_bold_mode,
    last_charset_preset, last_color_bg, last_color_scheme, last_color_tune, last_crystal_dragon,
    last_crystal_dragon_secs, last_density, last_fps, last_glitch_level, last_intro_color,
    last_message, last_message_border, last_monolith_size, last_msg_fill_style, last_msg_mode,
    last_power_dragon, last_scene_name, last_shading_mode, last_speed,
};
pub(crate) use final_state::{print_final_runtime_state, set_final_state, SessionState};

use std::sync::OnceLock;

/// AB-10 (rain-screen cleanliness): emit pre-alt-screen warnings to stderr
/// BEFORE `Terminal::with_signal_exit()` enters the alternate screen
/// (otherwise the lines leak into the rain matrix). Reads the terminal
/// size via crossterm (no raw mode needed), applying the same clamp
/// `Terminal::size()` uses. Warns when: (1) `--screen-size WxH` exceeds
/// the live terminal (clipped); (2) intro requested but the terminal is
/// smaller than MIN_INTRO_COLS x MIN_INTRO_LINES (intro silently skipped).
pub(crate) fn emit_pre_alt_screen_warnings(fixed_size: Option<(u16, u16)>, intro_enabled: bool) {
    use crate::constants::{
        MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
    };
    if let Some(fixed) = fixed_size {
        let (tw, th) = crossterm::terminal::size().unwrap_or((fixed.0, fixed.1));
        let tw = tw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let th = th.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
        if fixed.0 > tw || fixed.1 > th {
            crate::output::eprintln_safe!(
                "warning: --screen-size {}x{} exceeds terminal {}x{}; will clip to top-left",
                fixed.0,
                fixed.1,
                tw,
                th
            );
        }
    }
    if intro_enabled {
        let (tw, th) = crossterm::terminal::size().unwrap_or((0, 0));
        let tw = tw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let th = th.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
        if tw < crate::intro_style::MIN_INTRO_COLS || th < crate::intro_style::MIN_INTRO_LINES {
            crate::output::eprintln_safe!(
                "Terminal too small for intro ({}x{} < {}x{}). Starting rain...",
                tw,
                th,
                crate::intro_style::MIN_INTRO_COLS,
                crate::intro_style::MIN_INTRO_LINES
            );
        }
    }
}

// Startup ambient info — stored in a static so main.rs can print
// it AFTER Terminal::drop exits the alternate screen. Printing inside
// event_loop is invisible because the terminal is in alternate screen
// mode and the output is discarded on exit.
static STARTUP_AMBIENT_INFO: OnceLock<String> = OnceLock::new();

/// Store the startup ambient phase info for post-exit verbose summary.
/// Called from event_loop right after `apply_startup_ambient`. The string
/// is the fully-formatted verbose line (without the `[verbose]` prefix,
/// which `eprintln_verbose_raw` adds).
pub(crate) fn set_startup_ambient_info(info: &str) {
    let _ = STARTUP_AMBIENT_INFO.set(info.to_string());
}

/// Get the stored startup ambient info (None if no ambient schedule active
/// or if event_loop never ran). Used by main.rs post-exit verbose dump.
pub(crate) fn startup_ambient_info() -> Option<String> {
    STARTUP_AMBIENT_INFO.get().cloned()
}
