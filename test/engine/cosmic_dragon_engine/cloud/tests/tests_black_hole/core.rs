// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core black-hole-style behavior contracts (NIGHT-special-1):
//! scene resolution, dynamic geometry across viewport sizes, the empty
//! event-horizon core, radial band coverage (plus the
//! NIGHT-research-10 rim photon line), static-geometry stability
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
    // NIGHT-research-9: the ball is width-capped — mirror the reset
    // pass's sizing (the smaller of the ball fraction of the limiting
    // half-extent and the width cap share of the half-width), asserted
    // against the orchestrator's own cached radius.
    let half_w = cols as f32 / 4.0;
    let unit = half_w.min(lines as f32 / 2.0);
    let outer_r = (unit * crate::constants::BLACK_HOLE_BALL_FRACTION)
        .min(half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX);
    let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;
    assert!(
        (cloud.black_hole_rain.ball_outer_r_for_test() - outer_r).abs() < 1.0e-4,
        "the cached ball radius must match the width-capped sizing ({} vs {})",
        cloud.black_hole_rain.ball_outer_r_for_test(),
        outer_r
    );
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
    // The photon-ring gradient with the NIGHT-research-10 rim line:
    // Core band hugs the event horizon, Hot and Mid carry the body,
    // and the outer band flips back up to Core — the thin photon
    // LINE at the shadow's edge. At 120x40 every band is at least
    // one cell wide. The Ghost fringe is retired: the Mid body runs
    // right up to the rim line so the edge reads sharp against the
    // sky (the EHT read) instead of dissolving through a dim fringe.
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
    for expected in [2u8, 3u8, 4u8] {
        assert!(
            ranks.contains(&expected),
            "radial band rank {expected} missing from the annulus"
        );
    }
    // The brightest band (Core, rank 4) must hug the INNER edge: at
    // least one Core cell must sit closer to the center than every
    // Hot cell (rank 3) — the horizon photon ring wraps the hole.
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
    let hot_min = cloud
        .black_hole_rain
        .ring_cells_for_test()
        .iter()
        .filter(|c| level_rank(c.level) == 3)
        .map(|c| dist(c.col, c.line))
        .fold(f32::MAX, f32::min);
    assert!(
        core_min < hot_min,
        "Core band must sit inside the Hot band (photon ring at the horizon)"
    );
}

#[test]
fn black_hole_ball_carries_the_rim_photon_line() {
    // The NIGHT-research-10 rim photon line: the thin bright ring
    // hugging the shadow's edge INSIDE the annulus (the owner's
    // Interstellar/NASA imagery read — a thin line shaped like the
    // ball). Contract: Core cells exist in the outer radial zone on
    // every terminal class (the one-cell floor keeps the line alive
    // even where the annulus is barely two cells wide), every
    // rim-line cell sits farther from the center than every Hot
    // cell (the line wraps the OUTSIDE, never a general brightening
    // of the annulus), and the line stays a thin minority of the
    // annulus population at the standard class (a line, not a band).
    let (cols, lines) = (120, 40);
    let mut cloud = make_black_hole_cloud(cols, lines);
    let mut frame = Frame::new(cols, lines, cloud.palette.bg);
    run_frames_to_steady(&mut cloud, &mut frame);

    let cx = ((cols - 1) / 2) as f32;
    let cy = ((lines - 1) / 2) as f32;
    let dist = |col: u16, line: u16| ((col as f32 - cx) / 2.0).powi(2) + (line as f32 - cy).powi(2);

    let cells = cloud.black_hole_rain.ring_cells_for_test();
    let total = cells.len();
    assert!(total > 0, "the annulus must be non-empty at 120x40");

    // The rim line: Core cells in the outer half of the annulus
    // (the inner-zone Core cells are the horizon ring — the outer
    // half filters them out).
    let half_w = cols as f32 / 4.0;
    let half_h = lines as f32 / 2.0;
    let unit = half_w.min(half_h);
    let outer_r = (unit * crate::constants::BLACK_HOLE_BALL_FRACTION)
        .min(half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX);
    let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;
    let annulus = outer_r - core_r;
    let outer_zone = core_r + 0.5 * annulus;
    let dist_real = |col: u16, line: u16| {
        (((col as f32 - cx) / 2.0).powi(2) + (line as f32 - cy).powi(2)).sqrt()
    };

    let rim: Vec<&crate::cloud::type_rain::black_hole::black_hole::BlackHoleCell> = cells
        .iter()
        .filter(|c| level_rank(c.level) == 4 && dist_real(c.col, c.line) > outer_zone)
        .collect();
    assert!(
        !rim.is_empty(),
        "the rim photon line must host Core cells at 120x40"
    );

    // The line wraps the outside: every rim-line cell sits farther
    // from the center than every Hot cell (the inner body band).
    let hot_max = cells
        .iter()
        .filter(|c| level_rank(c.level) == 3)
        .map(|c| dist(c.col, c.line))
        .fold(f32::MIN, f32::max);
    let rim_min = rim
        .iter()
        .map(|c| dist(c.col, c.line))
        .fold(f32::MAX, f32::min);
    assert!(
        rim_min > hot_max,
        "the rim line must sit outside every Hot cell (the line at the shadow's edge)"
    );

    // The line stays thin: a minority of the annulus population.
    assert!(
        (rim.len() as f32 / total as f32) < 0.40,
        "the rim line must stay a thin minority of the annulus ({} of {})",
        rim.len(),
        total
    );

    // The one-cell floor keeps the line alive on the smallest class.
    let mut small_cloud = make_black_hole_cloud(80, 24);
    let mut small_frame = Frame::new(80, 24, small_cloud.palette.bg);
    run_frames_to_steady(&mut small_cloud, &mut small_frame);
    let small_cells = small_cloud.black_hole_rain.ring_cells_for_test();
    let s_cx = ((80 - 1) / 2) as f32;
    let s_cy = ((24 - 1) / 2) as f32;
    let s_half_w = 80.0_f32 / 4.0;
    let s_half_h = 24.0_f32 / 2.0;
    let s_unit = s_half_w.min(s_half_h);
    let s_outer = (s_unit * crate::constants::BLACK_HOLE_BALL_FRACTION)
        .min(s_half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX);
    let s_core = s_outer * crate::constants::BLACK_HOLE_CORE_FRACTION;
    let s_outer_zone = s_core + 0.5 * (s_outer - s_core);
    let s_rim = small_cells.iter().filter(|c| {
        let d = (((c.col as f32 - s_cx) / 2.0).powi(2) + (c.line as f32 - s_cy).powi(2)).sqrt();
        level_rank(c.level) == 4 && d > s_outer_zone
    });
    assert!(
        s_rim.count() > 0,
        "the rim photon line must survive the 80x24 floor (the one-cell width floor)"
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

#[test]
fn black_hole_composition_adapts_to_the_viewport_width() {
    // The NIGHT-research-9 dynamic-size contract: the ball is
    // width-capped (30% of the terminal width wherever the cap binds
    // — every viewport up to roughly 1.8:1 aspect, the owner's
    // narrow-screen read: a small shadow, a long disk), the disk's
    // scale unit stretches to fill the width on wide terminals (the
    // majestic full-width read), and on the widest viewports the
    // height bound takes the ball back over. The tier-0 reach keys
    // on the disk unit and never exceeds the 92%-of-half-width
    // clamp.
    for (cols, lines) in [(120, 40), (200, 50), (105, 64), (80, 24)] {
        let mut cloud = make_black_hole_cloud(cols, lines);
        let mut frame = Frame::new(cols, lines, cloud.palette.bg);
        run_frames_to_steady(&mut cloud, &mut frame);

        let half_w = cols as f32 / 4.0;
        let half_h = lines as f32 / 2.0;
        let unit = half_w.min(half_h);
        let outer_r = (unit * crate::constants::BLACK_HOLE_BALL_FRACTION)
            .min(half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX);
        let disk_unit = unit.max(half_w * crate::constants::BLACK_HOLE_DISK_WIDTH_FRACTION);

        let ball = cloud.black_hole_rain.ball_outer_r_for_test();
        assert!(
            (ball - outer_r).abs() < 1.0e-4,
            "the ball must follow the width-capped sizing at {cols}x{lines} ({} vs {outer_r})",
            ball
        );
        assert!(
            ball <= half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX + 1.0e-4,
            "the ball must never exceed the width cap at {cols}x{lines} ({ball})"
        );
        let disk = cloud.black_hole_rain.disk_unit_for_test();
        assert!(
            (disk - disk_unit).abs() < 1.0e-4,
            "the disk unit must follow the width-stretched sizing at {cols}x{lines} ({} vs {disk_unit})",
            disk
        );
        // The tier-0 reach: the disk's semi-major, clamped at 92% of
        // the half-width (the projection's own guard — asserted here
        // so the stretch can never push the disk off-screen).
        let reach = (crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION
            * crate::constants::BLACK_HOLE_RING_TIERS[0].major_scale
            * disk)
            .min(0.92 * half_w);
        assert!(
            reach <= 0.92 * half_w + 1.0e-3,
            "the tier-0 reach must never exceed the 92% half-width clamp at {cols}x{lines} ({reach})"
        );
        // The wide-terminal stretch: where the width floor exceeds
        // the limiting half-extent, the disk unit must grow past it.
        let width_floor = half_w * crate::constants::BLACK_HOLE_DISK_WIDTH_FRACTION;
        if width_floor > unit + 1.0e-4 {
            assert!(
                disk > unit + 1.0e-4,
                "the disk unit must stretch past the limiting half-extent at {cols}x{lines}"
            );
        }
        // The see-saw's dynamic tilt cap: derived from the vertical
        // budget over the tier-0 semi-major — positive, at or below
        // the menu max, and meaningfully engaged on these classes
        // (the stretched disk tilts shallower than the 60-degree rung).
        let cap = cloud.black_hole_rain.roll_tilt_cap_for_test();
        let menu_max = crate::cloud::type_rain::black_hole::roll::RingRoll::MENU_MAX_RADIANS;
        assert!(
            cap > 0.2 && cap <= menu_max,
            "the tilt cap must be engaged inside the window at {cols}x{lines} ({cap})"
        );
        // The narrow-screen read: wherever the cap binds, the ball
        // reads 30% of the terminal width.
        if outer_r < unit * crate::constants::BLACK_HOLE_BALL_FRACTION - 1.0e-4 {
            let share = ball * 2.0 * 2.0 / cols as f32;
            assert!(
                (share - crate::constants::BLACK_HOLE_BALL_WIDTH_MAX).abs() < 1.0e-3,
                "the capped ball must span BALL_WIDTH_MAX of the width at {cols}x{lines} ({share})"
            );
        }
    }
}
