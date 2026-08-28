// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fork guard — extracted from `main.rs` to keep that file under the
//! 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Protects the terminal from being left in raw mode when cosmostrix is
//! killed unexpectedly (SIGKILL, segfault, OOM). Three platform strategies:
//! Linux (fork + prctl), other Unix (background thread polling getppid),
//! Windows (no-op, ConPTY auto-restores).
//!
//! Re-exported from `main.rs` via `pub(crate) use` so the existing
//! `crate::spawn_kill9_terminal_guard()` call site resolves unchanged.

use crate::cosmic_dragon_engine::terminal::restore_terminal_best_effort;
use crate::diagnostics::info::env_var_truthy;
use std::io::IsTerminal;

/// Fork guard: protects the terminal from being left in raw mode when
/// cosmostrix is killed unexpectedly (SIGKILL, segfault, OOM).
///
/// When cosmostrix starts, it switches the terminal to raw mode. Normally
/// `Terminal::drop()` restores the original settings on graceful exit.
/// But SIGKILL bypasses all Rust cleanup — the terminal stays broken:
/// no echo, no line buffering, keys produce garbage. The user must blindly
/// type `reset` or `stty sane` to recover.
///
/// Three strategies by platform:
///
/// - **Linux**: `fork()` + `prctl(PR_SET_PDEATHSIG)`. A child process holds
///   the original termios and waits for SIGTERM (delivered instantly by the
///   kernel when the parent dies). Zero latency, zero CPU overhead. This is
///   the gold standard — `prctl` is Linux-only.
///
/// - **All other Unix** (macOS, FreeBSD, OpenBSD, NetBSD, Android/Termux):
///   A background thread polls `getppid()` every 500ms. When the parent dies,
///   the child is reparented to PID 1 (launchd/init) — ppid becomes 1. The
///   thread detects this and restores the terminal. 500ms worst-case latency
///   (typically ~250ms average), negligible CPU (one syscall per 500ms).
///   This covers macOS (no prctl), BSD (no prctl), and Android (fork may be
///   restricted by seccomp, but threads always work).
///
/// - **Windows**: No-op. ConPTY (Windows Terminal, PowerShell 7+) automatically
///   restores console state when the attached process exits, even on
///   Task Manager kill. Legacy cmd.exe has `SetConsoleMode` but it also
///   reverts on process exit. The panic hook and watchdog still cover the
///   graceful-shutdown path. Set `COSMOSTRIX_NO_FORK_GUARD=1` to skip.
//
// ── Linux: fork + prctl(PR_SET_PDEATHSIG) ─────────────────────────────
#[cfg(target_os = "linux")]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    // SAFETY: this Linux-only guard calls libc APIs that Rust cannot model
    // safely (`tcgetattr`, `fork`, signal-mask setup, `prctl`, `sigwait`, and
    // `_exit`). We only enter after confirming stdin/stdout are TTYs. `orig`
    // and `set` are initialized by the corresponding libc calls before
    // `assume_init`, the child process does not return into Rust application
    // flow, and restoration is limited to best-effort terminal recovery.
    unsafe {
        let mut orig: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
        if libc::tcgetattr(libc::STDIN_FILENO, orig.as_mut_ptr()) != 0 {
            return;
        }
        let orig = orig.assume_init();

        let pid = libc::fork();
        if pid != 0 {
            return;
        }

        // Initialize sigset_t via MaybeUninit — sigemptyset will fully
        // initialize it, so this is safe.
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        libc::sigaddset(set.as_mut_ptr(), libc::SIGTERM);
        let _ = libc::pthread_sigmask(libc::SIG_BLOCK, set.as_ptr(), std::ptr::null_mut());
        let set = set.assume_init();

        let _ = libc::prctl(
            libc::PR_SET_NAME,
            c"cx-term-guard".as_ptr() as usize,
            0,
            0,
            0,
        );
        let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);

        if libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
            libc::_exit(0);
        }

        let mut sig: libc::c_int = 0;
        let _ = libc::sigwait(&set, &mut sig);
        // Only restore terminal modes if the parent died abnormally
        // (SIGKILL, crash). When pkill -TERM is used, both parent and
        // child receive SIGTERM — the parent's Terminal::drop() handles
        // all terminal cleanup. After PR_SET_PDEATHSIG, check ppid:
        // - ppid == 1: parent already dead (SIGKILL or crash) → restore
        // - ppid != 1: parent still alive or exiting normally → do nothing
        if sig == libc::SIGTERM && libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
        }

        libc::_exit(0);
    }
}

// ── All other Unix (macOS, BSD, Android/Termux): getppid polling ───────

/// Unix fallback: background thread polling `getppid()`.
///
/// Used on all Unix platforms except Linux (which has the superior fork+prctl).
/// Covers macOS, FreeBSD, OpenBSD, NetBSD, DragonFly BSD, and Android/Termux.
///
/// When the parent cosmostrix process dies (SIGKILL, crash, OOM), the OS
/// reparents this thread to PID 1. The thread detects ppid==1 and restores
/// the terminal. Worst-case latency: 500ms. CPU overhead: one `getppid()`
/// syscall per 500ms — negligible.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    // SAFETY: tcgetattr is the standard POSIX call to read terminal
    // attributes. stdin is confirmed to be a TTY above.
    let orig = unsafe {
        let mut termios: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
        if libc::tcgetattr(libc::STDIN_FILENO, termios.as_mut_ptr()) != 0 {
            return;
        }
        termios.assume_init()
    };

    std::thread::Builder::new()
        .name("cx-term-guard".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                // SAFETY: getppid() is a simple POSIX call, always safe.
                // On parent death, OS reparents to PID 1 (launchd/init).
                if unsafe { libc::getppid() } == 1 {
                    // Parent died — restore terminal and exit this thread.
                    let _ = unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig) };
                    restore_terminal_best_effort();
                    return;
                }
            }
        })
        .expect("failed to spawn terminal guard thread");
}

// ── Windows: no-op (ConPTY auto-restores) ──────────────────────────────

/// Windows: no fork guard needed.
///
/// ConPTY (Windows Terminal, PowerShell 7+, VSCode) automatically restores
/// console mode when the attached process exits — even on Task Manager kill
/// or crash. Legacy cmd.exe with `SetConsoleMode` also reverts on exit.
/// The panic hook and watchdog still cover graceful shutdown.
#[cfg(not(unix))]
pub fn spawn_kill9_terminal_guard() {}
