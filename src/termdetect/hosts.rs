// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

/// `TERM_PROGRAM` values that identify an xterm.js-based Electron host.
/// All of these ship xterm.js as their terminal renderer and inherit the
/// same unbounded-buffer-growth failure mode at high ANSI byte rates.
///
/// Kept as a `const` slice (not a `match`) so the test suite can iterate
/// over the list and verify each host triggers detection.
///
/// **Adding a host**: append the exact `TERM_PROGRAM` string here. The
/// rest of the detection / capping / RIS-reset machinery keys off the
/// `xtermjs_host` boolean and is host-agnostic.
pub(super) const XTERMJS_HOSTS: &[&str] = &[
    // VSCode (and forks like VSCodium, code-oss, code-insiders). The
    // original crash report was inside VSCode.
    "vscode",
    // Hyper — Electron-based terminal, uses xterm.js as the renderer.
    // Sets TERM_PROGRAM=Hyper (capital H).
    "Hyper",
    // WaveTerminal — Electron-based, embeds xterm.js in a TilingWave pane.
    // Sets TERM_PROGRAM=WaveTerminal.
    "WaveTerminal",
    // Tabby — Electron-based terminal manager, embeds xterm.js for the
    // terminal pane. Sets TERM_PROGRAM=Tabby.
    "Tabby",
    // WarpTerminal — Rust+Electron hybrid; the renderer pane is xterm.js.
    // Sets TERM_PROGRAM=WarpTerminal.
    "WarpTerminal",
];

/// `TERM_PROGRAM` values that identify high-performance terminal
/// emulators capable of sustaining 144+ FPS without visual artifacts.
/// These terminals have GPU-accelerated or highly optimized renderers
/// that can keep up with cosmostrix's ANSI byte rate at high frame rates.
/// Used by the dynamic default FPS logic: if the user doesn't specify
/// `--fps` or `fps =`, these terminals default to 144 FPS instead of 60.
/// The user's explicit value ALWAYS wins over this default.
///
/// hotfix: matching is now CASE-INSENSITIVE (some terminals emit
/// `alacritty` vs `Alacritty`, `apple_Terminal` vs `Apple_Terminal`).
/// Also expanded the list with Konsole, Windows Terminal, and corrected
/// `Apple_Terminal` (the previous `apple_Terminal` lowercase form never
/// matched real Terminal.app emissions).
pub(super) const HIGH_PERF_TERMINALS: &[&str] = &[
    "Alacritty",
    "kitty",
    "WezTerm",
    "ghostty",
    "foot",
    "iTerm.app",
    "Apple_Terminal",
    "konsole",
    "WindowsTerminal",
];

/// `TERM` values that identify high-performance terminals when
/// `TERM_PROGRAM` is unset (some terminals don't set TERM_PROGRAM).
/// Matched case-insensitively as a SUBSTRING of TERM (e.g., `xterm-ghostty`
/// contains `ghostty`). Conservative list — false positives here would
/// push a slow terminal to 144 FPS, which it can't sustain.
pub(super) const HIGH_PERF_TERM_HINTS: &[&str] = &[
    "alacritty",
    "kitty",
    "ghostty",
    "foot",
    "wezterm",
    "konsole",
];

/// `TERM_PROGRAM` values that identify terminals known to support the
/// kitty keyboard protocol (CSI-u progressive enhancement). When matched,
/// cosmostrix pushes `DISAMBIGUATE_ESCAPE_CODES` so the terminal reports
/// the FULL modifier bitfield (incl. Super/Hyper/Meta) on every keypress.
///
/// This list is a DELIBERATE SUBSET of `HIGH_PERF_TERMINALS` — kitty
/// keyboard protocol is newer than "high-perf renderer" status, so some
/// high-perf terminals are excluded here:
///   - `iTerm.app` / `Apple_Terminal`: support is version-dependent
///     (macOS 12+ for Terminal.app, iTerm2 needs opt-in). Conservative
///     skip — false positive would push garbage `CSI >1u` to a terminal
///     that doesn't understand it, polluting the input stream.
///   - `WindowsTerminal`: only relevant on Windows, where crossterm's
///     PushKeyboardEnhancementFlags returns Err(Unsupported). Even if
///     we set this flag, the execute() call would silently fail.
///
/// Verified support (crossterm 0.29 docs + terminal docs):
///   - kitty: original implementer, always supported
///   - foot: supported since v1.4 (2020)
///   - WezTerm: supported, gated by `enable_kitty_keyboard` config
///     (defaults to "true" in current versions)
///   - alacritty: supported since v0.13.0 (2022)
///   - ghostty: supported, always on
///   - konsole: supported since 22.04 (2022)
pub(super) const KITTY_KEYBOARD_TERMINALS: &[&str] = &[
    "Alacritty",
    "kitty",
    "WezTerm",
    "ghostty",
    "foot",
    "konsole",
];

/// `TERM` substring hints for kitty keyboard protocol support. Mirrors
/// `KITTY_KEYBOARD_TERMINALS` for the case where `TERM_PROGRAM` is unset
/// but `TERM` contains the terminal name (e.g. `xterm-ghostty`,
/// `xterm-kitty`, `alacritty`, `foot-extra`). Matched case-insensitively
/// as a substring.
pub(super) const KITTY_KEYBOARD_TERM_HINTS: &[&str] = &[
    "alacritty",
    "kitty",
    "ghostty",
    "foot",
    "wezterm",
    "konsole",
];
