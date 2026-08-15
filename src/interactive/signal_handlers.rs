// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Signal handler setup for interactive mode.
//!
//! Extracted from event_loop.rs to keep that file under the 1500 LOC cap.
//!
//! - Unix: SIGTERM/SIGHUP/SIGQUIT → graceful shutdown
//! - Unix: SIGTSTP/SIGCONT → suspend/resume with terminal reinit
//! - Windows: Ctrl+Break → graceful shutdown (Ctrl+C deprecated, see below)
//!
//! (bug #15 follow-up): Ctrl+C (SIGINT on Unix, Ctrl+C on Windows)
//! is DEPRECATED as an exit method. Only 'q' exits cosmostrix. This matches
//! the cinematic design principle: the user must deliberately press 'q' to
//! quit — no accidental exits from terminal muscle memory. SIGINT is no
//! longer in the graceful-shutdown signal list. SIGTERM/SIGHUP/SIGQUIT
//! remain for true kill scenarios (system shutdown, terminal close, kill(1)).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::platform::TermReinit;

use super::watchdog::{spawn_watchdog, GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE, SHUTDOWN};

// restore_terminal_best_effort is used by BOTH the Unix SIGTSTP handler
// AND the Windows Ctrl+Break handler, so the import must be unconditional.
// The function itself is defined without a cfg gate in terminal.rs.
use crate::terminal::restore_terminal_best_effort;

#[cfg(unix)]
use signal_hook::consts::{SIGCONT, SIGHUP, SIGQUIT, SIGSTOP, SIGTERM, SIGTSTP};
#[cfg(unix)]
use signal_hook::iterator::Signals;
#[cfg(unix)]
use signal_hook::low_level;

/// Install signal handlers and spawn the watchdog thread.
///
/// Returns the `signal_exit` flag (shared with Terminal) and the
/// `term_reinit` flag (checked by the event loop after SIGCONT).
#[cfg(unix)]
pub(crate) fn install_signal_handlers() -> (Arc<AtomicBool>, TermReinit) {
    let signal_exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let term_reinit: TermReinit = TermReinit::new(AtomicBool::new(false));

    // SIGINT (Ctrl+C) is NOT in this list — only 'q' exits
    // cosmostrix. SIGTERM/SIGHUP/SIGQUIT remain for system-initiated
    // shutdown (kill(1), terminal close, SIGHUP on parent death).
    // SIGINT is intentionally ignored so the user's terminal Ctrl+C
    // muscle memory doesn't accidentally quit the cinematic experience.
    let se = signal_exit.clone();
    if let Ok(mut signals) = Signals::new([SIGTERM, SIGHUP, SIGQUIT]) {
        std::thread::spawn(move || {
            if let Some(_sig) = signals.forever().next() {
                GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
                se.store(true, Ordering::Release);
                // Wait for main loop to notice and clean up.
                // Bounded: max 30 iterations × 100ms = 3s (matches the
                // 2s watchdog threshold + 1s grace). The old 20s bound
                // was calibrated to the old 20s watchdog — now that the
                // watchdog fires at 2s, holding the signal thread for
                // 20s would leave a zombie thread around long after the
                // process should have exited. 3s gives the main loop
                // ample time to observe GRACEFUL_SHUTDOWN and run
                // Terminal::drop before the watchdog force-exits.
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if SHUTDOWN.load(Ordering::Acquire) {
                        break;
                    }
                }
            }
        });
    }

    // SIGTSTP/SIGCONT → suspend/resume
    let tr = term_reinit.clone();
    if let Ok(mut signals) = Signals::new([SIGTSTP, SIGCONT]) {
        std::thread::spawn(move || {
            for sig in signals.forever() {
                match sig {
                    SIGTSTP => {
                        if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                            use crossterm::ExecutableCommand;
                            let _ =
                                std::io::stdout().execute(crossterm::event::DisableMouseCapture);
                            MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
                        }
                        restore_terminal_best_effort();
                        tr.store(true, Ordering::Release);
                        let _ = low_level::raise(SIGSTOP);
                    }
                    SIGCONT => {
                        tr.store(true, Ordering::Release);
                    }
                    _ => {}
                }
            }
        });
    }

    spawn_watchdog();
    (signal_exit, term_reinit)
}

/// Windows: Ctrl+Break handler + watchdog.
///
/// (bug #15 follow-up): on Unix, SIGINT (Ctrl+C) is deprecated —
/// only 'q' exits. On Windows, the `ctrlc` crate handles both CTRL_C_EVENT
/// and CTRL_BREAK_EVENT, so Ctrl+C still triggers graceful shutdown here.
/// Fully filtering Ctrl+C on Windows would require direct Win32
/// `SetConsoleCtrlHandler` calls (filtering CTRL_C_EVENT while accepting
/// CTRL_BREAK_EVENT). That's a future enhancement; the primary target
/// platform is Linux (pro-linux-v3 build) where SIGINT is already excluded.
#[cfg(windows)]
pub(crate) fn install_signal_handlers() -> (Arc<AtomicBool>, TermReinit) {
    let signal_exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let term_reinit: TermReinit = ();

    let se = signal_exit.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
        se.store(true, Ordering::Release);
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !SHUTDOWN.load(Ordering::Acquire) {
            if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                use crossterm::ExecutableCommand;
                let _ = std::io::stdout().execute(crossterm::event::DisableMouseCapture);
            }
            restore_terminal_best_effort();
            std::process::exit(130);
        }
    }) {
        eprintln!("failed to install Ctrl-Break handler: {}", e);
    }

    spawn_watchdog();
    (signal_exit, term_reinit)
}
