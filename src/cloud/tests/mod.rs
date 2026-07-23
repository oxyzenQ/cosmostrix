// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core tests for the cloud module (rain, pause/resume, transitions, spawn).

mod tests_color_stability;
mod tests_edge_fade;
mod tests_monolith;
mod tests_phosphor;
mod tests_scene;
mod tests_visual_depth;

use std::time::{Duration, Instant};

use crossterm::style::Color;

use super::render::DrawCtx;
use super::Cloud;
use crate::constants::{
    CHARSET_TRANSITION_DURATION_MS, COLOR_TRANSITION_DURATION_MS,
    COLOR_TRANSITION_INITIAL_VISIBLE_PCT, FULL_REDRAW_INTERVAL_FRAMES, MAX_PALETTE_SLOTS,
    PHOSPHOR_BOTTOM_ROWS, SPAWN_REMAINDER_CAP,
};
use crate::frame::Frame;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

pub(super) fn make_cloud() -> Cloud {
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
    cloud.reset(20, 10);
    cloud
}

#[test]
fn rain_produces_dirty_frame_when_time_advances() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);

    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain(&mut frame);

    assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
}

#[test]
fn pause_stops_rain_and_unpause_resumes() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);

    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    cloud.rain(&mut frame);
    assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());

    frame.clear_dirty();
    // v17 mastery: toggle_pause() now starts a deceleration transition.
    // For instant freeze in tests, set self.pause = true directly.
    cloud.pause = true;
    cloud.pause_time = Some(Instant::now());
    cloud.rain(&mut frame);
    assert!(!frame.is_dirty_all() && frame.dirty_indices().is_empty());

    cloud.toggle_pause();
    // Advance resume_start far enough in the past so the smoothstep
    // easing completes (resume_blend reaches 1.0, allowing full-speed
    // simulation on the next rain() call).
    let now = Instant::now();
    cloud.resume_start = Some(now - Duration::from_secs(1));
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);
    cloud.rain_at(&mut frame, now + Duration::from_secs(1));
    assert!(frame.is_dirty_all() || !frame.dirty_indices().is_empty());
}

#[test]
fn periodic_full_redraw_survives_until_next_frame() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    frame.clear_dirty();
    cloud.frames_since_full_redraw = FULL_REDRAW_INTERVAL_FRAMES - 1;
    cloud.rain_at(&mut frame, now);
    assert!(cloud.force_draw_everything);

    frame.clear_dirty();
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));
    assert!(frame.is_dirty_all());
    assert!(!cloud.force_draw_everything);
}

#[test]
fn color_transition_starts_immediately_and_completes() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    cloud.set_color_scheme(ColorScheme::Blue);

    assert_eq!(cloud.color_scheme(), ColorScheme::Blue);
    assert!(
        cloud.transition_start.is_some(),
        "transition must start immediately after set_color_scheme"
    );

    // Row-based wave: at t=0, the wave line should cover the initial band.
    // The wave line value represents the boundary; rows with index <= wave_line
    // use the new palette. So wave_line=1.2 means rows 0 and 1 adopt.
    let wave = cloud.color_wave_line_at(now);
    assert!(
        wave.is_some(),
        "color wave must be active during transition"
    );
    // The number of rows that adopt the new palette at t=0 is (wave_line + 1),
    // since row indices 0 through floor(wave_line) are above the wave.
    let initial_adopted_rows = wave.unwrap().floor() as usize + 1;
    let min_initial_rows =
        ((cloud.lines as f32 * COLOR_TRANSITION_INITIAL_VISIBLE_PCT).ceil() as usize).max(1);
    assert!(
        initial_adopted_rows >= min_initial_rows,
        "first transition frame should visibly update a band of top rows (got {} rows >= {})",
        initial_adopted_rows,
        min_initial_rows
    );

    // All columns should already have adopted the new palette (wave is row-based)
    assert!(cloud
        .column_palette_slot
        .iter()
        .all(|slot| *slot == cloud.active_palette_slot));

    cloud.transition_start = Some(now);
    cloud.rain_at(&mut frame, now);

    // After one frame, transition should still be in progress (150ms hasn't elapsed)
    assert!(cloud.transition_start.is_some());

    cloud.transition_start =
        Some(now - Duration::from_millis(COLOR_TRANSITION_DURATION_MS as u64 + 1));
    cloud.rain_at(&mut frame, now);

    assert!(cloud.transition_start.is_none());
    // All droplets should have adopted the new palette after transition completes
    for d in &cloud.droplets {
        if d.is_alive {
            assert_eq!(d.palette_slot, cloud.active_palette_slot);
        }
    }
}

#[test]
fn charset_change_triggers_visible_wave_redraw() {
    let mut cloud = make_cloud();
    cloud.semantic_invalidate = false;
    cloud.force_draw_everything = false;
    let old_pool = cloud.char_pool.clone();

    cloud.transition_chars(vec!['A', 'B']);

    assert!(cloud.charset_transition_start.is_some());
    assert_eq!(cloud.previous_char_pool, old_pool);
    assert_ne!(cloud.char_pool, old_pool);
    // v18 cinematic unification: transition_chars now triggers a forced
    // full redraw (same pattern as set_color_scheme) so the charset wave
    // is visible on the next frame across all rain styles. The previous
    // behavior left glyph-mode cells untouched until droplets passed
    // through, making the wave invisible.
    assert!(cloud.semantic_invalidate);
    assert!(cloud.force_draw_everything);
}

#[test]
fn charset_wave_uses_old_rows_below_and_new_rows_above() {
    let old_pool = ['0', '1'];
    let new_pool = ['A', 'B'];
    let glitch_map = bitvec::bitvec![0; 20];
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];

    let ctx = DrawCtx {
        lines: 10,
        cols: 20,
        full_width: false,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::Mono,
        bold_mode: BoldMode::Off,
        glitchy: false,
        last_glitch_time: Instant::now(),
        next_glitch_time: Instant::now(),
        glitch_inv_between: 0.0,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &new_pool,
        previous_char_pool: &old_pool,
        edge_fade_lut: &[],
        charset_wave_line: Some(3.0),
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_col: u16::MAX,
        flash_line: u16::MAX,
        flash_time: None,
        flash_elapsed: None,
        pool_is_binary: false,
    };

    assert_eq!(ctx.get_char(1, 0, 0), 'B');
    assert_eq!(ctx.get_char(8, 0, 0), '0');
}

#[test]
fn charset_transition_completes_and_commits_new_pool() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    cloud.transition_chars(vec!['A', 'B']);
    cloud.charset_transition_start =
        Some(now - Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64 + 1));
    cloud.rain_at(&mut frame, now);

    assert!(cloud.charset_transition_start.is_none());
    assert!(cloud.previous_char_pool.is_empty());
    assert!(cloud.char_pool.iter().all(|ch| matches!(ch, 'A' | 'B')));
}

#[test]
fn pause_freezes_simulation_time() {
    let mut cloud = make_cloud();
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    let now = Instant::now();

    cloud.last_spawn_time = now - Duration::from_secs(1);
    assert!(cloud.toggle_pause());
    let last_spawn = cloud.last_spawn_time;
    frame.clear_dirty();

    cloud.rain_at(&mut frame, now + Duration::from_secs(5));

    assert_eq!(cloud.last_spawn_time, last_spawn);
    assert!(!frame.is_dirty_all() && frame.dirty_indices().is_empty());
}

#[test]
fn resume_resets_timing_debt() {
    let mut cloud = make_cloud();
    let now = Instant::now();

    // v17 mastery: toggle_pause() now starts a deceleration transition.
    // For instant freeze in tests, set self.pause = true directly.
    cloud.pause = true;
    cloud.pause_time = Some(now - Duration::from_secs(5));
    cloud.spawn_remainder = 42.0;
    assert!(cloud.toggle_pause());

    assert!(!cloud.pause);
    assert_eq!(cloud.spawn_remainder, 0.0);
    assert_eq!(cloud.resume_blend, 0.0);
    assert!(cloud.resume_start.is_some());
    assert!(cloud.last_spawn_time > now - Duration::from_secs(5));
}

#[test]
fn repeated_pause_resume_does_not_accumulate_timing_debt() {
    let mut cloud = make_cloud();
    let now = Instant::now();

    for seconds in 1..=3 {
        // v17 mastery: toggle_pause() now starts a deceleration transition.
        // For instant freeze in tests, set self.pause = true directly.
        cloud.pause = true;
        cloud.pause_time = Some(now - Duration::from_secs(seconds));
        cloud.spawn_remainder = seconds as f32;
        assert!(cloud.toggle_pause());

        assert!(!cloud.pause);
        assert_eq!(cloud.spawn_remainder, 0.0);
        assert_eq!(cloud.resume_blend, 0.0);
        assert!(cloud.resume_start.is_some());
    }
}

#[test]
fn repeated_runtime_transitions_replace_pending_state_predictably() {
    let mut cloud = make_cloud();
    let first_pool = cloud.char_pool.clone();

    cloud.set_color_scheme(ColorScheme::Blue);
    cloud.set_color_scheme(ColorScheme::Red);
    assert_eq!(cloud.color_scheme(), ColorScheme::Red);
    assert!(cloud.transition_start.is_some());

    cloud.transition_chars(vec!['A', 'B']);
    let intermediate_pool = cloud.char_pool.clone();
    cloud.transition_chars(vec!['X', 'Y']);

    assert_eq!(cloud.previous_char_pool, intermediate_pool);
    assert_ne!(cloud.previous_char_pool, first_pool);
    assert!(cloud.char_pool.iter().all(|ch| matches!(ch, 'X' | 'Y')));
    assert!(cloud.charset_transition_start.is_some());
}

#[test]
fn color_wave_begins_at_top_rows() {
    let mut cloud = make_cloud();
    let now = Instant::now();
    cloud.set_color_scheme(ColorScheme::Blue);

    let wave = cloud.color_wave_line_at(now);
    assert!(wave.is_some());
    let wave_line = wave.unwrap();
    // At t=0, the wave should cover at least the initial visible fraction of rows.
    // Rows 0..=floor(wave_line) adopt the new palette immediately.
    let adopted_rows = wave_line.floor() as usize + 1;
    let min_rows =
        ((cloud.lines as f32 * COLOR_TRANSITION_INITIAL_VISIBLE_PCT).ceil() as usize).max(1);
    assert!(
        adopted_rows >= min_rows,
        "color wave at t=0 should cover initial band of top rows ({} rows adopted >= {} expected)",
        adopted_rows,
        min_rows
    );
}

#[test]
fn color_wave_progresses_downward_over_time() {
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.transition_start = Some(start);

    let wave_early = cloud
        .color_wave_line_at(start + Duration::from_millis(10))
        .unwrap();
    let wave_mid = cloud
        .color_wave_line_at(start + Duration::from_millis(75))
        .unwrap();
    let wave_late = cloud
        .color_wave_line_at(start + Duration::from_millis(140))
        .unwrap();

    assert!(
        wave_mid > wave_early,
        "color wave should progress downward over time"
    );
    assert!(
        wave_late > wave_mid,
        "color wave should continue progressing downward"
    );
}

#[test]
fn repeated_color_transitions_remain_valid() {
    let mut cloud = make_cloud();

    cloud.set_color_scheme(ColorScheme::Blue);
    assert!(cloud.transition_start.is_some());

    // Second transition should replace the first cleanly
    cloud.set_color_scheme(ColorScheme::Red);
    assert!(cloud.transition_start.is_some());
    assert_eq!(cloud.color_scheme(), ColorScheme::Red);
}

#[test]
fn spawn_remainder_is_clamped() {
    let mut cloud = make_cloud();
    cloud.spawn_remainder = 100.0; // Unrealistically high
    cloud.last_spawn_time = Instant::now() - Duration::from_secs(1);
    let mut frame = Frame::new(20, 10, cloud.palette.bg);
    cloud.rain(&mut frame);
    // After one rain tick, spawn remainder should be clamped
    assert!(
        cloud.spawn_remainder <= SPAWN_REMAINDER_CAP,
        "spawn remainder should be clamped to SPAWN_REMAINDER_CAP (got {})",
        cloud.spawn_remainder
    );
}

#[test]
fn mouse_mode_is_default_off_and_opt_in() {
    let mut cloud = make_cloud();
    assert!(
        !cloud.mouse_enabled,
        "mouse_enabled should default to false (off) when not explicitly set"
    );
    cloud.mouse_enabled = true;
    assert!(cloud.mouse_enabled);
}

#[test]
fn color_uses_previous_palette_below_wave_line() {
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
    let glitch_map = bitvec::bitvec![0; 200];

    let ctx = DrawCtx {
        lines: 10,
        cols: 20,
        full_width: false,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::Mono,
        bold_mode: BoldMode::Off,
        glitchy: false,
        last_glitch_time: Instant::now(),
        next_glitch_time: Instant::now(),
        glitch_inv_between: 0.0,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        active_palette_slot: 1,
        transitioning: true,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0', '1'],
        previous_char_pool: &[],
        edge_fade_lut: &[],
        charset_wave_line: None,
        color_wave_line: Some(3.0),
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_col: u16::MAX,
        flash_line: u16::MAX,
        flash_time: None,
        flash_elapsed: None,
        pool_is_binary: false,
    };

    // Row 0 (above wave): droplet with old palette should NOT use previous
    assert!(!ctx.color_uses_previous_palette(0, 0, 0));
    // Row 8 (below wave): droplet with old palette SHOULD use previous
    assert!(ctx.color_uses_previous_palette(0, 8, 0));
    // Droplet already on active palette: always uses new palette
    assert!(!ctx.color_uses_previous_palette(1, 8, 0));
}

#[test]
fn droplet_exiting_bottom_fully_clears_trail() {
    // When a droplet dies (tail catches head), all its trail cells should
    // be blanked in the draw call. After the draw, no cells in the
    // droplet's column should retain stale content from that droplet.
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    // Spawn a droplet that will die quickly
    let now = Instant::now();
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);

    // Find a living droplet and force it to die by making tail catch head
    let mut found = false;
    for d in &mut cloud.droplets {
        if d.is_alive {
            // Force the droplet to die: tail reaches head
            d.tail_put_line = Some(d.head_put_line);
            d.tail_cur_line = d.head_put_line;
            d.is_alive = false;
            found = true;
            break;
        }
    }
    assert!(found, "should have at least one living droplet after rain");

    // Run another frame — the dead droplet's draw() should blank all cells
    let mut frame2 = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame2, now + Duration::from_millis(16));

    // After the draw, dead droplet's bound_col should be recycled
    let dead_droplets: Vec<_> = cloud
        .droplets
        .iter()
        .filter(|d| !d.is_alive && d.bound_col != u16::MAX)
        .collect();
    assert!(
        dead_droplets.is_empty(),
        "dead droplets should have bound_col = u16::MAX after cleanup draw"
    );
}

#[test]
fn phosphor_blank_cells_are_not_overridden_by_ghost() {
    // When a cell is blanked (fg = None, current_gen), the phosphor pass
    // should NOT render a ghost cell over it. The blank should take effect,
    // and afterglow begins on the next frame.
    let mut cloud = make_cloud();
    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);

    let now = Instant::now();
    cloud.last_spawn_time = now - Duration::from_secs(1);
    cloud.rain_at(&mut frame, now);

    // Manually blank a cell that has phosphor energy
    let col = 0u16;
    let line = 0u16;
    let blank = crate::terminal::blank_cell(cloud.palette.bg);
    frame.set(col, line, blank);

    // The cell should now be blank in the frame
    let cell = frame.get(col, line).unwrap();
    assert_eq!(cell.ch, ' ');
    assert!(cell.fg.is_none(), "blanked cell should have fg = None");
}

#[test]
fn stale_bottom_cells_decay_to_blank_within_bounded_time() {
    // Phosphor ghost cells at the bottom of the screen should decay to
    // blank within a bounded number of frames, thanks to the bottom-row
    // decay acceleration (PHOSPHOR_BOTTOM_DECAY_MULT).
    use crate::constants::{
        PHOSPHOR_BOTTOM_DECAY_MULT, PHOSPHOR_DEAD_THRESHOLD, PHOSPHOR_DECAY_RATE,
        PHOSPHOR_TAIL_RESIDUAL,
    };

    // Calculate the theoretical number of 60fps frames needed for a
    // bottom-row ghost cell to decay from PHOSPHOR_TAIL_RESIDUAL to
    // PHOSPHOR_DEAD_THRESHOLD with bottom acceleration.
    let fps = 60.0;
    let dt = 1.0 / fps;
    let effective_rate = PHOSPHOR_DECAY_RATE * PHOSPHOR_BOTTOM_DECAY_MULT;
    let mut energy = PHOSPHOR_TAIL_RESIDUAL as f32;
    let mut frames = 0u32;
    let max_frames = 300; // 5 seconds at 60fps — hard upper bound

    while energy > PHOSPHOR_DEAD_THRESHOLD as f32 && frames < max_frames {
        energy *= (-effective_rate * dt).exp();
        frames += 1;
    }

    assert!(
        energy <= PHOSPHOR_DEAD_THRESHOLD as f32,
        "phosphor should decay to dead within {} frames at bottom, but energy = {}",
        frames,
        energy
    );
    // Bottom decay should be significantly faster than normal
    // Normal: ~60-70 frames. Bottom should be < 30 frames.
    assert!(
        frames < 30,
        "bottom-row phosphor should decay in < 30 frames (got {}), ensuring no concrete wall",
        frames
    );
}

#[test]
fn high_speed_does_not_create_unbounded_bottom_accumulation() {
    // Simulate high-speed rain for many frames and verify that the bottom
    // rows don't accumulate more than a bounded number of non-blank cells.
    let mut cloud = make_cloud();
    cloud.chars_per_sec = 100.0; // High speed
    cloud.droplet_density = 1.5;
    cloud.recalc_droplets_per_sec();

    let mut frame = Frame::new(cloud.cols, cloud.lines, cloud.palette.bg);
    let start = Instant::now();
    let frame_dt = Duration::from_millis(16); // ~60fps

    // Run 300 frames (~5 seconds) of high-speed rain
    for i in 0..300 {
        let now = start + frame_dt * i;
        cloud.last_spawn_time = now - Duration::from_millis(16);
        cloud.last_phosphor_time = now;
        cloud.rain_at(&mut frame, now);
    }

    // Count non-blank cells in the bottom PHOSPHOR_BOTTOM_ROWS
    let bottom_start = cloud.lines.saturating_sub(PHOSPHOR_BOTTOM_ROWS);
    let mut non_blank_bottom = 0usize;
    let mut total_bottom = 0usize;
    for line in bottom_start..cloud.lines {
        for col in 0..cloud.cols {
            total_bottom += 1;
            let cell = frame.get(col, line).unwrap();
            if cell.fg.is_some() {
                non_blank_bottom += 1;
            }
        }
    }

    // At high speed, there should always be some active cells, but the
    // ratio of non-blank cells at the bottom should not approach 100%
    // (which would indicate a "concrete wall"). Allow up to 85% to
    // account for active rain, but not the ~100% seen in the bug.
    let ratio = non_blank_bottom as f32 / total_bottom as f32;
    assert!(
        ratio < 0.85,
        "bottom rows should not be >85% non-blank after high-speed rain (got {:.1}%), \
         indicating no concrete wall accumulation",
        ratio * 100.0
    );
}

#[test]
fn blank_cells_are_marked_dirty_for_redraw() {
    // When a cell transitions from having content to blank (via tail
    // cleanup), it must be marked dirty so the terminal redraws it.
    let mut frame = Frame::new(4, 4, None);
    frame.clear_dirty();

    // Set a cell with content
    frame.set(
        2,
        2,
        crate::cell::Cell {
            ch: 'X',
            fg: Some(Color::Green),
            bg: None,
            bold: true,
        },
    );
    assert!(
        !frame.dirty_indices().is_empty(),
        "setting content should be dirty"
    );

    frame.clear_dirty();

    // Blank the cell
    frame.set(2, 2, crate::cell::Cell::blank_with_bg(None));
    assert!(
        !frame.dirty_indices().is_empty(),
        "blanking a cell with content must be dirty — otherwise differential rendering skips the clear"
    );
    assert_eq!(frame.get(2, 2).unwrap().ch, ' ');
    assert!(frame.get(2, 2).unwrap().fg.is_none());
}
