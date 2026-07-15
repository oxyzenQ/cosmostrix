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
//! Unix signals (SIGINT, SIGTERM, SIGHUP, SIGTSTP, SIGCONT) are handled via
//! a dedicated signal thread that sets an atomic `GRACEFUL_SHUTDOWN` flag.
//! The main loop checks this flag each iteration and exits cleanly, allowing
//! `Terminal::drop()` to restore the terminal without racing on stdout.
//! A fallback force-restore fires after 1 second if the main loop is stuck.
//!
//! ## Watchdog
//!
//! A background watchdog thread monitors a global frame counter. If no frames
//! are produced for 10+ seconds, it restores the terminal and exits —
//! protecting against infinite loops that would leave the TTY in a broken state.

mod activity;
mod adaptive;
mod event_loop;
mod hud;
mod input;
mod watchdog;

#[cfg(test)]
mod tests;

// Re-export public API for the rest of the crate
pub(crate) use event_loop::run_interactive;
pub(crate) use watchdog::clear_mouse_capture_flag;

use crate::runtime::ColorScheme;

// Global state for final verbose summary after exit.
// Set by the event loop when cloud.raining becomes false.
use std::sync::atomic::{AtomicU8, Ordering};

static LAST_COLOR_SCHEME: AtomicU8 = AtomicU8::new(0);
static LAST_SCENE_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static LAST_CHARSET: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Store final runtime state for post-exit verbose summary.
pub(crate) fn set_final_state(color: ColorScheme, scene: &str, charset: &str) {
    LAST_COLOR_SCHEME.store(color as u8, Ordering::Relaxed);
    let _ = LAST_SCENE_NAME.set(scene.to_string());
    let _ = LAST_CHARSET.set(charset.to_string());
}

/// Get the final color scheme after the rain loop exited.
pub(crate) fn last_color_scheme() -> ColorScheme {
    // Reconstruct from u8 — ColorScheme is repr(u8) via clap ValueEnum.
    // This is a simple approach: match the stored value.
    match LAST_COLOR_SCHEME.load(Ordering::Relaxed) {
        x if x == ColorScheme::Green as u8 => ColorScheme::Green,
        x if x == ColorScheme::Green2 as u8 => ColorScheme::Green2,
        x if x == ColorScheme::Green3 as u8 => ColorScheme::Green3,
        x if x == ColorScheme::Yellow as u8 => ColorScheme::Yellow,
        x if x == ColorScheme::Orange as u8 => ColorScheme::Orange,
        x if x == ColorScheme::Red as u8 => ColorScheme::Red,
        x if x == ColorScheme::Blue as u8 => ColorScheme::Blue,
        x if x == ColorScheme::Cyan as u8 => ColorScheme::Cyan,
        x if x == ColorScheme::Gold as u8 => ColorScheme::Gold,
        x if x == ColorScheme::Rainbow as u8 => ColorScheme::Rainbow,
        x if x == ColorScheme::Purple as u8 => ColorScheme::Purple,
        x if x == ColorScheme::Neon as u8 => ColorScheme::Neon,
        x if x == ColorScheme::Fire as u8 => ColorScheme::Fire,
        x if x == ColorScheme::Ocean as u8 => ColorScheme::Ocean,
        x if x == ColorScheme::Forest as u8 => ColorScheme::Forest,
        x if x == ColorScheme::Vaporwave as u8 => ColorScheme::Vaporwave,
        x if x == ColorScheme::Gray as u8 => ColorScheme::Gray,
        x if x == ColorScheme::Snow as u8 => ColorScheme::Snow,
        x if x == ColorScheme::Aurora as u8 => ColorScheme::Aurora,
        x if x == ColorScheme::Cosmos as u8 => ColorScheme::Cosmos,
        _ => ColorScheme::Cosmos,
    }
}

/// Get the final scene name after the rain loop exited.
pub(crate) fn last_scene_name() -> &'static str {
    LAST_SCENE_NAME
        .get()
        .map(String::as_str)
        .unwrap_or("monolith")
}

/// Get the final charset preset after the rain loop exited.
pub(crate) fn last_charset_preset() -> &'static str {
    LAST_CHARSET.get().map(String::as_str).unwrap_or("binary")
}
