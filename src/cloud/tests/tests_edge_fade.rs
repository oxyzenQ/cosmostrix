// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for viewport edge fade, phosphor energy capping at screen borders,
//! and related bottom-row ghost cell behavior.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::constants::{
    CRT_VIGNETTE_EDGE_FACTOR, CRT_VIGNETTE_HEIGHT, EDGE_FADE_BOTTOM_MIN, EDGE_FADE_BOTTOM_ROWS,
    EDGE_FADE_ROWS, EDGE_FADE_TOP_MIN, PARALLAX_LAYERS, PHOSPHOR_EDGE_ENERGY_CAP,
    PHOSPHOR_EDGE_ROW_TAPER, RAIN_SHADOW_FLOOR, RAIN_SHADOW_PCT,
};

#[test]
fn viewport_edge_fade_is_bounded_and_smooth() {
    // Verify the viewport_edge_fade function produces expected values:
    // - Interior rows (outside both top and bottom fade zones) return 1.0
    // - Row 0 returns EDGE_FADE_TOP_MIN
    // - Last row returns EDGE_FADE_BOTTOM_MIN
    // - Values increase monotonically from top edge to interior
    // - Values decrease monotonically from interior to last row
    //
    // v17: the bottom fade zone is now EDGE_FADE_BOTTOM_ROWS (12) wide,
    // wider than EDGE_FADE_ROWS (3). Use lines=40 so a real interior
    // region exists between the top zone (rows 0-2) and the bottom zone
    // (rows 28-39).
    use crate::droplet::viewport_edge_fade;

    let lines: u16 = 40;

    // Interior rows: between top fade zone (EDGE_FADE_ROWS) and bottom
    // fade zone (EDGE_FADE_BOTTOM_ROWS). These should return exactly 1.0.
    let interior_start = EDGE_FADE_ROWS;
    let interior_end = lines.saturating_sub(EDGE_FADE_BOTTOM_ROWS);
    for line in interior_start..interior_end {
        let fade = viewport_edge_fade(line, lines);
        assert!(
            (fade - 1.0).abs() < 0.001,
            "interior row {} should have fade=1.0, got {}",
            line,
            fade
        );
    }

    // Row 0 should return EDGE_FADE_TOP_MIN
    let fade_top = viewport_edge_fade(0, lines);
    assert!(
        (fade_top - EDGE_FADE_TOP_MIN).abs() < 0.001,
        "row 0 should have fade=EDGE_FADE_TOP_MIN ({:?}), got {}",
        EDGE_FADE_TOP_MIN,
        fade_top
    );

    // Last row should return EDGE_FADE_BOTTOM_MIN
    let fade_bottom = viewport_edge_fade(lines - 1, lines);
    assert!(
        (fade_bottom - EDGE_FADE_BOTTOM_MIN).abs() < 0.001,
        "last row should have fade=EDGE_FADE_BOTTOM_MIN ({:?}), got {}",
        EDGE_FADE_BOTTOM_MIN,
        fade_bottom
    );

    // Top edge: monotonic increase from row 0 to EDGE_FADE_ROWS
    let mut prev = viewport_edge_fade(0, lines);
    for line in 1..EDGE_FADE_ROWS {
        let fade = viewport_edge_fade(line, lines);
        assert!(
            fade > prev,
            "top edge fade should increase monotonically: row {} ({}) > row {} ({})",
            line,
            fade,
            line - 1,
            prev
        );
        prev = fade;
    }

    // Bottom edge: monotonic decrease from interior to last row.
    // v17: the bottom zone is now EDGE_FADE_BOTTOM_ROWS wide, so start
    // from the row just above the bottom zone.
    let bottom_zone_start = lines.saturating_sub(EDGE_FADE_BOTTOM_ROWS);
    let mut prev = viewport_edge_fade(bottom_zone_start, lines);
    for line in (bottom_zone_start + 1)..lines {
        let fade = viewport_edge_fade(line, lines);
        assert!(
            fade < prev,
            "bottom edge fade should decrease monotonically: row {} ({}) < row {} ({})",
            line,
            fade,
            line - 1,
            prev
        );
        prev = fade;
    }
}

#[test]
fn viewport_edge_fade_with_small_terminal() {
    // When terminal is smaller than 2*EDGE_FADE_ROWS, the fade should
    // still work correctly without underflow. All rows get some fade.
    use crate::droplet::viewport_edge_fade;

    let lines: u16 = 4; // Smaller than 2*EDGE_FADE_ROWS=6
    for line in 0..lines {
        let fade = viewport_edge_fade(line, lines);
        assert!(
            fade > 0.0 && fade <= 1.0,
            "fade for line {} in {}-line terminal should be in (0,1], got {}",
            line,
            lines,
            fade
        );
    }
}

#[test]
fn viewport_edge_fade_bottom_more_aggressive_than_top() {
    // The bottom edge should be more aggressively faded than the top
    // to prevent the bright-head residue artifact at the terminal border.
    use crate::droplet::viewport_edge_fade;

    let lines: u16 = 20;
    let fade_top = viewport_edge_fade(0, lines);
    let fade_bottom = viewport_edge_fade(lines - 1, lines);

    assert!(
        fade_bottom < fade_top,
        "bottom edge fade ({}) should be more aggressive than top ({})",
        fade_bottom,
        fade_top
    );
}

#[test]
fn bottom_row_phosphor_energy_is_capped_after_rain() {
    // After rain renders cells in the bottom EDGE_FADE_ROWS, the phosphor
    // energy for those cells should be capped at PHOSPHOR_EDGE_ENERGY_CAP
    // instead of the normal 255. This prevents persistent bright ghost
    // residue from dying droplet heads at the viewport bottom.
    let mut cloud = super::make_cloud();
    cloud.chars_per_sec = 50.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    // Run multiple frames so droplets reach the bottom edge rows
    for i in 0..5 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Check phosphor energy in the bottom EDGE_FADE_ROWS
    let lines = cloud.lines;
    let mut capped_count = 0;
    let mut total_bottom_edge = 0;
    for line in (lines - EDGE_FADE_ROWS)..lines {
        for col in 0..cloud.cols {
            let pidx = col as usize * lines as usize + line as usize;
            if pidx < cloud.phosphor.len() && cloud.phosphor[pidx] > 0 {
                total_bottom_edge += 1;
                assert!(
                    cloud.phosphor[pidx] <= PHOSPHOR_EDGE_ENERGY_CAP,
                    "bottom-edge cell ({}, {}) should have phosphor <= PHOSPHOR_EDGE_ENERGY_CAP ({}), got {}",
                    col,
                    line,
                    PHOSPHOR_EDGE_ENERGY_CAP,
                    cloud.phosphor[pidx]
                );
                capped_count += 1;
            }
        }
    }

    // At least some bottom-edge cells should have phosphor energy after rain
    // (if droplets have reached the bottom rows)
    if total_bottom_edge > 0 {
        assert!(
            capped_count > 0,
            "some bottom-edge cells should have phosphor energy after rain (found {} with energy out of {} total)",
            capped_count,
            total_bottom_edge
        );
        // All bottom-edge cells with energy should be at or below the cap
        let capped_ratio = capped_count as f32 / total_bottom_edge as f32;
        assert_eq!(
            capped_ratio, 1.0,
            "all bottom-edge cells with energy should be capped at PHOSPHOR_EDGE_ENERGY_CAP"
        );
    }
}

#[test]
fn bottom_edge_phosphor_cap_tapers_toward_final_row() {
    let mut cloud = super::make_cloud();
    let lines = cloud.lines;
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let now = Instant::now();
    cloud.last_phosphor_time = now;

    for line in (lines - EDGE_FADE_ROWS)..lines {
        frame.set(
            0,
            line,
            crate::cell::Cell {
                ch: '0',
                fg: Some(Color::Green),
                bg: cloud.palette.bg,
                bold: false,
            },
        );
    }

    cloud.phosphor_decay_pass(&mut frame, 0.0);

    let upper_edge = lines - EDGE_FADE_ROWS;
    let final_row = lines - 1;
    let upper_idx = upper_edge as usize;
    let final_idx = final_row as usize;

    assert_eq!(cloud.phosphor[upper_idx], PHOSPHOR_EDGE_ENERGY_CAP);
    assert_eq!(
        cloud.phosphor[final_idx],
        PHOSPHOR_EDGE_ENERGY_CAP - (EDGE_FADE_ROWS as u8 - 1) * PHOSPHOR_EDGE_ROW_TAPER
    );
    assert!(
        cloud.phosphor[final_idx] < cloud.phosphor[upper_idx],
        "final row phosphor cap should be lower than the upper edge-fade row"
    );
}

#[test]
fn edge_fade_does_not_increase_ghost_background_fill() {
    // The viewport edge fade should not cause an increase in ghost background
    // fill. Specifically, the phosphor energy cap for bottom-edge cells means
    // fewer high-energy ghost cells at the bottom, which should reduce (not
    // increase) the ghost fill ratio.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = super::make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain for several frames to build up phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..20 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Count ghost glyph cells at bottom edge
    let lines = cloud.lines;
    let mut bottom_ghost_glyph = 0;
    let mut bottom_total_with_energy = 0;
    for line in (lines - EDGE_FADE_ROWS)..lines {
        for col in 0..cloud.cols {
            let pidx = col as usize * lines as usize + line as usize;
            if pidx < cloud.phosphor.len() && cloud.phosphor[pidx] > 0 {
                bottom_total_with_energy += 1;
                if cloud.phosphor_base_ch[pidx] != '\0'
                    && cloud.phosphor[pidx] >= PHOSPHOR_GLYPH_THRESHOLD
                {
                    bottom_ghost_glyph += 1;
                }
            }
        }
    }

    // Bottom-edge ghost glyph count should be bounded — not all cells
    // with energy should have visible ghost glyphs
    if bottom_total_with_energy > 0 {
        let ghost_ratio = bottom_ghost_glyph as f32 / bottom_total_with_energy as f32;
        assert!(
            ghost_ratio < 0.9,
            "bottom-edge ghost glyph ratio should be low (got {:.1}%), \
             indicating edge fade prevents excessive ghost background",
            ghost_ratio * 100.0
        );
    }
}

#[test]
fn high_speed_bottom_edge_cells_clear_bounded() {
    // At high speed, bottom-edge cells should fully clear within a bounded
    // number of frames. This specifically tests the fix for the bottom
    // bright-head residue artifact: dying droplets' heads at the bottom row
    // should not leave persistent bright ghost cells.
    use crate::constants::{
        PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_DEAD_THRESHOLD, PHOSPHOR_DECAY_RATE,
    };

    // Calculate frames needed for edge-capped energy to reach dead threshold
    let fps = 60.0;
    let dt = 1.0 / fps;
    let effective_rate = PHOSPHOR_DECAY_RATE * PHOSPHOR_BOTTOM_DECAY_MULT;
    let mut energy = PHOSPHOR_EDGE_ENERGY_CAP as f32;
    let mut frames = 0u32;
    let max_frames = 60;

    while energy > PHOSPHOR_DEAD_THRESHOLD as f32 && frames < max_frames {
        energy *= (-effective_rate * dt).exp();
        frames += 1;
    }

    assert!(
        energy <= PHOSPHOR_DEAD_THRESHOLD as f32,
        "edge-capped phosphor ({}) should decay to dead (<= {}) within {} frames, got energy={}",
        PHOSPHOR_EDGE_ENERGY_CAP,
        PHOSPHOR_DEAD_THRESHOLD,
        frames,
        energy
    );
    // With the edge cap instead of 160/255, bottom-edge cells should clear fast
    assert!(
        frames < 20,
        "edge-capped bottom cells should clear in < 20 frames (got {})",
        frames
    );
}

// ───  masterclass: rain shadow floor + SSOT compounded brightness ──────
//
// The following tests guard the  retune:
// 1. rain_shadow_factor floors at RAIN_SHADOW_FLOOR (0.50) instead of 0.0
// 2. crt_vignette_factor (extracted SSOT) returns expected smoothstep
// 3. compounded_brightness models all 4 dimming effects multiplicatively
//
// These tests are the regression contract for the  audit fix. If
// any of them fail, the bottom-row invisibility bug (compounded brightness
// at 0.08-0.11 = 89-92% dim) has regressed. See
// `docs/research/VISUAL_MODE_AUDIT.md` for the full 4-effect compounding
// model.

#[test]
fn rain_shadow_factor_floors_at_rain_shadow_floor() {
    // The previously curve faded to 0.0 (full black) at the bottom row.
    // caps the floor at RAIN_SHADOW_FLOOR (0.50) so the compounded
    // bottom-row brightness stays above the rain-visibility threshold
    // (~10%) when shadow multiplies with edge fade + radial vignette +
    // CRT vignette.
    //
    // Note: the floor is ASYMPTOTIC — the bottom row of a discrete
    // terminal reaches t = (lines-1-threshold)/span, which is always
    // < 1.0. For lines=40, threshold=34, span=6, the bottom row (line=39)
    // gets t = 5/6 ≈ 0.833, so factor = 0.5 + 0.5*(1 - 0.694) ≈ 0.653.
    // The floor (0.50) is only reached in the limit as lines → ∞. For
    // a tall terminal (lines=400), the bottom row gets t = 59/60 ≈ 0.983,
    // factor ≈ 0.517 — very close to the floor.
    use crate::droplet::rain_shadow_factor;

    let lines: u16 = 40;
    // The shadow zone is the bottom RAIN_SHADOW_PCT (10%) of the screen.
    // For lines=40, threshold = (1.0 - 0.10) * 40 = 36. Rows 36..=39 are
    // in the shadow zone.
    let threshold = ((1.0 - RAIN_SHADOW_PCT) * lines as f32) as u16;
    assert_eq!(threshold, 36, "shadow threshold for 40-line terminal");

    // Every row in the shadow zone must stay >= RAIN_SHADOW_FLOOR.
    for line in threshold..lines {
        let factor = rain_shadow_factor(line, lines);
        assert!(
            factor >= RAIN_SHADOW_FLOOR - 0.001,
            "row {} shadow factor {} should be >= RAIN_SHADOW_FLOOR ({})",
            line,
            factor,
            RAIN_SHADOW_FLOOR
        );
    }

    // The bottom row (line = lines-1) is the closest to the floor for
    // this terminal size. Compute the expected value:
    //   span = 40 - 36 = 4
    //   t = (39 - 36) / 4 = 3/4
    //   1 - t^2 = 1 - 9/16 = 7/16
    //   factor = 0.5 + 0.5 * 7/16 = 0.5 + 0.2188 = 0.7188
    let span = (lines - threshold) as f32;
    let bottom_t = (lines - 1 - threshold) as f32 / span;
    let expected_bottom =
        RAIN_SHADOW_FLOOR + (1.0 - RAIN_SHADOW_FLOOR) * (1.0 - bottom_t * bottom_t);
    let bottom = rain_shadow_factor(lines - 1, lines);
    assert!(
        (bottom - expected_bottom).abs() < 0.001,
        "bottom row shadow factor should be {} (t={},  remapped), got {}",
        expected_bottom,
        bottom_t,
        bottom
    );
    // The bottom row must be the minimum across the shadow zone.
    for line in threshold..(lines - 1) {
        let factor = rain_shadow_factor(line, lines);
        assert!(
            factor > bottom,
            "bottom row ({}) should be the minimum shadow factor, but row {} was lower ({})",
            bottom,
            line,
            factor
        );
    }

    // The first shadow-zone row (line = threshold, t=0) must equal exactly 1.0
    // (quadratic 1 - 0^2 = 1, linearly remapped to RAIN_SHADOW_FLOOR + (1 -
    // RAIN_SHADOW_FLOOR) * 1 = 1.0).
    let top_of_shadow = rain_shadow_factor(threshold, lines);
    assert!(
        (top_of_shadow - 1.0).abs() < 0.001,
        "first shadow-zone row should have factor=1.0, got {}",
        top_of_shadow
    );

    // Rows above the threshold (outside the shadow zone) return 1.0.
    for line in 0..threshold {
        let factor = rain_shadow_factor(line, lines);
        assert!(
            (factor - 1.0).abs() < 0.001,
            "non-shadow row {} should have factor=1.0, got {}",
            line,
            factor
        );
    }

    // Asymptotic floor check: a tall terminal (lines=400) should get its
    // bottom row close to RAIN_SHADOW_FLOOR (within 0.04). With
    // RAIN_SHADOW_PCT=0.10 the shadow zone is 40 rows, so the bottom
    // row reaches t=39/40=0.975, factor ~ 0.525 -- the tolerance is
    // wider than the pre-v50 value (0.02) because fewer rows means
    // the asymptote is approached more slowly.
    let tall_lines: u16 = 400;
    let tall_threshold = ((1.0 - RAIN_SHADOW_PCT) * tall_lines as f32) as u16;
    let tall_bottom = rain_shadow_factor(tall_lines - 1, tall_lines);
    assert!(
        tall_bottom <= RAIN_SHADOW_FLOOR + 0.04,
        "tall terminal (lines={}) bottom row factor {} should be within 0.04 of RAIN_SHADOW_FLOOR ({})",
        tall_lines,
        tall_bottom,
        RAIN_SHADOW_FLOOR
    );
    let _ = tall_threshold; // suppress unused warning
}

#[test]
fn rain_shadow_factor_curve_shape_preserved_by_floor_remapping() {
    // linearly remaps the quadratic `1 - t^2` from [0, 1] to
    // [RAIN_SHADOW_FLOOR, 1.0]. The curve SHAPE (slow start, accelerating
    // fade) must be preserved — only the floor moves. Verify by checking
    // that the  curve is monotonically decreasing across the shadow
    // zone and that the midpoint matches the expected remapped value.
    use crate::droplet::rain_shadow_factor;

    let lines: u16 = 40;
    let threshold = ((1.0 - RAIN_SHADOW_PCT) * lines as f32) as u16;
    let span = (lines - threshold) as f32;

    // Monotonic decrease from threshold (1.0) to lines-1 (RAIN_SHADOW_FLOOR).
    let mut prev = rain_shadow_factor(threshold, lines);
    for line in (threshold + 1)..lines {
        let factor = rain_shadow_factor(line, lines);
        assert!(
            factor < prev,
            "shadow factor should decrease monotonically: row {} ({}) < row {} ({})",
            line,
            factor,
            line - 1,
            prev
        );
        prev = factor;
    }

    // Midpoint of the shadow zone: t = 0.5, quadratic 1 - 0.25 = 0.75.
    // Remapped: RAIN_SHADOW_FLOOR + (1 - RAIN_SHADOW_FLOOR) * 0.75.
    let mid_line = threshold + (span / 2.0) as u16;
    let mid_factor = rain_shadow_factor(mid_line, lines);
    let expected = RAIN_SHADOW_FLOOR + (1.0 - RAIN_SHADOW_FLOOR) * 0.75;
    // Allow generous tolerance because the discrete line index may not
    // land exactly on t=0.5.
    assert!(
        (mid_factor - expected).abs() < 0.05,
        "midpoint shadow factor {} should be near remapped 0.75 (={}), got {}",
        mid_factor,
        expected,
        mid_factor
    );
}

#[test]
fn crt_vignette_factor_banded_correctly() {
    // The extracted `crt_vignette_factor` must return:
    // - 1.0 for rows outside both top and bottom CRT_VIGNETTE_HEIGHT bands
    // - CRT_VIGNETTE_EDGE_FACTOR at the extreme edge rows (row 0 and row lines-1)
    // - A smoothstep curve in between
    use crate::droplet::crt_vignette_factor;

    let lines: u16 = 40;
    let h = CRT_VIGNETTE_HEIGHT;

    // Extreme top edge (row 0): v=0, smoothstep(0) = 0,
    // factor = EDGE + (1 - EDGE) * 0 = EDGE.
    let top_edge = crt_vignette_factor(0, lines);
    assert!(
        (top_edge - CRT_VIGNETTE_EDGE_FACTOR).abs() < 0.001,
        "top edge row factor should be CRT_VIGNETTE_EDGE_FACTOR ({}), got {}",
        CRT_VIGNETTE_EDGE_FACTOR,
        top_edge
    );

    // Extreme bottom edge (row lines-1): v=0 (symmetric), same factor.
    let bottom_edge = crt_vignette_factor(lines - 1, lines);
    assert!(
        (bottom_edge - CRT_VIGNETTE_EDGE_FACTOR).abs() < 0.001,
        "bottom edge row factor should be CRT_VIGNETTE_EDGE_FACTOR ({}), got {}",
        CRT_VIGNETTE_EDGE_FACTOR,
        bottom_edge
    );

    // Interior rows (between top_end=h and bottom_start=lines-h): factor 1.0.
    for line in h..(lines - h) {
        let factor = crt_vignette_factor(line, lines);
        assert!(
            (factor - 1.0).abs() < 0.001,
            "interior row {} should have factor=1.0, got {}",
            line,
            factor
        );
    }

    // Symmetry: row v from top == row lines-1-v from bottom (same v).
    for v in 0..h {
        let top = crt_vignette_factor(v, lines);
        let bottom = crt_vignette_factor(lines - 1 - v, lines);
        assert!(
            (top - bottom).abs() < 0.001,
            "CRT vignette should be symmetric: row {} ({}) == row {} ({})",
            v,
            top,
            lines - 1 - v,
            bottom
        );
    }

    // Monotonic increase from extreme edge (EDGE) to interior edge (1.0).
    let mut prev = crt_vignette_factor(0, lines);
    for line in 1..h {
        let factor = crt_vignette_factor(line, lines);
        assert!(
            factor > prev,
            "top band factor should increase monotonically toward interior: row {} ({}) > row {} ({})",
            line,
            factor,
            line - 1,
            prev
        );
        prev = factor;
    }
}

#[test]
fn crt_vignette_factor_skipped_on_short_terminal() {
    // When lines < 2 * CRT_VIGNETTE_HEIGHT, the vignette is disabled
    // (would dim the entire screen). All rows return 1.0.
    use crate::droplet::crt_vignette_factor;

    let lines: u16 = 2 * CRT_VIGNETTE_HEIGHT - 1; // Too short
    for line in 0..lines {
        let factor = crt_vignette_factor(line, lines);
        assert!(
            (factor - 1.0).abs() < 0.001,
            "short-terminal row {} should have factor=1.0, got {}",
            line,
            factor
        );
    }
}

#[test]
fn compounded_brightness_bottom_row_above_visibility_threshold() {
    // THE  REGRESSION GUARD: the bottom row of an 80x40 terminal
    // must stay above the rain-visibility threshold (~10%) after all 4
    // dimming effects compound. Pre- the compounded brightness was
    // 0.080 (8%, rain invisible); RAIN_SHADOW_FLOOR cap brings
    // it to ~0.172 at the corner / ~0.241 at the center (rain visible).
    //
    // The threshold of 0.10 is the perceptual floor — anything below
    // reads as "no rain" to the eye at typical terminal brightness.
    use crate::droplet::compounded_brightness;

    let cols: u16 = 80;
    let lines: u16 = 40;
    let layer: usize = 0; // Back layer (full 4-effect compounding)
    let visibility_floor = 0.10;

    // Check every column at the bottom row.
    for col in 0..cols {
        let brightness = compounded_brightness(col, lines - 1, cols, lines, layer);
        assert!(
            brightness >= visibility_floor,
            "bottom row col {} compounded brightness {} should be >= visibility floor {} ( RAIN_SHADOW_FLOOR regression)",
            col,
            brightness,
            visibility_floor
        );
    }

    // Bottom-center (col=cols/2): vignette_factor is 1.0 (inside inner
    // radius), so compounded = shadow * edge * 1.0 * crt.
    //   shadow = 0.719 (lines=40, t=3/4, remapped)
    //   edge   = 0.55  (EDGE_FADE_BOTTOM_MIN)
    //   crt    = 0.90  (CRT_VIGNETTE_EDGE_FACTOR)
    //   product = 0.719 * 0.55 * 0.90 = 0.356
    let bottom_center = compounded_brightness(cols / 2, lines - 1, cols, lines, layer);
    assert!(
        (bottom_center - 0.356).abs() < 0.005,
        "bottom-center compounded brightness {} should be ~0.356 (documented  target)",
        bottom_center
    );

    // Bottom-corner (col=0 or col=cols-1): vignette_factor is ~0.80
    // (corner radial dimming, VIGNETTE_INTENSITY=0.20), so compounded =
    // shadow * edge * 0.80 * crt.
    //   product = 0.719 * 0.55 * 0.80 * 0.90 = 0.285
    for col in [0u16, cols - 1] {
        let brightness = compounded_brightness(col, lines - 1, cols, lines, layer);
        assert!(
            (brightness - 0.285).abs() < 0.005,
            "bottom-corner col {} compounded brightness {} should be ~0.285 (documented  target)",
            col,
            brightness
        );
    }
}

#[test]
fn compounded_brightness_top_row_visible() {
    // The top row should remain visibly dim (not destroyed). The
    // retune targeted a compounded top brightness of ~0.53 (visible
    // cinematic fade-in).  doesn't change the top row (no shadow
    // applies there) — this test guards against accidental regressions
    // in the CRT vignette or edge fade constants that would push the
    // top row below the visibility floor.
    use crate::droplet::compounded_brightness;

    let cols: u16 = 80;
    let lines: u16 = 40;
    let layer: usize = 0;

    // Top-center should be well above the visibility floor.
    let top_center = compounded_brightness(cols / 2, 0, cols, lines, layer);
    assert!(
        top_center >= 0.30,
        "top-center compounded brightness {} should be >= 0.30 (documented  target ~0.53)",
        top_center
    );

    // Top corners may be slightly dimmer due to radial vignette but
    // should still be visible.
    for col in [0, cols - 1] {
        let brightness = compounded_brightness(col, 0, cols, lines, layer);
        assert!(
            brightness >= 0.25,
            "top corner col {} compounded brightness {} should be >= 0.25",
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
