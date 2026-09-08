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
//!
//! NIGHT-hunter-4 (worker-panic containment): Rust runs the global
//! panic hook BEFORE unwinding — including for panics that a worker
//! thread's `catch_unwind` is about to catch and recover from. The
//! previous hook therefore defeated its own design intent (the v25
//! comment above) on the worker-thread path: a caught watcher/poller/
//! ambient panic first restored the terminal MID-RAIN (leaving the alt
//! screen while the main loop kept rendering) and armed the sticky
//! `TERMINAL_RESTORED_BY_PANIC` flag, which is never reset — so the
//! final `Terminal::drop` skipped `cleanup_terminal()` and leaked raw
//! mode / the alternate screen at process exit. The hook now captures
//! the installing (main) thread's id and only performs terminal
//! teardown for main-thread panics (the only panics that escape to
//! process death). Worker panics keep their designed recovery path:
//! `catch_unwind` sites buffer an AB-10 runtime warning, the poller
//! restarts after a backoff, and the rain keeps running on an intact
//! terminal.

/// Install the global panic hook. Call once at startup, before entering
/// the interactive rain loop.
///
/// Must be called from the main thread (it is — `main()` calls it as one
/// of its first statements): the captured thread id is what classifies a
/// later panic as "main-thread" (terminal teardown owned by the hook)
/// versus "worker-thread" (recovery owned by that thread's
/// `catch_unwind` + AB-10 buffered diagnostics).
pub(crate) fn install_panic_hook() {
    let main_thread_id = std::thread::current().id();
    std::panic::set_hook(Box::new(move |info| {
        if !panic_on_main_thread(main_thread_id) {
            // Worker-thread panic: the catch_unwind wrapper on that
            // thread owns recovery (buffered AB-10 warning, poller
            // restart). Restoring here would break the alt screen
            // mid-rain, and printing here would leak a stderr line
            // into the rain matrix — both violate the worker-panic
            // containment contract. Do nothing and let unwinding
            // continue into the catcher.
            return;
        }
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

/// NIGHT-hunter-4: is the panicking thread the thread that installed the
/// hook (the main thread)?
///
/// Split out as a pure predicate so the classification is unit-testable
/// without touching the global hook from a parallel test (installing a
/// real hook from a test would race other tests' panic handling).
#[inline]
fn panic_on_main_thread(main_thread_id: std::thread::ThreadId) -> bool {
    std::thread::current().id() == main_thread_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    /// The installing thread always classifies as the main thread — the
    /// production install site is `main()`, so production main-thread
    /// panics take the teardown path.
    #[test]
    fn installing_thread_classifies_as_main() {
        let main = std::thread::current().id();
        assert!(
            panic_on_main_thread(main),
            "the installing thread must be classified as the main thread"
        );
    }

    /// A spawned worker thread must NOT be classified as the main thread —
    /// this is the branch that keeps caught worker panics from tearing
    /// down the terminal.
    #[test]
    fn spawned_worker_thread_is_not_main() {
        let main = std::thread::current().id();
        let worker_is_not_main = std::thread::spawn(move || !panic_on_main_thread(main))
            .join()
            .expect("worker thread must join");
        assert!(
            worker_is_not_main,
            "a spawned worker thread must not be classified as the main thread"
        );
    }

    /// Integration contract of the fix: a real worker-thread panic under
    /// the installed hook must leave TERMINAL_RESTORED_BY_PANIC clear.
    /// The previous hook armed the flag (and restored the terminal
    /// mid-rain) even for panics that catch_unwind then swallowed.
    /// The hook is swapped in for the duration of the worker panic and
    /// the previous hook restored afterwards, so parallel tests are
    /// unaffected outside the microsecond window. `std::thread::scope`
    /// guarantees the worker has fully unwound (and the hook has fired)
    /// before the assertion reads the flag.
    #[test]
    fn worker_panic_under_installed_hook_leaves_restore_flag_clear() {
        let prev_hook = std::panic::take_hook();
        install_panic_hook();
        let joined = std::thread::scope(|s| {
            let handle = s.spawn(|| {
                panic!("worker panic under NIGHT-hunter-4 hook test");
            });
            handle.join()
        });
        std::panic::set_hook(prev_hook);
        assert!(joined.is_err(), "the worker panic must surface via join");
        assert!(
            !crate::terminal::TERMINAL_RESTORED_BY_PANIC.load(Ordering::Acquire),
            "worker-thread panic must not arm the terminal restore flag \
             (sticky flag would make Terminal::drop skip cleanup at exit)"
        );
    }
}
