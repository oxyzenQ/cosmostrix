// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
pub(crate) static MOUSE_CAPTURE_ACTIVE: std::sync::atomic::AtomicBool =
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
/// is observed.  If the main loop is truly stuck (e.g. crossterm 0.29's mio
/// read() spins forever on a dead PTY — EIO/EOF don't break the inner loop),
/// the watchdog (2 s timeout) is the sole fallback that calls
/// `restore_terminal_best_effort()` + `process::exit()`.
pub(crate) static GRACEFUL_SHUTDOWN: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Request graceful shutdown from any thread or module.
///
/// Used by the P3 stdout-fallback path in `Terminal::flush_ansi` to flag
/// that the primary stdout fd is broken and the process should exit via
/// the normal shutdown path rather than crashing on the next write.
/// Visibility is `pub(crate)` so `terminal.rs` can call it without
/// exposing the flag setter to downstream crates.
#[cfg(unix)]
pub(crate) fn request_graceful_shutdown() {
    GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
}

pub(super) fn spawn_watchdog() {
    let counter = &FRAME_COUNTER as &std::sync::atomic::AtomicU64;
    let shutdown = &SHUTDOWN as &std::sync::atomic::AtomicBool;
    // Track whether the main loop has started advancing the frame counter.
    // We don't start monitoring for "stuck" until at least one frame has
    // been rendered — this avoids false positives during the intro (which
    // can take several seconds before the rain main loop starts).
    let mut armed: bool = false;
    let mut last_counter: u64 = 0;
    // Capture stdout's terminal status at watchdog spawn time. The
    // dead-PTY probe only fires if stdout WAS a terminal at startup but
    // is NO LONGER one — this avoids false positives when stdout was
    // intentionally redirected from the start (e.g. `cosmostrix > file`
    // for debugging, though this is rare since cosmostrix requires a TTY
    // for interactive mode).
    let stdout_was_terminal = stdout_is_terminal();
    std::thread::spawn(move || loop {
        // Check shutdown flag before each sleep cycle
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        if shutdown.load(Ordering::Acquire) {
            return;
        }

        // ── Cross-platform dead-PTY probe ───────────────────────────
        //
        // Check if stdout is still a terminal. When the user force-closes
        // the terminal, the PTY master disappears and stdout's fd is no
        // longer connected to a tty. This is detectable via
        // `std::io::IsTerminal` on ALL platforms (Unix + Windows),
        // independent of crossterm's event source.
        //
        // This is the FIRST check because it catches the dead-PTY case
        // even when crossterm 0.29's mio source is stuck spinning inside
        // `read()` (EIO/EOF don't break the inner loop on Unix). The
        // frame-counter check below only fires if crossterm returns, but
        // crossterm may never return — so the isatty check is the
        // reliable cross-platform signal.
        //
        // The check runs every WATCHDOG_INTERVAL_SECS (1s). Cost: one
        // isatty syscall per second. Negligible.
        //
        // Platform notes:
        // - Linux: isatty() succeeds when fd is a PTY, fails after force-close
        // - macOS/BSD: same (kqueue-based, same behaviour)
        // - Windows: ConPTY handle becomes invalid after force-close,
        //   IsTerminal returns false
        // - SSH disconnect: sshd closes the PTY master, same signal
        //
        // False-positive guard: only fire if stdout WAS a terminal at
        // watchdog spawn time. If stdout was redirected from the start
        // (rare for interactive mode, but possible), we don't fire —
        // the user intentionally set up that redirection.
        if stdout_was_terminal && !stdout_is_terminal() {
            // stdout is no longer a terminal — terminal was force-closed
            // or SSH disconnected. Exit immediately without waiting for
            // the frame-counter check (which may never fire if crossterm
            // is stuck).
            restore_terminal_best_effort();
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[watchdog] stdout no longer a terminal — restoring and exiting\n"
            ));
            let _ = std::io::stderr().flush();
            std::process::exit(1);
        }

        let current = counter.load(Ordering::Relaxed);
        // Arm the watchdog once the main loop has rendered at least one
        // frame. Before that, the counter is 0 (intro playing or startup
        // in progress) — a "stuck" reading would be a false positive.
        if !armed {
            if current > 0 {
                armed = true;
                last_counter = current;
            }
            continue;
        }
        if current == last_counter {
            // Main loop has not advanced the frame counter in
            // `WATCHDOG_INTERVAL_SECS` seconds. With the current value of
            // 1s, this means 1s of zero progress — the main loop is
            // definitely stuck (max legitimate frame period is 250ms in
            // pause mode, so 1s = 4 missed frames).
            //
            // The most common cause is crossterm 0.29's mio source
            // spinning forever inside `read()` on a dead PTY: EIO and EOF
            // don't break the inner loop (only WouldBlock/Interrupted do),
            // so once the user force-closes the terminal, the main thread
            // is trapped inside `crossterm::event::read()` and never
            // returns to check `GRACEFUL_SHUTDOWN`. The watchdog is the
            // only escape — restore the terminal and force-exit.
            restore_terminal_best_effort();
            // v25: use write_fmt with error discarded — eprintln!
            // panics on broken stderr (terminal closed) → double-panic
            // → abort → coredump. The watchdog specifically fires
            // when the main loop is stuck, which is often caused by
            // the terminal being gone, so this path is hot.
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[watchdog] main loop stuck for {}s — restoring terminal and exiting\n",
                WATCHDOG_INTERVAL_SECS
            ));
            let _ = std::io::stderr().flush();
            std::process::exit(1);
        }
        last_counter = current;
    });
}

/// Cross-platform check: is stdout still a terminal?
///
/// Wrapper around `std::io::IsTerminal` that works on Unix, Windows, and
/// all other platforms. Returns `true` when stdout is connected to a TTY
/// (normal interactive use), `false` when stdout has been redirected or
/// the TTY has been destroyed (terminal force-close, SSH disconnect).
///
/// Used by the watchdog thread to detect dead-PTY scenarios that
/// crossterm's event source cannot escape from (notably the mio 0.29
/// inner-loop bug where `read()` spins forever on EIO/EOF).
///
/// # Platform behaviour
///
/// - Linux: `isatty(STDOUT_FILENO)`. Returns 0 (false) after the
///   PTY master is closed.
/// - macOS / BSD: same `isatty()` semantics via kqueue.
/// - Windows: checks if the stdout handle is a console buffer.
///   ConPTY handle becomes invalid after force-close.
/// - Redirected stdout (`cosmostrix > file`): returns false. The
///   watchdog's `stdout_was_terminal` guard suppresses the dead-PTY probe
///   in this case — the probe only fires when stdout WAS a terminal at
///   startup but is no longer one (force-close, SSH disconnect).
///
/// Note: this is intentionally a free function (not a method on
/// `Terminal`) so the watchdog thread can call it without holding a
/// reference to the `Terminal` struct (which lives on the main thread).
#[inline]
fn stdout_is_terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}
