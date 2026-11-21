// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial
// PLATFORM: Unix-only (the SIGTSTP/SIGCONT suspend-resume contract is a
//   POSIX tty contract; Windows ConPTY owns its own terminal state — the
//   non-unix stubs below keep call sites cfg-free, mirroring the
//   platform mod's TermReinit pattern).

//! The full-termios suspend snapshot — the SIGTSTP/SIGCONT contract's
//! exact-state guarantee (night-frontier-Z3, Lapis 3b).
//!
//! Before this module, suspend relied on crossterm's internal snapshot:
//! `disable_raw_mode()` restores the termios crossterm saved at
//! `enable_raw_mode()` — a SESSION-START state. Any termios drift since
//! session start (another process on the same device running `stty`, a
//! nested raw-mode tool the user ran through a multiplexer) made the
//! suspend path restore a STALE state, and resume rebuilt raw mode from
//! crossterm's canonical flags instead of the exact bytes the session
//! was running under — the README limitation's "recovery is automatic
//! as long as nothing else wrote" caveat lived on this axis.
//!
//! The contract is now exact:
//!
//! 1. At SIGTSTP — BEFORE raw mode is disabled — the CURRENT termios
//!    (the exact raw state the render loop is running under) is
//!    captured into the snapshot slot ([`unix_impl::capture_suspend_snapshot`]).
//! 2. The suspend path then restores the session-start cooked state
//!    for the shell (crossterm's `disable_raw_mode`, a direct
//!    `tcsetattr` syscall — immune to PTY byte jams) and emits the
//!    nonblocking escape restore.
//! 3. At SIGCONT, the event loop's reinit branch constructs a fresh
//!    `Terminal` (crossterm raw mode) and then overrides it with the
//!    EXACT pre-suspend bytes ([`unix_impl::restore_suspend_snapshot`])
//!    — the render loop resumes on the same termios it suspended with,
//!    byte for byte, regardless of what any other process did to the
//!    device while the session was stopped.
//!
//! Honest scope: the snapshot covers the termios axis (kernel-side
//! line discipline — a `tcsetattr` syscall, unreachable by PTY buffer
//! jams). The escape-sequence axis (alt screen, mouse, cursor) stays
//! on the existing nonblocking restore + full repaint contract, which
//! covers its own drift by design. A `SIGCONT` without a prior
//! snapshot (e.g. `kill -CONT` on a never-suspended session) is a
//! no-op — crossterm's canonical raw mode stands, exactly the
//! pre-3b behavior.

#[cfg(unix)]
pub(crate) mod unix_impl {
    use std::sync::Mutex;

    /// The captured pre-suspend termios. `None` = no snapshot in flight
    /// (nothing suspended yet, or the snapshot was already consumed by a
    /// restore). The Mutex is uncontended in practice: the SIGTSTP
    /// handler thread writes it, the event-loop reinit branch
    /// (single-threaded main loop) reads it, and the two moments are
    /// separated by the STOP itself.
    // const-new: no lazy init — the allocator is never touched from a
    // signal context (the SIGTSTP arm runs on the signal_hook worker
    // thread, not in the signal trampoline, so allocation would be
    // legal; const keeps it unconditional anyway).
    static SUSPEND_SNAPSHOT: Mutex<Option<libc::termios>> = Mutex::new(None);

    /// Run `f` with a tty fd bound to the controlling terminal device.
    ///
    /// Raw mode is a property of the DEVICE, not the fd, so any fd bound
    /// to the same tty reads and writes the same termios. Prefers stdin
    /// when it is a terminal (the fd crossterm's raw mode uses on
    /// interactive sessions); otherwise opens `/dev/tty` for the
    /// duration of `f` and closes it after — the device's termios state
    /// persists beyond close.
    fn with_tty_fd<R>(f: impl FnOnce(std::os::unix::io::RawFd) -> R) -> Option<R> {
        // SAFETY: `isatty` only reads an fd number; no memory ownership
        // transfers.
        if unsafe { libc::isatty(0) } == 1 {
            return Some(f(0));
        }
        // SAFETY: `/dev/tty` is a device path; open is the standard
        // syscall.
        let path = b"/dev/tty\0";
        // SAFETY: the fd is closed on every path below after `f`
        // returns — termios state set via `tcsetattr` persists on the
        // device.
        let fd = unsafe { libc::open(path.as_ptr() as *const _, libc::O_RDWR) };
        if fd < 0 {
            return None;
        }
        let result = f(fd);
        // SAFETY: fd is an open file descriptor from the open above.
        unsafe { libc::close(fd) };
        Some(result)
    }

    /// Capture the CURRENT termios (the render loop's exact raw state)
    /// into the suspend snapshot slot, replacing any stale snapshot.
    ///
    /// Returns `true` when the snapshot was captured; `false` when there
    /// is no tty to read (the caller proceeds without the exact-state
    /// guarantee — the pre-3b behavior). Called from the SIGTSTP handler
    /// arm BEFORE `disable_raw_mode`.
    pub(crate) fn capture_suspend_snapshot() -> bool {
        let captured = with_tty_fd(|fd| {
            // SAFETY: `tcgetattr` writes a kernel-provided `termios`
            // into a valid, caller-owned buffer; fd is a live tty fd.
            let mut termios: libc::termios = unsafe { std::mem::zeroed() };
            // SAFETY: fd refers to an open tty; the pointer is valid
            // for the duration of the call.
            if unsafe { libc::tcgetattr(fd, &mut termios) } == 0 {
                Some(termios)
            } else {
                None
            }
        });
        match captured {
            Some(Some(termios)) => {
                if let Ok(mut slot) = SUSPEND_SNAPSHOT.lock() {
                    *slot = Some(termios);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Restore the EXACT pre-suspend termios captured at SIGTSTP,
    /// consuming the snapshot.
    ///
    /// Returns `true` when the exact bytes were applied; `false` when
    /// there was nothing to restore (no prior suspend) or no tty to
    /// write. Called from the event loop's SIGCONT reinit branch AFTER
    /// the fresh `Terminal` enabled crossterm's canonical raw mode —
    /// the exact snapshot overrides it, so the render loop resumes on
    /// the precise pre-suspend state.
    pub(crate) fn restore_suspend_snapshot() -> bool {
        let snapshot = match SUSPEND_SNAPSHOT.lock() {
            Ok(mut slot) => slot.take(),
            Err(_) => return false,
        };
        let Some(termios) = snapshot else {
            return false;
        };
        let applied = with_tty_fd(|fd| {
            // SAFETY: fd refers to an open tty and the pointer is a
            // valid `termios` captured earlier; `TCSANOW` applies
            // immediately.
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) == 0 }
        });
        matches!(applied, Some(true))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// The slot contract: capture replaces, restore consumes. Without
        /// a tty the syscall side fails gracefully (returns false) and
        /// the slot contract still holds — this pins the no-tty
        /// degradation path (a test runner or `setsid` sandbox has no
        /// controlling terminal) that the signal arm and the reinit
        /// branch rely on to never panic.
        #[test]
        fn snapshot_slot_contract_without_tty() {
            // Whatever the environment: neither call may panic.
            let captured = capture_suspend_snapshot();
            // If a tty WAS available (interactive test run), the
            // snapshot is now Some; consume it either way.
            let restored = restore_suspend_snapshot();
            // The consume leaves nothing behind: a second restore is a
            // no-op returning false.
            assert!(
                !restore_suspend_snapshot(),
                "a consumed snapshot must not restore twice"
            );
            // Consistency: a successful capture must produce a
            // successful restore on the same device.
            if captured {
                assert!(restored, "a captured snapshot must restore");
            }
            // Clean up: drop any snapshot a tty-ful environment left
            // behind.
            if let Ok(mut slot) = SUSPEND_SNAPSHOT.lock() {
                *slot = None;
            }
        }
    }
}

/// Non-unix stub: no SIGTSTP/SIGCONT exists, no snapshot is ever in
/// flight, call sites stay cfg-free (the pre-Z3 behavior on Windows).
/// capture is referenced only from the unix SIGTSTP arm; allow(dead_code)
/// keeps the symmetric stub pair compiling on Windows (-D warnings).
#[cfg(not(unix))]
#[allow(dead_code)]
pub(crate) fn capture_suspend_snapshot() -> bool {
    false
}

/// Non-unix stub: always false (see `capture_suspend_snapshot`).
#[cfg(not(unix))]
pub(crate) fn restore_suspend_snapshot() -> bool {
    false
}

#[cfg(unix)]
pub(crate) use unix_impl::{capture_suspend_snapshot, restore_suspend_snapshot};
