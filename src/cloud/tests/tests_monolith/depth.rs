// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith depth tests: depth lab regression, sparse lane density,
//! empty-space threshold, no dense wall, brightness/depth hierarchy,
//! hero glyph guard, sparse comparison.

use std::time::{Duration, Instant};

use super::{
    make_cloud, make_monolith_cloud, run_frames, visible_cell_count, visible_chars, DrawnCellKind,
    Frame,
};
use crate::cloud::monolith::BrightnessLevel;
use crate::cloud::render::DrawCtx;
use crate::constants::MAX_PALETTE_SLOTS;
use crate::runtime::{BoldMode, ColorMode};
use crossterm::style::Color;

#[test]
fn monolith_does_not_draw_full_height_continuous_spine() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);

    let mut spine_by_col: Vec<Vec<u16>> = vec![Vec::new(); frame.width as usize];
    let mut spine_count = 0usize;
    let mut segment_count = 0usize;
    for cell in cloud.monolith_rain.drawn_cells_for_test() {
        match cell.kind {
            DrawnCellKind::Spine => {
                spine_by_col[cell.col as usize].push(cell.line);
                spine_count += 1;
            }
            DrawnCellKind::Segment => segment_count += 1,
        }
    }

    assert!(
        spine_count > 0,
        "monolith should still draw subtle local spines"
    );
    assert!(
        spine_count < segment_count,
        "spines should be accents, not the dominant visual"
    );
    for lines in spine_by_col.iter_mut().filter(|lines| !lines.is_empty()) {
        lines.sort_unstable();
        let longest_run = lines
            .windows(2)
            .fold((1usize, 1usize), |(best, run), pair| {
                let next_run = if pair[1] == pair[0] + 1 { run + 1 } else { 1 };
                (best.max(next_run), next_run)
            })
            .0;
        assert!(
            longest_run <= 2,
            "spine should not become a continuous vertical column"
        );
        assert!(
            lines.len() < frame.height as usize / 3,
            "spine should occupy only local fragments of a lane"
        );
    }
}

#[test]
fn monolith_hero_hash_glyph_is_absent() {
    let mut cloud = make_monolith_cloud(80, 24);
    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let chars = visible_chars(&frame);
    assert!(
        !chars.contains(&'#'),
        "monolith hero glyphs should not be dominated by harsh # marks"
    );
}

#[test]
fn monolith_rain_is_sparse_compared_to_dense_glyph_rain() {
    let mut monolith = make_monolith_cloud(80, 24);
    let mut monolith_frame = Frame::new(80, 24, monolith.palette.bg);
    run_frames(&mut monolith, &mut monolith_frame, 120, 16);

    let mut glyph = make_cloud();
    glyph.set_droplet_density(2.0);
    glyph.set_chars_per_sec(16.0);
    glyph.reset(80, 24);
    glyph.clear_redraw_flags_for_test();
    let mut glyph_frame = Frame::new(80, 24, glyph.palette.bg);
    run_frames(&mut glyph, &mut glyph_frame, 120, 16);

    let monolith_visible = visible_cell_count(&monolith_frame);
    let glyph_visible = visible_cell_count(&glyph_frame);

    assert!(
        monolith_visible < glyph_visible,
        "monolith should stay sparser than dense glyph rain (monolith={monolith_visible}, glyph={glyph_visible})"
    );
}

// -- Visual depth regression guards (v3.6.0) --

#[test]
fn monolith_color_for_level_ghost_is_faintest() {
    let colors: Vec<Color> = vec![
        Color::Rgb { r: 0, g: 0, b: 20 },
        Color::Rgb { r: 0, g: 30, b: 5 },
        Color::Rgb { r: 0, g: 60, b: 10 },
        Color::Rgb {
            r: 0,
            g: 100,
            b: 15,
        },
        Color::Rgb {
            r: 0,
            g: 140,
            b: 25,
        },
        Color::Rgb {
            r: 0,
            g: 180,
            b: 35,
        },
        Color::Rgb {
            r: 0,
            g: 220,
            b: 45,
        },
        Color::Rgb {
            r: 0,
            g: 250,
            b: 55,
        },
    ];
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&colors, empty, empty, empty];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 10,
        full_width: false,
        shading_distance: false,
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        color_mode: ColorMode::TrueColor,
        bold_mode: BoldMode::Off,
        glitchy: false,
        last_glitch_time: std::time::Instant::now(),
        next_glitch_time: std::time::Instant::now(),
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['A'],
        previous_char_pool: &[],
        charset_wave_line: None,
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_col: u16::MAX,
        flash_line: u16::MAX,
        flash_time: None,
    };

    let ghost = crate::cloud::monolith::color_for_level(&ctx, 0, 0, 0, BrightnessLevel::Ghost, 1.0);
    let dim = crate::cloud::monolith::color_for_level(&ctx, 0, 1, 0, BrightnessLevel::Dim, 1.0);
    let mid = crate::cloud::monolith::color_for_level(&ctx, 0, 2, 0, BrightnessLevel::Mid, 1.0);
    let hot = crate::cloud::monolith::color_for_level(&ctx, 0, 3, 0, BrightnessLevel::Hot, 1.0);
    let core = crate::cloud::monolith::color_for_level(&ctx, 0, 4, 0, BrightnessLevel::Core, 1.0);

    assert_eq!(
        ghost, dim,
        "ghost and dim should be equal -- both use first_visible"
    );
    assert_ne!(mid, ghost, "mid should differ from ghost/dim");
    assert_ne!(hot, mid, "hot should differ from mid");
    assert_ne!(core, hot, "core should differ from hot");
    if let Color::Rgb { r, g, b } = core.unwrap() {
        assert!(
            r > 200 || g > 200 || b > 200,
            "core should have a bright bloom"
        );
    }
}

#[test]
fn monolith_background_muddy_residue_guard() {
    let colors: Vec<Color> = (0..=7)
        .map(|i| Color::Rgb {
            r: 0,
            g: i * 36,
            b: 0,
        })
        .collect();
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&colors, empty, empty, empty];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 10,
        full_width: false,
        shading_distance: false,
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        color_mode: ColorMode::TrueColor,
        bold_mode: BoldMode::Off,
        glitchy: false,
        last_glitch_time: std::time::Instant::now(),
        next_glitch_time: std::time::Instant::now(),
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0'],
        previous_char_pool: &[],
        charset_wave_line: None,
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_col: u16::MAX,
        flash_line: u16::MAX,
        flash_time: None,
    };

    let ghost_idx =
        crate::cloud::monolith::color_for_level(&ctx, 0, 0, 0, BrightnessLevel::Ghost, 1.0);
    let dim_idx = crate::cloud::monolith::color_for_level(&ctx, 0, 1, 0, BrightnessLevel::Dim, 1.0);
    let mid_idx = crate::cloud::monolith::color_for_level(&ctx, 0, 2, 0, BrightnessLevel::Mid, 1.0);

    let ghost_rgb = ghost_idx.unwrap();
    let dim_rgb = dim_idx.unwrap();
    let mid_rgb = mid_idx.unwrap();

    if let Color::Rgb {
        r: gr,
        g: gg,
        b: gb,
    } = ghost_rgb
    {
        let sum = gr as u32 + gg as u32 + gb as u32;
        assert!(
            sum < 80,
            "ghost cell should be very dim on black bg (got sum={})",
            sum
        );
    }
    if let Color::Rgb {
        r: gr,
        g: gg,
        b: gb,
    } = dim_rgb
    {
        let sum = gr as u32 + gg as u32 + gb as u32;
        assert!(
            sum < 80,
            "dim cell should be very dim on black bg (got sum={})",
            sum
        );
    }
    if let Color::Rgb {
        r: mr,
        g: mg,
        b: mb,
    } = mid_rgb
    {
        let mid_sum = mr as u32 + mg as u32 + mb as u32;
        let ghost_sum = if let Color::Rgb {
            r: gr,
            g: gg,
            b: gb,
        } = ghost_rgb
        {
            gr as u32 + gg as u32 + gb as u32
        } else {
            0
        };
        assert!(
            mid_sum > ghost_sum,
            "mid should be brighter than ghost (mid_sum={} vs ghost_sum={})",
            mid_sum,
            ghost_sum
        );
    }
}

// ==========================================================================
// v4.5.0 Phase 3 -- Monolith Depth Regression Lab
// ==========================================================================

#[test]
fn depth_lab_monolith_sparse_lane_density_bounded_per_column() {
    let mut cloud = make_monolith_cloud(80, 30);
    let mut frame = Frame::new(80, 30, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    let cols = frame.width;
    let lines = frame.height;
    let mut max_col_density = 0usize;
    let mut total_cells = 0usize;
    let mut total_visible = 0usize;

    for col in 0..cols {
        let mut col_visible = 0usize;
        for line in 0..lines {
            total_cells += 1;
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                col_visible += 1;
                total_visible += 1;
            }
        }
        max_col_density = max_col_density.max(col_visible);
    }

    let overall_ratio = total_visible as f32 / total_cells as f32;
    assert!(
        overall_ratio < 0.35,
        "depth lab: monolith overall density must stay sparse (got {:.1}%)",
        overall_ratio * 100.0
    );
    let max_ratio = max_col_density as f32 / lines as f32;
    assert!(
        max_ratio < 0.60,
        "depth lab: no single column should exceed 60% (got {:.1}%)",
        max_ratio * 100.0
    );
}

#[test]
fn depth_lab_monolith_empty_space_ratio_above_min_threshold() {
    let mut cloud = make_monolith_cloud(96, 32);
    let mut frame = Frame::new(96, 32, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 200, 16);

    let mut blank_count = 0usize;
    let total = (frame.width as usize) * (frame.height as usize);
    for line in 0..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch == ' ' && cell.fg.is_none() {
                blank_count += 1;
            }
        }
    }
    let blank_ratio = blank_count as f32 / total as f32;
    assert!(
        blank_ratio > 0.50,
        "depth lab: monolith empty-space ratio must stay above 50% (got {:.1}%)",
        blank_ratio * 100.0
    );
}

#[test]
fn depth_lab_monolith_no_full_height_continuous_wall() {
    let mut cloud = make_monolith_cloud(64, 28);
    let mut frame = Frame::new(64, 28, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 200, 16);

    let mut max_consecutive = 0usize;
    for col in 0..frame.width {
        let mut current_run = 0usize;
        for line in 0..frame.height {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                current_run += 1;
                max_consecutive = max_consecutive.max(current_run);
            } else {
                current_run = 0;
            }
        }
    }

    let threshold = (frame.height as f32 * 0.70) as usize;
    assert!(
        max_consecutive < threshold,
        "depth lab: no column should have >70% consecutive fill (got {}/{})",
        max_consecutive,
        frame.height
    );
}

#[test]
fn depth_lab_monolith_bottom_residue_bounded_extended_rain() {
    let mut cloud = make_monolith_cloud(80, 28);
    cloud.set_droplet_density(3.0);
    cloud.set_chars_per_sec(25.0);
    cloud.reset(80, 28);
    cloud.clear_redraw_flags_for_test();
    let mut frame = Frame::new(80, 28, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 500, 16);
    let bottom_rows = 5u16;
    let bottom_start = frame.height.saturating_sub(bottom_rows);
    let mut visible = 0usize;
    let total = (frame.width as usize) * (bottom_rows as usize);
    for line in bottom_start..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                visible += 1;
            }
        }
    }
    let ratio = visible as f32 / total as f32;
    assert!(
        ratio < 0.50,
        "depth lab: bottom {} rows after 500 frames must stay < 50% (got {:.1}%)",
        bottom_rows,
        ratio * 100.0
    );
}
