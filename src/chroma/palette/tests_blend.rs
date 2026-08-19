// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

/// Factor=0 returns the original color unchanged.
#[test]
fn blend_toward_bg_zero_factor_unchanged() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let bg = Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    assert_eq!(blend_toward_bg(c, bg, 0.0), c);
}

/// Factor=1 returns approximately the target color (within ±1 unit per
/// channel — `lerp_u8` uses integer fixed-point with a +128 rounding
/// term that biases endpoints by 1 LSB).
#[test]
fn blend_toward_bg_full_factor_returns_bg() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let bg = Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    let result = blend_toward_bg(c, bg, 1.0);
    let Color::Rgb { r, g, b } = result else {
        panic!("expected Rgb");
    };
    assert!((10..=11).contains(&r), "r {r} should be 10 or 11 (±1 LSB)");
    assert!((20..=21).contains(&g), "g {g} should be 20 or 21 (±1 LSB)");
    assert!((30..=31).contains(&b), "b {b} should be 30 or 31 (±1 LSB)");
}

/// Factor=0.5 returns the midpoint between color and bg.
#[test]
fn blend_toward_bg_half_factor_returns_midpoint() {
    let c = Color::Rgb { r: 0, g: 0, b: 0 };
    let bg = Color::Rgb {
        r: 100,
        g: 200,
        b: 50,
    };
    let result = blend_toward_bg(c, bg, 0.5);
    // lerp_u8 uses integer fixed-point: (0 + (100-0)*128 + 128)/256 ≈ 50
    let Color::Rgb { r, g, b } = result else {
        panic!("expected Rgb");
    };
    assert_eq!(r, 50, "midpoint r");
    assert_eq!(g, 100, "midpoint g");
    assert_eq!(b, 25, "midpoint b");
}

/// Color::Reset on either input returns the original color.
#[test]
fn blend_toward_bg_reset_returns_original() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    assert_eq!(blend_toward_bg(Color::Reset, c, 0.5), Color::Reset);
    assert_eq!(blend_toward_bg(c, Color::Reset, 0.5), c);
}

/// Factor > 1.0 is clamped to 1.0 (within ±1 LSB of bg).
#[test]
fn blend_toward_bg_factor_above_one_clamps() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let bg = Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    let result = blend_toward_bg(c, bg, 2.0);
    let Color::Rgb { r, g, b } = result else {
        panic!("expected Rgb");
    };
    assert!((10..=11).contains(&r), "clamped r {r}");
    assert!((20..=21).contains(&g), "clamped g {g}");
    assert!((30..=31).contains(&b), "clamped b {b}");
}

/// Factor < 0.0 is treated as 0.0 (no blend).
#[test]
fn blend_toward_bg_negative_factor_unchanged() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let bg = Color::Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    assert_eq!(blend_toward_bg(c, bg, -0.5), c);
}

/// blend_toward_white is equivalent to blend_toward_bg with white target.
#[test]
fn blend_toward_white_delegates_to_blend_toward_bg() {
    let c = Color::Rgb {
        r: 100,
        g: 50,
        b: 200,
    };
    let via_white = blend_toward_white(c, 0.3);
    let via_bg = blend_toward_bg(
        c,
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        0.3,
    );
    assert_eq!(via_white, via_bg);
}

/// Halos on a dark background: blending a bright head color toward a
/// dark BG produces a darker halo (atmospheric dissolve into scene).
/// Each channel lands between the head value and the bg value.
#[test]
fn blend_toward_bg_dark_bg_darkens_halo() {
    // Bright green head on near-black cosmos background
    let head_r = 80u8;
    let head_g = 255u8;
    let head_b = 110u8;
    let head = Color::Rgb {
        r: head_r,
        g: head_g,
        b: head_b,
    };
    let bg_r = 8u8;
    let bg_g = 12u8;
    let bg_b = 20u8;
    let cosmos_bg = Color::Rgb {
        r: bg_r,
        g: bg_g,
        b: bg_b,
    };
    let halo = blend_toward_bg(head, cosmos_bg, 0.4);
    let Color::Rgb { r, g, b } = halo else {
        panic!("expected Rgb");
    };
    // Halo r must be between bg_r (8) and head_r (80). At factor=0.4
    // it's closer to head than bg: lerp(80, 8, 0.4) ≈ 80 - 28.8 ≈ 51.
    assert!(
        (bg_r..=head_r).contains(&r),
        "halo r {r} must be in [{bg_r}, {head_r}]"
    );
    assert!(
        (bg_g..=head_g).contains(&g),
        "halo g {g} must be in [{bg_g}, {head_g}]"
    );
    assert!(
        (bg_b..=head_b).contains(&b),
        "halo b {b} must be in [{bg_b}, {head_b}]"
    );
    // Sanity: halo is darker than head on all channels (blending toward
    // a darker BG must reduce each channel).
    assert!(r < head_r, "halo r {r} must be < head r {head_r}");
    assert!(g < head_g, "halo g {g} must be < head g {head_g}");
    assert!(b < head_b, "halo b {b} must be < head b {head_b}");
    // And the dominant channel (green) must still be brighter than bg
    // so the head silhouette remains visible.
    assert!(g > bg_g, "halo g {g} must be > bg g {bg_g} (head visible)");
}
