// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal protocol detection at startup.
//!
//! Organized into submodules:
//! - [`hosts`] — terminal name constant tables
//! - [`ancestor`] — Linux `/proc` ancestor process walk
//! - [`detect`] — high-perf and kitty-keyboard detection logic
//! - [`protocol`] — protocol constants (sync markers, FPS caps)
//! - [`tests`] — full test suite

mod ancestor;
mod detect;
mod hosts;
mod protocol;

#[cfg(test)]
mod tests;

use std::env;
// Re-export test-only items so tests.rs can find them via super::*.
#[cfg(test)]
#[cfg(not(target_os = "linux"))]
pub(crate) use ancestor::ancestor_matches_high_perf;
#[cfg(test)]
#[cfg(target_os = "linux")]
pub(crate) use ancestor::parse_proc_ppid;
#[cfg(test)]
#[cfg(target_os = "linux")]
pub(crate) use ancestor::{ancestor_matches_high_perf, ancestor_process_names};
#[cfg(test)]
pub(crate) use protocol::{known_xtermjs_hosts, RIS_RESET, VSCODE_FPS_CAP};

// Re-export protocol constants at crate level.
pub(crate) use protocol::{SYNC_END, SYNC_START};

// Items used by detect() below.
use detect::{high_perf_detection_source, kitty_keyboard_supported};
use hosts::XTERMJS_HOSTS;
use protocol::{HIGH_PERF_DEFAULT_FPS, STANDARD_DEFAULT_FPS, XTERMJS_FPS_CAP};

// Items used only by the test suite (tests.rs uses `use super::*;`).

/// # FPS Precedence Chain ( documentation)
///
/// The effective frame rate at any moment is the result of a 7-layer
/// precedence chain. Layers are listed highest-priority first; the
/// first layer that produces a value wins, and lower layers are not
/// consulted.
///
/// ## Resolution-time layers (run once at startup, in main.rs)
///
/// 1. **CLI `--fps`** — explicit user override. Detected via
///    `matches.value_source("fps") == CommandLine`. Always wins.
/// 2. **Scene `fps=`** — built-in scenes (e.g., `low-power` sets
///    fps=30, `cosmic-dragon` sets fps=60). Applied in
///    `config_apply::apply_scene_values` ONLY when the user did NOT
///    set `--fps` AND config.toml did NOT set `fps =`.
///    **audit note (FPS-F2/F3)**: scene-level `fps =` is
///    **startup-only by design**. `Cloud::apply_scene_runtime` does
///    NOT apply `fps` at runtime — only `rain_style`/`color`/`charset`/
///    `speed`/`density`/`glitch_level`. So when the self-healer
///    downgrades to "low-power" at runtime, or the ambient scheduler
///    fires a scene at runtime, the user's startup `--fps`/`fps =`
///    value stays in effect. The CPU shed from a self-healer
///    downgrade comes from `speed=5`+`density=0.45`+`glitch=None`,
///    not from `fps=30`. This is intentional — letting runtime
///    scene writers override `target_fps` would create a precedence
///    ambiguity (which user intent wins?).
/// 3. **Config.toml `fps =`** — user's persistent default. Applied
///    ONLY when the user did NOT set `--fps`.
/// 4. **Dynamic default fps** — terminal-aware default from this
///    module's `dynamic_default_fps` field (144 for high-perf
///    terminals, 60 for standard/unknown, 30 for xterm.js hosts).
///    Applied in `main.rs:669-674` ONLY when none of layers 1-3
///    produced a value.
/// 5. **xterm.js cap** — `default_fps_cap` (30 FPS) applied AFTER
///    layers 1-4 on xterm.js hosts. Even an explicit `--fps 120`
///    gets capped to 30 on VSCode's xterm.js to prevent OOM.
///    Bypassed in `--benchmark` mode.
///
/// ## Runtime layers (run every frame, in event_loop.rs:966-972)
///
/// 6. **Idle factor** — when `is_idle` is true (no user input for
///    `IDLE_THRESHOLD`), `frame_period` is multiplied by
///    `1.0 / IDLE_FPS_FACTOR` (i.e., 0.5× → half the FPS). This is
///    a frame_period adjustment, NOT a target_fps change.
/// 7. **Pause period** — when `cloud.pause` is true (user pressed
///    space), `frame_period` is replaced with `PAUSE_PERIOD_MS`
///    (250ms = 4 FPS). Highest runtime priority.
///
/// ## How to read this in verbose output
///
/// `cosmostrix -v` shows:
///   fps:           144.0
///   fps_source:    /proc ancestor (dynamic default)
///   fps_precedence: dynamic_default  <- which resolution layer won
///
/// The `fps_precedence` field is the visibility signal:
/// one of `cli`, `scene`, `config`, `dynamic_default`, or
/// `xtermjs_cap`. Runtime layers (idle, pause) are NOT shown here
/// because they change every frame — see the HUD's `tgt:` line for
/// the live frame_mode suffix (`idle` / `paused`).
///
/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalCaps {
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
    /// Kitty keyboard protocol (`ESC[>1u` push / `ESC[<1u` pop).
    ///
    /// True when the host terminal is known to support the kitty keyboard
    /// protocol's progressive enhancement flags (kitty, foot, WezTerm,
    /// alacritty, ghostty, konsole 22.04+). When true, `Terminal::init`
    /// pushes `DISAMBIGUATE_ESCAPE_CODES` so the terminal sends all key
    /// events as CSI-u sequences with the FULL modifier bitfield
    /// (1=SHIFT, 2=ALT, 4=CONTROL, 8=SUPER, 16=HYPER, 32=META).
    ///
    /// Without this flag, legacy escape sequences only encode
    /// SHIFT/ALT/CONTROL — Super/Hyper/Meta are silently stripped,
    /// reaching cosmostrix as `Char('c')` with `KeyModifiers::NONE`.
    /// That made Super+C indistinguishable from bare 'c', bypassing
    /// the modifier allowlist in `input.rs::is_unmodified_or_shift()`.
    ///
    /// False on: xterm.js hosts (VSCode/Hyper/Wave/Tabby/Warp — xterm.js
    /// doesn't implement kitty protocol), generic xterm (uncertain,
    /// conservative skip), Linux console (vt.c doesn't understand
    /// CSI->1u and emits literal characters), and any terminal not in
    /// the known-support list.
    pub kitty_keyboard: bool,
    /// True when the terminal supports the alternate screen buffer
    /// (`ESC[?1049h` / `ESC[?1049l`). Most terminals support it,
    /// INCLUDING the Linux virtual console (TERM=linux) via vt.c mode
    /// 1049 (kernel 2.6.x+) — entering the alt buffer saves the main
    /// screen state (incl. scrollback), leaving it restores the main
    /// screen intact. Only `dumb` terminals and an unset TERM lack alt
    /// screen support. When false, cosmostrix runs on the main screen
    /// directly (scrollback is preserved by not clearing it).
    pub has_alternate_screen: bool,
    /// True when running inside ANY xterm.js-based Electron host
    /// (`TERM_PROGRAM` matches an entry in `XTERMJS_HOSTS`). This is the
    /// primary Tier 2 signal — gating FPS cap, byte-budget backpressure,
    /// and periodic RIS reset.
    pub xtermjs_host: bool,
    /// Back-compat alias: true when `TERM_PROGRAM=vscode` specifically.
    /// Equivalent to `xtermjs_host && term_program == "vscode"`. Kept so
    /// existing call sites (verbose output, doc cross-references) can
    /// single out VSCode without re-reading the env var. New code should
    /// key off `xtermjs_host` instead.
    pub vscode_integrated: bool,
    /// Maximum recommended FPS for this terminal. Native terminals
    /// (Alacritty, Kitty, etc.) get 240 (effectively uncapped — the
    /// user's --fps value wins). xterm.js hosts get 30 to keep the
    /// worst-case byte rate under ~7 MB/sec.
    pub default_fps_cap: f64,
    /// masterclass: dynamic default FPS when the user does NOT
    /// specify `--fps` or `fps =` in config. Based on terminal tier:
    /// high-performance terminals (Alacritty, kitty, wezterm, ghostty,
    /// foot, iTerm2) get 144; standard/unknown terminals get 60; xterm.js
    /// hosts get 30. The user's explicit `--fps` / `fps =` ALWAYS wins
    /// over this default — it only applies when no FPS is specified.
    pub dynamic_default_fps: f64,
    /// hardening: human-readable source string identifying WHICH
    /// detection layer matched (e.g. "TERM_PROGRAM=Alacritty",
    /// "/proc ancestor 'alacritty'", "standard/unknown fallback").
    /// Shown in `-v` verbose output so the user can verify the detection
    /// chain. Essential for debugging "why is my fps 60 not 144?".
    pub dynamic_fps_source: &'static str,
}

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub(crate) fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    // Tier 2: match against the full list of known xterm.js hosts.
    // (FPS-F6): case-insensitive matching (mirrors HIGH_PERF_TERMINALS).
    // Casing is fragile across versions/forks — a future VSCode
    // fork emitting "VSCode" instead of "vscode" would silently bypass the
    // 30 FPS cap and resurrect the multi-hour OOM crash Tier 2 prevents.
    let tp_lower = term_program.to_ascii_lowercase();
    let xtermjs_host = XTERMJS_HOSTS
        .iter()
        .any(|&h| tp_lower == h.to_ascii_lowercase());

    // VSCode-specific alias for back-compat with Tier 1 code paths that
    // single out VSCode in user-facing strings (warnings, verbose output).
    let vscode_integrated = xtermjs_host && tp_lower == "vscode";

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe. Three exceptions:
    //   1. Linux console (TERM=linux) — does not understand the sequence.
    //   2. xterm.js hosts — xterm.js's mode 2026 buffer implementation
    //      amplifies memory pressure under high frame rates, contributing
    //      to the multi-hour SIGTRAP crash.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux") && !xtermjs_host;

    // xterm.js hosts get a 30 FPS cap; everything else is effectively
    // uncapped (the user's --fps value, validated to 1.0..=240.0, wins).
    let default_fps_cap = if xtermjs_host { XTERMJS_FPS_CAP } else { 240.0 };

    // masterclass: dynamic default FPS based on terminal tier.
    // hotfix: case-insensitive matching + env-var fallbacks.
    // hardening: Layer 5 (/proc ancestor walk) + source tracking.
    // The source string records WHICH layer matched — shown in -v output
    // so the user can verify the detection chain.
    let (dynamic_default_fps, dynamic_fps_source) = if xtermjs_host {
        (XTERMJS_FPS_CAP, "xtermjs_host (capped)")
    } else if let Some(source) = high_perf_detection_source(&term_program, &term) {
        (HIGH_PERF_DEFAULT_FPS, source)
    } else {
        (STANDARD_DEFAULT_FPS, "standard/unknown fallback")
    };

    // Alternate screen detection. Most terminal emulators support the
    // alternate screen buffer (\x1b[?1049h / \x1b[?1049l), INCLUDING the
    // Linux virtual console (TERM=linux) since kernel 2.6.x — the vt.c
    // driver implements modes 47/1047/1049. Entering the alt buffer saves
    // the main screen state (including scrollback); leaving it restores
    // the main screen intact. This is what preserves the user's terminal
    // history (e.g. `echo hello` output) after cosmostrix quits on a TTY
    // (ctrl+alt+fN).
    //
    // Only `dumb` terminals and an unset TERM truly lack alt screen
    // support — on those, \x1b[?1049h is silently ignored, so cosmostrix
    // falls back to rendering on the main screen directly.
    //
    // History: commit 6d0574b previously disabled alt screen for
    // TERM=linux based on the belief that Linux console didn't support
    // \x1b[?1049h. That belief was incorrect for modern kernels. The real
    // cause of the original scrollback loss was Clear(All) emitted inside
    // the alt screen (removed by commits 8b2a19b, 42c76a8, 246e9b9e,
    // 6ed244b). With Clear(All) gone and sync mode correctly placed
    // (commit 01ffda8), enabling alt screen for Linux console now
    // correctly preserves scrollback on TTY quit.
    let has_alternate_screen = !term.eq_ignore_ascii_case("dumb") && !term.is_empty();

    // Kitty keyboard protocol: enable ONLY on terminals known to support
    // it (kitty, foot, WezTerm, alacritty, ghostty, konsole). See the
    // KITTY_KEYBOARD_TERMINALS list above for the full rationale.
    //
    // Owner-reported bug: Super+C (Windows-key + c) cycled colors despite
    // the modifier allowlist in input.rs::is_unmodified_or_shift(). Root
    // cause: without kitty protocol enabled, the terminal sends Super+C
    // as legacy escape sequence that strips the SUPER bit, so cosmostrix
    // sees Char('c') with KeyModifiers::NONE — indistinguishable from a
    // bare 'c' press. Enabling the protocol makes the terminal report
    // the full modifier bitfield via CSI-u, allowing the allowlist to
    // correctly reject Super+C.
    let kitty_keyboard = kitty_keyboard_supported(&term_program, &term, xtermjs_host);

    TerminalCaps {
        sync_output: sync_ok,
        kitty_keyboard,
        has_alternate_screen,
        xtermjs_host,
        vscode_integrated,
        default_fps_cap,
        dynamic_default_fps,
        dynamic_fps_source,
    }
}
