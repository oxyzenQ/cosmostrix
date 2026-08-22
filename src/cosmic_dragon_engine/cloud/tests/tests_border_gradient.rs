// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Border message chroma dragon gradient tests.
//!
//! v50 (2026-08-17): regression tests for the smooth border gradient fix.
//! The owner reported visible "gaps" between palette stops in the
//! `--message` border sweep — e.g. a white→red sweep showed a white block
//! then a red block with no in-between, instead of the smooth white →
//! semi-red → red gradient the rain color already produced. Root cause:
//! the previous implementation rounded `t * (n-1)` to the nearest integer
//! palette index and picked that discrete stop. The fix uses
//! `interpolate_palette_color` to linearly blend between adjacent stops
//! using the fractional remainder.
//!
//! These tests verify the helper produces interpolated colors at non-
//! integer `t` values (no gaps), preserves palette-identity stops at
//! integer boundaries, and handles edge cases (empty palette, single
//! stop, NaN/Inf, out-of-range `t`) defensively without panicking.
//!
//! ## blend_toward_rgb rounding convention
//!
//! `crate::chroma_dragon_engine::legacy::blend_toward_rgb` uses integer math with a
//! `+128` rounding offset (half-up convention):
//! `out = src + (tgt - src) * wf / 256` where `wf = (factor * 256) as i32`
//! and `+128` is added before the divide to round half-up.
//!
//! This means exact 50% blends between adjacent stops may produce values
//! ±1 from the theoretical midpoint due to truncation toward zero on
//! negative deltas. The expected values in these tests are computed
//! directly from the formula (not from theoretical midpoint math).

use crossterm::style::Color;

use super::interpolate_palette_color;

#[test]
fn empty_palette_returns_none() {
    // An empty palette slice is a degenerate case (Mono mode usually
    // skips the gradient entirely), but the helper must NOT panic —
    // returns `None` so the caller falls back to `content_fg`.
    let palette: Vec<Color> = vec![];
    assert_eq!(interpolate_palette_color(&palette, 0.0), None);
    assert_eq!(interpolate_palette_color(&palette, 0.5), None);
    assert_eq!(interpolate_palette_color(&palette, 1.0), None);
}

#[test]
fn single_stop_palette_returns_that_stop_for_any_t() {
    // A one-stop palette has nothing to interpolate between — every `t`
    // returns the same stop. Important for tiny custom palettes.
    let palette = vec![Color::Rgb {
        r: 100,
        g: 200,
        b: 50,
    }];
    assert_eq!(
        interpolate_palette_color(&palette, 0.0),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
    assert_eq!(
        interpolate_palette_color(&palette, 0.5),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
    assert_eq!(
        interpolate_palette_color(&palette, 1.0),
        Some(Color::Rgb {
            r: 100,
            g: 200,
            b: 50
        })
    );
}

#[test]
fn integer_t_returns_exact_palette_stop_no_interpolation() {
    // At integer boundaries (t=0.0, t=1/n, t=2/n, ..., t=1.0), the helper
    // must return the exact palette stop — no interpolation. This preserves
    // palette-identity stops so the chroma dragon's anchor stops are
    // unchanged. A regression here would mean even integer stops are being
    // blended, which would shift the palette's identity hues.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        }, // idx 1: mid-grey
        Color::Rgb { r: 0, g: 0, b: 0 }, // idx 2: black
    ];
    // t = 0.0 → palette[0]
    assert_eq!(
        interpolate_palette_color(&palette, 0.0),
        Some(Color::Rgb {
            r: 255,
            g: 255,
            b: 255
        })
    );
    // t = 1/2 = 0.5 → palette[1]
    assert_eq!(
        interpolate_palette_color(&palette, 0.5),
        Some(Color::Rgb {
            r: 128,
            g: 128,
            b: 128
        })
    );
    // t = 1.0 → palette[2] (last)
    assert_eq!(
        interpolate_palette_color(&palette, 1.0),
        Some(Color::Rgb { r: 0, g: 0, b: 0 })
    );
}

#[test]
fn non_integer_t_interpolates_between_adjacent_stops() {
    // THE OWNER REGRESSION TEST: at non-integer `t`, the helper must
    // produce an interpolated color (not pick a discrete stop). This
    // eliminates the visible "gap" the owner reported between palette
    // stops. Test palette: white → red (so a 50% interpolation produces
    // a salmon-pink — the "semi-red" the owner wants visible between
    // pure white and pure red).
    //
    // blend_toward_rgb rounding: out = src + (tgt - src) * wf / 256
    // where wf = (factor * 256) as i32, plus a +128 rounding offset.
    // Truncation toward zero on negative deltas means exact 50% blends
    // produce values ±1 from the theoretical midpoint.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 1: red
    ];
    // t = 0.5 → scaled_t = 0.5, pos = 0, frac = 0.5.
    // wf = (0.5 * 256) as i32 = 128.
    // R: 255 + (255-255)*128/256 + 128/256 = 255 + 0 + 0 = 255.
    // G: 255 + (0-255)*128/256 + 128/256 = 255 + (-32512)/256 = 255 + (-127) = 128.
    // B: same as G = 128.
    // Result: RGB(255, 128, 128) — salmon-pink "semi-red".
    let interpolated = interpolate_palette_color(&palette, 0.5);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 128,
            b: 128
        }),
        "t=0.5 between white and red must produce salmon-pink RGB(255,128,128), \
         not discrete white or red block"
    );
    // t = 0.25 → scaled_t = 0.25, pos = 0, frac = 0.25.
    // wf = (0.25 * 256) as i32 = 64.
    // G: 255 + (0-255)*64/256 + 128/256 = 255 + (-16192)/256 = 255 + (-63) = 192.
    let interpolated = interpolate_palette_color(&palette, 0.25);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 192,
            b: 192
        }),
        "t=0.25 between white and red must produce a lighter salmon-pink"
    );
    // t = 0.75 → scaled_t = 0.75, pos = 0, frac = 0.75.
    // wf = (0.75 * 256) as i32 = 192.
    // G: 255 + (0-255)*192/256 + 128/256 = 255 + (-48832)/256 = 255 + (-190) = 65.
    let interpolated = interpolate_palette_color(&palette, 0.75);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 255,
            g: 65,
            b: 65
        }),
        "t=0.75 between white and red must produce a deeper salmon-pink"
    );
}

#[test]
fn three_stop_palette_interpolates_across_two_segments() {
    // With 3 stops, the helper interpolates across segment [0,1] for
    // t ∈ (0, 0.5), and segment [1, 2] for t ∈ (0.5, 1.0). At t = 0.5
    // (the boundary), it returns palette[1] exactly.
    //
    // Test palette: white → grey → black. Linear blend between adjacent
    // stops is exact (no chroma surprises) so we can assert precise
    // values derived from the blend_toward_rgb formula.
    let palette = vec![
        Color::Rgb {
            r: 255,
            g: 255,
            b: 255,
        }, // idx 0: white
        Color::Rgb {
            r: 128,
            g: 128,
            b: 128,
        }, // idx 1: mid-grey
        Color::Rgb { r: 0, g: 0, b: 0 }, // idx 2: black
    ];
    // t = 0.25 → scaled_t = 0.5, pos = 0, frac = 0.5.
    // wf = 128. R: 255 + (128-255)*128/256 + 128/256 = 255 + (-16128)/256
    // = 255 + (-63) = 192 (truncation toward zero, -16128/256 = -63).
    let interpolated = interpolate_palette_color(&palette, 0.25);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 192,
            g: 192,
            b: 192
        })
    );
    // t = 0.75 → scaled_t = 1.5, pos = 1, frac = 0.5.
    // wf = 128. R: 128 + (0-128)*128/256 + 128/256 = 128 + (-16256)/256
    // = 128 + (-63) = 65 (truncation toward zero, -16256/256 = -63).
    let interpolated = interpolate_palette_color(&palette, 0.75);
    assert_eq!(
        interpolated,
        Some(Color::Rgb {
            r: 65,
            g: 65,
            b: 65
        })
    );
}

#[test]
fn out_of_range_t_clamps_to_endpoints() {
    // The helper clamps `t` to [0.0, 1.0] before computing scaled_t.
    // t < 0 → returns palette[0] (first stop).
    // t > 1 → returns palette[n-1] (last stop).
    // This matches the visual semantics — t represents the parametric
    // position around the border box perimeter, which is naturally
    // bounded [0, 1].
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red (first)
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue (last)
    ];
    // t = -0.5 → clamps to 0.0 → palette[0]
    assert_eq!(
        interpolate_palette_color(&palette, -0.5),
        Some(Color::Rgb { r: 255, g: 0, b: 0 })
    );
    // t = 1.5 → clamps to 1.0 → palette[2]
    assert_eq!(
        interpolate_palette_color(&palette, 1.5),
        Some(Color::Rgb { r: 0, g: 0, b: 255 })
    );
}

#[test]
fn nan_t_falls_back_to_first_stop_defensive() {
    // NaN `t` must NOT panic — returns the first stop defensively.
    // The owner mandate for HUD metric stability extends to all runtime
    // math; a NaN t (e.g. from a 0/0 division upstream) would propagate
    // as a NaN color otherwise and could crash the renderer.
    let palette = vec![
        Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        }, // idx 0: first
        Color::Rgb {
            r: 200,
            g: 100,
            b: 50,
        }, // idx 1
    ];
    assert_eq!(
        interpolate_palette_color(&palette, f32::NAN),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "NaN t must fall back to first stop, not panic"
    );
    assert_eq!(
        interpolate_palette_color(&palette, f32::INFINITY),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "+Inf t must fall back to first stop, not panic"
    );
    assert_eq!(
        interpolate_palette_color(&palette, f32::NEG_INFINITY),
        Some(Color::Rgb {
            r: 10,
            g: 20,
            b: 30
        }),
        "-Inf t must fall back to first stop, not panic"
    );
}

#[test]
fn adjacent_cells_produce_distinct_colors_no_gaps() {
    // THE OWNER REGRESSION TEST (visual gap elimination): with a small
    // palette and many border cells, consecutive cells must produce
    // DISTINCT colors — not the same discrete palette stop repeated.
    // This is what eliminates the visible "gap" the owner reported.
    //
    // Test setup: 3-stop palette (red/green/blue), 10 cells (so t steps
    // of 1/9 ≈ 0.111). Without interpolation, cells 0/1/2/3 would all
    // be palette[0] (red), cells 4/5 would be palette[1] (green), cells
    // 6/7/8/9 would be palette[2] (blue) — only 3 distinct colors.
    // With interpolation, every cell gets a slightly different color.
    let palette = vec![
        Color::Rgb { r: 255, g: 0, b: 0 }, // idx 0: red
        Color::Rgb { r: 0, g: 255, b: 0 }, // idx 1: green
        Color::Rgb { r: 0, g: 0, b: 255 }, // idx 2: blue
    ];
    let total_cells = 10usize;
    let mut colors: Vec<Color> = Vec::with_capacity(total_cells);
    for i in 0..total_cells {
        let t = i as f32 / (total_cells - 1) as f32;
        colors.push(interpolate_palette_color(&palette, t).unwrap());
    }
    // Count distinct colors — with interpolation we expect at least 5
    // distinct values (the 3 palette stops + interpolated intermediates).
    // The old discrete-sampling implementation would have produced only
    // 3 distinct values (one per palette stop).
    let distinct_count = {
        let mut unique: Vec<Color> = colors.clone();
        unique.dedup();
        unique.len()
    };
    assert!(
        distinct_count >= 5,
        "interpolated border must produce >= 5 distinct colors across 10 cells \
         (got {distinct_count}) — the old discrete-sampling implementation would \
         have produced only 3 (one per palette stop), causing the visible gap"
    );
    // Also assert that NO two adjacent cells share the same color when
    // the palette has 3+ stops and there are 5+ cells — interpolation
    // guarantees monotonic transitions.
    for window in colors.windows(2) {
        assert_ne!(
            window[0], window[1],
            "adjacent border cells must NOT share the same color — that's the \
             visible gap the owner reported"
        );
    }
}
