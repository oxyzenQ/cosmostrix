// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Phosphor, ghost, and droplet afterglow tests for the cloud module.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::frame::Frame;
use crate::runtime::ShadingMode;

#[test]
fn active_trail_cells_are_protected_from_phosphor_decay() {
    // Cells within a living droplet's range should NOT be ghosted by
    // the phosphor system, even if they weren't redrawn this frame.
    // This test verifies that Pass 2 of phosphor_decay_pass marks
    // active trail cells as fresh.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 50.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);

    // After rain, find a living droplet
    let living: Vec<_> = cloud.droplets.iter().filter(|d| d.is_alive).collect();
    assert!(!living.is_empty(), "should have living droplets after rain");

    // Verify that cells within living droplet ranges have phosphor = 255
    // (protected from decay by Pass 2)
    let lines = cloud.lines;
    let mut protected_count = 0;
    for d in &living {
        let start = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
        for line in start..=d.head_put_line {
            if line >= lines {
                break;
            }
            let pidx = d.bound_col as usize * lines as usize + line as usize;
            if pidx < cloud.phosphor.len() && cloud.phosphor[pidx] == 255 {
                protected_count += 1;
            }
        }
    }
    assert!(
        protected_count > 0,
        "living droplet cells should have phosphor = 255 (protected from decay)"
    );
}

#[test]
fn phosphor_ghost_cells_use_original_character() {
    // Ghost cells should render with the original character (not a space)
    // so trail afterglow looks like fading text rather than dim colored patches.
    // We verify this by checking that phosphor_base_ch is populated when
    // cells have phosphor energy, and that ghost cells in the frame have
    // non-space characters.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to spawn and advance droplets, creating phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, now);

    // Verify that phosphor has been populated — at least some cells should
    // have energy after droplets draw.
    let total_with_energy = cloud.phosphor.iter().filter(|&&e| e > 0).count();
    assert!(
        total_with_energy > 0,
        "after rain, some cells should have phosphor energy"
    );

    // Verify that phosphor_base_ch is set for cells that have phosphor energy.
    // These are cells drawn by droplets (Pass 1) or protected by active
    // droplets (Pass 2). The character tracking is essential for rendering
    // ghost cells with the original glyph instead of a blank space.
    let cells_with_char_and_energy: usize = cloud
        .phosphor
        .iter()
        .zip(cloud.phosphor_base_ch.iter())
        .filter(|(&energy, &ch)| energy > 0 && ch != '\0')
        .count();
    assert!(
        cells_with_char_and_energy > 0,
        "after rain, cells with phosphor energy should have tracked characters (found {} with energy, {} with both energy and char)",
        total_with_energy,
        cells_with_char_and_energy
    );
}

#[test]
fn spawn_phase_jitter_produces_varied_advance_remainder() {
    // With SPAWN_PHASE_JITTER enabled, newly spawned droplets should
    // have varied advance_remainder values, not all zero.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, now);

    let living: Vec<_> = cloud.droplets.iter().filter(|d| d.is_alive).collect();

    if living.len() < 2 {
        // Not enough droplets to test variance — skip
        return;
    }

    let remainders: Vec<f32> = living.iter().map(|d| d.advance_remainder).collect();
    // Use bitwise comparison for uniqueness (f32 doesn't impl Hash/Eq)
    let unique_count: usize = {
        let mut sorted_bits: Vec<u32> = remainders.iter().map(|r| r.to_bits()).collect();
        sorted_bits.sort_unstable();
        sorted_bits.dedup();
        sorted_bits.len()
    };

    // With jitter, we expect at least some variety in remainders.
    // Without jitter, all would be 0.0 (or very similar after one frame).
    // Note: this test is probabilistic — with many droplets, it's
    // overwhelmingly likely that at least 2 have different remainders.
    assert!(
        unique_count > 1 || living.len() < 3,
        "spawn phase jitter should produce varied advance_remainder values, but all {} droplets have the same: {:?}",
        living.len(),
        remainders
    );
}

#[test]
fn consecutive_frames_produce_visual_changes() {
    // At default speed, consecutive frames should produce visible dirty
    // cells even when droplet heads don't advance rows — because phosphor
    // ghost characters create per-frame visual differences.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let base = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // First rain tick: spawn and advance
    cloud.last_spawn_time = base - Duration::from_secs(1);
    cloud.last_phosphor_time = base;
    cloud.rain_at(&mut frame, base);

    // Second tick: small time step (1/60s = ~16.7ms)
    let frame2_time = base + Duration::from_micros(16_667);
    frame.clear_dirty();
    cloud.last_phosphor_time = base; // will be corrected by rain_at
    cloud.rain_at(&mut frame, frame2_time);
    let dirty2 = frame.dirty_indices().len();

    // Third tick: another 16.7ms
    let frame3_time = base + Duration::from_micros(33_334);
    frame.clear_dirty();
    cloud.last_phosphor_time = frame2_time;
    cloud.rain_at(&mut frame, frame3_time);
    let dirty3 = frame.dirty_indices().len();

    // At least one of these frames should have visual changes
    // (phosphor ghost characters, head brightness modulation, etc.)
    assert!(
        dirty2 > 0 || dirty3 > 0,
        "consecutive frames at default speed should produce visual changes (dirty2={}, dirty3={})",
        dirty2,
        dirty3
    );
}

#[test]
fn fractional_head_brightness_varies_between_frames() {
    // The Droplet's fractional_progress should vary based on advance_remainder,
    // producing different values across frames even when the head doesn't move rows.
    // This is the public API that drives head brightness modulation.
    use crate::droplet::Droplet;

    let now = Instant::now();
    let mut d = Droplet::new();
    d.is_alive = true;
    d.is_head_crawling = true;
    d.is_tail_crawling = true;
    d.bound_col = 5;
    d.head_put_line = 3;
    d.end_line = 9;
    d.length = 8;
    d.chars_per_sec = 8.0;
    d.velocity = 8.0;
    d.last_time = Some(now);
    // birth_time is private, so activate the droplet to set it
    d.activate(now);

    // At advance_remainder = 0.0 (just advanced)
    d.advance_remainder = 0.0;
    let progress_0 = d.fractional_progress();

    // At advance_remainder = 0.5 (halfway to next row)
    d.advance_remainder = 0.5;
    let progress_5 = d.fractional_progress();

    // At advance_remainder = 0.9 (about to advance)
    d.advance_remainder = 0.9;
    let progress_9 = d.fractional_progress();

    // Fractional progress should increase monotonically
    assert!(
        progress_9 > progress_5,
        "fractional progress at 0.9 should exceed 0.5: {} vs {}",
        progress_5,
        progress_9
    );
    assert!(
        progress_5 > progress_0,
        "fractional progress at 0.5 should exceed 0.0: {} vs {}",
        progress_5,
        progress_0
    );
    // Verify exact values
    assert!((progress_0 - 0.0).abs() < 0.001);
    assert!((progress_5 - 0.5).abs() < 0.001);
    assert!((progress_9 - 0.9).abs() < 0.001);
}

#[test]
fn paste_discard_does_not_increase_background_ghost_fill() {
    // When force_draw_everything is triggered (simulating a paste/focus
    // event), the background should NOT fill with ghost charset glyphs.
    // The fix clears phosphor_base_ch on force_draw_everything, so stale
    // afterglow cells don't render character glyphs during the full redraw.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain for several frames to build up phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..10 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Count ghost glyph cells before force_draw_everything
    let ghost_fill_before: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // Trigger force_draw_everything (simulating paste/focus event)
    cloud.force_draw_everything();
    cloud.rain_at(&mut frame, now + Duration::from_millis(160));

    // After force_draw_everything + one frame, count ghost glyph cells
    // that are NOT part of active droplet trails (i.e., stale cells).
    // Active trail cells will have been repopulated by Pass 1 & 2.
    // The key invariant: stale cells should NOT have phosphor_base_ch set.
    let ghost_fill_after: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // The ghost fill after should be LESS than or equal to before,
    // because force_draw_everything clears all phosphor_base_ch and
    // only active trail cells get repopulated.
    assert!(
        ghost_fill_after <= ghost_fill_before,
        "force_draw_everything should not increase background ghost fill (before={}, after={})",
        ghost_fill_before,
        ghost_fill_after
    );

    // Also verify that remaining ghost cells all have high enough energy
    // for glyph rendering (>= PHOSPHOR_GLYPH_THRESHOLD), meaning they're
    // active trail cells that were just repopulated.
    for (i, (&ch, &energy)) in cloud
        .phosphor_base_ch
        .iter()
        .zip(cloud.phosphor.iter())
        .enumerate()
    {
        if ch != '\0' {
            assert!(
                energy >= PHOSPHOR_GLYPH_THRESHOLD,
                "ghost glyph cell {} should have energy >= GLYPH_THRESHOLD, got {}",
                i,
                energy
            );
        }
    }
}

#[test]
fn focus_event_does_not_repaint_stale_charset_background() {
    // Simulating a focus-gained event (which triggers force_draw_everything)
    // should not cause a full-screen ghost glyph repaint. We verify that
    // after force_draw_everything, the ratio of cells with phosphor_base_ch
    // set (relative to total cells) is low — not close to 100%.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain for a while to build up phosphor everywhere
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..30 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Now trigger force_draw_everything (simulating focus regained)
    cloud.force_draw_everything();
    cloud.rain_at(&mut frame, now + Duration::from_millis(480));
    frame.clear_dirty();

    let total = cloud.phosphor_base_ch.len();
    let ghost_count: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // After force_draw_everything, ghost cells should only be active trail
    // cells — a small fraction of the total. 80% threshold is generous;
    // in practice it should be much lower.
    let ghost_ratio = ghost_count as f32 / total as f32;
    assert!(
        ghost_ratio < 0.8,
        "after focus event, ghost fill ratio should be low, got {:.1}% ({}/{})",
        ghost_ratio * 100.0,
        ghost_count,
        total
    );
}

#[test]
fn stale_phosphor_chars_expire() {
    // A cell with phosphor_base_ch set should eventually stop rendering
    // the glyph character as phosphor energy decays below the threshold.
    // This tests the PHOSPHOR_GLYPH_THRESHOLD mechanism.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let base = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to create phosphor state
    cloud.last_spawn_time = base - Duration::from_secs(1);
    cloud.last_phosphor_time = base;
    cloud.rain_at(&mut frame, base);

    // Find cells that have both phosphor energy and base_ch
    let mut test_cells: Vec<usize> = Vec::new();
    for (i, (&energy, &ch)) in cloud
        .phosphor
        .iter()
        .zip(cloud.phosphor_base_ch.iter())
        .enumerate()
    {
        if energy > 0 && ch != '\0' {
            test_cells.push(i);
        }
    }

    if test_cells.is_empty() {
        // No phosphor cells to test — skip
        return;
    }

    // Kill all droplets and prevent new spawning so phosphor can decay
    // without being refreshed by active trail cells or new droplets.
    for d in &mut cloud.droplets {
        d.is_alive = false;
    }
    cloud.droplets_per_sec = 0.0;
    cloud.spawn_remainder = 0.0;

    // Simulate multiple frames of decay. We must clear the frame between
    // rain_at calls to properly simulate the frame lifecycle: in the real
    // pipeline, dead droplets' tail cleanup blanks old cells (fg=None),
    // preventing Pass 1 from marking them as fresh. Since we killed all
    // droplets before tail cleanup could run, we clear_with_bg to achieve
    // the same effect — old cells are no longer current_gen, so Pass 1
    // won't mark them fresh, and Pass 3 can decay them.
    for frame_idx in 1..=15 {
        let t = base + Duration::from_millis(frame_idx * 17);
        cloud.last_phosphor_time = base + Duration::from_millis((frame_idx - 1) * 17);
        frame.clear_with_bg(cloud.palette.bg);
        cloud.rain_at(&mut frame, t);
    }

    // Now check: cells that had phosphor_base_ch should have it cleared
    // if their energy dropped below the glyph threshold.
    let mut expired_count = 0;
    for &i in &test_cells {
        if i < cloud.phosphor_base_ch.len() {
            let energy = cloud.phosphor[i];
            let ch = cloud.phosphor_base_ch[i];
            if energy < PHOSPHOR_GLYPH_THRESHOLD {
                // Glyph should be cleared
                assert_eq!(
                    ch, '\0',
                    "cell {} with energy {} < GLYPH_THRESHOLD should have cleared base_ch",
                    i, energy
                );
                expired_count += 1;
            }
        }
    }

    // At least some cells should have expired (energy < threshold)
    assert!(
        expired_count > 0,
        "at least some phosphor_base_ch entries should have expired after 15 frames of decay (tested {} cells)",
        test_cells.len()
    );
}

#[test]
fn active_trail_afterglow_still_has_glyphs() {
    // Active trail cells should still show subtle glyph afterglow with
    // their original characters. This verifies that the glyph threshold
    // mechanism preserves organic smoothness for recently drawn trails.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to spawn and advance droplets
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, now);

    // Find living droplets
    let living: Vec<_> = cloud.droplets.iter().filter(|d| d.is_alive).collect();
    if living.is_empty() {
        return;
    }

    // Verify that cells within living droplet ranges have both
    // phosphor energy >= GLYPH_THRESHOLD and phosphor_base_ch set.
    let lines = cloud.lines;
    let mut glyph_count = 0;
    for d in &living {
        let start = d.tail_put_line.map(|v| v.saturating_add(1)).unwrap_or(0);
        for line in start..=d.head_put_line {
            if line >= lines {
                break;
            }
            let pidx = d.bound_col as usize * lines as usize + line as usize;
            if pidx < cloud.phosphor.len() {
                let energy = cloud.phosphor[pidx];
                let ch = cloud.phosphor_base_ch[pidx];
                if energy >= PHOSPHOR_GLYPH_THRESHOLD && ch != '\0' {
                    glyph_count += 1;
                }
            }
        }
    }

    assert!(
        glyph_count > 0,
        "active trail cells should have glyph afterglow (energy >= GLYPH_THRESHOLD and base_ch set)"
    );
}

#[test]
fn background_remains_clean_after_safe_redraw() {
    // After a safe redraw (force_draw_everything), the background should
    // not contain stale charset glyphs. Only active trail cells should
    // have phosphor_base_ch set, and only with energy >= GLYPH_THRESHOLD.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain for a while to build up extensive phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..20 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Trigger safe redraw
    cloud.force_draw_everything();
    cloud.rain_at(&mut frame, now + Duration::from_millis(320));
    frame.clear_dirty();

    // Check that no cells have phosphor_base_ch set with energy below
    // the glyph threshold — this would indicate stale ghost glyphs.
    let mut stale_glyph_count = 0;
    for (&ch, &energy) in cloud.phosphor_base_ch.iter().zip(cloud.phosphor.iter()) {
        if ch != '\0' && energy < PHOSPHOR_GLYPH_THRESHOLD {
            stale_glyph_count += 1;
        }
    }

    assert_eq!(
        stale_glyph_count, 0,
        "no cells should have ghost glyphs with energy below GLYPH_THRESHOLD after safe redraw (found {})",
        stale_glyph_count
    );
}

#[test]
fn semantic_invalidation_clears_stale_ghost_glyphs() {
    // When semantic_invalidate is set (e.g., from set_shading_mode),
    // the rain_at() path should also clear phosphor_base_ch, just like
    // force_draw_everything does. This prevents the ghost background
    // bug when shading mode or other semantic mutations occur.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain for several frames to build up phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..20 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Count ghost glyph cells before semantic invalidation
    let ghost_fill_before: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // Trigger semantic invalidation (simulating shading mode change)
    cloud.set_shading_mode(ShadingMode::DistanceFromHead);
    assert!(
        cloud.semantic_invalidate,
        "set_shading_mode should set semantic_invalidate"
    );

    // Run one frame to process the semantic invalidation
    cloud.rain_at(&mut frame, now + Duration::from_millis(320));
    frame.clear_dirty();

    // After semantic invalidation, ghost fill should be reduced because
    // phosphor_base_ch was cleared. Active trail cells are repopulated
    // by Pass 1 & 2, but stale cells should be clean.
    let ghost_fill_after: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    assert!(
        ghost_fill_after <= ghost_fill_before,
        "semantic invalidation should not increase ghost fill (before={}, after={})",
        ghost_fill_before,
        ghost_fill_after
    );

    // No low-energy ghost glyphs should exist
    for (&ch, &energy) in cloud.phosphor_base_ch.iter().zip(cloud.phosphor.iter()) {
        if ch != '\0' {
            assert!(
                energy >= PHOSPHOR_GLYPH_THRESHOLD,
                "ghost glyph cell should have energy >= GLYPH_THRESHOLD after semantic invalidation, got {}",
                energy
            );
        }
    }
}

#[test]
fn tab_after_phosphor_activity_does_not_increase_ghost_fill() {
    // Simulates the Tab key bug scenario: after rain creates phosphor
    // state, a semantic invalidation (previously caused by Tab toggling
    // shading mode) should not cause a ghost background flood. With the
    // fix, semantic invalidation also clears phosphor_base_ch, and the
    // PHOSPHOR_GLYPH_THRESHOLD prevents low-energy glyphs from rendering.
    use crate::constants::PHOSPHOR_GLYPH_THRESHOLD;

    let mut cloud = make_cloud();
    cloud.chars_per_sec = 8.0;
    cloud.recalc_droplets_per_sec();

    let now = Instant::now();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Run rain to create phosphor state
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.last_phosphor_time = now;
    for i in 0..20 {
        cloud.rain_at(&mut frame, now + Duration::from_millis(i * 16));
        frame.clear_dirty();
    }

    // Count ghost glyph cells before semantic invalidation
    let ghost_fill_before: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // Simulate the Tab key effect: set_shading_mode (which Tab used to
    // call) triggers semantic_invalidate. This is the exact path that
    // caused the ghost background bug.
    cloud.set_shading_mode(ShadingMode::DistanceFromHead);
    cloud.rain_at(&mut frame, now + Duration::from_millis(320));
    frame.clear_dirty();

    let ghost_fill_after: usize = cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch != '\0')
        .count();

    // Ghost fill should not have increased due to semantic invalidation
    assert!(
        ghost_fill_after <= ghost_fill_before,
        "semantic invalidation from Tab-like action should not increase ghost fill (before={}, after={})",
        ghost_fill_before,
        ghost_fill_after
    );

    // No low-energy ghost glyphs should exist
    for (&ch, &energy) in cloud.phosphor_base_ch.iter().zip(cloud.phosphor.iter()) {
        if ch != '\0' {
            assert!(
                energy >= PHOSPHOR_GLYPH_THRESHOLD,
                "ghost glyph cell should have energy >= GLYPH_THRESHOLD, got {}",
                energy
            );
        }
    }
}
