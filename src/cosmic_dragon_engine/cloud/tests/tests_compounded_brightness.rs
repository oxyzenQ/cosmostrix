// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for `compounded_brightness` — verifies the top-row visibility,
//! interior unity, front-layer shadow exclusion, and inline-render-path
//! parity. Extracted from `tests_edge_fade.rs` to keep the source file
//! under the 800-LOC cap. Pure code motion — no behavior change.

#[test]
fn compounded_brightness_top_row_visible() {
    // The top row should remain visibly dim (not destroyed). Deep Focus
    // targets a compounded top brightness of ~0.36 (dark but visible —
    // gentler dark entry than noir, deep-focus fade-in from shadow). Rain
    // shadow doesn't apply at the top row — this test guards against
    // accidental regressions in the CRT vignette or edge fade constants
    // that would push the top row below the visibility floor.
    use crate::droplet::compounded_brightness;

    let cols: u16 = 80;
    let lines: u16 = 40;
    let layer: usize = 0;

    // Top-center should be well above the visibility floor.
    // Deep Focus: ~0.36 (dark entry, deep-focus aesthetic)
    let top_center = compounded_brightness(cols / 2, 0, cols, lines, layer);
    assert!(
        top_center >= 0.25,
        "top-center compounded brightness {} should be >= 0.25 (Deep Focus target ~0.36)",
        top_center
    );

    // Top corners may be slightly dimmer due to radial vignette but
    // should still be visible.
    for col in [0, cols - 1] {
        let brightness = compounded_brightness(col, 0, cols, lines, layer);
        assert!(
            brightness >= 0.15,
            "top corner col {} compounded brightness {} should be >= 0.15",
            col,
            brightness
        );
    }
}

#[test]
fn compounded_brightness_interior_is_one() {
    // Interior cells (no shadow, no edge fade, no CRT vignette, inside
    // the radial vignette inner radius) should compound to exactly 1.0
    // — no dimming. The radial vignette's VIGNETTE_INNER_RADIUS=0.7
    // means cells within 70% of the screen half-extent are unmodified.
    use crate::droplet::compounded_brightness;

    let cols: u16 = 80;
    let lines: u16 = 40;
    let layer: usize = 0;

    // Pick a cell firmly in the interior: well inside the top/bottom
    // bands and well inside the radial inner radius.
    let interior_col = cols / 2;
    let interior_line = lines / 2;
    let brightness = compounded_brightness(interior_col, interior_line, cols, lines, layer);
    assert!(
        (brightness - 1.0).abs() < 0.001,
        "interior cell ({}, {}) compounded brightness should be 1.0, got {}",
        interior_col,
        interior_line,
        brightness
    );
}

#[test]
fn compounded_brightness_front_layer_excludes_shadow_and_radial_vignette() {
    // Front layer (layer=2) is exempt from rain shadow + radial vignette
    // (RAIN_SHADOW_LAYER_MULT[2] = 0.0, VIGNETTE_LAYER_MULT[2] = 0.0).
    // Only edge fade + CRT vignette apply. This keeps front-layer neon
    // at full fidelity except at the very top/bottom edge bands.
    use crate::constants::PARALLAX_LAYERS;
    use crate::droplet::compounded_brightness;

    let cols: u16 = 80;
    let lines: u16 = 40;
    let front_layer: usize = PARALLAX_LAYERS - 1; // 2

    // At the bottom-center, the back layer would compound shadow * edge
    // * radial * crt. The front layer should compound ONLY edge * crt
    // (shadow and radial are suppressed by LAYER_MULT=0.0).
    let back_bottom = compounded_brightness(cols / 2, lines - 1, cols, lines, 0);
    let front_bottom = compounded_brightness(cols / 2, lines - 1, cols, lines, front_layer);

    // The front layer should be BRIGHTER than the back layer at the
    // bottom row (no shadow dimming).
    assert!(
        front_bottom > back_bottom,
        "front layer bottom brightness ({}) should exceed back layer ({}) — shadow + radial suppression",
        front_bottom,
        back_bottom
    );

    // At the interior, both layers should be 1.0 (no dimming applies
    // to either).
    let back_interior = compounded_brightness(cols / 2, lines / 2, cols, lines, 0);
    let front_interior = compounded_brightness(cols / 2, lines / 2, cols, lines, front_layer);
    assert!(
        (back_interior - 1.0).abs() < 0.001 && (front_interior - 1.0).abs() < 0.001,
        "interior cells should be 1.0 for both layers: back={}, front={}",
        back_interior,
        front_interior
    );
}

#[test]
fn compounded_brightness_matches_inline_render_path() {
    // Verify the SSOT `compounded_brightness` function agrees with the
    // inline render-path math at a sample cell. The render path computes
    // each effect as `1.0 - (1.0 - raw) * LAYER_MULT[layer]` and then
    // multiplies the RGB tuple in sequence. The SSOT must produce the
    // same final multiplier.
    //
    // We don't call the actual render path here (it requires a full
    // Cloud + Frame + DrawCtx setup); instead we replicate the inline
    // formula manually and compare. This catches drift between the
    // SSOT model and the render-path formula.
    use crate::constants::{RAIN_SHADOW_LAYER_MULT, VIGNETTE_LAYER_MULT};
    use crate::droplet::{
        compounded_brightness, crt_vignette_factor, rain_shadow_factor, viewport_edge_fade,
        vignette_factor,
    };

    let cols: u16 = 80;
    let lines: u16 = 40;
    let layer: usize = 0; // Back layer (LAYER_MULT = 1.0 for both)

    // Sample at the bottom-corner (worst case — all 4 effects active).
    let col = 0u16;
    let line = lines - 1;

    // Inline render-path computation (mirrors droplet.rs:885-924).
    let shadow_raw = rain_shadow_factor(line, lines);
    let shadow_inline = 1.0 - (1.0 - shadow_raw) * RAIN_SHADOW_LAYER_MULT[layer];
    let edge_inline = viewport_edge_fade(line, lines);
    let vignette_raw = vignette_factor(col, line, cols, lines);
    let radial_inline = 1.0 - (1.0 - vignette_raw) * VIGNETTE_LAYER_MULT[layer];
    let crt_inline = crt_vignette_factor(line, lines);
    let inline_product = shadow_inline * edge_inline * radial_inline * crt_inline;

    // SSOT computation.
    let ssot = compounded_brightness(col, line, cols, lines, layer);

    assert!(
        (inline_product - ssot).abs() < 0.0001,
        "SSOT compounded_brightness ({}) must match inline render-path product ({}) at bottom-corner cell. Drift indicates the SSOT model is out of sync with the render path.",
        ssot,
        inline_product
    );
}
