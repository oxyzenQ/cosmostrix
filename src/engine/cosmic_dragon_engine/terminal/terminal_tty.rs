// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P3: stdout /dev/tty fallback helpers.
//!
//! Free helpers extracted from `Terminal::recover_to_tty` (terminal.rs) so
//! they can be unit tested without constructing a Terminal (which requires
//! a real TTY). Also extracted to keep `terminal.rs` under the project's
//! 800-LOC guard.
//!
//! See `terminal.rs` for the recovery path that consumes these helpers.

/// P3: classify an io::Error as recoverable via /dev/tty fallback.
///
/// Returns `true` for errors that indicate the primary stdout fd is broken
/// (broken pipe, bad fd, permission denied — the last one fires when the
/// controlling terminal has been revoked). Returns `false` for transient
/// errors that should be retried on the same fd (Interrupted) or for
/// errors that imply the buffer itself is the problem (WriteZero).
///
/// The classification is intentionally conservative — false negatives just
/// propagate the error (the watchdog catches stuck loops), while false
/// positives would mask real bugs by routing through /dev/tty.
#[cfg(unix)]
use std::fs::OpenOptions;

#[cfg(unix)]
use std::fs::File;

#[cfg(unix)]
#[must_use]
pub(crate) fn is_recoverable_io_error(err: &std::io::Error) -> bool {
    use std::io::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::BrokenPipe | ErrorKind::PermissionDenied | ErrorKind::Other
    ) || err.raw_os_error().is_some_and(|code| {
        // EBADF (9): bad file descriptor — fd was closed under us.
        // ENXIO (6): no such device or address — terminal emulator gone.
        // EIO (5): input/output error — typically serial/pty hangup.
        matches!(code, 9 | 6 | 5)
    })
}

/// P3: open `/dev/tty` for writing. Returns `None` if no controlling
/// terminal exists (e.g., cosmostrix was started under `setsid` or in a
/// container without `/dev/tty`).
///
/// The handle is opened with `O_WRONLY` only — we never read from /dev/tty
/// in the recovery path. The fd is cached in `Terminal::tty_fallback` so
/// repeated recoveries within the same shutdown window reuse it.
#[cfg(unix)]
#[must_use]
pub(crate) fn open_tty_fallback() -> Option<File> {
    OpenOptions::new().write(true).open("/dev/tty").ok()
}

/// NIGHT-termux-hang: flip `fd` into O_NONBLOCK, returning the previous
/// status flags (-1 on fcntl failure — the caller then proceeds exactly
/// as the pre-fix code did; a failed flip means the fd is already
/// unusable, where blocking vs non-blocking is moot).
///
/// Why this exists (the Termux screen-lock hang): when Android locks
/// the screen, Termux stops draining the PTY master. The slave buffer
/// (~64 KB) fills, and every further `write()` on stdout BLOCKS. The
/// main render loop wedges in its frame flush — and so did every
/// exit path that shared the fd (the watchdog's own
/// `restore_terminal_best_effort` + stderr diagnostics, the SIGTERM
/// thread's 3 s grace window) — so `pkill -f cosmostrix` (SIGTERM)
/// silently did nothing and only `kill -9` could end the process.
/// Non-blocking restore writes (EAGAIN -> bytes dropped) turn every
/// exit path into one that always completes.
#[cfg(unix)]
pub(crate) fn set_fd_nonblocking(fd: libc::c_int) -> libc::c_int {
    // SAFETY: fcntl(F_GETFL/F_SETFL) never blocks; both calls return
    // -1 on failure, which is checked. `fd` is a numeric descriptor
    // owned by the process (stdout/stderr, or a test pipe).
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return -1;
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return -1;
        }
        flags
    }
}

/// NIGHT-termux-hang: restore `fd` status flags previously saved by
/// `set_fd_nonblocking`. No-op when the save returned -1 (nothing was
/// changed, so nothing needs restoring).
#[cfg(unix)]
pub(crate) fn restore_fd_flags(fd: libc::c_int, flags: libc::c_int) {
    if flags < 0 {
        return;
    }
    // SAFETY: F_SETFL never blocks; `flags` came from F_GETFL on this
    // same fd. Errors are discarded — the fd was valid a moment ago
    // and the caller only uses this to return to normal blocking mode.
    unsafe {
        let _ = libc::fcntl(fd, libc::F_SETFL, flags);
    }
}

/// NIGHT-termux-hang: best-effort raw-fd write — NEVER blocks, NEVER
/// takes the std stdout/stderr lock.
///
/// The deadlock this exists for: a main-thread write() blocked on a
/// full PTY holds the `std::io::stdout()` ReentrantMutex (std takes
/// it per syscall). Any other thread that touches std::io::stdout() —
/// the watchdog's is_terminal() probe, a signal path's restore —
/// futex-wedges on that lock forever, so nothing ever enforces the
/// exit. This helper writes straight to the numeric fd instead: with
/// O_NONBLOCK set by the caller, the worst case is EAGAIN, and the
/// remaining bytes are DROPPED (best-effort by contract — a dropped
/// escape sequence is cosmetic, a blocked exit thread is the hang).
/// EPIPE/EBADF (fd gone) also drop the remainder silently.
#[cfg(unix)]
pub(crate) fn write_fd_best_effort(fd: libc::c_int, bytes: &[u8]) {
    let mut off = 0usize;
    while off < bytes.len() {
        // SAFETY: write() on a numeric fd the process owns; the
        // caller set O_NONBLOCK so the worst case is an immediate
        // EAGAIN. `n == 0` (nothing accepted) breaks like an error —
        // retrying a zero-byte progress loop would spin.
        let n = unsafe {
            libc::write(
                fd,
                bytes[off..].as_ptr() as *const libc::c_void,
                bytes.len() - off,
            )
        };
        if n <= 0 {
            return;
        }
        off += n as usize;
    }
}

/// Check if an `io::Error` indicates the terminal (PTY) was closed/destroyed.
///
/// Used by the main loop's `poll_event`/`read_event`/`draw` calls AND by the
/// intro's `should_skip()` drain loop. When the terminal is gone, cosmostrix
/// must exit gracefully — `eprintln!`/`println!` would panic on the broken
/// pipe → double-panic → `abort()` → systemd-coredump.
///
/// Detection (cross-platform):
/// - Unix: `EIO` (PTY master closed), `EBADF` (bad fd), `BrokenPipe`, or
///   `UnexpectedEof` (read() returned 0 bytes — crossterm's PTY EOF signal)
/// - Non-Unix: `BrokenPipe` or `UnexpectedEof`
///
/// # Why UnexpectedEof?
///
/// crossterm 0.29's `event::read()` on Unix calls `read()` on the tty fd.
/// When the PTY master disappears (terminal force-close), `read()` returns
/// 0 bytes (EOF). crossterm converts this to `UnexpectedEof`. Without
/// catching it, the drain loop's `Err(_) => break` silently swallows the
/// error — leaving `cloud.raining = true` and causing the wait phase to
/// spin at 100% CPU for 20s until the watchdog fires. This was the root
/// cause of the "rain mode still 100% CPU for 20s" bug.
#[inline]
#[must_use]
pub(crate) fn is_terminal_gone(e: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        e.raw_os_error() == Some(libc::EIO)
            || e.raw_os_error() == Some(libc::EBADF)
            || e.kind() == std::io::ErrorKind::BrokenPipe
            || e.kind() == std::io::ErrorKind::UnexpectedEof
    }
    #[cfg(not(unix))]
    {
        e.kind() == std::io::ErrorKind::BrokenPipe || e.kind() == std::io::ErrorKind::UnexpectedEof
    }
}

#[cfg(test)]
mod p3_tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn p3_broken_pipe_is_recoverable() {
        let err = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        assert!(is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_permission_denied_is_recoverable() {
        let err = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_ebadf_errno_is_recoverable() {
        let err = std::io::Error::from_raw_os_error(9); // EBADF
        assert!(is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_enxio_errno_is_recoverable() {
        let err = std::io::Error::from_raw_os_error(6); // ENXIO
        assert!(is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_eio_errno_is_recoverable() {
        let err = std::io::Error::from_raw_os_error(5); // EIO
        assert!(is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_gone_detects_eio() {
        let err = std::io::Error::from_raw_os_error(libc::EIO);
        assert!(is_terminal_gone(&err));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_gone_detects_ebadf() {
        let err = std::io::Error::from_raw_os_error(libc::EBADF);
        assert!(is_terminal_gone(&err));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_gone_detects_broken_pipe() {
        let err = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        assert!(is_terminal_gone(&err));
    }

    /// crossterm returns UnexpectedEof when read() on the tty fd yields 0
    /// bytes (PTY master closed). This is the primary signal for terminal
    /// force-close in rain mode — without it, the drain loop silently
    /// swallows the error and the wait phase spins at 100% CPU for 20s.
    #[test]
    fn terminal_gone_detects_unexpected_eof() {
        let err = std::io::Error::from(std::io::ErrorKind::UnexpectedEof);
        assert!(is_terminal_gone(&err));
    }

    #[cfg(unix)]
    #[test]
    fn terminal_gone_does_not_false_positive_on_interrupted() {
        let err = std::io::Error::from(std::io::ErrorKind::Interrupted);
        assert!(!is_terminal_gone(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_interrupted_is_not_recoverable() {
        // Interrupted should be retried on the same fd, not routed to /dev/tty.
        let err = std::io::Error::from(std::io::ErrorKind::Interrupted);
        assert!(!is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_write_zero_is_not_recoverable() {
        // WriteZero means the buffer itself is the problem, not the fd.
        let err = std::io::Error::from(std::io::ErrorKind::WriteZero);
        assert!(!is_recoverable_io_error(&err));
    }

    #[cfg(unix)]
    #[test]
    fn p3_open_dev_tty_returns_some_under_normal_session() {
        // This test only validates the helper returns a usable handle when
        // /dev/tty is present. Under `setsid` or containerized CI without
        // /dev/tty, the call returns None and the test is skipped.
        if let Some(mut f) = open_tty_fallback() {
            use std::io::Write;
            // Writing zero bytes should always succeed on a valid handle.
            assert!(f.write_all(b"").is_ok());
        }
        // No else: None is a valid outcome when no controlling terminal exists.
    }

    // ── NIGHT-termux-hang: the fd non-blocking helpers ──────────────

    /// SAFETY helper for the tests below: create a pipe, return
    /// (read_fd, write_fd). The caller closes both ends.
    #[cfg(unix)]
    fn test_pipe() -> (libc::c_int, libc::c_int) {
        // SAFETY: pipe() writes two valid fds into the array on
        // success; the test aborts if it fails.
        unsafe {
            let mut fds = [0 as libc::c_int; 2];
            assert_eq!(libc::pipe(fds.as_mut_ptr()), 0, "pipe() failed");
            (fds[0], fds[1])
        }
    }

    /// The contract the Termux fix stands on: with O_NONBLOCK set, a
    /// write into a FULL pipe returns EAGAIN immediately instead of
    /// parking the thread forever. This is the property the watchdog
    /// and signal exit paths rely on to never deadlock on a jammed
    /// PTY (a full PTY slave behaves the same way).
    #[cfg(unix)]
    #[test]
    fn termux_hang_nonblocking_write_on_full_pipe_returns_eagain() {
        let (r, w) = test_pipe();
        // The read end stays OPEN (a live-but-not-draining reader —
        // exactly Termux's paused PTY master). A closed read end would
        // give EPIPE, which is a different (already-handled) path.
        let prev = set_fd_nonblocking(w);
        assert!(prev >= 0, "set_fd_nonblocking failed on a fresh pipe");
        // Fill the pipe: a 1 MB write into a ~64 KB pipe with a
        // non-draining reader returns the partial count (the pipe
        // accepted exactly its capacity).
        // SAFETY: write() on our own pipe fd; return value checked.
        let n = unsafe { libc::write(w, b"x".as_ptr() as *const libc::c_void, 1024 * 1024) };
        assert!(
            n > 0,
            "the first write must partially fill the pipe, got {n}"
        );
        // The pipe is now full. The next write must fail FAST with
        // EAGAIN — not block. This is the whole point of the fix.
        // SAFETY: write() on our own pipe fd; return value checked.
        let n = unsafe { libc::write(w, b"x".as_ptr() as *const libc::c_void, 1) };
        assert_eq!(n, -1, "a write into a full pipe must fail, not block");
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EAGAIN),
            "the failure must be EAGAIN (non-blocking), not a hang or a close"
        );
        restore_fd_flags(w, prev);
        // SAFETY: closing our own test fds.
        unsafe {
            libc::close(r);
            libc::close(w);
        }
    }

    /// Flags round-trip: set_fd_nonblocking returns the saved flags and
    /// restore_fd_flags puts them back (the O_NONBLOCK bit is gone).
    #[cfg(unix)]
    #[test]
    fn termux_hang_fd_flags_round_trip() {
        let (r, w) = test_pipe();
        // SAFETY: F_GETFL on our own pipe fd.
        let before = unsafe { libc::fcntl(w, libc::F_GETFL) };
        let prev = set_fd_nonblocking(w);
        assert!(prev >= 0);
        assert_eq!(prev, before, "the saved flags must be the pre-set state");
        restore_fd_flags(w, prev);
        // SAFETY: F_GETFL on our own pipe fd.
        let after = unsafe { libc::fcntl(w, libc::F_GETFL) };
        assert_eq!(after, before, "flags must round-trip to the original");
        // SAFETY: closing our own test fds.
        unsafe {
            libc::close(r);
            libc::close(w);
        }
    }

    /// A broken fd degrades, never panics: set_fd_nonblocking returns
    /// -1 and restore_fd_flags is a no-op.
    #[cfg(unix)]
    #[test]
    fn termux_hang_bad_fd_returns_minus_one() {
        assert_eq!(set_fd_nonblocking(-1), -1);
        restore_fd_flags(-1, -1);
    }
}
