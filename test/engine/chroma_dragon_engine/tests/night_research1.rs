// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-1 (2026-09-05): Chroma Dragon pipeline coverage tests.
//!
//! Owner question: "is the chroma dragon included when the user runs
//! `cosmostrix --benchmark`, and is there an auto fallback to legacy
//! when the OS/terminal cannot use the chroma dragon (tty, non-terminal,
//! unsupported terminal, new unknown terminal)?"
//!
//! Answers locked here as executable contracts:
//!
//! 1. CHROMA IN BENCHMARK: yes — benchmark mode never disables the chroma
//!    engine (only `crystal_dragon` palette drift is forced off for p99
//!    determinism). Every terminal that resolves `ColorMode::TrueColor`
//!    renders every benchmark cell through the Chroma Dragon branches
//!    (`is_chroma()` call sites in droplet/draw.rs, rain_post.rs,
//!    phosphor.rs, ...), and the report discloses
//!    `color_pipeline: chroma_dragon` + `chroma_in_benchmark` in the
//!    CONFIG block (INV-19 locks the routing).
//!
//! 2. AUTO FALLBACK: yes — every mode that cannot represent truecolor
//!    (Color256, Color16, Mono — tty consoles, unknown terminals, unset
//!    TERM, non-terminal contexts) routes to `ColorPipeline::LegacyRgb`
//!    automatically, with a disclosed `disable_reason`.
//!
//! 3. THE GAP THIS HUNT FOUND AND FIXED: sessions where `COLORTERM` was
//!    stripped in transit (SSH without `SendEnv COLORTERM`, `sudo -s`,
//!    terminal versions that never set it) previously degraded
//!    truecolor-NATIVE terminals (alacritty, xterm-kitty, xterm-ghostty,
//!    wezterm, foot, contour) to Color16 -> legacy_rgb — silently losing
//!    the OKLab gradient, climate post-FX, and halos. The owner
//!    directive is "chroma dragon first -> fallback legacy rgb/srgb":
//!    a terminal that is truecolor by construction must keep the chroma
//!    engine active. `termdetect::hosts::TRUECOLOR_TERM_HINTS` (matched
//!    case-insensitively as a TERM substring) now upgrades exactly those
//!    terminals to TrueColor before the conservative Color16 default.
//!
//!    The conservative default is untouched: plain `xterm`, `screen`,
//!    `st`, unknown names, unset TERM, and the raw Linux console still
//!    fall back to legacy — the fallback direction the owner asked about
//!    stays exactly as it was.

use crate::cli::detect_color_mode_from_terms;
use crate::runtime::{ColorMode, ColorPipeline};
use crate::termdetect::{term_hints_truecolor, TRUECOLOR_TERM_HINTS};

// ── 1. Truecolor-native TERM hints upgrade to TrueColor ─────────────────────

#[test]
fn truecolor_native_term_names_resolve_truecolor_without_colorterm() {
    // The SSH/sudo case: COLORTERM stripped, TERM carries the terminal
    // identity. Each of these terminals is truecolor by construction.
    for term in [
        "alacritty",
        "xterm-kitty",
        "kitty",
        "xterm-ghostty",
        "ghostty",
        "wezterm",
        "foot",
        "foot-extra",
        "contour",
    ] {
        assert_eq!(
            detect_color_mode_from_terms("", term),
            ColorMode::TrueColor,
            "TERM={term} with no COLORTERM must resolve TrueColor (chroma dragon first)"
        );
    }
}

#[test]
fn truecolor_native_hint_matching_is_case_insensitive() {
    // Terminals are inconsistent about casing (Alacritty vs alacritty);
    // matching mirrors HIGH_PERF_TERM_HINTS and must not silently miss
    // an upper-case emission.
    for term in ["ALACRITTY", "Xterm-Kitty", "WezTerm", "FOOT"] {
        assert_eq!(
            detect_color_mode_from_terms("", term),
            ColorMode::TrueColor,
            "TERM={term} must resolve TrueColor regardless of casing"
        );
    }
}

#[test]
fn every_hint_entry_resolves_truecolor_through_the_full_chain() {
    // Lock the TABLE itself: every entry in TRUECOLOR_TERM_HINTS must
    // actually upgrade the resolution. If someone adds an entry without
    // wiring it through detect_color_mode_from_terms, this fails.
    for hint in TRUECOLOR_TERM_HINTS {
        assert_eq!(
            detect_color_mode_from_terms("", hint),
            ColorMode::TrueColor,
            "TRUECOLOR_TERM_HINTS entry '{hint}' must resolve TrueColor"
        );
    }
}

#[test]
fn colorterm_advertisement_still_wins_over_term_hints() {
    // Ordering contract: COLORTERM is the primary advertisement. A hint
    // match must never DOWNGRADE a COLORTERM-truecolor session, and
    // COLORTERM=truecolor keeps winning even over a non-hint TERM.
    assert_eq!(
        detect_color_mode_from_terms("truecolor", "xterm"),
        ColorMode::TrueColor
    );
    assert_eq!(
        detect_color_mode_from_terms("24bit", "screen"),
        ColorMode::TrueColor
    );
    // Mono/dumb must NOT be overridden by a hint-style TERM (dumb is
    // checked before the hint layer — an explicit dumb beats a lying
    // TERM string).
    assert_eq!(detect_color_mode_from_terms("", "dumb"), ColorMode::Mono);
}

// ── 2. The auto-fallback matrix the owner asked about stays conservative ────

#[test]
fn raw_linux_console_tty_falls_back_to_legacy_color16() {
    // Owner scenario "tty": the raw Linux virtual console (TERM=linux)
    // cannot represent truecolor — Color16 -> LegacyRgb.
    let mode = detect_color_mode_from_terms("", "linux");
    assert_eq!(mode, ColorMode::Color16);
    assert_eq!(ColorPipeline::detect(mode), ColorPipeline::LegacyRgb);
}

#[test]
fn unset_term_non_terminal_falls_back_to_legacy_color16() {
    // Owner scenario "non terminal": no TERM at all (cron, CI, a service
    // context, or a stripped environment) — conservative Color16.
    let mode = detect_color_mode_from_terms("", "");
    assert_eq!(mode, ColorMode::Color16);
    assert_eq!(ColorPipeline::detect(mode), ColorPipeline::LegacyRgb);
}

#[test]
fn unknown_new_terminal_falls_back_to_legacy_color16() {
    // Owner scenario "new unknown terminal": a name cosmostrix has never
    // seen (e.g. a future terminal with no COLORTERM and no hint match)
    // gets the safe Color16 default, never a truecolor guess. Note: a
    // name CONTAINING a known hint substring (e.g. `wezterm-xyz`, a
    // WezTerm fork) correctly upgrades — substring semantics mirror
    // HIGH_PERF_TERM_HINTS; these probes contain no hint substring.
    for term in ["some-brand-new-term", "nextgen-2027", "vertex", "hyperion"] {
        let mode = detect_color_mode_from_terms("", term);
        assert_eq!(
            mode,
            ColorMode::Color16,
            "unknown TERM={term} must stay conservative (Color16)"
        );
        assert_eq!(ColorPipeline::detect(mode), ColorPipeline::LegacyRgb);
    }
}

#[test]
fn unsupported_256color_terminals_fall_back_to_legacy() {
    // Owner scenario "unsupported terminal": 256-color-only TERM values
    // resolve Color256 -> LegacyRgb (the OKLab palette would be
    // quantized away, so legacy sRGB-linear math is used directly).
    for term in [
        "xterm-256color",
        "screen-256color",
        "tmux-256color",
        "st-256color",
    ] {
        let mode = detect_color_mode_from_terms("", term);
        assert_eq!(mode, ColorMode::Color256);
        assert_eq!(ColorPipeline::detect(mode), ColorPipeline::LegacyRgb);
    }
    // Plain xterm/screen/st without the 256 suffix: Color16.
    for term in ["xterm", "screen", "st"] {
        let mode = detect_color_mode_from_terms("", term);
        assert_eq!(mode, ColorMode::Color16);
        assert_eq!(ColorPipeline::detect(mode), ColorPipeline::LegacyRgb);
    }
}

#[test]
fn every_fallback_state_discloses_its_disable_reason() {
    // Honesty contract: whenever the pipeline falls back to legacy, the
    // user must be told why (-v / --doctor disclose disable_reason).
    for mode in [ColorMode::Color256, ColorMode::Color16, ColorMode::Mono] {
        let pipeline = ColorPipeline::detect(mode);
        assert!(pipeline.disable_reason(mode).is_some());
    }
}

// ── 3. Hint helper unit contracts ───────────────────────────────────────────

#[test]
fn term_hints_truecolor_matches_known_names_and_substrings() {
    // Substring semantics mirror HIGH_PERF_TERM_HINTS: `xterm-kitty`
    // contains `kitty`, `foot-extra` contains `foot`.
    assert!(term_hints_truecolor("xterm-kitty"));
    assert!(term_hints_truecolor("foot-extra"));
    assert!(term_hints_truecolor("Alacritty"));
    assert!(term_hints_truecolor("wezterm"));
}

#[test]
fn term_hints_truecolor_rejects_non_hint_terms() {
    // The dangerous substrings must stay rejected: `rio` (3 letters,
    // false-positive prone — deliberately not in the table) and the
    // 256-color family that must resolve Color256, not TrueColor.
    assert!(!term_hints_truecolor("rio"));
    assert!(!term_hints_truecolor("xterm-256color"));
    assert!(!term_hints_truecolor("screen"));
    assert!(!term_hints_truecolor(""));
    assert!(!term_hints_truecolor("linux"));
    assert!(!term_hints_truecolor("some-curiosity-term"));
}

#[test]
fn chroma_first_end_to_end_pipeline_resolution() {
    // End-to-end: a truecolor-native TERM keeps the Chroma Dragon
    // engine; everything the owner listed as "cannot use chroma" falls
    // back to legacy. This is the one-line summary of the whole hunt.
    assert_eq!(
        ColorPipeline::detect(detect_color_mode_from_terms("", "alacritty")),
        ColorPipeline::ChromaDragon
    );
    for term in ["linux", "", "xterm", "unknown-term-2099", "xterm-256color"] {
        assert_eq!(
            ColorPipeline::detect(detect_color_mode_from_terms("", term)),
            ColorPipeline::LegacyRgb,
            "TERM={term} must fall back to the legacy pipeline"
        );
    }
}
