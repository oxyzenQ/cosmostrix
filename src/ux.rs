// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unified CLI user-experience output.
//!
//! Every user-facing message MUST flow through this module so
//! error/warning formatting is consistent across the entire codebase.
//!
//! # Convention
//!
//! **Fatal errors** (exit the process):
//! - `die_input(msg)`  — exit 2, for invalid user arguments / usage errors
//! - `die_config(msg)` — exit 1, for config file / runtime failures
//!
//! **Non-fatal warnings** (do not exit):
//! - `warn(msg)` — printed to stderr, process continues
//!
//! The caller is responsible for formatting the complete message
//! (including the `error:` / `warning:` prefix).  This module is the
//! *delivery channel*, not the formatter — it guarantees one message,
//! one line, one exit code, never a Rust `Debug` wrapper.
//!
//! # Anti-pattern (never do this)
//!
//! ```ignore
//! eprintln!("{e}");
//! return Err(std::io::Error::new(..., e));  // ← prints TWICE
//! ```
//!
//! Use `ux::die_input(e)` or `ux::or_exit(r)` instead.

use std::process;

// ── Fatal exit helpers ─────────────────────────────────────────────────────

/// Print `msg` to stderr and exit 2 (invalid input / usage error).
#[cold]
pub fn die_input(msg: impl AsRef<str>) -> ! {
    eprintln!("{}", msg.as_ref());
    process::exit(2);
}

/// Print `msg` to stderr and exit 1 (config / runtime failure).
#[cold]
pub fn die_config(msg: impl AsRef<str>) -> ! {
    eprintln!("{}", msg.as_ref());
    process::exit(1);
}

// ── Non-fatal helpers ──────────────────────────────────────────────────────

/// Print `msg` to stderr (non-fatal). Does not exit.
pub fn warn(msg: impl AsRef<str>) {
    eprintln!("{}", msg.as_ref());
}

// ── Combinators ────────────────────────────────────────────────────────────

/// Unwrap a `Result<T, E>` whose `Err` carries a pre-formatted error string.
///
/// On `Err` the message is printed to stderr and the process exits 2.
/// On `Ok` the inner value is returned directly — no `?`, no propagation.
///
/// # Example
///
/// ```ignore
/// let speed = ux::or_exit(validate_speed(args.speed));
/// ```
pub fn or_exit<T, E: AsRef<str>>(r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => die_input(e),
    }
}
