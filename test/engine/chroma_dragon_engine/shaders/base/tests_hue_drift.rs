// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for global hue drift (`hue_drift_offset` + integration with
//! `resolve_cell_color`). Extracted from `tests.rs` to keep the source
//! file under the 800-LOC cap. Pure code motion — no behavior change.

use super::*;

// ── Phase 3-H: global hue drift ───────────────────────────────────────

/// hue_drift_offset maps drift values to integer offsets:
///   0 → 0, π/2 → +1, -π/2 → -1, π → +2, -π → -2.
#[test]
fn hue_drift_offset_known_values() {
    assert_eq!(hue_drift_offset(0.0), 0);
    assert_eq!(hue_drift_offset(std::f32::consts::PI), 2);
    assert_eq!(hue_drift_offset(-std::f32::consts::PI), -2);
    assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_2), 1);
    assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_2), -1);
}

/// Small drifts (|drift| < π/4) round to 0 — the common production
/// case because COLOR_HUE_DRIFT_RATE is small (0.015 rad/tick).
#[test]
fn hue_drift_offset_small_drifts_round_to_zero() {
    assert_eq!(hue_drift_offset(std::f32::consts::FRAC_PI_8), 0);
    assert_eq!(hue_drift_offset(-std::f32::consts::FRAC_PI_8), 0);
    assert_eq!(hue_drift_offset(0.78), 0);
    assert_eq!(hue_drift_offset(-0.78), 0);
}

/// Offset is bounded to {-2, -1, 0, +1, +2} across [-π, π] and is
/// monotonic non-decreasing + odd (offset(-x) = -offset(x)).
#[test]
fn hue_drift_offset_bounded_monotonic_odd() {
    let steps = 1000;
    let mut prev = hue_drift_offset(-std::f32::consts::PI);
    for i in 0..=steps {
        let drift =
            -std::f32::consts::PI + 2.0 * std::f32::consts::PI * (i as f32) / (steps as f32);
        let offset = hue_drift_offset(drift);
        let neg_offset = hue_drift_offset(-drift);
        assert!(
            (-2..=2).contains(&offset),
            "drift {drift} → {offset} out of [-2,2]"
        );
        assert!(
            offset >= prev,
            "non-monotonic at drift {drift}: {offset} < {prev}"
        );
        assert_eq!(
            offset, -neg_offset,
            "not odd: offset({drift})={offset} != -offset(-drift)"
        );
        prev = offset;
    }
}

/// Integration: resolve_cell_color with hue_drift applies the offset
/// to Middle cells. Verify a Middle cell's color shifts when hue_drift
/// is non-zero (vs. None which leaves it unchanged).
#[test]
fn hue_drift_shifts_middle_color() {
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
    let shader_none = make_test_shader(&slots, color_map, false);
    let (fg_none, _) = resolve_cell_color(&shader_none, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_none, Some(palette[3]));

    let mut shader_drift = make_test_shader(&slots, color_map, false);
    shader_drift.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));
    let (fg_drift, _) = resolve_cell_color(&shader_drift, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_drift, Some(palette[5]), "hue_drift=π should shift 3 → 5");
}

/// hue_drift does NOT affect Head or Tail — those are pinned.
#[test]
fn hue_drift_does_not_affect_head_or_tail() {
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
    let mut shader = make_test_shader(&slots, color_map, false);
    shader.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));

    let (fg_head, _) = resolve_cell_color(&shader, 0, 20, 5, 'x', CharLoc::Head, 20, 12);
    assert_eq!(fg_head, Some(palette[7]));

    let (fg_tail, _) = resolve_cell_color(&shader, 0, 9, 5, 'x', CharLoc::Tail, 20, 12);
    assert_eq!(fg_tail, Some(palette[0]));
}

/// hue_drift is skipped under shading_distance — that path has its own
/// length-aware gradient and stacking a hue shift would muddy the signal.
#[test]
fn hue_drift_skipped_under_shading_distance() {
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
    let mut shader_off = make_test_shader(&slots, color_map, true);
    shader_off.hue_drift_offset = None;
    let mut shader_on = make_test_shader(&slots, color_map, true);
    shader_on.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));

    let (fg_off, _) = resolve_cell_color(&shader_off, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    let (fg_on, _) = resolve_cell_color(&shader_on, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(
        fg_off, fg_on,
        "hue_drift must not affect shading_distance path"
    );
}

/// hue_drift clamps to valid palette range — offset that would push
/// color_idx below 0 or above last is clamped.
#[test]
fn hue_drift_clamps_to_palette_range() {
    let palette: Vec<Color> = (0..3)
        .map(|i| Color::Rgb {
            r: i as u8 * 100,
            g: i as u8 * 100,
            b: i as u8 * 100,
        })
        .collect();
    let palette: &[Color] = &palette;

    // Lower bound: color_map=0, hue_drift=-π → offset -2, clamped to 0.
    let color_map_lo: Vec<u8> = vec![0u8; 50 * 100];
    let color_map_lo: &[u8] = &color_map_lo;
    let slots_lo = slot_array(palette);
    let mut shader_lo = make_test_shader(&slots_lo, color_map_lo, false);
    shader_lo.hue_drift_offset = Some(hue_drift_offset(-std::f32::consts::PI));
    let (fg_lo, _) = resolve_cell_color(&shader_lo, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_lo, Some(palette[0]));

    // Upper bound: color_map=2, hue_drift=+π → offset +2, clamped to 2.
    let color_map_hi: Vec<u8> = vec![2u8; 50 * 100];
    let color_map_hi: &[u8] = &color_map_hi;
    let slots_hi = slot_array(palette);
    let mut shader_hi = make_test_shader(&slots_hi, color_map_hi, false);
    shader_hi.hue_drift_offset = Some(hue_drift_offset(std::f32::consts::PI));
    let (fg_hi, _) = resolve_cell_color(&shader_hi, 0, 19, 5, 'x', CharLoc::Middle, 20, 12);
    assert_eq!(fg_hi, Some(palette[2]));
}
