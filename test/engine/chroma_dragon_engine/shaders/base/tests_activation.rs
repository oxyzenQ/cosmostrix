// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Activation tests for chroma::shaders::base (Phase 4-A/B/D).
//!
//! Split from base_tests.rs to keep that file under the 800-LOC source cap.
//! Uses `use super::*` to access base_tests's private helpers.

use super::*;

// ── Phase 4-A: column_coherence activation ────────────────────────────
//
// Phase 4-A wires `column_coherence_lut` through `DrawCtx` →
// `ShaderCtx` (previously hard-coded `None`). Phase D (hot-path) changed
// the field from `Option<f32>` (per-cell phase → sinf) to `Option<&[i32]>`
// (precomputed LUT). These tests verify the end-to-end path: when
// `Some(lut)` is set, `resolve_cell_color` actually applies the
// perturbation (produces different output than `None`). A regression
// that reverts the wiring to `None` would fail these tests.

/// `column_coherence_lut: Some(...)` perturbs the Middle cell's
/// color_idx, producing a different palette stop than `None` for at
/// least one (phase, col) combination.
///
/// Setup: 5-stop palette, color_map=2 (Middle would normally land on
/// stop 2). With phase=π/2 and col=0, perturbation=+1 → color_idx=3.
/// With phase=3π/2 and col=0, perturbation=-1 → color_idx=1.
/// Both must differ from the `None` result (color_idx=2).
///
/// Phase D: the LUT is built from the phase using the production helper
/// `column_coherence_perturbation(phase, col)`, mirroring how `rain.rs`
/// builds the LUT once per frame.
#[test]
fn phase4a_column_coherence_perturbs_middle_cell() {
    let palette: Vec<Color> = (0..5)
        .map(|i| Color::Rgb {
            r: i as u8 * 60,
            g: i as u8 * 60,
            b: i as u8 * 60,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![2u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let mut shader_off = make_test_shader(&slots, color_map, false);
    shader_off.column_coherence_lut = None;
    let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_off, Some(palette[2]));

    // phase=π/2, col=0 → perturbation +1 → color_idx 3.
    // Build the LUT from the phase using the production helper.
    let lut_up: Vec<i32> = (0..6)
        .map(|c| column_coherence_perturbation(std::f32::consts::FRAC_PI_2, c))
        .collect();
    let mut shader_up = make_test_shader(&slots, color_map, false);
    shader_up.column_coherence_lut = Some(&lut_up);
    let (fg_up, _) = resolve_cell_color(&shader_up, 0, 19, 0, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(
        fg_up,
        Some(palette[3]),
        "phase=π/2 should shift color_idx 2 → 3"
    );

    // phase=3π/2, col=0 → perturbation -1 → color_idx 1.
    let lut_dn: Vec<i32> = (0..6)
        .map(|c| column_coherence_perturbation(3.0 * std::f32::consts::FRAC_PI_2, c))
        .collect();
    let mut shader_dn = make_test_shader(&slots, color_map, false);
    shader_dn.column_coherence_lut = Some(&lut_dn);
    let (fg_dn, _) = resolve_cell_color(&shader_dn, 0, 19, 0, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(
        fg_dn,
        Some(palette[1]),
        "phase=3π/2 should shift color_idx 2 → 1"
    );
}

/// `column_coherence_lut` is skipped under `shading_distance` (that
/// path has its own length-aware gradient). Verified by asserting
/// identical output with and without the LUT set.
#[test]
fn phase4a_column_coherence_skipped_under_shading_distance() {
    let palette: Vec<Color> = (0..8)
        .map(|i| Color::Rgb {
            r: i as u8 * 30,
            g: i as u8 * 30,
            b: i as u8 * 30,
        })
        .collect();
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![3u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    // Build a LUT from phase=π/2 — same phase the pre-Phase-D test used
    // directly via `column_coherence_phase = Some(π/2)`.
    let lut: Vec<i32> = (0..6)
        .map(|c| column_coherence_perturbation(std::f32::consts::FRAC_PI_2, c))
        .collect();

    let mut shader_off = make_test_shader(&slots, color_map, true);
    shader_off.column_coherence_lut = None;
    let mut shader_on = make_test_shader(&slots, color_map, true);
    shader_on.column_coherence_lut = Some(&lut);

    let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let (fg_on, _) = resolve_cell_color(&shader_on, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(
        fg_off, fg_on,
        "column_coherence must not affect shading_distance path"
    );
}

// ── Phase 4-B: subpixel_jitter activation ─────────────────────────────
//
// Phase 4-B wires `subpixel_jitter_amplitude` through `DrawCtx` →
// `ShaderCtx` (previously hard-coded `None`). These tests verify the
// end-to-end path: when `Some(amp)` is set, `resolve_cell_color`
// perturbs the resolved RGB (produces different output than `None`).

/// `subpixel_jitter_amplitude: Some(amp)` perturbs the Middle cell's
/// resolved RGB. With amp=3 and a known cell hash, the result must
/// differ from the `None` result (which returns the palette stop
/// unchanged) and stay within ±amp per channel.
#[test]
fn phase4b_subpixel_jitter_perturbs_resolved_rgb() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    // None: result is exactly palette[0].
    let mut shader_off = make_test_shader(&slots, color_map, false);
    shader_off.subpixel_jitter_amplitude = None;
    let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_off, Some(palette[0]));

    // Some(3): result is palette[0] perturbed by ±3 per channel.
    let mut shader_on = make_test_shader(&slots, color_map, false);
    shader_on.subpixel_jitter_amplitude = Some(3);
    let (fg_on, _) = resolve_cell_color(&shader_on, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let Color::Rgb { r, g, b } = fg_on.expect("Some when amp set") else {
        panic!("expected Rgb");
    };
    assert!(
        (i32::from(r) - 100).abs() <= 3
            && (i32::from(g) - 100).abs() <= 3
            && (i32::from(b) - 100).abs() <= 3,
        "jittered RGB ({r}, {g}, {b}) out of ±3 bounds from (100, 100, 100)"
    );
    // Must actually be perturbed (deterministic hash rarely gives 0,0,0).
    // If the hash happens to give all-zero offsets, this could false-fail;
    // use a cell where we know the hash is nonzero (line=19, col=5).
    // cell_hash(19, 5) = FNV(19) then FNV(5) — guaranteed nonzero.
    assert_ne!(
        fg_on, fg_off,
        "jitter must produce a visible change for (19, 5)"
    );
}

/// `subpixel_jitter_amplitude: Some(0)` is a no-op — matches `None`.
/// This guards the `if amplitude == 0 { return color; }` fast path.
#[test]
fn phase4b_subpixel_jitter_zero_amplitude_matches_none() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 100,
        g: 100,
        b: 100,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let mut shader_none = make_test_shader(&slots, color_map, false);
    shader_none.subpixel_jitter_amplitude = None;
    let mut shader_zero = make_test_shader(&slots, color_map, false);
    shader_zero.subpixel_jitter_amplitude = Some(0);

    let (fg_none, _) = resolve_cell_color(&shader_none, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let (fg_zero, _) = resolve_cell_color(&shader_zero, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_none, fg_zero, "amplitude=0 must match None (both no-op)");
}

// ── Phase 4-D: head halo activation ──────────────────────────────────────
//
// Phase 4-D wires `head_halo_factor` + `bg` through `DrawCtx` → `ShaderCtx`
// (previously `blend_toward_bg` existed but had zero production callers).
// These tests verify the end-to-end path: when both are Some, the shader
// blends the Head cell color toward the background.

/// `head_halo_factor: Some(factor)` + `bg: Some(bg)` blends the Head cell's
/// resolved color toward the background. The result must differ from the
/// `None` result (which returns the palette stop unchanged) and lie between
/// the head color and the bg color.
#[test]
fn phase4d_head_halo_blends_toward_bg() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let bg = Color::Rgb { r: 0, g: 0, b: 0 };

    // None: Head returns exactly palette[0] = (200, 200, 200).
    let mut shader_off = make_test_shader(&slots, color_map, false);
    shader_off.head_halo_factor = None;
    let (fg_off, bold_off) = resolve_cell_color(&shader_off, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg_off, Some(palette[0]));
    assert!(bold_off, "Head must be bold");

    // Some(0.5) + bg=(0,0,0): Head blends 50% toward black. lerp_u8 uses
    // integer rounding with +128 bias, so the exact result is (101, 101, 101)
    // rather than (100, 100, 100) — we assert the mathematical guarantees
    // (between head and bg, strictly dimmer than unhaloed) rather than the
    // exact rounding.
    let mut shader_on = make_test_shader(&slots, color_map, false);
    shader_on.head_halo_factor = Some(0.5);
    shader_on.bg = Some(bg);
    let (fg_on, bold_on) = resolve_cell_color(&shader_on, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    let Color::Rgb { r, g, b } = fg_on.expect("Some when factor+bg set") else {
        panic!("expected Rgb");
    };
    assert!(
        r > 0 && r < 200 && g > 0 && g < 200 && b > 0 && b < 200,
        "haloed RGB ({r}, {g}, {b}) must be strictly between bg (0) and head (200)"
    );
    assert!(
        r < 200 && g < 200 && b < 200,
        "halo must dim the head toward bg"
    );
    assert_ne!(fg_on, fg_off, "halo must produce a visible change");
    assert!(bold_on, "halo must not change bold state");
}

/// Halo applies ONLY to Head cells, not Middle or Tail. Middle cells with
/// the same factor+bg must return the palette stop unchanged.
#[test]
fn phase4d_head_halo_skipped_for_middle_and_tail() {
    let palette: Vec<Color> = vec![
        Color::Rgb {
            r: 50,
            g: 50,
            b: 50,
        }, // stop 0 (tail)
        Color::Rgb {
            r: 150,
            g: 150,
            b: 150,
        }, // stop 1 (middle)
        Color::Rgb {
            r: 250,
            g: 250,
            b: 250,
        }, // stop 2 (head)
    ];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![1u8; 50 * 100]; // middle → stop 1
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);
    let bg = Color::Rgb { r: 0, g: 0, b: 0 };

    let mut shader = make_test_shader(&slots, color_map, false);
    shader.head_halo_factor = Some(0.5);
    shader.bg = Some(bg);

    // Head: haloed (250 blended 50% toward 0 → strictly between 0 and 250).
    // lerp_u8 integer rounding produces ~126, not exactly 125 — we assert
    // the value is strictly dimmer than the unhaloed head (250) and strictly
    // brighter than the bg (0).
    let (fg_head, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    let Color::Rgb { r, .. } = fg_head.expect("Some") else {
        panic!("expected Rgb");
    };
    assert!(
        r > 0 && r < 250,
        "haloed head r={r} must be strictly between bg (0) and head (250)"
    );

    // Middle: NOT haloed (returns stop 1 = 150 unchanged).
    let (fg_mid, _) = resolve_cell_color(&shader, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let Color::Rgb { r, .. } = fg_mid.expect("Some") else {
        panic!("expected Rgb");
    };
    assert_eq!(r, 150, "Middle must NOT be haloed");

    // Tail: NOT haloed (returns stop 0 = 50 unchanged).
    let (fg_tail, _) = resolve_cell_color(&shader, 0, 18, 5, 'x', CharLoc::Tail, 20, 12);
    let Color::Rgb { r, .. } = fg_tail.expect("Some") else {
        panic!("expected Rgb");
    };
    assert_eq!(r, 50, "Tail must NOT be haloed");
}

/// `head_halo_factor: None` disables the halo even when bg is Some.
/// Matches pre-Phase-4-D dormant behavior.
#[test]
fn phase4d_head_halo_none_factor_is_noop() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let mut shader = make_test_shader(&slots, color_map, false);
    shader.head_halo_factor = None;
    shader.bg = Some(Color::Rgb { r: 0, g: 0, b: 0 });
    let (fg, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg, Some(palette[0]), "None factor must be a no-op");
}

/// `bg: None` or `bg: Color::Reset` disables the halo even when factor is
/// Some. This guards the auto-no-op path in the shader's match.
#[test]
fn phase4d_head_halo_none_or_reset_bg_is_noop() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    // bg = None
    let mut shader_none = make_test_shader(&slots, color_map, false);
    shader_none.head_halo_factor = Some(0.5);
    shader_none.bg = None;
    let (fg_none, _) = resolve_cell_color(&shader_none, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg_none, Some(palette[0]), "None bg must be a no-op");

    // bg = Color::Reset
    let mut shader_reset = make_test_shader(&slots, color_map, false);
    shader_reset.head_halo_factor = Some(0.5);
    shader_reset.bg = Some(Color::Reset);
    let (fg_reset, _) = resolve_cell_color(&shader_reset, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg_reset, Some(palette[0]), "Reset bg must be a no-op");
}

/// `head_halo_factor: Some(0.0)` is a no-op — blend_toward_bg returns the
/// original color when factor ≤ 0. Matches the `None` path.
#[test]
fn phase4d_head_halo_zero_factor_is_noop() {
    let palette: Vec<Color> = vec![Color::Rgb {
        r: 200,
        g: 200,
        b: 200,
    }];
    let palette: &[Color] = &palette;
    let color_map: Vec<u8> = vec![0u8; 50 * 100];
    let color_map: &[u8] = &color_map;
    let slots = slot_array(palette);

    let mut shader = make_test_shader(&slots, color_map, false);
    shader.head_halo_factor = Some(0.0);
    shader.bg = Some(Color::Rgb { r: 0, g: 0, b: 0 });
    let (fg, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg, Some(palette[0]), "factor=0.0 must be a no-op");
}
