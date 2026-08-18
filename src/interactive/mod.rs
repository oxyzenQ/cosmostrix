// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Interactive runtime loop for Cosmostrix.
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

mod activity;
mod adaptive;
mod bg_fill;
mod event_loop;
mod event_loop_finalize;
mod hud;
mod input;
mod intro;
pub(crate) mod intro_cosmic;
mod intro_logo;
mod signal_handlers;
mod watchdog;

#[cfg(test)]
mod tests;

// Re-export ambient_diag from crystal_dragon_engine
pub(crate) use crate::crystal_dragon_engine::ambient_diag::{
    ambient_diag_config_rebuild, ambient_diag_consistency_fix, ambient_diag_reapply,
    ambient_diag_rx, ambient_diag_scene_change, ambient_diag_schedule_empty,
    ambient_diag_schedule_reload, ambient_diag_snapback, ambient_diag_snapback_guard,
    ambient_diag_snapback_killed, ambient_diag_startup, ambient_diag_summary,
};
pub(crate) use bg_fill::fill_terminal_bg;
pub(crate) use event_loop::run_interactive;
// `clear_mouse_capture_flag` is called cross-platform (terminal.rs:508).
// `request_graceful_shutdown` is only called from the Unix `recover_to_tty`
// path (terminal.rs:425) — gate the re-export so Windows doesn't warn.
pub(crate) use watchdog::clear_mouse_capture_flag;
#[cfg(unix)]
pub(crate) use watchdog::request_graceful_shutdown;

use std::sync::OnceLock;

// Final runtime state — stored as Strings to avoid enum discriminant issues
// with 52 ColorScheme variants. Set once by event loop before returning.
// OnceLock eliminates mutex overhead for write-once-read-many semantics.
static FINAL_COLOR: OnceLock<String> = OnceLock::new();
static FINAL_SCENE: OnceLock<String> = OnceLock::new();
static FINAL_CHARSET: OnceLock<String> = OnceLock::new();
static FINAL_SPEED: OnceLock<f32> = OnceLock::new();
static FINAL_DENSITY: OnceLock<f32> = OnceLock::new();

/// Store final runtime state for post-exit verbose summary.
pub(crate) fn set_final_state(color: &str, scene: &str, charset: &str, speed: f32, density: f32) {
    let _ = FINAL_COLOR.set(color.to_string());
    let _ = FINAL_SCENE.set(scene.to_string());
    let _ = FINAL_CHARSET.set(charset.to_string());
    let _ = FINAL_SPEED.set(speed);
    let _ = FINAL_DENSITY.set(density);
}

/// AB-10 (rain-screen cleanliness): emit pre-alt-screen warnings to stderr
/// BEFORE `Terminal::with_signal_exit()` enters the alternate screen.
/// Otherwise the warning lines leak into the rain matrix on startup.
///
/// Reads the terminal size via `crossterm::terminal::size()`, which does NOT
/// require raw mode or alt-screen entry. Applies the same clamp(MIN, MAX)
/// used by `Terminal::size()` so the comparison reflects the renderer's
/// actual working area.
///
/// Two warnings:
///   1. `--screen-size WxH` exceeds the live terminal size (clipped).
///   2. Intro requested but terminal smaller than MIN_INTRO_COLS x
///      MIN_INTRO_LINES (intro will be silently skipped by `run_intro`).
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
        if tw < intro::MIN_INTRO_COLS || th < intro::MIN_INTRO_LINES {
            crate::output::eprintln_safe!(
                "Terminal too small for intro ({}x{} < {}x{}). Starting rain...",
                tw,
                th,
                intro::MIN_INTRO_COLS,
                intro::MIN_INTRO_LINES
            );
        }
    }
}

/// Get the final color scheme name after the rain loop exited.
pub(crate) fn last_color_scheme() -> String {
    FINAL_COLOR
        .get()
        .cloned()
        .unwrap_or_else(|| "cosmos".to_string())
}

/// Get the final scene name after the rain loop exited.
pub(crate) fn last_scene_name() -> String {
    FINAL_SCENE
        .get()
        .cloned()
        .unwrap_or_else(|| "monolith".to_string())
}

/// Get the final charset preset after the rain loop exited.
pub(crate) fn last_charset_preset() -> String {
    FINAL_CHARSET
        .get()
        .cloned()
        .unwrap_or_else(|| "binary".to_string())
}

/// Get the final rain speed after the rain loop exited.
pub(crate) fn last_speed() -> f32 {
    *FINAL_SPEED.get().unwrap_or(&9.0)
}

/// Get the final density after the rain loop exited.
pub(crate) fn last_density() -> f32 {
    *FINAL_DENSITY.get().unwrap_or(&0.75)
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
