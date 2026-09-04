// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-9: Palette PartialEq tests.
//!
//! `Palette` now derives `PartialEq` (added in S-master-HUNT-9) so the
//! live-reload rebuild path (`event_loop_config_rebuild`) can detect
//! palette CONTENT changes — not just scheme-enum changes. This enables
//! the 300ms palette transition wave to fire for custom palette edits
//! and color-tune edits, which previously produced instant color jumps.
//!
//! These tests verify the derive is correct (deep content comparison
//! over `colors: Vec<Color>` + `bg: Option<Color>`) and that the
//! equality semantics match the live-reload guard's expectations.

use crossterm::style::Color;

use super::Palette;

#[test]
fn palette_partial_eq_compares_colors_vec_content() {
    // Two palettes with identical colors + bg must be equal.
    let p1 = Palette {
        colors: vec![
            Color::Rgb {
                r: 10,
                g: 20,
                b: 30,
            },
            Color::Rgb {
                r: 40,
                g: 50,
                b: 60,
            },
        ],
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    let p2 = Palette {
        colors: vec![
            Color::Rgb {
                r: 10,
                g: 20,
                b: 30,
            },
            Color::Rgb {
                r: 40,
                g: 50,
                b: 60,
            },
        ],
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    assert_eq!(p1, p2, "identical palettes must be equal");

    // Different colors (one stop changed) must be unequal.
    let p3 = Palette {
        colors: vec![
            Color::Rgb {
                r: 10,
                g: 20,
                b: 30,
            },
            Color::Rgb {
                r: 99,
                g: 50,
                b: 60,
            },
        ],
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    assert_ne!(
        p1, p3,
        "palettes with different color stops must be unequal"
    );

    // Different colors length must be unequal.
    let p4 = Palette {
        colors: vec![Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }],
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    assert_ne!(
        p1, p4,
        "palettes with different colors length must be unequal"
    );
}

#[test]
fn palette_partial_eq_compares_bg_option() {
    let colors = vec![Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    }];
    let with_bg = Palette {
        colors: colors.clone(),
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    let no_bg = Palette { colors, bg: None };
    assert_ne!(
        with_bg, no_bg,
        "palettes with different bg presence must be unequal"
    );

    let bg_a = Palette {
        colors: vec![Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }],
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
    };
    let bg_b = Palette {
        colors: vec![Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }],
        bg: Some(Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }),
    };
    assert_ne!(
        bg_a, bg_b,
        "palettes with different bg color must be unequal"
    );
}

#[test]
fn palette_partial_eq_detects_color_tune_mutation() {
    // Simulates the live-reload color-tune path: apply_tune_to_palette
    // mutates `cloud.palette.colors` in place (e.g. shifts saturation).
    // The PartialEq must detect this mutation so the live-reload guard
    // `cloud.palette != preserved_palette` fires correctly.
    let original = Palette {
        colors: vec![
            Color::Rgb {
                r: 100,
                g: 150,
                b: 200,
            },
            Color::Rgb {
                r: 50,
                g: 100,
                b: 150,
            },
        ],
        bg: Some(Color::Rgb {
            r: 10,
            g: 10,
            b: 10,
        }),
    };
    // Clone + mutate one color stop (simulating sat=1.5 tune).
    let mut tuned = original.clone();
    tuned.colors[0] = Color::Rgb {
        r: 150,
        g: 200,
        b: 250,
    };
    assert_ne!(
        original, tuned,
        "color-tune mutation must be detected by PartialEq"
    );
}

#[test]
fn palette_partial_eq_is_reflexive_and_symmetric() {
    // Sanity check: PartialEq derive is reflexive (a == a) and symmetric
    // (a == b implies b == a). The derive macro guarantees this, but the
    // test documents the contract for the live-reload guard.
    let p1 = Palette {
        colors: vec![Color::AnsiValue(42)],
        bg: None,
    };
    let p2 = p1.clone();
    assert_eq!(p1, p1, "PartialEq must be reflexive");
    assert_eq!(p1, p2, "clone must be equal to original");
    assert_eq!(p2, p1, "PartialEq must be symmetric");
}
