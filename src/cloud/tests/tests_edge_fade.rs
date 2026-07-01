// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for viewport edge fade, phosphor energy capping at screen borders,
//! and related bottom-row ghost cell behavior.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::constants::{
    EDGE_FADE_BOTTOM_MIN, EDGE_FADE_ROWS, EDGE_FADE_TOP_MIN, PHOSPHOR_EDGE_ENERGY_CAP,
    PHOSPHOR_EDGE_ROW_TAPER,
};

#[test]
fn viewport_edge_fade_is_bounded_and_smooth() {
    // Verify the viewport_edge_fade function produces expected values:
    // - Interior rows return 1.0
    // - Row 0 returns EDGE_FADE_TOP_MIN
    // - Last row returns EDGE_FADE_BOTTOM_MIN
    // - Values increase monotonically from edges to interior
    use crate::droplet::viewport_edge_fade;

    let lines: u16 = 20;

    // Interior rows should return 1.0
    for line in EDGE_FADE_ROWS..(lines - EDGE_FADE_ROWS) {
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

    // Bottom edge: monotonic decrease from interior to last row
    let mut prev = viewport_edge_fade(lines - 1 - EDGE_FADE_ROWS, lines);
    for offset in 1..=EDGE_FADE_ROWS {
        let line = lines - 1 - EDGE_FADE_ROWS + offset;
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
