// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith Rain tests.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::cloud::monolith::DrawnCellKind;
use crate::cloud::Cloud;
use crate::constants::CHARSET_TRANSITION_DURATION_MS;
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

fn visible_chars(frame: &Frame) -> Vec<char> {
    let mut chars = Vec::new();
    for line in 0..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' {
                chars.push(cell.ch);
            }
        }
    }
    chars
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

fn phosphor_index(cloud: &Cloud, col: u16, line: u16) -> usize {
    col as usize * cloud.lines as usize + line as usize
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
fn monolith_previous_spine_cells_are_cleared_when_stream_moves() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    let spine_cells: Vec<(u16, u16)> = cloud
        .monolith_rain
        .drawn_cells_for_test()
        .iter()
        .filter(|cell| matches!(cell.kind, DrawnCellKind::Spine))
        .map(|cell| (cell.col, cell.line))
        .collect();
    assert!(!spine_cells.is_empty());

    disable_monolith_spawning(&mut cloud);
    frame.clear_dirty();
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));

    for (col, line) in spine_cells {
        let cell = frame.get(col, line).expect("cell in bounds");
        assert_eq!(cell.ch, ' ');
        assert!(cell.fg.is_none());
    }
}

#[test]
fn monolith_spine_cells_do_not_retain_long_lived_phosphor_metadata() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);

    let spine_cells: Vec<(u16, u16)> = cloud
        .monolith_rain
        .drawn_cells_for_test()
        .iter()
        .filter(|cell| matches!(cell.kind, DrawnCellKind::Spine))
        .map(|cell| (cell.col, cell.line))
        .collect();
    assert!(!spine_cells.is_empty());

    for (col, line) in spine_cells {
        let pidx = phosphor_index(&cloud, col, line);
        assert_eq!(cloud.phosphor[pidx], 0);
        assert_eq!(cloud.phosphor_base_fg[pidx], None);
        assert_eq!(cloud.phosphor_base_ch[pidx], '\0');
    }
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
    assert!(cloud.monolith_rain.draw_history_count_for_test() > 0);

    cloud.set_shading_mode(ShadingMode::DistanceFromHead);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
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
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));
    assert_eq!(seeded_residue_count(&cloud), 0);

    seed_stale_phosphor(&mut cloud);
    cloud.transition_chars(vec!['a', 'b']);
    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    cloud.rain_at(&mut frame, start + Duration::from_millis(32));
    assert_eq!(seeded_residue_count(&cloud), 0);
}

#[test]
fn monolith_charset_transition_changes_segment_glyph_style() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    let before = visible_chars(&frame);
    assert!(before.iter().any(|&ch| ch == '0' || ch == '1'));

    frame.clear_dirty();
    cloud.transition_chars(vec!['A', 'B', 'C', 'D']);
    cloud.charset_transition_start =
        Some(start - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
    cloud.rain_at(&mut frame, start + Duration::from_millis(16));
    let after = visible_chars(&frame);

    assert!(
        !after.iter().any(|&ch| ch == '0' || ch == '1'),
        "code-like charset should remove binary segment accents"
    );
    assert_ne!(before, after);
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
