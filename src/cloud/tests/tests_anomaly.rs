// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Integration tests for `cloud::phosphor::apply_anomalies` — Phase 6
//! (palette-aware anomaly halos).
//!
//! Pre-Phase-6, `apply_anomalies` had zero test coverage. These tests
//! verify both the existing behavior (anomaly zones brighten cells
//! within their radius/ring, leave outside cells unchanged) and the
//! Phase 6 behavioral contract (the halo target is derived from the
//! active palette, not hardcoded to pure white).

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::super::Cloud;
use crate::cell::Cell;
use crate::chroma::palette::{blend_toward_bg, blend_toward_white, color_to_rgb};
use crate::constants::{ANOMALY_DURATION_SECS, ANOMALY_LUMINANCE_INTENSITY, ANOMALY_MAX_ZONES};
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

use super::super::state::{AnomalyKind, AnomalyZone};

/// Build a TrueColor Green cloud for anomaly tests. The default
/// `make_cloud()` uses ColorMode::Mono (palette = `[Color::White]`),
/// which can't distinguish palette-derived halos from pure white.
/// TrueColor Green has 7 distinct stops — bright enough to verify
/// the halo target is the palette's brightest stop, not pure white.
fn make_truecolor_green_cloud() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

/// Manually inject an anomaly zone at a known position. `spawn_anomaly`
/// picks a random position, which is unsuitable for tests that need to
/// assert specific cells were modified.
fn inject_anomaly(
    cloud: &mut Cloud,
    kind: AnomalyKind,
    col: u16,
    line: u16,
    radius: u16,
    now: Instant,
) {
    assert!(
        cloud.anomaly_zones.len() < ANOMALY_MAX_ZONES,
        "test setup: anomaly_zones already at max capacity"
    );
    cloud.anomaly_zones.push(AnomalyZone {
        col,
        line,
        radius,
        kind,
        start_time: now,
    });
}

/// Set every cell in the frame to a known fg color, so we can verify
/// which cells the anomaly modified. Uses a dark green that's clearly
/// distinct from both pure white and the Green palette's brightest stop.
fn fill_frame_with_known_fg(frame: &mut Frame, fg: Color) {
    for line in 0..frame.height {
        for col in 0..frame.width {
            frame.set(
                col,
                line,
                Cell {
                    ch: '0',
                    fg: Some(fg),
                    bg: None,
                    bold: false,
                },
            );
        }
    }
    frame.clear_dirty();
}

/// Helper: extract the RGB of a cell's fg, panicking if None.
fn cell_rgb(frame: &Frame, col: u16, line: u16) -> (u8, u8, u8) {
    let cell = frame.get(col, line).expect("cell in bounds");
    let fg = cell.fg.expect("cell has fg");
    color_to_rgb(fg)
}

// ── No-op / skip conditions ──────────────────────────────────────────

/// Empty anomaly_zones → apply_anomalies is a no-op. Every cell retains
/// its original fg.
#[test]
fn apply_anomalies_no_op_when_no_zones() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    cloud.apply_anomalies(&mut frame, now);

    // Every cell should be unchanged.
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            assert_eq!(
                cell.fg,
                Some(known_fg),
                "cell ({col},{line}) should be unchanged"
            );
        }
    }
}

/// Expired anomaly zones (elapsed >= ANOMALY_DURATION_SECS) are skipped.
/// The zone is in the list but past its lifetime, so no cells are modified.
#[test]
fn apply_anomalies_skips_expired_zones() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    // Inject an anomaly that started ANOMALY_DURATION_SECS + 0.1s ago —
    // past its lifetime.
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::LuminanceSurge,
        zone_col,
        zone_line,
        4,
        now - Duration::from_millis(((ANOMALY_DURATION_SECS + 0.1) * 1000.0) as u64),
    );

    cloud.apply_anomalies(&mut frame, now);

    // Every cell should still be unchanged — the expired zone is skipped.
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            assert_eq!(
                cell.fg,
                Some(known_fg),
                "expired zone should not modify cell ({col},{line})"
            );
        }
    }
}

// ── LuminanceSurge behavior ──────────────────────────────────────────

/// LuminanceSurge brightens cells within the circular zone. Cells
/// inside the radius should have a DIFFERENT fg after the anomaly.
/// Cells outside should be unchanged.
#[test]
fn luminance_surge_brightens_cells_within_radius() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 4u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::LuminanceSurge,
        zone_col,
        zone_line,
        radius,
        now,
    );

    cloud.apply_anomalies(&mut frame, now);

    // The cell at the zone center (distance 0) should be brightened.
    let center_rgb = cell_rgb(&frame, zone_col, zone_line);
    assert_ne!(
        center_rgb,
        color_to_rgb(known_fg),
        "zone center should be brightened (different from original)"
    );

    // A cell well outside the radius should be unchanged.
    // Pick a cell at the corner — guaranteed outside any reasonable radius.
    let far_col = 0u16;
    let far_line = 0u16;
    // Verify the far cell is actually outside the zone.
    let dist_sq =
        (zone_col as i32 - far_col as i32).pow(2) + (zone_line as i32 - far_line as i32).pow(2);
    assert!(
        dist_sq > (radius * radius) as i32,
        "test setup: far cell must be outside zone radius"
    );
    let far_cell = frame.get(far_col, far_line).expect("cell in bounds");
    assert_eq!(
        far_cell.fg,
        Some(known_fg),
        "cell outside zone should be unchanged"
    );
}

/// LuminanceSurge preserves cells outside the radius exactly. Every
/// cell with dist > radius should have its original fg.
#[test]
fn luminance_surge_preserves_cells_outside_radius() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 3u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::LuminanceSurge,
        zone_col,
        zone_line,
        radius,
        now,
    );

    cloud.apply_anomalies(&mut frame, now);

    // Check every cell outside the radius.
    let r_sq = (radius as i32).pow(2);
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let dist_sq =
                (col as i32 - zone_col as i32).pow(2) + (line as i32 - zone_line as i32).pow(2);
            if dist_sq > r_sq {
                let cell = frame.get(col, line).expect("cell in bounds");
                assert_eq!(
                    cell.fg,
                    Some(known_fg),
                    "cell ({col},{line}) outside zone should be unchanged (dist_sq={dist_sq})"
                );
            }
        }
    }
}

/// Phase 6 contract: LuminanceSurge uses the palette's brightest stop
/// as the halo target, NOT pure white. The result differs from what
/// `blend_toward_white` would produce.
///
/// On a TrueColor Green palette, the brightest stop is `(201, 244, 210)`
/// (pale mint) — visibly different from pure white `(255, 255, 255)`.
/// So `blend_toward_bg(known_fg, brightest_stop, intensity)` must
/// differ from `blend_toward_white(known_fg, intensity)`.
#[test]
fn luminance_surge_uses_palette_brightest_not_pure_white() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 4u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::LuminanceSurge,
        zone_col,
        zone_line,
        radius,
        now,
    );

    cloud.apply_anomalies(&mut frame, now);

    // The center cell was brightened. Compute what we WOULD have gotten
    // with pure white (the pre-Phase-6 behavior).
    let actual_rgb = cell_rgb(&frame, zone_col, zone_line);

    // Compute the expected intensity at the center (dist=0):
    //   falloff = 1.0 - 0 / radius = 1.0
    //   fade = 1.0 - 0 / DURATION = 1.0
    //   intensity = ANOMALY_LUMINANCE_INTENSITY * 1.0 * 1.0
    let intensity = ANOMALY_LUMINANCE_INTENSITY;
    let white_result = blend_toward_white(known_fg, intensity);
    let white_rgb = color_to_rgb(white_result);

    // The actual result must differ from the pure-white result. This is
    // the Phase 6 behavioral contract: palette-derived target, not white.
    assert_ne!(
        actual_rgb, white_rgb,
        "Phase 6: LuminanceSurge should use palette-brightest target, not pure white"
    );

    // Compute what we SHOULD get with the palette's brightest stop.
    let palette_brightest = cloud
        .palette
        .colors
        .last()
        .copied()
        .expect("Green TrueColor palette is non-empty");
    let expected_result = blend_toward_bg(known_fg, palette_brightest, intensity);
    let expected_rgb = color_to_rgb(expected_result);

    // The actual result should match the palette-derived expected result.
    // Note: lerp_u8 uses integer fixed-point with +128 rounding, so the
    // result is deterministic given (fg, target, intensity).
    assert_eq!(
        actual_rgb, expected_rgb,
        "Phase 6: LuminanceSurge result should match blend_toward_bg(fg, palette_brightest, intensity)"
    );
}

// ── PulseWave behavior ───────────────────────────────────────────────

/// PulseWave brightens cells within the expanding ring. At t=0, the
/// wave radius is 0, so the ring is at the center — but the ring
/// width is 2.0, so cells within dist < 2.0 should be brightened.
#[test]
fn pulse_wave_brightens_cells_within_ring() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 5u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::PulseWave,
        zone_col,
        zone_line,
        radius,
        now,
    );

    // Advance time slightly so the ring has expanded to a non-zero radius.
    // At progress=0.5 * radius * 2.0 = radius, the ring is at distance=radius.
    // Use elapsed = ANOMALY_DURATION_SECS / 2.0 → progress = 0.5.
    let elapsed = ANOMALY_DURATION_SECS / 2.0;
    let later = now + Duration::from_millis((elapsed * 1000.0) as u64);

    cloud.apply_anomalies(&mut frame, later);

    // The ring is at distance = progress * radius * 2.0 = 0.5 * 5 * 2 = 5.
    // Cells at distance ~5 from the center should be brightened.
    // Check a cell at (zone_col + 5, zone_line) — should be in the ring.
    let ring_col = zone_col + 5;
    if ring_col < cloud.cols {
        let ring_rgb = cell_rgb(&frame, ring_col, zone_line);
        assert_ne!(
            ring_rgb,
            color_to_rgb(known_fg),
            "cell in the expanding ring should be brightened"
        );
    }

    // A cell far outside the ring (corner) should be unchanged.
    let far_cell = frame.get(0, 0).expect("cell in bounds");
    assert_eq!(
        far_cell.fg,
        Some(known_fg),
        "cell far outside the ring should be unchanged"
    );
}

/// Phase 6 contract: PulseWave uses a hue-cycled palette stop as the
/// halo target, NOT pure white. The result differs from what
/// `blend_toward_white` would produce.
#[test]
fn pulse_wave_uses_palette_cycled_stop_not_pure_white() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 5u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::PulseWave,
        zone_col,
        zone_line,
        radius,
        now,
    );

    // Advance time to a known elapsed. The ring will be at a specific
    // radius, and the hue-cycled target will be at a specific stop.
    let elapsed = 0.5; // 0.5 sec → 4.0 * 0.5 = 2.0 → stop idx 2
    let later = now + Duration::from_millis(500);

    cloud.apply_anomalies(&mut frame, later);

    // Find a cell that was brightened by the ring.
    // At elapsed=0.5, progress = 0.5/1.5 = 0.333, wave_radius = 0.333 * 5 * 2 = 3.33.
    // Cells at distance ~3.33 from center should be in the ring.
    let wave_radius = 0.5 / ANOMALY_DURATION_SECS * radius as f32 * 2.0;
    let ring_col = zone_col + (wave_radius as u16);
    if ring_col < cloud.cols {
        let ring_rgb = cell_rgb(&frame, ring_col, zone_line);

        // Compute what pure-white would have produced (pre-Phase-6).
        // At the ring center, t = 1.0 - 0/2 = 1.0, intensity = 0.2 * 1.0 * fade.
        // fade = 1.0 - 0.5/1.5 = 0.667.
        let fade = 1.0 - elapsed / ANOMALY_DURATION_SECS;
        let intensity = 0.2 * fade;
        let white_result = blend_toward_white(known_fg, intensity);
        let white_rgb = color_to_rgb(white_result);

        // The actual result must differ from pure white.
        assert_ne!(
            ring_rgb, white_rgb,
            "Phase 6: PulseWave should use palette-cycled target, not pure white"
        );

        // Compute the expected palette-derived result.
        // Stop idx = (0.5 * 4.0) as usize % palette.len()
        let stop_idx = (elapsed * crate::chroma::tuning::ANOMALY_HALO_CYCLE_RATE) as usize
            % cloud.palette.colors.len();
        let palette_target = cloud.palette.colors[stop_idx];
        let expected_result = blend_toward_bg(known_fg, palette_target, intensity);
        let expected_rgb = color_to_rgb(expected_result);

        // The actual result should match the palette-derived expected result.
        // Note: the actual cell may be slightly off from the ring center,
        // so its intensity might be slightly lower. We just verify the
        // direction (closer to palette target than to white).
        let dist_to_palette = ((ring_rgb.0 as i32 - expected_rgb.0 as i32).abs()
            + (ring_rgb.1 as i32 - expected_rgb.1 as i32).abs()
            + (ring_rgb.2 as i32 - expected_rgb.2 as i32).abs())
            / 3;
        let dist_to_white = ((ring_rgb.0 as i32 - white_rgb.0 as i32).abs()
            + (ring_rgb.1 as i32 - white_rgb.1 as i32).abs()
            + (ring_rgb.2 as i32 - white_rgb.2 as i32).abs())
            / 3;

        // The actual result should be closer to the palette-derived target
        // than to pure white. This verifies the halo used the palette
        // target, not white.
        assert!(
            dist_to_palette <= dist_to_white,
            "Phase 6: PulseWave result should be closer to palette-cycled target than to white \
             (dist_to_palette={dist_to_palette}, dist_to_white={dist_to_white})"
        );
    }
}

// ── GlyphCorruption sanity ───────────────────────────────────────────

/// GlyphCorruption replaces the character but preserves the cell's fg
/// color. Phase 6 doesn't touch this branch — the helper is only
/// called for LuminanceSurge and PulseWave.
#[test]
fn glyph_corruption_preserves_fg_color() {
    let mut cloud = make_truecolor_green_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 3u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::GlyphCorruption,
        zone_col,
        zone_line,
        radius,
        now,
    );

    cloud.apply_anomalies(&mut frame, now);

    // GlyphCorruption may or may not replace a given cell's char
    // (depends on a hash modulo). But it must NEVER change the fg
    // color of any cell — that's the Phase 6 contract for this branch.
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            assert_eq!(
                cell.fg,
                Some(known_fg),
                "GlyphCorruption must not change fg color of cell ({col},{line})"
            );
        }
    }
}

// ── Edge case: empty palette falls back to white ────────────────────

/// When the palette is empty (degenerate case), `anomaly_halo_target`
/// returns `None`, and `apply_anomalies` falls back to
/// `blend_toward_white`. This preserves pre-Phase-6 behavior for
/// edge cases.
///
/// Note: in practice, `build_palette` always returns at least
/// `[Color::White]` for Mono mode, so an empty palette is rare.
/// But the fallback is the defensive contract.
#[test]
fn luminance_surge_falls_back_to_white_for_empty_palette() {
    // Build a cloud, then manually clear the palette to simulate the
    // degenerate empty-palette case.
    let mut cloud = make_truecolor_green_cloud();
    cloud.palette.colors.clear();
    // Verify the palette is actually empty.
    assert!(
        cloud.palette.colors.is_empty(),
        "test setup: palette should be empty after clear"
    );

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let known_fg = Color::Rgb {
        r: 10,
        g: 50,
        b: 20,
    };
    fill_frame_with_known_fg(&mut frame, known_fg);

    let now = Instant::now();
    let zone_col = cloud.cols / 2;
    let zone_line = cloud.lines / 2;
    let radius = 3u16;
    inject_anomaly(
        &mut cloud,
        AnomalyKind::LuminanceSurge,
        zone_col,
        zone_line,
        radius,
        now,
    );

    cloud.apply_anomalies(&mut frame, now);

    // The center cell should be brightened using blend_toward_white
    // (the fallback for empty palette).
    let actual_rgb = cell_rgb(&frame, zone_col, zone_line);
    let intensity = ANOMALY_LUMINANCE_INTENSITY; // falloff=1, fade=1 at center, t=0
    let expected = blend_toward_white(known_fg, intensity);
    let expected_rgb = color_to_rgb(expected);

    assert_eq!(
        actual_rgb, expected_rgb,
        "empty palette should fall back to blend_toward_white"
    );
}
