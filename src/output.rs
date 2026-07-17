// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Branded CLI output — purple brand color for all non-render text.
//!
//! This module provides color-coded output helpers for CLI text (help,
//! --doctor, --verbose, --list-*, errors, warnings). It does NOT touch
//! the rain renderer — that uses its own palette system.
//!
//! ## Color scheme
//!
//! | Semantic | Color | RGB | Use |
//! |----------|-------|-----|-----|
//! | Brand | Purple | #A855F7 (168,85,247) | Normal CLI output: --help, --list-*, --doctor, info |
//! | Error | Red | #EF4444 (239,68,68) | Error messages |
//! | Warn | Yellow | #EAB308 (234,179,8) | Warning messages |
//! | Verbose prefix | Bold purple | #A855F7 bold | `[verbose]` tag |
//! | Verbose label | Purple | #A855F7 | Field labels in --verbose |
//! | Verbose value | Default | terminal default | Field values (readable) |
//!
//! ## TTY detection
//!
//! Colors are only emitted when the output stream is a terminal (TTY).
//! When piped or redirected, all output is plain text — no escape codes.
//! This prevents ANSI garbage in log files and pipes.

use std::io::IsTerminal;

// ── ANSI color constants ─────────────────────────────────────────────────────

/// Brand purple: #A855F7 (168, 85, 247). Used for normal CLI output.
///
/// This is the canonical brand color for all non-render CLI text: help,
/// verbose, errors, version, doctor, list printers. The rain renderer
/// uses its own palette system — never import this into render code.
pub const BRAND_PURPLE: &str = "\x1b[38;2;168;85;247m";

/// Backwards-compatible alias for [`BRAND_PURPLE`]. Prefer the explicit
/// `BRAND_PURPLE` name in new code.
pub const BRAND: &str = BRAND_PURPLE;

/// Bold brand purple. Used for verbose prefix and headers.
pub const BRAND_BOLD: &str = "\x1b[1;38;2;168;85;247m";

/// Error red: #EF4444 (239, 68, 68). Used for error messages.
pub const ERROR: &str = "\x1b[38;2;239;68;68m";

/// Bold error red. Used for error labels (e.g. "error:").
pub const ERROR_BOLD: &str = "\x1b[1;38;2;239;68;68m";

/// Warning yellow: #EAB308 (234, 179, 8). Used for warning messages.
pub const WARN: &str = "\x1b[38;2;234;179;8m";

/// Bold warning yellow. Used for warning labels (e.g. "⚠").
pub const WARN_BOLD: &str = "\x1b[1;38;2;234;179;8m";

/// ANSI reset. Restores terminal default color/style.
pub const RESET: &str = "\x1b[0m";

// ── TTY detection ────────────────────────────────────────────────────────────

/// Returns true if stderr is a terminal (and colors should be emitted).
#[inline]
pub fn stderr_is_tty() -> bool {
    std::io::stderr().is_terminal()
}

// ── Color application helpers ────────────────────────────────────────────────

/// Wrap `msg` in bold brand purple. Returns plain text if not a TTY.
#[must_use]
pub fn brand_bold(msg: &str) -> String {
    if stderr_is_tty() {
        format!("{BRAND_BOLD}{msg}{RESET}")
    } else {
        msg.to_string()
    }
}

/// Wrap `msg` in bold error red. Returns plain text if not a TTY.
#[must_use]
pub fn error_bold(msg: &str) -> String {
    if stderr_is_tty() {
        format!("{ERROR_BOLD}{msg}{RESET}")
    } else {
        msg.to_string()
    }
}

/// Wrap `msg` in error red. Returns plain text if not a TTY.
#[must_use]
pub fn error(msg: &str) -> String {
    if stderr_is_tty() {
        format!("{ERROR}{msg}{RESET}")
    } else {
        msg.to_string()
    }
}

/// Wrap `msg` in bold warning yellow. Returns plain text if not a TTY.
#[must_use]
pub fn warn_bold(msg: &str) -> String {
    if stderr_is_tty() {
        format!("{WARN_BOLD}{msg}{RESET}")
    } else {
        msg.to_string()
    }
}

/// Wrap `msg` in warning yellow. Returns plain text if not a TTY.
#[must_use]
pub fn warn(msg: &str) -> String {
    if stderr_is_tty() {
        format!("{WARN}{msg}{RESET}")
    } else {
        msg.to_string()
    }
}

// ── Print helpers (stderr) ───────────────────────────────────────────────────

/// Print a labeled error to stderr: "error: <msg>" in red.
pub fn eprintln_error_labeled(msg: &str) {
    eprintln!("{} {}", error_bold("error:"), error(msg));
}

/// Print a labeled warning to stderr: "⚠ <msg>" in yellow.
pub fn eprintln_warn_labeled(msg: &str) {
    eprintln!("{} {}", warn_bold("⚠"), warn(msg));
}

// ── Verbose helpers ──────────────────────────────────────────────────────────

/// Format a verbose line: bold purple `[verbose]` prefix + purple label +
/// default-color value. Not "boring" because the prefix is bold and the
/// value stays readable in terminal default color.
///
/// Example: `verbose_line("scene:", " monolith")`
/// → `[verbose]  scene:        monolith` (prefix bold purple, label purple, value default)
#[must_use]
pub fn verbose_line(label: &str, value: &str) -> String {
    if stderr_is_tty() {
        format!("{BRAND_BOLD}[verbose]{RESET} {BRAND}{label:<14}{RESET}{value}")
    } else {
        format!("[verbose] {label:<14}{value}")
    }
}

/// Print a verbose line directly to stderr. Convenience wrapper for
/// `eprintln!("{}", verbose_line(label, value))`.
pub fn eprintln_verbose(label: &str, value: &str) {
    eprintln!("{}", verbose_line(label, value));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_constants_are_nonempty() {
        assert!(!BRAND.is_empty());
        assert!(!ERROR.is_empty());
        assert!(!WARN.is_empty());
        assert!(!RESET.is_empty());
    }

    #[test]
    fn brand_bold_wraps_message() {
        let wrapped = brand_bold("hello");
        assert!(wrapped.contains("hello"));
    }

    #[test]
    fn error_bold_wraps_message() {
        let wrapped = error_bold("error:");
        assert!(wrapped.contains("error:"));
    }

    #[test]
    fn verbose_line_contains_prefix_and_label() {
        let line = verbose_line("scene:", " monolith");
        assert!(line.contains("[verbose]"));
        assert!(line.contains("scene:"));
        assert!(line.contains("monolith"));
    }
}
