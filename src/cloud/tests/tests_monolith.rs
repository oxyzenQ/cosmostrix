// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith Rain tests.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

fn make_monolith_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::BlackHole,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.75);
    cloud.set_chars_per_sec(10.0);
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

fn run_frames(cloud: &mut Cloud, frame: &mut Frame, frames: u32, step_ms: u64) {
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx as u64 * step_ms);
        cloud.rain_at(frame, now);
        frame.clear_dirty();
    }
}

fn visible_cell_count(frame: &Frame) -> usize {
    let mut count = 0usize;
    for line in 0..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                count += 1;
            }
        }
    }
    count
}

fn seed_stale_phosphor(cloud: &mut Cloud) {
    cloud.phosphor.fill(180);
    cloud
        .phosphor_base_fg
        .fill(Some(crossterm::style::Color::Grey));
    cloud.phosphor_base_ch.fill('x');
}

fn seeded_residue_count(cloud: &Cloud) -> usize {
    cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch == 'x')
        .count()
}

fn disable_monolith_spawning(cloud: &mut Cloud) {
    cloud.resume_blend = 0.0;
    cloud.resume_start = None;
    cloud.spawn_remainder = 0.0;
    cloud.monolith_rain.deactivate_all_for_test();
}

#[test]
fn monolith_rain_state_initializes_without_allocation_panic() {
    let cloud = make_monolith_cloud(120, 40);

    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.droplet_count(), 0);
    assert_eq!(cloud.active_droplet_count(), 0);
}

#[test]
fn monolith_rain_produces_dirty_frames() {
    let mut cloud = make_monolith_cloud(80, 24);
    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    frame.clear_dirty();

    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain(&mut frame);

    assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
}

#[test]
fn monolith_previous_drawn_cells_are_cleared_when_stream_moves() {
    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    assert!(visible_cell_count(&frame) > 0);
    assert!(cloud.monolith_rain.draw_history_count_for_test() > 0);

    frame.clear_dirty();
    disable_monolith_spawning(&mut cloud);
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));

    assert_eq!(
        visible_cell_count(&frame),
        0,
        "inactive monolith draw history should be blanked deterministically"
    );
}

#[test]
fn monolith_inactive_spine_cells_do_not_persist_beyond_bounded_frames() {
    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    assert!(visible_cell_count(&frame) > 0);

    disable_monolith_spawning(&mut cloud);
    for idx in 1..=4 {
        frame.clear_dirty();
        cloud.rain_at(&mut frame, start + Duration::from_millis(idx * 16));
    }

    assert_eq!(
        visible_cell_count(&frame),
        0,
        "inactive monolith spine residue should not survive bounded cleanup frames"
    );
}

#[test]
fn monolith_semantic_invalidation_clears_stale_residue() {
    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    seed_stale_phosphor(&mut cloud);
    assert!(seeded_residue_count(&cloud) > 0);

    cloud.set_shading_mode(ShadingMode::DistanceFromHead);
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));

    assert_eq!(seeded_residue_count(&cloud), 0);
}

#[test]
fn monolith_color_and_charset_transitions_clear_stale_residue() {
    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    seed_stale_phosphor(&mut cloud);
    cloud.set_color_scheme(ColorScheme::DeepSpace);
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));
    assert_eq!(seeded_residue_count(&cloud), 0);

    seed_stale_phosphor(&mut cloud);
    cloud.transition_chars(vec!['a', 'b']);
    cloud.rain_at(&mut frame, start + Duration::from_millis(32));
    assert_eq!(seeded_residue_count(&cloud), 0);
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

#[test]
fn monolith_bottom_residue_stays_bounded() {
    let mut cloud = make_monolith_cloud(80, 24);
    cloud.set_droplet_density(5.0);
    cloud.set_chars_per_sec(30.0);
    cloud.reset(80, 24);
    cloud.clear_redraw_flags_for_test();

    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 300, 16);

    let bottom_rows = 4u16;
    let bottom_start = frame.height.saturating_sub(bottom_rows);
    let mut visible = 0usize;
    let mut total = 0usize;
    for line in bottom_start..frame.height {
        for col in 0..frame.width {
            total += 1;
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                visible += 1;
            }
        }
    }
    let ratio = visible as f32 / total as f32;

    assert!(
        ratio < 0.45,
        "monolith bottom rows should stay bounded, got {:.1}% visible",
        ratio * 100.0
    );
}
