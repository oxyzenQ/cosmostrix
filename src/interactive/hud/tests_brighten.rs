// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Hue-preserving brighten_color tests, extracted from `hud_tests.rs`
//! as a pre-emptive split to keep both files below the 800-LOC guard
//! (`scripts/check-rs-loc.sh`). The 8 tests here lock in the
//! hue-preserving behavior so a future change back to a white-blend
//! would fail loudly.

use super::*;

// ── Hue-preserving brighten_color tests ───────────────────────────
//
// The HUD must follow the rain's actual color scheme, not wash out
// to grey. These tests lock in the hue-preserving behavior so a
// future change back to a white-blend would fail loudly.

#[test]
fn brighten_color_preserves_vivid_green_hue() {
    // Vivid green RGB(0,255,0): max=255 >= TARGET_V(200), returned
    // as-is. The HUD line for this palette color must be vivid green,
    // not washed-out grey-green.
    let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
    assert_eq!(out, Color::Rgb { r: 0, g: 255, b: 0 });
}

#[test]
fn brighten_color_preserves_vivid_amber_hue() {
    // Amber/orange RGB(255,176,0): max=255 >= TARGET_V, returned as-is.
    // An amber rain palette must produce an amber HUD, not grey.
    let out = brighten_color(Color::Rgb {
        r: 255,
        g: 176,
        b: 0,
    });
    assert_eq!(
        out,
        Color::Rgb {
            r: 255,
            g: 176,
            b: 0
        }
    );
}

#[test]
fn brighten_color_scales_dark_green_preserving_hue() {
    // Dark green RGB(0,50,0): max=50 < TARGET_V, scale=400.
    // Result must be RGB(0,200,0) — bright green, NOT grey-green.
    // The old white-blend produced RGB(166,183,166) (washed grey).
    let out = brighten_color(Color::Rgb { r: 0, g: 50, b: 0 });
    assert_eq!(out, Color::Rgb { r: 0, g: 200, b: 0 });
}

#[test]
fn brighten_color_scales_dark_blue_preserving_hue_ratio() {
    // Dark blue RGB(50,100,150): max=150 < TARGET_V, scale=133
    // (integer: 200*100/150=133, truncated from 133.33).
    // Result: RGB(66,133,199) — preserves the blue hue ratio.
    // The old white-blend produced RGB(183,201,218) (washed grey-blue).
    // (199 not 200 because 150*133/100=199.5 → truncates to 199.)
    let out = brighten_color(Color::Rgb {
        r: 50,
        g: 100,
        b: 150,
    });
    assert_eq!(
        out,
        Color::Rgb {
            r: 66,
            g: 133,
            b: 199
        }
    );
}

#[test]
fn brighten_color_pure_black_falls_back_to_neutral_grey() {
    // Pure black RGB(0,0,0): max=0, can't scale (0*x=0). Must fall
    // back to a neutral dim grey RGB(120,120,120) so the HUD is
    // still readable. This is the only case where hue is not
    // preserved (there's no hue to preserve in pure black).
    let out = brighten_color(Color::Rgb { r: 0, g: 0, b: 0 });
    assert_eq!(
        out,
        Color::Rgb {
            r: 120,
            g: 120,
            b: 120
        }
    );
}

#[test]
fn brighten_color_named_cyan_preserves_hue_when_bright_enough() {
    // Named Cyan = RGB(0,255,255): max=255 >= TARGET_V, returned as
    // RGB(0,255,255). The old code returned named colors as-is (no
    // conversion), which was fine for Cyan but broke for DarkCyan
    // (next test). This test locks in the conversion behavior.
    let out = brighten_color(Color::Cyan);
    assert_eq!(
        out,
        Color::Rgb {
            r: 0,
            g: 255,
            b: 255
        }
    );
}

#[test]
fn brighten_color_named_darkcyan_gets_scaled_to_readable_cyan() {
    // Named DarkCyan = RGB(0,128,128): max=128 < TARGET_V, scale=156
    // (integer: 200*100/128=156, truncated from 156.25).
    // Result: RGB(0,199,199) — bright cyan, preserving the hue.
    // (199 not 200 because 128*156/100=199.68 → truncates to 199.)
    // The old code returned DarkCyan as-is (too dim on black bg).
    let out = brighten_color(Color::DarkCyan);
    assert_eq!(
        out,
        Color::Rgb {
            r: 0,
            g: 199,
            b: 199
        }
    );
}

#[test]
fn brighten_color_does_not_wash_vivid_colors_to_grey() {
    // Regression guard: the user explicitly flagged "HUD metrics
    // colors too grey". The old 35% source + 65% white blend turned
    // RGB(0,255,0) into RGB(89,255,89) — a washed pale green. The
    // new code must return vivid colors unchanged. Verify the green
    // channel is NOT reduced and the red/blue channels stay at 0.
    let out = brighten_color(Color::Rgb { r: 0, g: 255, b: 0 });
    match out {
        Color::Rgb { r, g, b } => {
            assert_eq!(r, 0, "red channel must stay 0 for pure green");
            assert_eq!(b, 0, "blue channel must stay 0 for pure green");
            assert_eq!(g, 255, "green channel must stay 255 (not washed)");
        }
        other => panic!("expected Rgb, got {other:?}"),
    }
}
