// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Branded CLI output — cosmostrix neon-family palette with graceful
//! degradation.
//!
//! This module provides color-coded output helpers for CLI text (help,
//! --doctor, --verbose, --list-*, errors, warnings, suggestions). It does
//! NOT touch the rain renderer — that uses its own palette system.
//!
//! ## Color palette (cosmostrix neon family — v80.0.0-alpha.1 S-master-HUNT-5)
//!
//! | Semantic | Color | RGB | Neon lineage | Use |
//! |----------|-------|-----|--------------|-----|
//! | Brand | Purple | #A855F7 (168,85,247) | NeonPurple band midpoint | --help, --list-*, --doctor, info |
//! | Error | Red | #FF5A5A (255,90,90) | NeonRed bright body | Error messages |
//! | Warn | Yellow | #FFEB3C (255,235,60) | NeonYellow head | Warning messages |
//! | Suggestion | Crystal white | #DCEBFF (220,235,255) | NeonWhite head | tip:/hint:/did-you-mean lines |
//! | Verbose prefix | Bold purple | #A855F7 bold | NeonPurple band midpoint + bold | `[verbose]` tag |
//! | Verbose label | Purple | #A855F7 | NeonPurple band midpoint | Field labels in --verbose |
//! | Verbose value | Default | terminal default | — | Field values (readable) |
//!
//! Owner mandate (2026-09-03): the CLI semantic colors are drawn from the
//! rain renderer's own neon theme family
//! (`engine/chroma_dragon_engine/catalog/themes.rs`) so the CLI surface
//! harmonizes with the rain aesthetic:
//! - error = the NeonRed bright-body stop (255,90,90) — attention-grade
//!   red that stays readable on black;
//! - warn = the NeonYellow head stop (255,235,60) — the electric yellow
//!   that identifies the theme (replacing the darker gold #EAB308);
//! - suggestion = the NeonWhite head stop (220,235,255) — the tinted
//!   "crystal white" glow;
//! - brand keeps #A855F7 — it sits at the NeonPurple band midpoint
//!   (between stops (134,58,208) and (180,90,255)), a deliberate
//!   accessibility choice retained as the identity color.
//!
//! Every color degrades intelligently to legacy palettes when the
//! terminal/OS lacks truecolor support (see the capability table below)
//! — the "legacy colors" fallback the owner specified: Color256 gets the
//! nearest xterm-256 index, Color16 gets the classic ANSI slot, Mono
//! gets plain text.
//!
//! ## Capability detection (world-class graceful degradation)
//!
//! Colors are emitted based on the terminal's detected color capability:
//!
//! | Capability | Detection | Output |
//! |------------|-----------|--------|
//! | TrueColor | COLORTERM=truecolor/24bit, TERM=-direct/-truecolor | `\x1b[38;2;R;G;Bm` (24-bit RGB) |
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

// ── RGB color constants (cosmostrix neon family) ────────────────────────────
//
// These are the canonical 24-bit RGB values. The capability-aware escape
// functions below select the right encoding (TrueColor / 256 / 16 / none)
// based on the terminal's detected capability. Lineage: every value is a
// stop from (or midpoint of) the rain renderer's neon theme catalog
// (engine/chroma_dragon_engine/catalog/themes.rs) — the CLI surface
// shares the rain's aesthetic instead of an external design system
// (S-master-HUNT-5 owner mandate 2026-09-03; previously Tailwind v3).

/// Brand purple RGB: #A855F7 (168,85,247).
///
/// NeonPurple band midpoint — sits between the catalog stops
/// (134,58,208) and (180,90,255); a deliberate accessibility choice
/// retained as the identity color (unchanged by the neon retune).
///
/// Source of truth for the brand color. The TrueColor escape in
/// [`brand_open`] encodes these exact values; the 256-color fallback in
/// [`brand_open`] uses palette index 135 (the closest xterm-256 match,
/// computed via the 6x6x6 cube: 16 + 363 + 61 + 5 = 135).
///
/// Referenced by `rgb_constants_match_neon_family_palette` test to verify
/// the escape sequences stay in sync with the documented palette.
#[cfg(test)] // referenced in tests; kept as source-of-truth documentation
pub const BRAND_PURPLE_RGB: (u8, u8, u8) = (168, 85, 247);

/// Error red RGB: #FF5A5A (255,90,90).
///
/// NeonRed bright-body stop (catalog `themes.rs`: the (255,90,90) stop
/// between body (224,66,66) and shoulder (255,115,118)). Brighter than
/// the previous Tailwind red-500 (239,68,68) — keeps small error text
/// readable on black while reading unmistakably as "red".
///
/// 256-color fallback: index 203 (closest match in the 6x6x6 cube:
/// 16 + 365 + 61 + 1 = 203 → (255,95,95) — 5/channel off).
#[cfg(test)] // referenced in tests; kept as source-of-truth documentation
pub const ERROR_RGB: (u8, u8, u8) = (255, 90, 90);

/// Warning yellow RGB: #FFEB3C (255,235,60).
///
/// NeonYellow head stop (catalog `themes.rs`: (255,235,60) — the
/// electric yellow that IS the theme's identity; the stop the rain's
/// leading glyphs glow in). Replaces the previous Tailwind gold
/// (234,179,8): the neon head is the family's true yellow and pops
/// harder on black.
///
/// 256-color fallback: index 220 (gold — brightest visible yellow in the
/// xterm-256 palette, chosen over the nearer exact-match indices for
/// warning visibility; at 256 depth the blue-tinted 60 offset is lost
/// anyway, so visibility wins).
#[cfg(test)] // referenced in tests; kept as source-of-truth documentation
pub const WARN_RGB: (u8, u8, u8) = (255, 235, 60);

/// Suggestion crystal-white RGB: #DCEBFF (220,235,255).
///
/// NeonWhite head stop (catalog `themes.rs`: (220,235,255) — the
/// blue-tinted crystal glow the rain's white theme leads with). Owner
/// color contract (2026-09-03): suggestions render WHITE, distinct from
/// error red and warning yellow. Applies to "tip: a similar argument
/// exists", "hint:", "did you mean", and "[possible values: …]" lines —
/// whether printed standalone (`suggestion_open`) or embedded inside an
/// error/warning block (the line-aware `eprintln_error_labeled`).
///
/// 256-color fallback: index 255 (238,238,238 — the nearest near-white;
/// the blue tint cannot survive the 256 palette). Color16 fallback:
/// bright white (97) — the aixterm bright slot, universally supported,
/// reads clearly as "white" where the normal 37 can render dim gray.
#[cfg(test)] // referenced in tests; kept as source-of-truth documentation
pub const SUGGESTION_RGB: (u8, u8, u8) = (220, 235, 255);

// ── Color capability detection ──────────────────────────────────────────────

/// Terminal color capability, detected once and cached for the process.
///
/// The capability is probed lazily on first use via [`color_capability()`],
/// then memoized in a `OnceLock` so repeated calls are branch-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorCapability {
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
pub(crate) fn detect_color_capability() -> ColorCapability {
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
pub(crate) fn color_capability() -> ColorCapability {
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
pub(crate) fn brand_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;168;85;247m",
        ColorCapability::Color256 => "\x1b[38;5;135m",
        ColorCapability::Color16 => "\x1b[35m",
        ColorCapability::Mono => "",
    }
}

/// Bold brand purple open sequence, capability-aware.
#[must_use]
pub(crate) fn brand_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;168;85;247m",
        ColorCapability::Color256 => "\x1b[1;38;5;135m",
        ColorCapability::Color16 => "\x1b[1;35m",
        ColorCapability::Mono => "",
    }
}

/// Error red open sequence, capability-aware.
#[must_use]
pub(crate) fn error_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;255;90;90m",
        ColorCapability::Color256 => "\x1b[38;5;203m",
        ColorCapability::Color16 => "\x1b[31m",
        ColorCapability::Mono => "",
    }
}

/// Bold error red open sequence, capability-aware.
#[must_use]
pub(crate) fn error_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;255;90;90m",
        ColorCapability::Color256 => "\x1b[1;38;5;203m",
        ColorCapability::Color16 => "\x1b[1;31m",
        ColorCapability::Mono => "",
    }
}

/// Warning yellow open sequence, capability-aware.
#[must_use]
pub(crate) fn warn_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;255;235;60m",
        ColorCapability::Color256 => "\x1b[38;5;220m",
        ColorCapability::Color16 => "\x1b[33m",
        ColorCapability::Mono => "",
    }
}

/// Bold warning yellow open sequence, capability-aware.
#[must_use]
pub(crate) fn warn_bold_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[1;38;2;255;235;60m",
        ColorCapability::Color256 => "\x1b[1;38;5;220m",
        ColorCapability::Color16 => "\x1b[1;33m",
        ColorCapability::Mono => "",
    }
}

/// Suggestion crystal-white open sequence, capability-aware.
///
/// v80.0.0-alpha.1 S-master-HUNT-5 (owner color contract 2026-09-03):
/// suggestions ("tip:" / "hint:" / did-you-mean / "[possible values: …]"
/// lines) render WHITE — distinct from error red and warning yellow.
/// TrueColor: the NeonWhite head stop (220,235,255). Color256: 255
/// (238,238,238 — nearest near-white; the blue tint cannot survive the
/// 256 palette). Color16: 97 — the aixterm BRIGHT white slot (universally
/// supported by xterm-compatible terminals; the normal 37 can render as
/// dim gray on classic 16-color palettes, which would blur suggestions
/// into body text). Mono: no color, plain text.
#[must_use]
pub(crate) fn suggestion_open() -> &'static str {
    match color_capability() {
        ColorCapability::TrueColor => "\x1b[38;2;220;235;255m",
        ColorCapability::Color256 => "\x1b[38;5;255m",
        ColorCapability::Color16 => "\x1b[97m",
        ColorCapability::Mono => "",
    }
}

/// Reset sequence (closes any open color/style). Universal across all modes
/// except Mono, where it's a no-op.
#[must_use]
pub(crate) fn reset() -> &'static str {
    match color_capability() {
        ColorCapability::Mono => "",
        _ => "\x1b[0m",
    }
}

// ── Color application helpers ────────────────────────────────────────────────

/// Wrap `msg` in bold brand purple. Returns plain text if color is disabled.
#[must_use]
pub(crate) fn brand_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", brand_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in bold error red. Returns plain text if color is disabled.
#[must_use]
pub(crate) fn error_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", error_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in error red. Returns plain text if color is disabled.
#[must_use]
pub(crate) fn error(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", error_open(), msg, reset()),
    }
}

/// Wrap `msg` in bold warning yellow. Returns plain text if color is disabled.
#[must_use]
pub(crate) fn warn_bold(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", warn_bold_open(), msg, reset()),
    }
}

/// Wrap `msg` in warning yellow. Returns plain text if color is disabled.
#[must_use]
pub(crate) fn warn(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", warn_open(), msg, reset()),
    }
}

/// Wrap `msg` in suggestion crystal white. Returns plain text if color
/// is disabled.
///
/// S-master-HUNT-5: the suggestion semantic — see [`suggestion_open`].
#[must_use]
pub(crate) fn suggestion(msg: &str) -> String {
    match color_capability() {
        ColorCapability::Mono => msg.to_string(),
        _ => format!("{}{}{}", suggestion_open(), msg, reset()),
    }
}

// ── Broken-pipe-safe eprintln ────────────────────────────────────────────────

/// Like `eprintln!` but never panics on broken stderr.
///
/// Uses `write_fmt` with the error explicitly discarded. When the terminal
/// is closed (SIGHUP, PTY destroyed), stderr becomes a broken pipe.
/// `eprintln!` calls `stderr().write_fmt(...)` which panics on write
/// failure (Rust std intentionally panics to surface I/O errors). A panic
/// in post-exit paths (verbose dump, live-reload error print, debug trace
/// drain) would unwind main(), hit the panic hook (which is safe), and
/// abort the process with exit code 101 instead of the intended 0/2.
///
/// This macro breaks the chain by discarding the write error — safe to
/// call in any context where stderr may be broken: post-exit paths,
/// signal handlers, the watchdog thread, and the panic hook itself.
///
/// # When to use
///
/// Use `eprintln_safe!` instead of `eprintln!` in:
/// - Post-exit verbose dumps (after `Terminal::drop` restored the terminal)
/// - Live-reload error printing (terminal may have been closed mid-session)
/// - Debug trace draining (same reason)
/// - Any code path that runs after the rain loop exits
///
/// `eprintln!` remains fine for:
/// - Startup output (before alt screen — stderr is a healthy TTY)
/// - In-loop verbose output (stderr is captured by alt screen but NOT broken)
///
/// # Example
///
/// ```no_run
/// # use crate::output::eprintln_safe;
/// eprintln_safe!("error: {}", "config not found");
/// eprintln_safe!("[verbose] color_scheme:  {} (was {})", "nebula", "vaporwave");
/// ```
macro_rules! eprintln_safe {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = std::io::stderr().write_fmt(format_args!($($arg)*));
        let _ = std::io::stderr().write_fmt(format_args!("\n"));
        let _ = std::io::stderr().flush();
    }};
}
pub(crate) use eprintln_safe;

// ── Print helpers (stderr) ───────────────────────────────────────────────────

/// Print a labeled error to stderr: "error: <msg>" in red.
pub(crate) fn eprintln_error_labeled(msg: &str) {
    eprintln_safe!("{}", render_labeled_block("error:", error_bold, error, msg));
}

/// Print a labeled warning to stderr: "! <msg>" in yellow.
/// v80.0.0-beta.2: ASCII symbol only — icon glyphs ("\u{26a0}" and every
/// other pictograph/emoji) render as tofu on some terminals. Enforced by
/// `scripts/check-symbol-only-output.sh` (gate-keepers + build.sh check-all).
pub(crate) fn eprintln_warn_labeled(msg: &str) {
    // Phase 5 closure (P3-5): increment the startup warning counter so the
    // caller can emit a summary line at the end of config apply. This helps
    // users who miss individual warnings in noisy startup output.
    STARTUP_WARNING_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    eprintln_safe!("{}", render_labeled_block("!", warn_bold, warn, msg));
}

// ── Line-aware semantic rendering (S-master-HUNT-5) ──────────────────────
//
// Owner color contract (2026-09-03): error = red, warning = yellow,
// SUGGESTION = white — including suggestions that are EMBEDDED inside
// an error/warning message. Before this, a multi-line error like
//
//   error: invalid value for --glitch-level: sutble
//   expected one of: none, subtle, default, intense
//     tip: a similar value exists: 'subtle'
//
// wrapped EVERY line in red, so the tip (a suggestion, semantically)
// drowned in the error color. The renderer now recognizes suggestion
// lines and styles them with the suggestion semantic instead.

/// Recognize a suggestion line inside an error/warning block.
///
/// Matches lines whose trimmed content starts with one of the project's
/// suggestion prefixes: `tip:` (clap-style did-you-mean, the value-
/// suggestion engine in `cli/suggestion.rs`), `hint:` (config-hints,
/// testconf), `[possible values: …]` / `(possible values: …)` (enum
/// value lists), and `did you mean` (legacy phrasing kept for safety).
/// Indentation is irrelevant — messages compose these lines at varying
/// depths (`"\n  tip: …"` and friends).
fn is_suggestion_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("tip:")
        || t.starts_with("hint:")
        || t.starts_with("[possible values")
        || t.starts_with("(possible values")
        || t.starts_with("did you mean")
}

/// Render a labeled multi-line message with per-line semantic colors.
///
/// The FIRST line gets `{label} {body}` with the label bold in the
/// message color. Every subsequent line keeps the message color — EXCEPT
/// suggestion lines (see [`is_suggestion_line`]), which render in the
/// suggestion (white) semantic. In Mono mode everything is plain text.
///
/// `label_wrap`/`body_wrap` are the bold/regular colorizers of the
/// message semantic (error or warn); the suggestion colorizer is fixed.
fn render_labeled_block(
    label: &str,
    label_wrap: fn(&str) -> String,
    body_wrap: fn(&str) -> String,
    msg: &str,
) -> String {
    use std::fmt::Write as _;

    let mut lines = msg.split('\n');
    let mut out = String::with_capacity(msg.len() + 32);
    // First line: always the labeled head, always the message semantic.
    if let Some(first) = lines.next() {
        let _ = write!(out, "{} {}", label_wrap(label), body_wrap(first));
    }
    // Subsequent lines: suggestion lines switch to the white semantic.
    for line in lines {
        out.push('\n');
        let styled = if is_suggestion_line(line) {
            suggestion(line)
        } else {
            body_wrap(line)
        };
        out.push_str(&styled);
    }
    out
}

/// Print a standalone suggestion line to stderr in suggestion white
/// (S-master-HUNT-5 owner color contract 2026-09-03).
///
/// For hint/tip lines that are NOT embedded in an error/warning block —
/// e.g. the `testconf: hint: …` guidance lines. Embedded tip lines are
/// handled automatically by the line-aware [`eprintln_error_labeled`] /
/// [`eprintln_warn_labeled`] renderers; this helper is for the standalone
/// case. Broken-pipe-safe via `eprintln_safe!`.
pub(crate) fn eprintln_suggestion_line(msg: &str) {
    eprintln_safe!("{}", suggestion(msg));
}

/// v80.0.0-beta.1 killer-features hardening: route a warning that can fire on BOTH
/// sides of the interactive session boundary (startup AND mid-rain via
/// scene changes / live reload). Before the rain session starts, print
/// immediately (same behavior as `eprintln_warn_labeled`); while the
/// alternate screen is active, buffer into the session warning log
/// (`live_config::push_runtime_warning`) so the line lands on the main
/// screen post-exit instead of leaking into the rain matrix (AB-10).
///
/// Use this for config-block warnings (colors-custom / charset-custom /
/// scene-custom). Pure startup-only warnings can keep using
/// `eprintln_warn_labeled` directly.
pub(crate) fn warn_runtime_or_now(msg: &str) {
    if crate::live_config::interactive_session_active() {
        crate::live_config::push_runtime_warning(msg);
    } else {
        eprintln_warn_labeled(msg);
    }
}

/// v50.0.0-beta.6: warn when a custom config block shadows a builtin
/// preset/scene/color with the same name. Owner Option D mandate: custom
/// wins, but the user must be informed so silent shadowing never happens.
///
/// v80.0.0-beta.1 killer-features hardening: routed through `warn_runtime_or_now` —
/// the charset-custom collision site fires per scene change / live reload
/// (mid-rain), so the notice must buffer during the session instead of
/// eprintln-ing into the alt screen. Startup callers (config_apply, main)
/// print immediately exactly as before.
///
/// Emits a 3-line warning (increments the startup warning counter when
/// printed directly so the summary line fires at the end of config apply).
/// No return value — pure side-effect.
///
/// `category` is "charset" / "color" / "scene".
/// `name` is the colliding name (e.g. "zen").
/// `builtin_desc` is a short description of the builtin (e.g. "pipe char |").
/// `custom_desc` is a short description of the custom (e.g. "$ from config").
pub(crate) fn warn_name_collision(
    category: &str,
    name: &str,
    builtin_desc: &str,
    custom_desc: &str,
) {
    warn_runtime_or_now(&format!(
        "custom {category} '{name}' overrides builtin — custom wins (Option D policy)\n  builtin: {builtin_desc}\n  custom:  {custom_desc}\n  To use the builtin, rename the custom block in config.toml."
    ));
}

/// Phase 5 closure (P3-5): process-lifetime counter for warnings emitted via
/// `eprintln_warn_labeled`. Reset at the start of `apply_config_and_runtime_defaults`
/// and read at the end to emit a summary line.
pub(crate) static STARTUP_WARNING_COUNT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// Reset the startup warning counter. Call at the start of config apply.
pub(crate) fn reset_startup_warning_count() {
    STARTUP_WARNING_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Read the current startup warning count. Call at the end of config apply
/// to decide whether to emit a summary line.
#[must_use]
pub(crate) fn startup_warning_count() -> u64 {
    STARTUP_WARNING_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

// ── Verbose helpers ──────────────────────────────────────────────────────────

/// Get the current local time as `[HH:MM]` (24-hour, zero-padded).
///
/// Returns `[--:--]` if the system clock is unavailable (extremely rare —
/// only happens on platforms without a working localtime). This keeps
/// verbose output readable even in degraded environments.
///
/// v30 (Hinnant-style): delegates to `clock::now_hhmm()` which uses direct
/// `libc::localtime_r` on Unix (no chrono dependency, no allocation in the
/// chrono wrapper layer). Previously called `chrono::Local::now()` which
/// pulled in 8 transitive crates including `wasm-bindgen`, `js-sys`, and
/// `iana-time-zone-haiku` — all dead weight on a Linux-native CLI.
#[must_use]
pub(crate) fn now_hhmm() -> String {
    crate::clock::now_hhmm()
}

/// Format a verbose line: bold purple `[verbose] [HH:MM]` prefix + purple
/// label + default-color value.
///
/// The timestamp is captured once per call so all lines in a single verbose
/// dump show the same minute (unless the dump spans a minute boundary).
///
/// Example: `verbose_line("scene:", " monolith")`
/// → `[verbose] [12:01] scene:             monolith`
///
/// Label gutter: 18 columns (v100.0.0-nightly.1 verbose-format audit,
/// owner hunt 2026-09-04). The previous 14-column gutter silently
/// overflowed for every label wider than 13 chars — the live dump
/// showed three different value columns (16, 17, 18) — because Rust's
/// `{:<14}` pads to a MINIMUM, not a fixed width. 18 covers every
/// curated label in the startup dump and the final-runtime-state dump
/// (longest: `  chroma_features:` / `  ambient_entries:` /
/// `config candidates:` at exactly 18); labels longer than 18 are a
/// naming bug, not a rendering case.
#[must_use]
pub(crate) fn verbose_line(label: &str, value: &str) -> String {
    let ts = now_hhmm();
    match color_capability() {
        ColorCapability::Mono => format!("[verbose] {ts} {label:<18}{value}"),
        _ => format!(
            "{}[verbose]{} {ts} {}{label:<18}{}{value}",
            brand_bold_open(),
            reset(),
            brand_open(),
            reset()
        ),
    }
}

/// Print a verbose line directly to stderr. Convenience wrapper for
/// `eprintln!("{}", verbose_line(label, value))`.
pub(crate) fn eprintln_verbose(label: &str, value: &str) {
    eprintln_safe!("{}", verbose_line(label, value));
}

/// Print a raw verbose message (no label/value split) with the
/// `[verbose] [HH:MM]` prefix. Use this for one-off verbose lines that
/// don't fit the label:value pattern (e.g. multi-line dumps, free-form
/// diagnostics). The body remains default-colored.
pub(crate) fn eprintln_verbose_raw(msg: &str) {
    let ts = now_hhmm();
    match color_capability() {
        ColorCapability::Mono => eprintln_safe!("[verbose] {ts} {msg}"),
        _ => eprintln_safe!("{}[verbose]{} {ts} {msg}", brand_bold_open(), reset()),
    }
}

/// Print a verbose line where the body is also brand purple (matching the
/// visual weight of `verbose_line` labels). Use this for section headers
/// and one-shot diagnostics that should visually pop alongside the label
/// coloring of the surrounding verbose dump. Example outputs that should
/// use this: `ambient: startup phase ...`, `final runtime state`.
pub(crate) fn eprintln_verbose_purple(msg: &str) {
    let ts = now_hhmm();
    match color_capability() {
        ColorCapability::Mono => eprintln_safe!("[verbose] {ts} {msg}"),
        _ => eprintln_safe!(
            "{}[verbose]{} {ts} {}{}{}",
            brand_bold_open(),
            reset(),
            brand_open(),
            msg,
            reset()
        ),
    }
}

#[cfg(test)]
#[path = "../../test/output/output_tests.rs"]
mod tests;

// Submodules (moved from src/ root for clean src/ layout)
pub(crate) mod message;
pub(crate) mod post_exit;
pub(crate) mod report;
pub(crate) mod startup_verbose;
pub(crate) mod verbose;
