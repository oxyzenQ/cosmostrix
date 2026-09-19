// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Monolith residue tests: bottom residue, top clear, stale spine/phosphor
//! cleanup, muddy residue guards, semantic invalidation residue.

use std::time::{Duration, Instant};

use super::{
    disable_monolith_spawning, make_monolith_cloud, phosphor_index, run_frames,
    seed_stale_phosphor, seeded_residue_count, visible_cell_count, ColorScheme, Frame, ShadingMode,
};

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
fn monolith_spine_cells_do_not_retain_long_lived_phosphor_metadata() {
    use super::DrawnCellKind;
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
    use crate::constants::MONOLITH_EFFECTIVE_SPEED_MAX;
    let mut cloud = make_monolith_cloud(96, 32);
    cloud.set_chars_per_sec(999.0);
    assert_eq!(cloud.chars_per_sec, MONOLITH_EFFECTIVE_SPEED_MAX);
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
    cloud.set_color_scheme(ColorScheme::Cosmos);
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

/// NIGHT-hunt-38 (in-process oracle): a visible frame cell must always
/// be OWNED — either by the monolith's current draw (previous_cells
/// after the frame's swap) or by live phosphor energy (a decaying
/// ghost). A glyph cell with neither is an ORPHAN: unbacked content
/// that persists until the style's own motion happens to pass over it
/// (the PTY force-repaint classifier's erased-cell finding, first
/// observed by the NIGHT-hunt-38 one-off classifier harness — since
/// removed in the NIGHT-cleanup-1 dead-script sweep; this oracle is
/// the in-tree lock for that finding).
///
/// The oracle runs every 60th frame (~1 s at 60 fps) for ~20 s — the
/// horizon over which the PTY probe observed 12+ second frozen cells.
#[test]
fn hunt38_monolith_oracle_no_orphan_cells_persist() {
    let (cols, lines) = (140u16, 46u16);
    let lines_us = lines as usize;
    let mut cloud = make_monolith_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;

    let step_ms = 16;
    let frames = 1200u64;
    let mut orphans: Vec<(u16, u16)> = Vec::new();
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx * step_ms);
        cloud.rain_at(&mut frame, now);
        if idx % 60 == 0 {
            let drawn: std::collections::BTreeSet<(u16, u16)> = cloud
                .monolith_rain
                .drawn_cells_for_test()
                .iter()
                .map(|c| (c.col, c.line))
                .collect();
            for line in 0..lines {
                for col in 0..cols {
                    let cell = frame.get(col, line).expect("cell in bounds");
                    if cell.ch == ' ' && cell.fg.is_none() {
                        continue;
                    }
                    if drawn.contains(&(col, line)) {
                        continue;
                    }
                    let pidx = col as usize * lines_us + line as usize;
                    if cloud.phosphor[pidx] > 0 {
                        continue;
                    }
                    orphans.push((col, line));
                }
            }
        }
        frame.clear_dirty();
    }
    assert!(
        orphans.is_empty(),
        "monolith orphan cells (visible glyph, not drawn, no phosphor energy): {:?}",
        &orphans[..orphans.len().min(12)]
    );
}
