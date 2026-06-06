// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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
mod event_loop;
mod input;
mod watchdog;

#[cfg(test)]
mod tests;

// Re-export public API for the rest of the crate
pub(crate) use event_loop::run_interactive;
pub(crate) use watchdog::clear_mouse_capture_flag;
