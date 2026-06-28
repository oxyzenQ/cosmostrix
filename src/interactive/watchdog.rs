// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Watchdog thread and global atomic flags for the interactive runtime.
//!
//! Manages the global frame counter (used by benchmarking too), shutdown
//! flags, mouse capture state, and the background watchdog thread that
//! restores the terminal if the main loop gets stuck.

use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::constants::*;
use crate::terminal::restore_terminal_best_effort;

/// Global flag set when mouse capture was successfully enabled.
/// Signal handlers check this to decide whether DisableMouseCapture is needed.
pub(super) static MOUSE_CAPTURE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Clear the global `MOUSE_CAPTURE_ACTIVE` flag. Called by `Terminal` when
/// mouse capture is disabled (e.g. on drop) so that signal handlers don't
/// attempt a redundant `DisableMouseCapture` on an already-restored terminal.
pub(crate) fn clear_mouse_capture_flag() {
    MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
}

/// Global frame counter for the watchdog thread (AtomicU64 for lock-free watchdog).
pub(crate) static FRAME_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Global shutdown flag. Set to `true` when the main loop exits so the
/// watchdog thread can terminate instead of running forever.
pub(super) static SHUTDOWN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Graceful shutdown request flag. Set by signal handler threads instead of
/// calling `restore_terminal_best_effort()` + `process::exit()` directly.
/// The main loop checks this flag each iteration and exits cleanly, allowing
/// `Terminal::drop()` to restore the terminal without racing on stdout.
/// Signal handler threads simply set this flag and then block until `SHUTDOWN`
/// is observed.  If the main loop is truly stuck, the watchdog (20 s timeout)
/// is the sole fallback that calls `restore_terminal_best_effort()` +
/// `process::exit()`.
pub(super) static GRACEFUL_SHUTDOWN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(super) fn spawn_watchdog() {
    let counter = &FRAME_COUNTER as &std::sync::atomic::AtomicU64;
    let shutdown = &SHUTDOWN as &std::sync::atomic::AtomicBool;
    let mut stuck_count: u32 = 0;
    std::thread::spawn(move || loop {
        // Check shutdown flag before each sleep cycle
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let current = counter.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let next = counter.load(Ordering::Relaxed);
        if current == next {
            stuck_count += 1;
            if stuck_count >= 2 {
                // Main loop has been stuck for multiple check intervals.
                // Attempt to restore the terminal so the user isn't left
                // with a broken shell, then exit.
                restore_terminal_best_effort();
                eprintln!(
                    "[watchdog] main loop stuck for {}s — restoring terminal and exiting",
                    WATCHDOG_INTERVAL_SECS * 2 * stuck_count as u64
                );
                std::process::exit(1);
            }
            eprintln!(
                "[watchdog] main loop appears stuck (frame counter unchanged for {}s)",
                WATCHDOG_INTERVAL_SECS * 2
            );
        } else {
            stuck_count = 0;
        }
    });
}
