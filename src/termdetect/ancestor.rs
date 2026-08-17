// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use crate::termdetect::hosts::HIGH_PERF_TERM_HINTS;

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    /// When set to true on the current thread, `ancestor_process_names`
    /// short-circuits and returns an empty vec. This isolates tests that
    /// assert on `detect()` results from the host machine's actual
    /// terminal-emulator process ancestry.
    ///
    /// Without this, running `cargo test` from inside Alacritty/Kitty/
    /// WezTerm/Ghostty/Foot/Konsole makes the test process's ancestor
    /// chain contain the terminal name, firing Layer 5 of
    /// `high_perf_detection_source` and Layer 4 of
    /// `kitty_keyboard_supported`. That contaminates tests which set
    /// `TERM=dumb` or `TERM=xterm-256color` (no high-perf hint) and
    /// assert `!caps.kitty_keyboard` / `dynamic_default_fps == 60.0` —
    /// they fail on the developer's machine but pass in CI/headless.
    ///
    /// Thread-local (not global) so that tests which DO want the real
    /// walk (`ancestor_process_names_returns_nonempty_in_test_env`)
    /// can run concurrently on their own thread without seeing the flag.
    /// The flag is never set outside `#[cfg(test)]`, so production
    /// behavior is unchanged.
    static INHIBIT_ANCESTOR_WALK: Cell<bool> = const { Cell::new(false) };
}

/// Test-only: set the thread-local inhibit flag for `ancestor_process_names`.
/// Returns the previous value so callers can restore it (RAII via `EnvGuard`
/// in `tests.rs`). Has no effect in production builds (`#[cfg(test)]` only).
#[cfg(test)]
pub(crate) fn set_ancestor_walk_inhibited(inhibit: bool) -> bool {
    INHIBIT_ANCESTOR_WALK.with(|flag| {
        let prev = flag.get();
        flag.set(inhibit);
        prev
    })
}

/// Parse the `ppid` field from a `/proc/<pid>/stat` line. The stat format
/// is `pid (comm) state ppid ...` where `comm` can contain spaces and
/// parens. We parse from the right of the LAST `)` to avoid ambiguity
/// with parens inside `comm`. Returns None if the line is malformed.
///
/// Pure function — unit-testable without touching the filesystem.
#[cfg(target_os = "linux")]
pub(crate) fn parse_proc_ppid(stat_line: &str) -> Option<i32> {
    let rparen = stat_line.rfind(')')?;
    let after_comm = &stat_line[rparen + 1..];
    let mut fields = after_comm.split_whitespace();
    fields.next()?; // state (S, R, D, T, Z, etc.)
    fields.next()?.parse().ok()
}

/// Read the `comm` name (process name) for a given PID on Linux. Returns
/// None if /proc is not available or the PID doesn't exist. The kernel
/// truncates `comm` to 15 characters (TASK_COMM_LEN=16 including NUL).
#[cfg(target_os = "linux")]
pub(super) fn read_proc_comm(pid: i32) -> Option<String> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Walk the process ancestor chain on Linux and return the list of
/// process names (comm) from parent → grandparent → ... → init. Stops
/// after `max_depth` hops or when reaching PID 1 (init/systemd).
/// Returns an empty vec on non-Linux platforms or if /proc is unavailable.
///
/// This is the fallback detection layer for terminals that don't set
/// `TERM_PROGRAM` AND have a non-standard `TERM` (e.g., Alacritty with
/// `TERM=xterm-direct`). Walking the process tree finds the terminal
/// emulator process by name (e.g., "alacritty", "kitty", "ghostty").
#[cfg(target_os = "linux")]
pub(crate) fn ancestor_process_names(max_depth: usize) -> Vec<String> {
    // Test isolation: when the current thread has set the inhibit flag
    // (via EnvGuard in tests.rs), skip the /proc walk and return empty.
    // This lets tests that mutate TERM/TERM_PROGRAM assert on `detect()`
    // results without the host's actual terminal ancestry contaminating
    // Layer 5 (high_perf_detection_source) or Layer 4 (kitty_keyboard).
    // The flag is thread-local and `#[cfg(test)]`-only, so production
    // behavior is unchanged.
    #[cfg(test)]
    if INHIBIT_ANCESTOR_WALK.with(|flag| flag.get()) {
        return Vec::new();
    }
    let mut names = Vec::with_capacity(max_depth);
    let mut pid = std::process::id() as i32;
    for _ in 0..max_depth {
        let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => s,
            Err(_) => break,
        };
        let ppid = match parse_proc_ppid(&stat) {
            Some(p) => p,
            None => break,
        };
        if ppid <= 1 {
            break;
        }
        if let Some(name) = read_proc_comm(ppid) {
            names.push(name);
        }
        pid = ppid;
    }
    names
}

/// No-op stub on non-Linux platforms. macOS users rely on TERM_PROGRAM
/// (iTerm.app, Apple_Terminal) which is always set by those terminals.
#[cfg(not(target_os = "linux"))]
pub(crate) fn ancestor_process_names(_max_depth: usize) -> Vec<String> {
    Vec::new()
}

/// Returns true if any name in `names` matches a HIGH_PERF_TERM_HINT
/// (case-insensitive substring). Extracted from `is_high_perf_terminal`
/// for unit testability — the ancestor walk itself requires /proc and
/// can't be tested in isolation, but the matching logic can.
pub(crate) fn ancestor_matches_high_perf(names: &[String]) -> bool {
    names.iter().any(|name| {
        let name_lower = name.to_ascii_lowercase();
        HIGH_PERF_TERM_HINTS
            .iter()
            .any(|&hint| name_lower.contains(hint))
    })
}
