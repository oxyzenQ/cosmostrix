// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Doctor module tests, extracted from inline `mod tests { ... }` block
//! in doctor.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access doctor.rs's private items unchanged.

use super::*;

#[test]
fn terminal_family_detects_common_terms() {
    assert_eq!(terminal_family("xterm-direct"), "xterm-direct");
    assert_eq!(terminal_family("xterm-256color"), "xterm-256color");
    assert_eq!(terminal_family("tmux-256color"), "tmux");
    assert_eq!(terminal_family("screen-256color"), "screen");
    assert_eq!(terminal_family("dumb"), "dumb/unknown");
}

#[test]
fn doctor_guidance_distinguishes_truecolor_and_256_color() {
    assert_eq!(color_capability(ColorMode::TrueColor), "truecolor");
    assert_eq!(color_capability(ColorMode::Color256), "256-color");
    assert!(should_advise_truecolor(
        "xterm-256color",
        "",
        ColorMode::Color256
    ));
    assert!(!should_advise_truecolor(
        "xterm-direct",
        "",
        ColorMode::TrueColor
    ));
}

#[test]
fn doctor_background_guidance_mentions_modes() {
    assert_eq!(
        background_guidance(ColorBg::Black),
        "black paints solid black"
    );
    assert_eq!(
        background_guidance(ColorBg::DefaultBackground),
        "default-background uses terminal default background"
    );
}

#[test]
fn doctor_environment_hints_are_actionable() {
    let hints = environment_hints("tmux-256color", "", false, true, true, true);
    assert!(hints.contains(&"tmux detected"));
    assert!(hints.contains(&"ssh detected"));
    assert!(hints.contains(&"headless/non-TTY detected"));
    assert!(hints.contains(&"COLORTERM missing"));
    assert!(hints.contains(&"locale not UTF-8"));
}
