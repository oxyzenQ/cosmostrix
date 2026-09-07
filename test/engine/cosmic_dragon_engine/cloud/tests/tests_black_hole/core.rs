// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core black-hole-style behavior contracts (NIGHT-special-1):
//! scene resolution, dynamic geometry across viewport sizes, the empty
//! event-horizon core, radial band coverage, static-geometry stability
//! across frames, and the drawn-cell bounds contract.

use super::*;
use crate::cloud::type_rain::black_hole::black_hole::level_rank;

#[test]
fn black_hole_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("sorgonemous_intrascals")
        .expect("sorgonemous_intrascals scene exists");
    assert_eq!(s.config.rain_style, RainStyle::BlackHole);
    assert_eq!(s.config.color, Some("energy-zen"));
    assert_eq!(s.config.charset, Some("binary"));
    // Style dispatch sanity: the style helper families classify the
    // black hole as structured (not droplet family). Since stage 2 the
    // orbital ring spawns through the shared fractional accumulator
    // (family contract — the stage 3 glyph infall will reuse it).
    assert!(!RainStyle::BlackHole.is_droplet_family());
    assert!(RainStyle::BlackHole.uses_spawn_remainder());
    // Label roundtrip: the canonical CLI label parses back to the
    // variant and renders forward to the same string (the scene-custom
    // `rain` field depends on this contract).
    assert_eq!(
        RainStyle::from_label("black_hole"),
        Some(RainStyle::BlackHole)
    );
    assert_eq!(RainStyle::BlackHole.as_str(), "black_hole");
    assert!(RainStyle::valid_labels_hint().contains("black_hole"));
}

#[test]
fn black_hole_ball_draws_cells_at_every_viewport_size() {
    // Dynamic sizing contract: the ball must render a non-empty ring on
    // every terminal class from the 80x24 floor to a 200x60 wide screen.
    for (cols, lines) in [(80, 24), (105, 64), (120, 40), (200, 60), (400, 100)] {
        let mut cloud = make_black_hole_cloud(cols, lines);
        let mut frame = Frame::new(cols, lines, cloud.palette.bg);
        run_frames_to_steady(&mut cloud, &mut frame);
        let ring = cloud.black_hole_rain.ring_cells_for_test();
        assert!(!ring.is_empty(), "ball must draw cells at {cols}x{lines}");
        for cell in ring {
            assert!(cell.col < cols, "cell col out of bounds at {cols}x{lines}");
            assert!(
                cell.line < lines,
                "cell line out of bounds at {cols}x{lines}"
            );
        }
        // Every ring cell is also a drawn cell this frame (the ball
        // redraws its full annulus each pass — no orphan geometry),
        // and the stage-2 orbital ring adds its mote cells on top.
        let drawn = cloud.black_hole_rain.drawn_cells_for_test();
        assert!(
            drawn.len() >= ring.len(),
            "drawn cells must cover the ball at {cols}x{lines}"
        );
        assert!(
            cloud.black_hole_rain.active_count() >= ring.len(),
            "active count must cover the ball cells at {cols}x{lines}"
        );
    }
}

#[test]
fn black_hole_core_is_empty_and_ball_is_centered() {
    // The owner spec: center position, medium size, core black/empty.
    // Verify from the drawn geometry: no cell may sit inside the core
    // radius, and the annulus bounding box must be centered on the
    // viewport center (aspect-corrected: cols count double).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let ring = cloud.black_hole_rain.ring_cells_for_test();
    let unit = (cols as f32 / 4.0).min(lines as f32 / 2.0);
    let outer_r = unit * crate::constants::BLACK_HOLE_BALL_FRACTION;
    let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;
    // Integer center cell — the same discrete midpoint the renderer
    // scans around ((n - 1) / 2).
    let cx = ((cols - 1) / 2) as f32;
    let cy = ((lines - 1) / 2) as f32;

    // Medium size: the ball spans a visible fraction of the viewport.
    assert!(
        outer_r > 2.0,
        "ball outer radius must be visible (got {outer_r})"
    );

    let mut min_col = u16::MAX;
    let mut max_col = 0u16;
    let mut min_line = u16::MAX;
    let mut max_line = 0u16;
    for cell in ring {
        let dx = cell.col as f32 - cx;
        let dy = cell.line as f32 - cy;
        let dist = ((dx / 2.0).powi(2) + dy.powi(2)).sqrt();
        assert!(
            dist >= core_r - 0.5,
            "cell inside the event horizon at ({}, {}), dist {dist} < {core_r}",
            cell.col,
            cell.line
        );
        assert!(
            dist <= outer_r + 0.5,
            "cell outside the ball at ({}, {}), dist {dist} > {outer_r}",
            cell.col,
            cell.line
        );
        min_col = min_col.min(cell.col);
        max_col = max_col.max(cell.col);
        min_line = min_line.min(cell.line);
        max_line = max_line.max(cell.line);
    }

    // Centered: the annulus bounding box centers on the viewport center
    // within one cell of tolerance (discrete raster + half-cell offsets).
    let center_col = (min_col + max_col) as f32 / 2.0;
    let center_line = (min_line + max_line) as f32 / 2.0;
    assert!(
        (center_col - cx).abs() <= 1.0,
        "ball must be horizontally centered (got {center_col}, want {cx})"
    );
    assert!(
        (center_line - cy).abs() <= 1.0,
        "ball must be vertically centered (got {center_line}, want {cy})"
    );

    // Round silhouette: the bounding box reads ~2:1 in cell space
    // (cells are 1:2, so a screen circle is a 2:1 cell ellipse).
    let width = (max_col - min_col + 1) as f32;
    let height = (max_line - min_line + 1) as f32;
    assert!(
        (width / height - 2.0).abs() <= 0.9,
        "ball silhouette must be round on screen (cell aspect {}/{})",
        width,
        height
    );
}

#[test]
fn black_hole_radial_bands_all_present() {
    // The photon-ring gradient: Core band hugs the event horizon, Ghost
    // band fades at the outer rim — the inverted drain look. At 120x40
    // every band is at least one cell wide.
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let ranks: Vec<u8> = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .map(|c| level_rank(c.level))
        .collect();
    for expected in [0u8, 2u8, 3u8, 4u8] {
        assert!(
            ranks.contains(&expected),
            "radial band rank {expected} missing from the annulus"
        );
    }
    // The brightest band (Core, rank 4) must hug the INNER edge: at
    // least one Core cell must sit closer to the center than every
    // Ghost cell (rank 0) — the photon ring wraps the hole, not the rim.
    let cx = ((cols - 1) / 2) as f32;
    let cy = ((lines - 1) / 2) as f32;
    let dist = |col: u16, line: u16| ((col as f32 - cx) / 2.0).powi(2) + (line as f32 - cy).powi(2);
    let core_min = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .filter(|c| level_rank(c.level) == 4)
        .map(|c| dist(c.col, c.line))
        .fold(f32::MAX, f32::min);
    let ghost_min = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .filter(|c| level_rank(c.level) == 0)
        .map(|c| dist(c.col, c.line))
        .fold(f32::MAX, f32::min);
    assert!(
        core_min < ghost_min,
        "Core band must sit inside the Ghost band (photon ring at the horizon)"
    );
}

#[test]
fn black_hole_geometry_is_static_across_frames() {
    // Stage 1 contract: the ball geometry does not move. The ring
    // geometry must be identical across frames (only the shimmering
    // glyphs and the stage-2 orbiting motes change).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let before: Vec<(u16, u16)> = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();
    run_frames(&mut cloud, &mut frame, 120, 16);
    let after: Vec<(u16, u16)> = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .map(|c| (c.col, c.line))
        .collect();
    assert_eq!(before, after, "ball geometry must not drift across frames");
}

#[test]
fn black_hole_style_transition_rebuilds_cleanly() {
    // The scene-cycle contract: leaving and re-entering the style must
    // rebuild the ball from a clean baseline (no stale drawn-cell
    // history, no shrunken annulus).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);
    let ring_before = cloud.black_hole_rain.ring_cells_for_test().len();

    cloud.transition_rain_style(RainStyle::Vortex);
    cloud.transition_rain_style(RainStyle::BlackHole);
    // Re-entry replays the formation intro — fast-forward to the
    // steady state before asserting the rebuild.
    run_frames_to_steady(&mut cloud, &mut frame);

    assert_eq!(
        cloud.black_hole_rain.ring_cells_for_test().len(),
        ring_before,
        "re-entry must rebuild the same annulus"
    );
    assert!(
        cloud.black_hole_rain.drawn_cells_for_test().len() >= ring_before,
        "re-entry must draw the full annulus (plus any spawned ring motes)"
    );
}
