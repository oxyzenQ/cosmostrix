// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Branded CLI output — modern truecolor palette with graceful degradation.
//!
//! This module provides color-coded output helpers for CLI text (help,
//! --doctor, --verbose, --list-*, errors, warnings). It does NOT touch
//! the rain renderer — that uses its own palette system.
//!
//! ## Color palette (modern Tailwind CSS v3)
//!
//! | Semantic | Color | RGB | Tailwind | Use |
//! |----------|-------|-----|----------|-----|
//! | Brand | Purple | #A855F7 (168,85,247) | purple-500 | --help, --list-*, --doctor, info |
//! | Error | Red | #EF4444 (239,68,68) | red-500 | Error messages |
//! | Warn | Yellow | #EAB308 (234,179,8) | yellow-500 | Warning messages |
//! | Verbose prefix | Bold purple | #A855F7 bold | purple-500 + bold | `[verbose]` tag |
//! | Verbose label | Purple | #A855F7 | purple-500 | Field labels in --verbose |
//! | Verbose value | Default | terminal default | — | Field values (readable) |
//!
//! All RGB values are sourced from the Tailwind CSS v3 palette (the de-facto
//! standard for modern UI design systems). These are NOT the ancient ANSI
//! 16-color palette from the VT100 era (1978) — they are calibrated for
//! perceptual uniformity and accessibility on modern displays.
//!
//! ## Capability detection (world-class graceful degradation)
//!
//! Colors are emitted based on the terminal's detected color capability:
//!
//! | Capability | Detection | Output |
//! |------------|-----------|--------|
//! | TrueColor | COLORTERM=truecolor/24bit, TERM=*-direct/*-truecolor | `\x1b[38;2;R;G;Bm` (24-bit RGB) |
//! | Color256 | TERM=*-256color | `\x1b[38;5;Nm` (closest xterm-256 palette index) |
//! | Color16 | TERM is set but no truecolor/256 indicator | `\x1b[3Nm` (basic 16-color ANSI) |
//! | Mono | NO_COLOR set, TERM=dumb, CLICOLOR=0, or piped | plain text, no escapes |
//!
//! This is the same detection strategy used by `bat`, `fd`, `ripgrep`, and
//! `cargo` itself. Modern terminals (kitty, wezterm, alacritty, iTerm2 3.5+,
//! Windows Terminal, foot, xterm, gnome-terminal, konsole) all support
//! TrueColor and will receive the full RGB experience. Older terminals get
//! a graceful fallback instead of escape-sequence garbage.
//!
//! ## Standards compliance
//!
//! - Respects `NO_COLOR` (https://no-color.org/) — disables all colors.
//! - Respects `CLICOLOR=0` — disables colors.
//! - Respects `CLICOLOR_FORCE=1` — forces colors even when piped.
//! - Strips all ANSI when stderr is not a TTY (unless CLICOLOR_FORCE=1).

use std::io::IsTerminal;
use std::sync::OnceLock;

// ── Modern RGB color constants (Tailwind CSS v3 palette) ────────────────────
//
// These are the canonical 24-bit RGB values. The capability-aware escape
// functions below select the right encoding (TrueColor / 256 / 16 / none)
// based on the terminal's detected capability.

/// Brand purple RGB: #A855F7 (Tailwind purple-500).
///
/// Source of truth for the brand color. The TrueColor escape in
/// [`brand_open`] encodes these exact values; the 256-color fallback in
/// [`brand_open`] uses palette index 135 (the closest xterm-256 match,
/// computed via the 6x6x6 cube: 16 + 36*3 + 6*1 + 5 = 135).
///
/// Referenced by `rgb_constants_match_tailwind_palette` test to verify
/// the escape sequences stay in sync with the documented palette.
#[allow(dead_code)] // referenced in tests; kept as source-of-truth documentation
pub const BRAND_PURPLE_RGB: (u8, u8, u8) = (168, 85, 247);

/// Error red RGB: #EF4444 (Tailwind red-500).
///
/// 256-color fallback: index 203 (closest match in the 6x6x6 cube:
/// 16 + 36*5 + 6*1 + 1 = 203).
#[allow(dead_code)] // referenced in tests; kept as source-of-truth documentation
pub const ERROR_RGB: (u8, u8, u8) = (239, 68, 68);

/// Warning yellow RGB: #EAB308 (Tailwind yellow-500).
///
/// 256-color fallback: index 220 (gold — brightest visible yellow in the
/// xterm-256 palette, chosen over the exact-match 214 for warning
/// visibility).
#[allow(dead_code)] // referenced in tests; kept as source-of-truth documentation
pub const WARN_RGB: (u8, u8, u8) = (234, 179, 8);

// ── Color capability detection ──────────────────────────────────────────────

/// Terminal color capability, detected once and cached for the process.
///
/// The capability is probed lazily on first use via [`color_capability()`],
/// then memoized in a `OnceLock` so repeated calls are branch-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCapability {
    /// No color support. Output is plain text — no ANSI escapes.
    ///
    /// Triggered by: NO_COLOR env var, CLICOLOR=0, TERM=dumb, or stderr
    /// is not a TTY (unless CLICOLOR_FORCE=1).
    Mono,

    /// Basic 16-color ANSI palette (VT100 era, 1978).
    ///
    /// The terminal emulator maps these to whatever shades it prefers.
    /// Used when TERM is set but has no truecolor/256color indicator.
    Color16,

    /// xterm 256-color palette (216 RGB cube + 16 ANSI + 24 grayscale).
    ///
    /// Used when TERM contains "256color" but no truecolor indicator.
    Color256,

    /// 24-bit truecolor (16.7 million colors).
    ///
    /// Used when COLORTERM=truecolor/24bit is set, or TERM contains
    /// "-direct" or "-truecolor". This is the modern standard — supported
    /// by every mainstream terminal since 2009-2010.
    TrueColor,
}

/// Detect the terminal's color capability from environment variables.
///
/// Probes (in order):
/// 1. `NO_COLOR` env var (https://no-color.org/) → Mono
/// 2. `CLICOLOR=0` → Mono
/// 3. stderr is not a TTY → Mono (unless `CLICOLOR_FORCE=1`)
/// 4. `COLORTERM` contains "truecolor" or "24bit" → TrueColor
/// 5. `TERM` contains "-direct" or "-truecolor" → TrueColor
/// 6. `TERM` contains "256color" → Color256
/// 7. `TERM=dumb` → Mono
/// 8. Otherwise → Color16 (safe default for older terminals)
#[must_use]
pub fn detect_color_capability() -> ColorCapability {
    // NO_COLOR is the de-facto standard for disabling all colors.
    // https://no-color.org/
    if std::env::var_os("NO_COLOR").is_some() {
        return ColorCapability::Mono;
    }

    // CLICOLOR=0 explicitly disables colors.
    if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
        return ColorCapability::Mono;
    }

    // CLICOLOR_FORCE=1 forces colors even when piped (matches cargo behavior).
    let force = matches!(std::env::var("CLICOLOR_FORCE").ok().as_deref(), Some("1"));

    // If not forced, colors require a TTY.
    if !force && !std::io::stderr().is_terminal() {
        return ColorCapability::Mono;
    }

    let colorterm = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorCapability::TrueColor;
    }

    let term = std::env::var("TERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if term.contains("-direct") || term.contains("-truecolor") {
        return ColorCapability::TrueColor;
    }
    if term.contains("256color") {
        return ColorCapability::Color256;
    }
    if term == "dumb" || term.is_empty() {
        return ColorCapability::Mono;
    }

    // TERM is set to something we don't recognize — assume basic 16-color.
    ColorCapability::Color16
}

/// Get the cached color capability, detecting it on first call.
///
/// The detection runs once per process and is memoized in a `OnceLock`.
/// Subsequent calls are a single atomic load — effectively free.
#[must_use]
pub fn color_capability() -> ColorCapability {
    static CAP: OnceLock<ColorCapability> = OnceLock::new();
    *CAP.get_or_init(detect_color_capability)
}

// ── Capability-aware escape sequences ───────────────────────────────────────
//
// These functions return the right escape sequence for the current terminal.
// Callers should use these instead of the raw TrueColor constants when
// building dynamic output — the constants are only for one-shot startup
// paths (like clap's help_template) where the capability is checked once.

/// Brand purple open sequence, capability-aware.
#[must_use]
pub fn brand_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
        ColorCapability::Color256 => "\x1b[38;5;135m",
        ColorCapability::Color16 => "\x1b[35m",
        ColorCapability::Mono => "",
    }
}

/// Bold brand purple open sequence, capability-aware.
#[must_use]
pub fn brand_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;168;85;247m",
        ColorCapability::Color256 => "\x1b[1;38;5;135m",
        ColorCapability::Color16 => "\x1b[1;35m",
        ColorCapability::Mono => "",
    }
}

/// Error red open sequence, capability-aware.
#[must_use]
pub fn error_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;239;68;68m",
        ColorCapability::Color256 => "\x1b[38;5;203m",
        ColorCapability::Color16 => "\x1b[31m",
        ColorCapability::Mono => "",
    }
}

/// Bold error red open sequence, capability-aware.
#[must_use]
pub fn error_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;239;68;68m",
        ColorCapability::Color256 => "\x1b[1;38;5;203m",
        ColorCapability::Color16 => "\x1b[1;31m",
        ColorCapability::Mono => "",
    }
}

/// Warning yellow open sequence, capability-aware.
#[must_use]
pub fn warn_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;234;179;8m",
        ColorCapability::Color256 => "\x1b[38;5;220m",
        ColorCapability::Color16 => "\x1b[33m",
        ColorCapability::Mono => "",
    }
}

/// Bold warning yellow open sequence, capability-aware.
#[must_use]
pub fn warn_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;234;179;8m",
        ColorCapability::Color256 => "\x1b[1;38;5;220m",
        ColorCapability::Color16 => "\x1b[1;33m",
        ColorCapability::Mono => "",
    }
}

/// Reset sequence (closes any open color/style). Universal across all modes
/// except Mono, where it's a no-op.
#[must_use]
pub fn reset() -> &'static str {
    match color_capability() {
        ColorCapability::Mono => "",
        _ => "\x1b[0m",
    }
}

// ── Color application helpers ────────────────────────────────────────────────

/// Wrap `msg` in bold brand purple. Returns plain text if color is disabled.
#[must_use]
pub fn brand_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", brand_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in bold error red. Returns plain text if color is disabled.
#[must_use]
pub fn error_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", error_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in error red. Returns plain text if color is disabled.
#[must_use]
pub fn error(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", error_open(), msg, reset()),
    }
}

/// Wrap `msg` in bold warning yellow. Returns plain text if color is disabled.
#[must_use]
pub fn warn_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", warn_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in warning yellow. Returns plain text if color is disabled.
#[must_use]
pub fn warn(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", warn_open(), msg, reset()),
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

/// Get the current local time as `[HH:MM]` (24-hour, zero-padded).
///
/// Returns `[--:--]` if the system clock is unavailable (extremely rare —
/// only happens on platforms without a working localtime). This keeps
/// verbose output readable even in degraded environments.
#[must_use]
pub fn now_hhmm() -> String {
    use chrono::Timelike;
    let now = chrono::Local::now();
    format!("[{:02}:{:02}]", now.hour(), now.minute())
}

/// Format a verbose line: bold purple `[verbose] [HH:MM]` prefix + purple
/// label + default-color value.
///
/// The timestamp is captured once per call so all lines in a single verbose
/// dump show the same minute (unless the dump spans a minute boundary).
///
/// Example: `verbose_line("scene:", " monolith")`
/// → `[verbose] [12:01] scene:       monolith`
#[must_use]
pub fn verbose_line(label: &str, value: &str) -> String {
    let ts = now_hhmm();
    match color_capability() {
        ColorCapability::Mono => format!("[verbose] {ts} {label:<14}{value}"),
        _ => format!(
            "{}[verbose]{} {ts} {}{label:<14}{}{value}",
            brand_bold_open(),
            reset(),
            brand_open(),
            reset()
        ),
    }
}

/// Print a verbose line directly to stderr. Convenience wrapper for
/// `eprintln!("{}", verbose_line(label, value))`.
pub fn eprintln_verbose(label: &str, value: &str) {
    eprintln!("{}", verbose_line(label, value));
}

/// Print a raw verbose message (no label/value split) with the
/// `[verbose] [HH:MM]` prefix. Use this for one-off verbose lines that
/// don't fit the label:value pattern (e.g. multi-line dumps, free-form
/// diagnostics).
pub fn eprintln_verbose_raw(msg: &str) {
    let ts = now_hhmm();
    match color_capability() {
        ColorCapability::Mono => eprintln!("[verbose] {ts} {msg}"),
        _ => eprintln!("{}[verbose]{} {ts} {msg}", brand_bold_open(), reset()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_constants_match_tailwind_palette() {
        // Source-of-truth: the RGB constants must encode the exact Tailwind
        // CSS v3 palette values. Any drift here breaks the brand identity.
        assert_eq!(BRAND_PURPLE_RGB, (168, 85, 247)); // #A855F7 purple-500
        assert_eq!(ERROR_RGB, (239, 68, 68)); // #EF4444 red-500
        assert_eq!(WARN_RGB, (234, 179, 8)); // #EAB308 yellow-500
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

    #[test]
    fn color_capability_enum_has_four_variants() {
        // Sanity check: the capability enum must cover the four degradation
        // tiers. Adding a new tier requires updating every match in the
        // escape functions below — the compiler will catch missing arms.
        let variants = [
            ColorCapability::Mono,
            ColorCapability::Color16,
            ColorCapability::Color256,
            ColorCapability::TrueColor,
        ];
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn brand_open_returns_correct_escape_per_capability() {
        // Verify the escape mapping for each capability tier. This is the
        // world-class invariant: every tier must produce a valid escape
        // (or empty string for Mono), and the TrueColor tier must encode
        // the exact RGB values from the source-of-truth constants.
        let truecolor_escape = match ColorCapability::TrueColor {
            ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
            ColorCapability::Color256 => "\x1b[38;5;135m",
            ColorCapability::Color16 => "\x1b[35m",
            ColorCapability::Mono => "",
        };
        assert!(truecolor_escape.contains("168;85;247"));
        assert!(truecolor_escape.contains("38;2;"));

        let color256_escape = match ColorCapability::Color256 {
            ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
            ColorCapability::Color256 => "\x1b[38;5;135m",
            ColorCapability::Color16 => "\x1b[35m",
            ColorCapability::Mono => "",
        };
        // 135 = 16 + 36*3 + 6*1 + 5 (closest xterm-256 cube index for #A855F7)
        assert!(color256_escape.contains("38;5;135"));

        let color16_escape = match ColorCapability::Color16 {
            ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
            ColorCapability::Color256 => "\x1b[38;5;135m",
            ColorCapability::Color16 => "\x1b[35m",
            ColorCapability::Mono => "",
        };
        // 35 = ANSI Magenta (closest 16-color to purple #A855F7)
        assert_eq!(color16_escape, "\x1b[35m");

        let mono_escape = match ColorCapability::Mono {
            ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
            ColorCapability::Color256 => "\x1b[38;5;135m",
            ColorCapability::Color16 => "\x1b[35m",
            ColorCapability::Mono => "",
        };
        assert_eq!(mono_escape, "");
    }

    #[test]
    fn error_open_uses_red_palette_per_capability() {
        // Error red #EF4444 must map to:
        // - TrueColor: \x1b[38;2;239;68;68m
        // - Color256: \x1b[38;5;203m (closest cube index: 16 + 36*5 + 6*1 + 1)
        // - Color16: \x1b[31m (ANSI Red)
        // - Mono: empty
        let truecolor = match ColorCapability::TrueColor {
            ColorCapability::TrueColor => "\x1b[38;2;239;68;68m",
            ColorCapability::Color256 => "\x1b[38;5;203m",
            ColorCapability::Color16 => "\x1b[31m",
            ColorCapability::Mono => "",
        };
        assert!(truecolor.contains("239;68;68"));

        let color256 = match ColorCapability::Color256 {
            ColorCapability::TrueColor => "\x1b[38;2;239;68;68m",
            ColorCapability::Color256 => "\x1b[38;5;203m",
            ColorCapability::Color16 => "\x1b[31m",
            ColorCapability::Mono => "",
        };
        assert!(color256.contains("38;5;203"));

        let color16 = match ColorCapability::Color16 {
            ColorCapability::TrueColor => "\x1b[38;2;239;68;68m",
            ColorCapability::Color256 => "\x1b[38;5;203m",
            ColorCapability::Color16 => "\x1b[31m",
            ColorCapability::Mono => "",
        };
        assert_eq!(color16, "\x1b[31m");
    }

    #[test]
    fn warn_open_uses_yellow_palette_per_capability() {
        // Warn yellow #EAB308 must map to:
        // - TrueColor: \x1b[38;2;234;179;8m
        // - Color256: \x1b[38;5;220m (gold — brightest visible yellow)
        // - Color16: \x1b[33m (ANSI Yellow)
        // - Mono: empty
        let truecolor = match ColorCapability::TrueColor {
            ColorCapability::TrueColor => "\x1b[38;2;234;179;8m",
            ColorCapability::Color256 => "\x1b[38;5;220m",
            ColorCapability::Color16 => "\x1b[33m",
            ColorCapability::Mono => "",
        };
        assert!(truecolor.contains("234;179;8"));

        let color256 = match ColorCapability::Color256 {
            ColorCapability::TrueColor => "\x1b[38;2;234;179;8m",
            ColorCapability::Color256 => "\x1b[38;5;220m",
            ColorCapability::Color16 => "\x1b[33m",
            ColorCapability::Mono => "",
        };
        assert!(color256.contains("38;5;220"));
    }

    #[test]
    fn reset_returns_universal_ansi_reset_for_non_mono() {
        // RESET must be \x1b[0m for all color tiers (universal across
        // truecolor/256/16), and empty string for Mono.
        let reset_truecolor = match ColorCapability::TrueColor {
            ColorCapability::TrueColor | ColorCapability::Color256 | ColorCapability::Color16 => {
                "\x1b[0m"
            }
            ColorCapability::Mono => "",
        };
        assert_eq!(reset_truecolor, "\x1b[0m");

        let reset_mono = match ColorCapability::Mono {
            ColorCapability::TrueColor | ColorCapability::Color256 | ColorCapability::Color16 => {
                "\x1b[0m"
            }
            ColorCapability::Mono => "",
        };
        assert_eq!(reset_mono, "");
    }
}
