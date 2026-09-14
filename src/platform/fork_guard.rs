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

// Unix-only imports: every call site lives in the cfg(unix) guard
// implementations (Linux fork+prctl and the non-Linux getppid poller).
// The Windows body is a no-op, so an unconditional import would be
// flagged as unused by -D warnings on Windows builds.
#[cfg(unix)]
use crate::cosmic_dragon_engine::terminal::restore_terminal_best_effort;
#[cfg(unix)]
use crate::diagnostics::info::env_var_truthy;
#[cfg(unix)]
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
/// - Linux: `fork()` + `prctl(PR_SET_PDEATHSIG)`. A child process holds
///   the original termios and waits for SIGTERM (delivered instantly by the
///   kernel when the parent dies). Zero latency, zero CPU overhead. This is
///   the gold standard — `prctl` is Linux-only.
///
/// - All other Unix (macOS, FreeBSD, OpenBSD, NetBSD, Android/Termux):
///   A background thread polls `getppid()` every 500ms. When the parent dies,
///   the child is reparented to PID 1 (launchd/init) — ppid becomes 1. The
///   thread detects this and restores the terminal. 500ms worst-case latency
///   (typically ~250ms average), negligible CPU (one syscall per 500ms).
///   This covers macOS (no prctl), BSD (no prctl), and Android (fork may be
///   restricted by seccomp, but threads always work).
///
/// - Windows: No-op. ConPTY (Windows Terminal, PowerShell 7+) automatically
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

        // NIGHT-hunt-47-depthbore: capture the renderer's pid at the
        // earliest possible instant (before the sigmask/prctl setup),
        // so the fork-vs-prctl race window below is covered. Every
        // liveness decision in this guard compares getppid() against
        // THIS pid -- never against a magic value like 1, which loses
        // in two real environments: the kernel reparent window (see
        // the sigwait comment) and subreaper containers (an orphan
        // lands on the subreaper, not on init).
        let orig_ppid = libc::getppid();

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

        if libc::getppid() != orig_ppid {
            // The fork-vs-prctl race: the parent died between fork() and
            // prctl(), so PR_SET_PDEATHSIG never fires. The reparent has
            // completed long ago; getppid() moved away from the renderer.
            // Restore only if the terminal is still broken (raw): a
            // parent that already ran its own cleanup leaves nothing
            // for us to do, and re-issuing the escape sequence would
            // only append trailing noise to an already-clean exit.
            if termios_still_broken(&orig) {
                let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
                restore_terminal_best_effort();
            }
            libc::_exit(0);
        }

        let mut sig: libc::c_int = 0;
        let _ = libc::sigwait(&set, &mut sig);
        // PDEATHSIG wakes this guard microseconds after the parent task
        // exits, but the kernel can take up to ~200 ms to finish
        // reparenting us -- during that window getppid() still reports
        // the DEAD renderer pid, and the old `getppid() == 1` check read
        // exactly that stale value and silently skipped the restore (the
        // depthbore SIGKILL bore measured ~25-75% restore rates). The
        // decision is now liveness-based on the captured orig_ppid:
        // - getppid() != orig_ppid: the reparent landed (init or a
        //   subreaper took us) -- the parent is gone, and its exit path
        //   has already flushed every buffered write (reparenting happens
        //   in exit_notify, after all userspace exit handlers), so our
        //   restore cannot interleave with the parent's writer.
        // - getppid() == orig_ppid after the wait: the parent received
        //   the same SIGTERM but is still alive -- the pkill case. Its
        //   own Terminal::drop() owns the cleanup; exit silently.
        // The 6 s patience (300 x 20 ms) also covers the watchdog's
        // force-exit window for a wedged parent: it kills the parent,
        // the reparent lands, and the guard still fires.
        if sig == libc::SIGTERM {
            let mut parent_gone = false;
            for _ in 0..300 {
                if libc::getppid() != orig_ppid {
                    parent_gone = true;
                    break;
                }
                let ts = libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 20_000_000,
                };
                let _ = libc::nanosleep(&ts, std::ptr::null_mut());
            }
            if parent_gone && termios_still_broken(&orig) {
                // Only a still-broken terminal needs the guard: after a
                // graceful parent exit the current termios equals the
                // saved snapshot, and a silent exit keeps the output
                // stream exactly as the parent's cleanup left it.
                let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
                restore_terminal_best_effort();
            }
        }

        libc::_exit(0);
    }
}

/// Compare the live stdin termios against the snapshot taken before
/// the parent enabled raw mode. True when the terminal is still in the
/// state the parent left it in (raw mode / alt screen active), i.e.
/// the guard's restore is actually needed. A failed read is treated as
/// broken -- restoring is the safe default (tcsetattr with the saved
/// snapshot is idempotent and the escape writes are best-effort).
///
/// NIGHT-hunt-47-depthbore: without this gate the deterministic guard
/// re-issued the full restore escape sequence after EVERY exit --
/// including the graceful 'q' path where the parent's own Terminal
/// drop already restored everything -- appending trailing escapes that
/// shifted exit-output parsing windows downstream (the depthtest
/// harnesses pin the deferred diagnostics to appear after the parent's
/// last alt-screen leave).
#[cfg(target_os = "linux")]
fn termios_still_broken(orig: &libc::termios) -> bool {
    let mut cur: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
    // SAFETY: tcgetattr on stdin, which the caller verified is a TTY
    // before forking; a raw syscall with no allocation.
    unsafe {
        if libc::tcgetattr(libc::STDIN_FILENO, cur.as_mut_ptr()) != 0 {
            return true;
        }
    }
    let cur = unsafe { cur.assume_init() };
    orig.c_iflag != cur.c_iflag
        || orig.c_oflag != cur.c_oflag
        || orig.c_cflag != cur.c_cflag
        || orig.c_lflag != cur.c_lflag
        || orig.c_line != cur.c_line
        || orig.c_cc.iter().zip(cur.c_cc.iter()).any(|(a, b)| a != b)
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

    // S4 (stability harden): if thread spawn fails (extreme resource
    // exhaustion / RLIMIT_NPROC), silently skip the guard instead of
    // panicking. The panic hook + watchdog still cover graceful shutdown;
    // this guard only adds SIGKILL/crash recovery. A missing guard is
    // strictly better than a crash at startup.
    if std::thread::Builder::new()
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
        .is_err()
    {
        // Spawn failed — log to stderr (best-effort, never panic here).
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!("cosmostrix: warning — terminal guard thread spawn failed; SIGKILL recovery disabled\n"),
        );
    }
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
