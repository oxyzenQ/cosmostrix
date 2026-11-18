// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The synthetic terminal-gone quarantine (night-frontier-Z3, Lapis 3a).
//!
//! KNOWN_ISSUES, "Windows / Termux: the `i` key may cause sudden exit":
//! the root cause — long listed as "suspected" — is a SYNTHETIC
//! terminal-gone error. Android/Termux delivers a synthetic EOF
//! immediately after certain key events; Windows Console API event
//! coalescing can surface a transient I/O error right after a key
//! burst. The event loop treated the FIRST such error as a dead PTY
//! (`is_terminal_gone` → `raining = false`) and exited the session —
//! the user's "sudden exit with no panic message".
//!
//! The two error classes separate on PERSISTENCE, and the quarantine
//! exploits exactly that line:
//!
//! - a REAL dead terminal (force-closed window, SSH disconnect) errors
//!   persistently: every poll and every read after the death returns
//!   EIO/EBADF/BrokenPipe;
//! - a SYNTHETIC terminal-gone errors ONCE, in the immediate wake of a
//!   key event, and the next poll behaves normally.
//!
//! The guard therefore quarantines a terminal-gone error ONLY when it
//! lands inside a short window after an accepted key event AND the
//! error is the first in that window: one absorbed synthetic event,
//! then the loop re-polls. Any terminal-gone error outside the window,
//! or a second one inside it, is persistent — real death — and exits
//! the session exactly as fast as before (the NIGHT-improve-8
//! post-SIGHUP CPU-burn contract is untouched: a truly dead fd errors
//! on the immediate re-poll and exits within milliseconds).
//!
//! Honest scope: the write side (`term.draw()` failures) is NOT
//! quarantined — a write error is the kernel reporting a broken fd
//! from the syscall itself, not an event-delivery artifact, so it
//! exits immediately as before. The Windows/Termux hosts cannot be
//! exercised in this repo's CI; the guard's guarantees are pinned by
//! unit tests here and its non-regression by the supermassive PTY
//! battery (a real EIO mid-run still exits within the watchdog
//! budget, and the quit paths are unchanged).

use std::io;
use std::time::{Duration, Instant};

use super::super::terminal::terminal_tty::is_terminal_gone;

/// How long after an accepted key event a single terminal-gone error is
/// treated as potentially synthetic. 150 ms covers the documented
/// delivery pattern (EOF "immediately after" the key event) with
/// headroom for scheduler jitter, while staying far below any cadence
/// the watchdog would call a stall.
const QUARANTINE_WINDOW: Duration = Duration::from_millis(150);

/// How many synthetic errors one key-activity window may absorb. One is
/// the documented pattern; the cap keeps a hostile repeated-error loop
/// bounded (a second error inside the window is treated as real).
const MAX_ABSORBED_PER_WINDOW: u32 = 1;

/// The quarantine verdict.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The error looks synthetic — absorb it and re-poll; the session
    /// continues.
    Absorb,
    /// The terminal is really gone (persistent error, or the error
    /// arrived outside any key-activity window) — stop the session.
    Exit,
}

/// The quarantine guard, owned by the event loop.
#[derive(Debug, Default)]
pub(crate) struct TerminalGoneGuard {
    last_key_activity: Option<Instant>,
    absorbed_in_window: u32,
}

impl TerminalGoneGuard {
    /// Record an accepted key event (called AFTER the Release-kind
    /// filter, so only events the loop actually processed count).
    pub(crate) fn note_key(&mut self) {
        self.last_key_activity = Some(Instant::now());
        self.absorbed_in_window = 0;
    }

    /// Classify a poll/read error.
    ///
    /// Non-terminal-gone errors are never quarantined (the caller
    /// propagates them exactly as before); terminal-gone errors are
    /// absorbed only under the documented synthetic pattern.
    pub(crate) fn classify(&mut self, err: &io::Error) -> Verdict {
        if !is_terminal_gone(err) {
            return Verdict::Exit;
        }
        let in_window = self
            .last_key_activity
            .is_some_and(|t| t.elapsed() <= QUARANTINE_WINDOW);
        if in_window && self.absorbed_in_window < MAX_ABSORBED_PER_WINDOW {
            self.absorbed_in_window += 1;
            return Verdict::Absorb;
        }
        Verdict::Exit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gone_error() -> io::Error {
        io::Error::from(io::ErrorKind::BrokenPipe)
    }

    fn other_error() -> io::Error {
        io::Error::from(io::ErrorKind::InvalidData)
    }

    /// The documented synthetic pattern: key event, then one EIO right
    /// after it — absorbed once, and a second error in the same window
    /// is persistent (real death exits).
    #[test]
    fn single_error_right_after_a_key_is_absorbed() {
        let mut guard = TerminalGoneGuard::default();
        guard.note_key();
        assert_eq!(guard.classify(&gone_error()), Verdict::Absorb);
        // A second error inside the same window: persistent.
        assert_eq!(guard.classify(&gone_error()), Verdict::Exit);
    }

    /// A terminal-gone error with NO recent key activity is real death —
    /// the NIGHT-improve-8 fast-exit contract (force-closed window,
    /// SSH disconnect, SIGHUP'd PTY must exit immediately, not wait
    /// out any window).
    #[test]
    fn spontaneous_error_exits_immediately() {
        let mut guard = TerminalGoneGuard::default();
        assert_eq!(guard.classify(&gone_error()), Verdict::Exit);
        // A key event long past the window does not re-arm the
        // quarantine for a later error.
        guard.note_key();
        std::thread::sleep(QUARANTINE_WINDOW + Duration::from_millis(60));
        assert_eq!(guard.classify(&gone_error()), Verdict::Exit);
    }

    /// Non-terminal-gone errors always exit (the caller propagates
    /// them); the quarantine never widens its scope.
    #[test]
    fn non_gone_errors_are_never_absorbed() {
        let mut guard = TerminalGoneGuard::default();
        guard.note_key();
        assert_eq!(guard.classify(&other_error()), Verdict::Exit);
    }

    /// Each new key event resets the per-window budget: two separate
    /// key-tap synthetic EOFs (tap, error, tap, error) are each
    /// absorbed, but two errors after ONE tap are not.
    #[test]
    fn a_new_key_event_rearms_the_budget() {
        let mut guard = TerminalGoneGuard::default();
        guard.note_key();
        assert_eq!(guard.classify(&gone_error()), Verdict::Absorb);
        guard.note_key();
        assert_eq!(guard.classify(&gone_error()), Verdict::Absorb);
    }
}
