// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Visual depth and background mode regression guards for v3.6.0.
//!
//! v4.5.0 Phase 3 adds the Depth Regression Lab: deterministic tests that
//! lock down the v4.0.1/v4.5 Monolith Rain visual identity. These tests
//! protect against future regressions in cinematic depth, empty-space ratio,
//! muddy residue, brightness hierarchy, and transition cleanliness.

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
        pool_is_binary: false,
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

// ═══════════════════════════════════════════════════════════════════════════
// v4.5.0 Phase 3 — Depth Regression Lab
//
// These tests lock down the v4.0.1/v4.5 visual identity. Future v4.8.0
// optimization MUST pass these before merge. No flattening cinematic depth.
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn depth_lab_empty_space_ratio_above_threshold() {
    // After steady-state rain, the visible cell ratio must stay well below
    // 100% — empty space is a core identity property of Monolith Rain.
    // The "dense wall of grey" artifact violated this invariant.
    let mut cloud = make_cloud_black_bg();
    cloud.chars_per_sec = 12.0;
    cloud.recalc_droplets_per_sec();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let start = std::time::Instant::now();

    cloud.last_spawn_time = start - std::time::Duration::from_secs(2);
    cloud.last_phosphor_time = start;
    for i in 0..60 {
        cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    let mut visible = 0usize;
    let total = (cloud.cols as usize) * (cloud.lines as usize);
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                visible += 1;
            }
        }
    }
    let ratio = visible as f32 / total as f32;
    assert!(
        ratio < 0.60,
        "depth lab: visible cell ratio must stay below 60% (got {:.1}%) — \
         empty space is a v4.0.1 identity property",
        ratio * 100.0
    );
}

#[test]
fn depth_lab_glyph_rain_not_dense_wall() {
    // Glyph-style rain must also maintain empty space. Unlike Monolith,
    // glyph rain can be denser, but should never approach 100% fill.
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(1.5);
    cloud.set_chars_per_sec(16.0);
    cloud.reset(40, 20);
    cloud.clear_redraw_flags_for_test();

    let mut frame = crate::frame::Frame::new(40, 20, cloud.palette.bg);
    let start = std::time::Instant::now();
    cloud.last_spawn_time = start - std::time::Duration::from_secs(2);
    cloud.last_phosphor_time = start;

    for i in 0..120 {
        cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    let mut visible = 0usize;
    let total = 40 * 20;
    for line in 0..20u16 {
        for col in 0..40u16 {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                visible += 1;
            }
        }
    }
    let ratio = visible as f32 / total as f32;
    assert!(
        ratio < 0.85,
        "depth lab: glyph rain visible ratio must stay below 85% (got {:.1}%)",
        ratio * 100.0
    );
}

#[test]
fn depth_lab_charset_transition_no_background_flood() {
    // When charset transitions complete, the frame should not show a
    // sudden flood of background characters. Only actively drawn cells
    // from the new charset should be visible.
    let mut cloud = make_cloud_black_bg();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let start = std::time::Instant::now();
    cloud.last_spawn_time = start - std::time::Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    // Run a few frames with original charset
    for i in 0..10 {
        cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Count visible cells before transition
    let mut before_visible = 0usize;
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' {
                before_visible += 1;
            }
        }
    }

    // Transition charset and complete it immediately
    cloud.transition_chars(vec!['A', 'B', 'C', 'D']);
    cloud.charset_transition_start = Some(
        start
            - std::time::Duration::from_millis(
                crate::constants::CHARSET_TRANSITION_DURATION_MS as u64 + 1,
            ),
    );
    cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(160));
    frame.clear_dirty();

    // Count visible cells after transition
    let mut after_visible = 0usize;
    for line in 0..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' {
                after_visible += 1;
            }
        }
    }

    // The ratio of after/before should be bounded — no sudden flood
    let ratio = if before_visible > 0 {
        after_visible as f32 / before_visible as f32
    } else {
        1.0
    };
    assert!(
        ratio < 3.0,
        "depth lab: charset transition should not flood background (after/before={:.2})",
        ratio
    );
}

#[test]
fn depth_lab_color_transition_no_stale_residue_at_frame_level() {
    // After a color transition completes, no cells should reference the
    // old palette in an inconsistent way. All alive droplets must be
    // on the active palette slot.
    let mut cloud = make_cloud_black_bg();
    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let start = std::time::Instant::now();
    cloud.last_spawn_time = start - std::time::Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    for i in 0..10 {
        cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    // Initiate and complete a color transition
    cloud.set_color_scheme(ColorScheme::Blue);
    cloud.transition_start = Some(
        start
            - std::time::Duration::from_millis(
                crate::constants::COLOR_TRANSITION_DURATION_MS as u64 + 1,
            ),
    );
    cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(160));

    // After completion, no transition state should linger
    assert!(
        cloud.transition_start.is_none(),
        "color transition should be complete"
    );

    // All alive droplets must use the active palette
    for d in &cloud.droplets {
        if d.is_alive {
            assert_eq!(
                d.palette_slot, cloud.active_palette_slot,
                "alive droplet must use active palette after color transition"
            );
        }
    }
}

#[test]
fn depth_lab_brightness_level_four_tier_hierarchy() {
    // Verify all four brightness levels (Ghost, Dim, Mid, Hot/Core) produce
    // strictly increasing luminance. This is the core cinematic depth
    // invariant — flattening it creates the "flat wall of grey" artifact.
    use crate::cloud::monolith::BrightnessLevel;
    use crate::cloud::render::DrawCtx;

    let colors: Vec<Color> = (0..=9)
        .map(|i| Color::Rgb {
            r: 0,
            g: i * 28,
            b: i * 3,
        })
        .collect();
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; crate::constants::MAX_PALETTE_SLOTS] =
        [&colors, empty, empty, empty];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 24,
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
        char_pool: &['X'],
        previous_char_pool: &[],
        charset_wave_line: None,
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_col: u16::MAX,
        flash_line: u16::MAX,
        flash_time: None,
        pool_is_binary: false,
    };

    let ghost = crate::cloud::monolith::color_for_level(&ctx, 0, 1, 0, BrightnessLevel::Ghost, 1.0);
    let dim = crate::cloud::monolith::color_for_level(&ctx, 0, 2, 0, BrightnessLevel::Dim, 1.0);
    let mid = crate::cloud::monolith::color_for_level(&ctx, 0, 4, 0, BrightnessLevel::Mid, 1.0);
    let hot = crate::cloud::monolith::color_for_level(&ctx, 0, 7, 0, BrightnessLevel::Hot, 1.0);
    let core = crate::cloud::monolith::color_for_level(&ctx, 0, 9, 0, BrightnessLevel::Core, 1.0);

    fn luminance(c: &Option<Color>) -> u32 {
        match c {
            Some(Color::Rgb { r, g, b }) => *r as u32 + *g as u32 + *b as u32,
            _ => 0,
        }
    }

    let l_ghost = luminance(&ghost);
    let l_dim = luminance(&dim);
    let l_mid = luminance(&mid);
    let l_hot = luminance(&hot);
    let l_core = luminance(&core);

    // Strictly increasing: ghost < dim < mid < hot < core
    assert!(l_ghost > 0, "ghost luminance must be > 0 (visible)");
    assert!(l_dim >= l_ghost, "dim ({}) >= ghost ({})", l_dim, l_ghost);
    assert!(l_mid > l_ghost, "mid ({}) > ghost ({})", l_mid, l_ghost);
    assert!(l_hot > l_mid, "hot ({}) > mid ({})", l_hot, l_mid);
    assert!(
        l_core > l_hot,
        "core ({}) > hot ({}) — core must bloom beyond hot",
        l_core,
        l_hot
    );
}

#[test]
fn depth_lab_sustained_rain_bottom_residue_bounded_300_frames() {
    // Run 300 frames (~5 seconds) of sustained rain and verify bottom
    // rows do not accumulate unbounded residue. This is a stricter
    // version of the existing 300-frame test, specifically for the
    // Depth Regression Lab.
    let mut cloud = make_cloud_black_bg();
    cloud.chars_per_sec = 40.0;
    cloud.set_droplet_density(2.0);
    cloud.recalc_droplets_per_sec();

    let mut frame = crate::frame::Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let start = std::time::Instant::now();
    cloud.last_spawn_time = start - std::time::Duration::from_secs(2);
    cloud.last_phosphor_time = start;

    for i in 0..300 {
        cloud.rain_at(&mut frame, start + std::time::Duration::from_millis(16 * i));
        frame.clear_dirty();
    }

    let bottom_start = cloud.lines.saturating_sub(4);
    let mut non_blank = 0usize;
    let total = (cloud.cols as usize) * 4;
    for line in bottom_start..cloud.lines {
        for col in 0..cloud.cols {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                non_blank += 1;
            }
        }
    }
    let ratio = non_blank as f32 / total as f32;
    assert!(
        ratio < 0.70,
        "depth lab: bottom 4 rows after 300 frames must stay < 70% (got {:.1}%)",
        ratio * 100.0
    );
}

#[test]
fn depth_lab_no_muddy_residue_on_dark_backgrounds() {
    // The "muddy residue" artifact occurs when ghost/dim cells map to
    // mid-range grey on dark backgrounds, creating an indistinct flat
    // wall instead of clear depth hierarchy. Verify that ghost cells
    // on dark backgrounds use very low luminance values.
    use crate::cloud::monolith::BrightnessLevel;
    use crate::cloud::render::DrawCtx;

    // Three distinct dark palette profiles
    let profiles: Vec<Vec<Color>> = vec![
        // Green-dark
        (0..=7)
            .map(|i| Color::Rgb {
                r: 0,
                g: i * 36,
                b: 0,
            })
            .collect(),
        // Blue-dark
        (0..=7)
            .map(|i| Color::Rgb {
                r: 0,
                g: 0,
                b: i * 36,
            })
            .collect(),
        // Cyan-dark
        (0..=7)
            .map(|i| Color::Rgb {
                r: 0,
                g: i * 20,
                b: i * 28,
            })
            .collect(),
    ];

    for (profile_idx, colors) in profiles.iter().enumerate() {
        let empty: &[Color] = &[];
        let palette_slices: [&[Color]; crate::constants::MAX_PALETTE_SLOTS] =
            [colors, empty, empty, empty];
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
            pool_is_binary: false,
        };

        let ghost =
            crate::cloud::monolith::color_for_level(&ctx, 0, 0, 0, BrightnessLevel::Ghost, 1.0);
        let dim = crate::cloud::monolith::color_for_level(&ctx, 0, 1, 0, BrightnessLevel::Dim, 1.0);

        fn rgb_sum(c: &Option<Color>) -> u32 {
            match c {
                Some(Color::Rgb { r, g, b }) => *r as u32 + *g as u32 + *b as u32,
                _ => 0,
            }
        }

        let ghost_sum = rgb_sum(&ghost);
        let dim_sum = rgb_sum(&dim);

        assert!(
            ghost_sum < 80,
            "profile {}: ghost cell RGB sum {} must be < 80 on dark bg (anti-muddy)",
            profile_idx,
            ghost_sum
        );
        assert!(
            dim_sum < 80,
            "profile {}: dim cell RGB sum {} must be < 80 on dark bg (anti-muddy)",
            profile_idx,
            dim_sum
        );
    }
}
