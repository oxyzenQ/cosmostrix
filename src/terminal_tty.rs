// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P3: stdout /dev/tty fallback helpers.
//!
//! Free helpers extracted from `Terminal::recover_to_tty` (terminal.rs) so
//! they can be unit tested without constructing a Terminal (which requires
//! a real TTY). Also extracted to keep `terminal.rs` under the project's
//! 1500-LOC guard.
//!
//! See `terminal.rs` for the recovery path that consumes these helpers.

use std::fs::OpenOptions;

use std::fs::File;

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
}
