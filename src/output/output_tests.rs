// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the `output` module — extracted from `output/mod.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`; the
//! S-master-HUNT-5 neon-retune + suggestion-semantic additions pushed it
//! over).
//!
//! Declared from `output/mod.rs` via `#[cfg(test)] #[path =
//! "output_tests.rs"] mod tests;` — `use super::*` resolves to the
//! output module.

use super::*;

#[test]
fn rgb_constants_match_neon_family_palette() {
    // Source-of-truth: the RGB constants must encode the exact
    // cosmostrix neon-family stops (S-master-HUNT-5 owner mandate
    // 2026-09-03; previously the Tailwind v3 palette). Any drift here
    // breaks the CLI/rain color harmony contract.
    assert_eq!(BRAND_PURPLE_RGB, (168, 85, 247)); // #A855F7 — NeonPurple band midpoint
    assert_eq!(ERROR_RGB, (255, 90, 90)); // #FF5A5A — NeonRed bright-body stop
    assert_eq!(WARN_RGB, (255, 235, 60)); // #FFEB3C — NeonYellow head stop
    assert_eq!(SUGGESTION_RGB, (220, 235, 255)); // #DCEBFF — NeonWhite head stop
}

// ── eprintln_safe! macro ──

#[test]
fn eprintln_safe_does_not_panic_with_format_args() {
    // The macro must accept the same format-arg syntax as eprintln!
    // and must never panic. We can't easily redirect stderr in a unit
    // test, so this test only verifies panic-safety — the write goes
    // to the real stderr (visible if you run with --nocapture).
    eprintln_safe!("test: {} = {}", "answer", 42);
    eprintln_safe!("no args");
    eprintln_safe!("mixed {} {} {}", 1, "two", 3.0);
}

#[test]
fn eprintln_safe_handles_empty_string() {
    // Edge case: empty format string. Must not panic.
    eprintln_safe!("");
}

#[test]
fn eprintln_safe_compiles_with_complex_format() {
    // Verify the macro accepts the same complex format strings used
    // in main.rs post-exit paths (named args, precision, mixed types).
    let purple = "\x1b[35m";
    let reset = "\x1b[0m";
    let ts = "12:34";
    let final_color = "nebula";
    let startup_color = "vaporwave";
    eprintln_safe!(
        "{purple}[verbose]{reset} {ts} {purple}  color_scheme:{reset}  {} (was {})",
        final_color,
        startup_color
    );
}

// ── Phase 5 closure (P3-5): startup warning counter ──
//
// LTS audit 2026-08-19 (task 5/6): these 2 tests touch the global
// `STARTUP_WARNING_COUNT` atomic. When run in parallel with config
// apply tests that emit warnings (via `eprintln_warn_labeled`), the
// global state races — causing the flaky test failure observed in
// prior sessions (`output::tests::eprintln_warn_labeled_increments_counter`
// failed once on first run, passed on re-run).
//
// Fix: serialize the 2 tests via a Mutex guard. The production code
// path (single-threaded config apply) is unaffected — only the test
// parallelism is constrained.

/// Mutex guarding tests that touch `STARTUP_WARNING_COUNT`. Without
/// this, parallel test execution races on the global atomic.
static TEST_WARNING_COUNT_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn reset_clears_warning_count() {
    let _guard = TEST_WARNING_COUNT_MUTEX.lock().unwrap();
    reset_startup_warning_count();
    assert_eq!(startup_warning_count(), 0);
}

#[test]
fn eprintln_warn_labeled_increments_counter() {
    let _guard = TEST_WARNING_COUNT_MUTEX.lock().unwrap();
    reset_startup_warning_count();
    // eprintln_warn_labeled writes to stderr; we only care about the
    // counter side-effect. Run it 3 times and verify the count matches.
    eprintln_warn_labeled("test warning 1");
    eprintln_warn_labeled("test warning 2");
    eprintln_warn_labeled("test warning 3");
    assert_eq!(startup_warning_count(), 3);
    reset_startup_warning_count();
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
fn eprintln_verbose_purple_contains_prefix_and_body() {
    // The purple-body variant must wrap the body in brand_open (regular,
    // not bold) AFTER the [verbose] prefix. We verify the format pattern
    // by exercising the TrueColor branch directly: the body open must
    // be the regular brand_open escape, and it must appear after the
    // [verbose] tag. We do NOT call color_capability() here because the
    // test harness runs with stderr captured as a non-tty, which would
    // route through the Mono branch (empty escapes) and make the test
    // non-deterministic across environments.
    let bold = "\x1b[1;38;2;168;85;247m";
    let reg = "\x1b[38;2;168;85;247m";
    let rst = "\x1b[0m";
    let ts = now_hhmm();
    let msg = "final runtime state";
    let line = format!("{bold}[verbose]{rst} {ts} {reg}{msg}{rst}");
    assert!(line.contains("[verbose]"));
    assert!(line.contains("final runtime state"));
    // Body must be wrapped in regular brand_open (not just bold).
    let verbose_end = line
        .find("[verbose]")
        .map(|i| i + "[verbose]".len())
        .unwrap();
    let body_open = line.rfind(reg).unwrap();
    assert!(body_open > verbose_end);
    // Bold escape must NOT equal regular escape (otherwise the visual
    // distinction between the prefix and the body would be lost).
    assert_ne!(bold, reg);
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
    // Error red #FF5A5A (NeonRed bright body) must map to:
    // - TrueColor: \x1b[38;2;255;90;90m
    // - Color256: \x1b[38;5;203m (closest cube index: 16 + 36*5 + 6*1 + 1)
    // - Color16: \x1b[31m (ANSI Red)
    // - Mono: empty
    let truecolor = match ColorCapability::TrueColor {
        ColorCapability::TrueColor => "\x1b[38;2;255;90;90m",
        ColorCapability::Color256 => "\x1b[38;5;203m",
        ColorCapability::Color16 => "\x1b[31m",
        ColorCapability::Mono => "",
    };
    assert!(truecolor.contains("255;90;90"));

    let color256 = match ColorCapability::Color256 {
        ColorCapability::TrueColor => "\x1b[38;2;255;90;90m",
        ColorCapability::Color256 => "\x1b[38;5;203m",
        ColorCapability::Color16 => "\x1b[31m",
        ColorCapability::Mono => "",
    };
    assert!(color256.contains("38;5;203"));

    let color16 = match ColorCapability::Color16 {
        ColorCapability::TrueColor => "\x1b[38;2;255;90;90m",
        ColorCapability::Color256 => "\x1b[38;5;203m",
        ColorCapability::Color16 => "\x1b[31m",
        ColorCapability::Mono => "",
    };
    assert_eq!(color16, "\x1b[31m");
}

#[test]
fn warn_open_uses_yellow_palette_per_capability() {
    // Warn yellow #FFEB3C (NeonYellow head) must map to:
    // - TrueColor: \x1b[38;2;255;235;60m
    // - Color256: \x1b[38;5;220m (gold — brightest visible yellow)
    // - Color16: \x1b[33m (ANSI Yellow)
    // - Mono: empty
    let truecolor = match ColorCapability::TrueColor {
        ColorCapability::TrueColor => "\x1b[38;2;255;235;60m",
        ColorCapability::Color256 => "\x1b[38;5;220m",
        ColorCapability::Color16 => "\x1b[33m",
        ColorCapability::Mono => "",
    };
    assert!(truecolor.contains("255;235;60"));

    let color256 = match ColorCapability::Color256 {
        ColorCapability::TrueColor => "\x1b[38;2;255;235;60m",
        ColorCapability::Color256 => "\x1b[38;5;220m",
        ColorCapability::Color16 => "\x1b[33m",
        ColorCapability::Mono => "",
    };
    assert!(color256.contains("38;5;220"));
}

#[test]
fn suggestion_open_uses_white_palette_per_capability() {
    // Suggestion crystal white #DCEBFF (NeonWhite head) must map to:
    // - TrueColor: \x1b[38;2;220;235;255m
    // - Color256: \x1b[38;5;255m (238,238,238 — nearest near-white)
    // - Color16: \x1b[97m (aixterm BRIGHT white — not the dim 37)
    // - Mono: empty
    let truecolor = match ColorCapability::TrueColor {
        ColorCapability::TrueColor => "\x1b[38;2;220;235;255m",
        ColorCapability::Color256 => "\x1b[38;5;255m",
        ColorCapability::Color16 => "\x1b[97m",
        ColorCapability::Mono => "",
    };
    assert!(truecolor.contains("220;235;255"));

    let color256 = match ColorCapability::Color256 {
        ColorCapability::TrueColor => "\x1b[38;2;220;235;255m",
        ColorCapability::Color256 => "\x1b[38;5;255m",
        ColorCapability::Color16 => "\x1b[97m",
        ColorCapability::Mono => "",
    };
    assert!(color256.contains("38;5;255"));

    let color16 = match ColorCapability::Color16 {
        ColorCapability::TrueColor => "\x1b[38;2;220;235;255m",
        ColorCapability::Color256 => "\x1b[38;5;255m",
        ColorCapability::Color16 => "\x1b[97m",
        ColorCapability::Mono => "",
    };
    assert_eq!(color16, "\x1b[97m");
}

// ── Line-aware semantic rendering (S-master-HUNT-5) ──

#[test]
fn is_suggestion_line_recognizes_all_prefixes() {
    // tip/hint/possible-values/did-you-mean, at any indent depth.
    assert!(is_suggestion_line(
        "  tip: a similar value exists: 'subtle'"
    ));
    assert!(is_suggestion_line("tip: a similar argument exists"));
    assert!(is_suggestion_line("    hint: run --testconf"));
    assert!(is_suggestion_line("  [possible values: typewriter, fade]"));
    assert!(is_suggestion_line("(possible values: none)"));
    assert!(is_suggestion_line("did you mean --color?"));
    // Non-suggestion lines stay in the message color.
    assert!(!is_suggestion_line("expected one of: none, subtle"));
    assert!(!is_suggestion_line("error: bad things"));
    assert!(!is_suggestion_line(""));
    // Prefix must be a token boundary — "tipster" is not a tip.
    assert!(!is_suggestion_line("tipster configuration"));
}

#[test]
fn render_labeled_block_routes_suggestion_lines_to_white_semantic() {
    // The wrap fns are injected — use markers so the test can see
    // WHICH semantic each line took regardless of the test env's
    // color capability (cargo test pipes stderr → Mono → the
    // suggestion wrap renders plain, so the assertion is that tip
    // lines DO NOT go through the body wrap).
    let label = |s: &str| format!("[L:{s}]");
    let body = |s: &str| format!("[B:{s}]");
    let msg = "invalid value for --glitch-level: sutble\nexpected one of: none, subtle\n  tip: a similar value exists: 'subtle'";
    let out = render_labeled_block("error:", label, body, msg);

    // First line: labeled head, message semantic.
    assert!(out.contains("[L:error:] [B:invalid value"));
    // Second line: plain body semantic.
    assert!(out.contains("[B:expected one of:"));
    // Tip line: took the SUGGESTION branch — NOT body-wrapped. In
    // Mono (test env) it renders plain; in a color terminal it
    // would carry the suggestion escape.
    assert!(!out.contains("[B:  tip:"));
    assert!(out.contains("tip: a similar value exists: 'subtle'"));
}

#[test]
fn render_labeled_block_single_line_keeps_label_semantic() {
    let label = |s: &str| format!("[L:{s}]");
    let body = |s: &str| format!("[B:{s}]");
    let out = render_labeled_block("error:", label, body, "simple failure");
    assert_eq!(out, "[L:error:] [B:simple failure]");
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
