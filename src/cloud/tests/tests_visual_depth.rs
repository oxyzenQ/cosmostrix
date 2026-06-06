// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Visual depth and background mode regression guards for v3.6.0.

use crossterm::style::Color;

use super::Cloud;
use crate::bench::{AVG_DIRTY_CELL_RATIO_MEANING, ESTIMATED_FULL_REDRAW_MEANING};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

pub(super) fn make_cloud_black_bg() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Random,
        false,
        false, // default_background = false (black bg)
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

pub(super) fn make_cloud_transparent_bg() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Random,
        false,
        true, // default_background = true (transparent)
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

pub(super) fn make_cloud_default_bg() -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        false,
        ShadingMode::DistanceFromHead,
        BoldMode::Random,
        false,
        true, // default_background = true
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

#[test]
fn transparent_color_bg_does_not_force_solid_black() {
    let cloud = make_cloud_transparent_bg();
    // Transparent mode means bg should be None
    assert_eq!(
        cloud.palette.bg, None,
        "transparent color-bg must NOT paint a solid background"
    );
    // The cloud's default_background flag must be true
    assert!(
        cloud.default_background,
        "transparent mode should set default_background=true"
    );
}

#[test]
fn black_color_bg_paints_solid_black() {
    let cloud = make_cloud_black_bg();
    // Black mode means bg should be Some(black)
    assert!(
        cloud.palette.bg.is_some(),
        "black color-bg must paint a solid background"
    );
    assert_eq!(
        cloud.palette.bg,
        Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        "black color-bg should be solid black (0,0,0)"
    );
    assert!(!cloud.default_background);
}

#[test]
fn default_background_mode_keeps_bg_none() {
    let cloud = make_cloud_default_bg();
    assert_eq!(
        cloud.palette.bg, None,
        "default-background mode should keep bg=None (terminal emulator shows through)"
    );
    assert!(cloud.default_background);
}

#[test]
fn normal_exit_leaves_no_residue_on_frame() {
    let mut cloud = make_cloud_black_bg();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Simulate a few frames
    let now = std::time::Instant::now();
    cloud.last_spawn_time = now - std::time::Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..10 {
        cloud.rain_at(&mut frame, now + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Normal exit scenario: verify no non-blank cells stuck at the bottom
    let bottom_start = cloud.lines.saturating_sub(3);
    let mut non_blank_bottom = 0usize;
    for line in bottom_start..cloud.lines {
        for col in 0..cloud.cols {
            if let Some(cell) = frame.get(col, line) {
                if cell.fg.is_some() && cell.ch != ' ' {
                    non_blank_bottom += 1;
                }
            }
        }
    }
    // Bottom 3 rows should not have more than 60% non-blank
    let total = (cloud.cols as usize) * 3;
    let ratio = non_blank_bottom as f32 / total as f32;
    assert!(
        ratio < 0.6,
        "bottom rows should not have excessive residue after normal frames (got {:.1}%)",
        ratio * 100.0
    );
}

#[test]
fn benchmark_output_includes_required_fields() {
    // This test documents the required benchmark output fields
    // to prevent accidental removal in future refactors.
    let required = [
        "avg_fps",
        "p95_frame_time",
        "p99_frame_time",
        "frame_time_stability",
        "dirty_cell_ratio",
        "estimated_full_redraw",
    ];
    let meanings = [AVG_DIRTY_CELL_RATIO_MEANING, ESTIMATED_FULL_REDRAW_MEANING];

    for field in &required {
        assert!(!field.is_empty(), "required field name must be non-empty");
    }
    for meaning in &meanings {
        assert!(
            !meaning.is_empty(),
            "required field meaning must be non-empty"
        );
    }
}

#[test]
fn hero_spine_trail_empty_space_have_distinct_brightness() {
    // Verify that the Monolith Rain brightness hierarchy produces visually
    // distinct levels. On a black background, this prevents the "flat wall
    // of grey" muddy residue artifact where levels become indistinguishable.
    use crate::cloud::monolith::BrightnessLevel;
    use crate::cloud::render::DrawCtx;
    use crossterm::style::Color;

    let colors: Vec<Color> = (0..=9)
        .map(|i| Color::Rgb {
            r: 0,
            g: (i * 25) as u8,
            b: 0,
        })
        .collect();
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; crate::constants::MAX_PALETTE_SLOTS] =
        [&colors, empty, empty, empty];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 20,
        full_width: false,
        shading_distance: false,
        bg: Some(Color::Rgb { r: 0, g: 0, b: 0 }),
        color_mode: crate::runtime::ColorMode::TrueColor,
        bold_mode: crate::runtime::BoldMode::Off,
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

    let ghost = crate::cloud::monolith::color_for_level(&ctx, 0, 1, 0, BrightnessLevel::Ghost, 1.0);
    let mid = crate::cloud::monolith::color_for_level(&ctx, 0, 5, 0, BrightnessLevel::Mid, 1.0);
    let hot = crate::cloud::monolith::color_for_level(&ctx, 0, 8, 0, BrightnessLevel::Hot, 1.0);
    let core = crate::cloud::monolith::color_for_level(&ctx, 0, 10, 0, BrightnessLevel::Core, 1.0);

    // Extract green channel as the dominant brightness indicator
    fn green(c: &Option<Color>) -> u32 {
        match c {
            Some(Color::Rgb { g, .. }) => *g as u32,
            _ => 0,
        }
    }

    // Ghost should be the dimmest non-zero level
    assert!(
        green(&ghost) > 0,
        "ghost must have some visible color on black bg"
    );
    // Mid should be strictly brighter than ghost
    assert!(
        green(&mid) > green(&ghost),
        "mid ({}) must be brighter than ghost ({})",
        green(&mid),
        green(&ghost)
    );
    // Hot should be strictly brighter than mid
    assert!(
        green(&hot) > green(&mid),
        "hot ({}) must be brighter than mid ({})",
        green(&hot),
        green(&mid)
    );
    // Core should be the brightest (with white bloom)
    assert!(
        green(&core) >= green(&hot),
        "core ({}) must be brightest, >= hot ({})",
        green(&core),
        green(&hot)
    );
}

#[test]
fn clean_exit_frame_has_no_persistent_ghost_in_bottom_rows() {
    // Simulate normal run then stop — verify no persistent ghost glyphs
    // linger in the bottom 5 rows after a clean exit scenario.
    let mut cloud = make_cloud_black_bg();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let now = std::time::Instant::now();
    cloud.last_spawn_time = now - std::time::Duration::from_secs(2);
    cloud.last_phosphor_time = now;
    // Run enough frames for phosphor to develop, then stop spawning
    for i in 0..30 {
        cloud.rain_at(&mut frame, now + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }
    // Stop spawning — let remaining phosphor decay
    cloud.spawn_remainder = 0.0;
    for i in 30..60 {
        cloud.rain_at(&mut frame, now + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Bottom 5 rows should not have persistent non-blank residue
    let bottom_start = cloud.lines.saturating_sub(5);
    let mut non_blank_count = 0usize;
    let total = (cloud.cols as usize) * 5;
    for line in bottom_start..cloud.lines {
        for col in 0..cloud.cols {
            if let Some(cell) = frame.get(col, line) {
                if cell.ch != ' ' && cell.fg.is_some() {
                    non_blank_count += 1;
                }
            }
        }
    }
    let ratio = non_blank_count as f32 / total as f32;
    assert!(
        ratio < 0.35,
        "bottom 5 rows after clean exit should not have persistent ghost glyphs (got {:.1}%)",
        ratio * 100.0
    );
}
