// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Monolith transition tests: resize reset, semantic invalidation,
//! previous cells clearing when stream moves, spine cell transitions.

use std::time::{Duration, Instant};

use super::{
    disable_monolith_spawning, make_monolith_cloud, visible_cell_count, DrawnCellKind, Frame,
};

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
