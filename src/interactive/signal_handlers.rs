// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Signal handler setup for interactive mode.
//!
//! Extracted from event_loop.rs to keep that file under the 1200 LOC cap.
//!
//! - Unix: SIGINT/SIGTERM/SIGHUP/SIGQUIT → graceful shutdown
//! - Unix: SIGTSTP/SIGCONT → suspend/resume with terminal reinit
//! - Windows: Ctrl+C → graceful shutdown

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::watchdog::{spawn_watchdog, GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE, SHUTDOWN};

// restore_terminal_best_effort is used by BOTH the Unix SIGTSTP handler
// AND the Windows Ctrl+C handler, so the import must be unconditional.
// The function itself is defined without a cfg gate in terminal.rs.
use crate::terminal::restore_terminal_best_effort;

#[cfg(unix)]
use signal_hook::consts::{SIGCONT, SIGHUP, SIGINT, SIGQUIT, SIGSTOP, SIGTERM, SIGTSTP};
#[cfg(unix)]
use signal_hook::iterator::Signals;
#[cfg(unix)]
use signal_hook::low_level;

/// Install signal handlers and spawn the watchdog thread.
///
/// Returns the `signal_exit` flag (shared with Terminal) and the
/// `term_reinit` flag (checked by the event loop after SIGCONT).
#[cfg(unix)]
pub(crate) fn install_signal_handlers() -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    let signal_exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let term_reinit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // SIGINT/SIGTERM/SIGHUP/SIGQUIT → graceful shutdown
    let se = signal_exit.clone();
    if let Ok(mut signals) = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGQUIT]) {
        std::thread::spawn(move || {
            if let Some(_sig) = signals.forever().next() {
                GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
                se.store(true, Ordering::Release);
                // Wait for main loop to notice and clean up.
                // Bounded: max 200 iterations × 100ms = 20s (matches watchdog).
                for _ in 0..200 {
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

/// Windows: Ctrl+C handler + watchdog.
#[cfg(windows)]
pub(crate) fn install_signal_handlers() -> (Arc<AtomicBool>, Arc<AtomicBool>) {
    let signal_exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let term_reinit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

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
        eprintln!("failed to install Ctrl-C handler: {}", e);
    }

    spawn_watchdog();
    (signal_exit, term_reinit)
}
