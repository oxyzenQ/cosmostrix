// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Global panic hook installation.
//!
//! v16: Windows silent-exit fix. v25: terminal-close double-panic guard.
//!
//! The alt screen captures stdout AND stderr. The old hook printed to stderr
//! without restoring terminal first, so the panic message was trapped in
//! the alt screen and discarded on LeaveAlternateScreen — "silent exit".
//! Fix: restore terminal BEFORE printing, set global flag so Terminal::drop
//! skips cleanup (prevents rain data leaking to main screen).
//!
//! v25 (terminal-close coredump fix): the previous hook used `eprintln!`
//! to print the panic message. When the terminal is closed (SIGHUP /
//! PTY destroyed), stderr becomes a broken pipe. `eprintln!` calls
//! `stderr().write_fmt(...)` which panics on write failure (Rust std
//! intentionally panics to surface I/O errors). A panic inside the
//! panic hook is treated as a double-panic by the Rust runtime, which
//! calls `abort()` → systemd-coredump fires.
//!
//! This is the root cause of the journal entry:
//!   `Process N (cosmostrix) of user 1000 dumped core.`
//!   Stack trace: pthread_kill → raise → abort → cosmostrix internal.
//!
//! Fix: use `write_fmt` directly with the error explicitly discarded
//! (`let _ = ...`). This makes the hook bulletproof — it cannot panic,
//! so any panic in worker threads (notify watcher, polling heartbeat,
//! crossterm event read) is cleanly caught by `catch_unwind` instead
//! of escalating to abort.

/// Install the global panic hook. Call once at startup, before entering
/// the interactive rain loop.
pub(crate) fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        crate::terminal::TERMINAL_RESTORED_BY_PANIC
            .store(true, std::sync::atomic::Ordering::Release);
        crate::terminal::restore_terminal_best_effort();
        // SAFETY: write_fmt returns Err if stderr is broken (terminal
        // closed). We discard the error — never panic from the panic
        // hook, or Rust will abort (double-panic → coredump).
        let _ = std::io::stderr().write_fmt(format_args!("{info}\n"));
        let _ = std::io::stderr().flush();
    }));
}
