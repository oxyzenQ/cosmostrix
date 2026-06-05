// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith Rain tests.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::charset::{build_chars, charset_from_str};
use crate::cloud::monolith::DrawnCellKind;
use crate::cloud::Cloud;
use crate::constants::CHARSET_TRANSITION_DURATION_MS;
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};
use std::collections::BTreeSet;

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

fn visible_char_signature(frame: &Frame) -> BTreeSet<char> {
    visible_chars(frame).into_iter().collect()
}

fn average_head_delta(before: &[f32], after: &[f32]) -> f32 {
    let len = before.len().min(after.len()).max(1);
    before
        .iter()
        .zip(after.iter())
        .take(len)
        .map(|(a, b)| b - a)
        .sum::<f32>()
        / len as f32
}

fn segment_draw_count(cloud: &Cloud) -> usize {
    cloud
        .monolith_rain
        .drawn_cells_for_test()
        .iter()
        .filter(|cell| matches!(cell.kind, DrawnCellKind::Segment))
        .count()
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
fn monolith_high_speed_top_cells_clear_after_bounded_frames() {
    let mut cloud = make_monolith_cloud(96, 32);
    cloud.set_chars_per_sec(999.0);
    assert_eq!(
        cloud.chars_per_sec,
        crate::constants::MONOLITH_EFFECTIVE_SPEED_MAX
    );
    let mut frame = Frame::new(96, 32, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);

    disable_monolith_spawning(&mut cloud);
    for idx in 1..=4 {
        frame.clear_dirty();
        cloud.rain_at(&mut frame, Instant::now() + Duration::from_millis(idx * 16));
    }

    let top_rows = 4u16;
    let mut visible = 0usize;
    for line in 0..top_rows {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                visible += 1;
            }
        }
    }
    assert_eq!(visible, 0, "high-speed monolith top residue should clear");
}

#[test]
fn active_monolith_streams_update_speed_without_respawn() {
    let mut cloud = make_monolith_cloud(96, 36);
    let mut frame = Frame::new(96, 36, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    let initial = cloud.monolith_rain.active_heads_for_test();
    assert!(initial.len() > 4);

    cloud.set_chars_per_sec(1.0);
    frame.clear_dirty();
    cloud.rain_at(&mut frame, start + Duration::from_millis(100));
    let slow = cloud.monolith_rain.active_heads_for_test();
    assert_eq!(slow.len(), initial.len());

    cloud.set_chars_per_sec(100.0);
    frame.clear_dirty();
    cloud.rain_at(&mut frame, start + Duration::from_millis(200));
    let fast = cloud.monolith_rain.active_heads_for_test();
    assert_eq!(fast.len(), slow.len());

    let slow_delta = average_head_delta(&initial, &slow);
    let fast_delta = average_head_delta(&slow, &fast);
    assert!(
        fast_delta > slow_delta * 40.0,
        "active streams should use the new global speed immediately (slow={slow_delta}, fast={fast_delta})"
    );
}

#[test]
fn monolith_resize_reset_clears_draw_caches_and_requests_semantic_sync() {
    let mut cloud = make_monolith_cloud(64, 24);
    let mut frame = Frame::new(64, 24, cloud.palette.bg);
    let start = Instant::now();

    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.rain_at(&mut frame, start);
    assert!(cloud.monolith_rain.draw_history_count_for_test() > 0);

    cloud.reset(120, 40);

    assert_eq!(cloud.monolith_rain.draw_history_count_for_test(), 0);
    assert!(cloud.is_semantic_invalidate());
    assert!(cloud.phosphor.iter().all(|&energy| energy == 0));
    assert!(cloud.phosphor_base_ch.iter().all(|&ch| ch == '\0'));
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
fn monolith_charset_cycles_produce_multiple_glyph_styles() {
    let mut cloud = make_monolith_cloud(80, 24);
    let mut frame = Frame::new(80, 24, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    cloud.rain_at(&mut frame, start);
    let binary = visible_char_signature(&frame);

    let styles = [
        vec!['.', '-', '=', '+'],
        vec!['A', 'B', 'C', 'D'],
        vec!['▀', '▄', '▌', '▐'],
    ];
    let mut signatures = vec![binary];
    for (idx, chars) in styles.into_iter().enumerate() {
        frame.clear_dirty();
        cloud.transition_chars(chars);
        cloud.charset_transition_start =
            Some(start - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
        cloud.rain_at(
            &mut frame,
            start + Duration::from_millis(16 + idx as u64 * 16),
        );
        signatures.push(visible_char_signature(&frame));
    }
    signatures.sort();
    signatures.dedup();

    assert!(
        signatures.len() >= 3,
        "monolith charset cycling should produce at least three visible glyph styles"
    );
}

#[test]
fn monolith_charset_presets_drive_distinct_segment_glyphs() {
    let presets = [
        "binary",
        "matrix",
        "katakana",
        "code",
        "hacker",
        "cyberpunk",
    ];
    let mut signatures = Vec::new();

    for preset in presets {
        let charset = charset_from_str(preset, false).expect("known charset");
        let mut cloud = make_monolith_cloud(80, 24);
        cloud.init_chars(build_chars(charset, &[], false));
        cloud.reset(80, 24);
        cloud.clear_redraw_flags_for_test();

        let mut frame = Frame::new(80, 24, cloud.palette.bg);
        run_frames(&mut cloud, &mut frame, 8, 16);
        let signature = visible_char_signature(&frame);
        assert!(
            !signature.is_empty(),
            "{preset} should render monolith glyphs"
        );

        if preset == "binary" {
            assert!(signature.iter().any(|ch| matches!(ch, '0' | '1')));
        }
        if preset == "katakana" {
            assert!(signature.iter().any(|ch| ('ｦ'..='ﾝ').contains(ch)));
        }
        signatures.push(signature);
    }

    signatures.sort();
    signatures.dedup();
    assert!(
        signatures.len() >= 5,
        "monolith should reflect real charset presets, got {} distinct styles",
        signatures.len()
    );
}

#[test]
fn monolith_size_changes_segment_coverage() {
    let mut small = make_monolith_cloud(80, 24);
    small.set_monolith_size(MonolithSize::Small);
    small.reset(80, 24);
    small.clear_redraw_flags_for_test();
    let mut small_frame = Frame::new(80, 24, small.palette.bg);
    run_frames(&mut small, &mut small_frame, 20, 16);

    let mut large = make_monolith_cloud(80, 24);
    large.set_monolith_size(MonolithSize::Large);
    large.reset(80, 24);
    large.clear_redraw_flags_for_test();
    let mut large_frame = Frame::new(80, 24, large.palette.bg);
    run_frames(&mut large, &mut large_frame, 20, 16);

    let small_segments = segment_draw_count(&small);
    let large_segments = segment_draw_count(&large);
    assert!(
        large_segments > small_segments,
        "large monolith size should draw more segment cells than small (large={large_segments}, small={small_segments})"
    );
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
