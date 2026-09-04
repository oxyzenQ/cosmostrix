// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use crate::termdetect::ancestor::{ancestor_matches_high_perf, ancestor_process_names};
use crate::termdetect::hosts::{
    CPU_RENDERER_TERMINALS, CPU_RENDERER_TERM_HINTS, HIGH_PERF_TERMINALS, HIGH_PERF_TERM_HINTS,
    KITTY_KEYBOARD_TERMINALS, KITTY_KEYBOARD_TERM_HINTS,
};

/// Returns the detection source if the terminal appears to be a
/// high-performance emulator. Checks (in order, 5 layers):
///   1. TERM_PROGRAM (case-insensitive exact match)
///   2. WT_SESSION env var (Windows Terminal)
///   3. TERM substring hints (e.g., `xterm-ghostty` contains `ghostty`)
///   4. Linux ancestor process name via /proc walk (catches Alacritty
///      launched with TERM=xterm-direct — no TERM_PROGRAM, no hint in TERM)
///
/// Returns Some(source_str) if matched, None if no layer matched.
/// The source string is shown in `-v` verbose output for transparency.
///
/// S-master-HUNT-24: the KONSOLE_VERSION layer was REMOVED. Konsole is
/// CPU-rendered (QPainter); its old high-perf classification gave the
/// 144 FPS dynamic default, which konsole's renderer cannot drain at
/// fullscreen cell counts. KONSOLE_VERSION now feeds
/// `cpu_renderer_detection_source` instead (effects auto-disable +
/// standard 60 FPS tier). The kitty-keyboard path still reads
/// KONSOLE_VERSION — protocol support is orthogonal to renderer class.
pub(super) fn high_perf_detection_source(term_program: &str, term: &str) -> Option<&'static str> {
    let tp_lower = term_program.to_ascii_lowercase();
    if !tp_lower.is_empty()
        && HIGH_PERF_TERMINALS
            .iter()
            .any(|&t| t.eq_ignore_ascii_case(&tp_lower))
    {
        return Some("TERM_PROGRAM");
    }
    // Windows Terminal: sets WT_SESSION (not TERM_PROGRAM).
    if std::env::var("WT_SESSION").is_ok() {
        return Some("WT_SESSION");
    }
    // Layer 3: TERM substring hints (case-insensitive).
    let term_lower = term.to_ascii_lowercase();
    if !term_lower.is_empty()
        && HIGH_PERF_TERM_HINTS
            .iter()
            .any(|&hint| term_lower.contains(hint))
    {
        return Some("TERM substring");
    }
    // Layer 4: Linux /proc ancestor process name.
    let ancestors = ancestor_process_names(10);
    if ancestor_matches_high_perf(&ancestors) {
        // Find the matching ancestor name for the source string.
        for name in &ancestors {
            let name_lower = name.to_ascii_lowercase();
            if HIGH_PERF_TERM_HINTS
                .iter()
                .any(|&hint| name_lower.contains(hint))
            {
                // Leak-alloc the name for a 'static lifetime. This is
                // called at most once per process (detect() is cached
                // via OnceLock in production), so the leak is bounded.
                // The string is ≤15 chars (kernel TASK_COMM_LEN limit).
                return Some("/proc ancestor");
            }
        }
    }
    None
}

/// Returns the detection source when the terminal is KNOWN to render its
/// cell grid on the CPU (S-master-HUNT-24). Checked layers:
///   1. VTE_VERSION env var — every libvte-based terminal (GNOME
///      Terminal, kgx, Xfce/Mate/Terminology…) exports it.
///   2. KONSOLE_VERSION env var — KDE Konsole (own renderer, CPU).
///   3. TERM_PROGRAM (case-insensitive exact match: foot, konsole).
///   4. TERM substring hints (foot, vte, gnome, kgx, konsole).
///
/// xterm.js hosts and the raw Linux console are handled by the caller
/// (`TerminalCaps::cpu_rendered` in mod.rs) — they already carry their
/// own detection booleans.
///
/// Terminals NOT matched here (Alacritty, kitty, ghostty, WezTerm —
/// GPU composited) keep cosmetic effects enabled. Unknown/unmatched
/// terminals also keep effects on; the event loop's dynamic congestion
/// gate (event_loop_post_draw.rs) is the safety net that catches an
/// undetected CPU renderer at runtime by watching the drain backoff.
pub(super) fn cpu_renderer_detection_source(
    term_program: &str,
    term: &str,
) -> Option<&'static str> {
    // Layer 1: VTE family. GNOME Terminal / kgx do not set TERM_PROGRAM
    // and use a generic TERM (xterm-256color) — VTE_VERSION is the only
    // reliable env marker.
    if std::env::var("VTE_VERSION").is_ok() {
        return Some("VTE_VERSION");
    }
    // Layer 2: KDE Konsole exports KONSOLE_VERSION.
    if std::env::var("KONSOLE_VERSION").is_ok() {
        return Some("KONSOLE_VERSION");
    }
    // Layer 3: TERM_PROGRAM exact match (foot sets TERM_PROGRAM=foot
    // in some launch modes).
    let tp_lower = term_program.to_ascii_lowercase();
    if !tp_lower.is_empty()
        && CPU_RENDERER_TERMINALS
            .iter()
            .any(|&t| t.eq_ignore_ascii_case(&tp_lower))
    {
        return Some("TERM_PROGRAM");
    }
    // Layer 4: TERM substring hints (foot's terminfo is `foot` /
    // `foot-extra`; some distros ship TERM=vte-256color / gnome / kgx).
    let term_lower = term.to_ascii_lowercase();
    if !term_lower.is_empty()
        && CPU_RENDERER_TERM_HINTS
            .iter()
            .any(|&hint| term_lower.contains(hint))
    {
        return Some("TERM substring");
    }
    None
}

/// Returns true if the host terminal is known to support the kitty
/// keyboard protocol (CSI-u progressive enhancement). Uses the SAME
/// 5-layer detection chain as `high_perf_detection_source()` but with
/// the stricter `KITTY_KEYBOARD_*` lists — a terminal may be high-perf
/// (renderer-wise) without supporting kitty keyboard protocol (e.g.
/// iTerm2, Apple Terminal).
///
/// When this returns true, `Terminal::init` pushes
/// `KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES` so the terminal
/// reports the full modifier bitfield on every keypress. Without this
/// push, terminals fall back to legacy escape sequences that ONLY encode
/// SHIFT/ALT/CONTROL — Super/Hyper/Meta are silently stripped, making
/// Super+C indistinguishable from bare 'c' and bypassing the modifier
/// allowlist in `input.rs::is_unmodified_or_shift()`.
///
/// Special cases:
///   - xterm.js hosts (VSCode/Hyper/Wave/Tabby/Warp): always false.
///     xterm.js doesn't implement kitty protocol; pushing the flag would
///     pollute the input stream with literal `CSI >1u` characters.
///   - Linux console (TERM=linux): always false. vt.c doesn't understand
///     kitty protocol sequences and would emit them as literal chars.
///   - Generic xterm (TERM=xterm or xterm-256color): false. Support is
///     version-dependent and xterm is the default TERM for many setups
///     that aren't actually xterm (e.g. SSH to a server). Conservative
///     skip to avoid pushing garbage to non-xterm terminals claiming
///     `xterm` TERM.
pub(super) fn kitty_keyboard_supported(term_program: &str, term: &str, xtermjs_host: bool) -> bool {
    // Never enable on terminals known to NOT support kitty protocol.
    if xtermjs_host {
        return false;
    }
    // Linux console: vt.c emits literal chars for CSI->1u, polluting
    // the input stream. Skip unconditionally — even if some future
    // kernel adds support, the user can override via TERM change.
    if term.eq_ignore_ascii_case("linux") {
        return false;
    }
    // Layer 1: TERM_PROGRAM exact match (case-insensitive).
    let tp_lower = term_program.to_ascii_lowercase();
    if !tp_lower.is_empty()
        && KITTY_KEYBOARD_TERMINALS
            .iter()
            .any(|&t| t.eq_ignore_ascii_case(&tp_lower))
    {
        return true;
    }
    // Layer 2: KONSOLE_VERSION env var (KDE Konsole sets this even when
    // TERM_PROGRAM is unset — same pattern as high_perf_detection_source).
    if std::env::var("KONSOLE_VERSION").is_ok() {
        return true;
    }
    // Layer 3: TERM substring hints (case-insensitive). Catches terminals
    // like `xterm-ghostty`, `xterm-kitty`, `alacritty`, `foot-extra`.
    let term_lower = term.to_ascii_lowercase();
    if !term_lower.is_empty()
        && KITTY_KEYBOARD_TERM_HINTS
            .iter()
            .any(|&hint| term_lower.contains(hint))
    {
        return true;
    }
    // Layer 4: Linux /proc ancestor process name. Catches Alacritty
    // launched with TERM=xterm-direct (no TERM_PROGRAM, no hint in TERM).
    // Reuses the same ancestor walk as high_perf detection — the ancestor
    // list is shared because every kitty-keyboard-supporting terminal is
    // also a high-perf terminal (the kitty list is a strict subset).
    let ancestors = ancestor_process_names(10);
    ancestors.iter().any(|name| {
        let name_lower = name.to_ascii_lowercase();
        KITTY_KEYBOARD_TERM_HINTS
            .iter()
            .any(|&hint| name_lower.contains(hint))
    })
}
