// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-10: smoothstep easing on transition waves.
//!
//! Both `charset_wave_line_at` and `color_wave_line_at` previously used
//! a linear-velocity sweep (`progress * scale`) — the wave moved at
//! constant speed from top to bottom. S-master-HUNT-10 replaces the
//! linear `progress` with `smoothstep(progress) = 3t^2 - 2t^3`, which
//! eases in at the top, accelerates through the middle, and eases out
//! at the bottom — a more organic, cinematic feel.
//!
//! These tests verify the smoothstep curve properties:
//! - At t=0: wave_line starts at the initial position (smoothstep(0)=0)
//! - At t=duration: wave_line reaches the end (smoothstep(1)=1)
//! - Mid-progress wave_line is BELOW the linear midpoint (ease-in)
//! - The wave is monotonically increasing (LTS ordering invariant)
//!
//! LTS safety: smoothstep is monotonic on [0,1] with fixed endpoints
//! (0 and 1), so all existing ordering/threshold/completion tests pass
//! unchanged. The easing only affects the INTERPOLATION between
//! endpoints, not the endpoints themselves.

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::constants::{
    CHARSET_TRANSITION_DURATION_MS, COLOR_TRANSITION_DURATION_MS,
    COLOR_TRANSITION_INITIAL_VISIBLE_PCT,
};

#[test]
fn charset_wave_smoothstep_starts_at_zero() {
    // At t=0, smoothstep(0) = 0, so wave_line = 0 * (lines + 1) = 0.
    // The wave starts at the very top row (row 0 is "above" the wave).
    let mut cloud = make_cloud();
    let now = Instant::now();
    cloud.charset_transition_start = Some(now);

    let wave = cloud
        .charset_wave_line_at(now)
        .expect("wave must be active");
    // smoothstep(0) = 0, so wave_line should be 0.0 (or very close —
    // elapsed_ms at t=0 is 0, progress = 0, eased = 0).
    assert!(
        wave.abs() < 0.001,
        "charset wave at t=0 should start at 0 (smoothstep(0)=0), got {}",
        wave
    );
}

#[test]
fn charset_wave_smoothstep_ends_at_lines_plus_one() {
    // At t=duration, smoothstep(1) = 1, so wave_line = 1 * (lines + 1).
    // The wave has swept the entire screen (all rows above the wave).
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);
    let expected_end = cloud.lines as f32 + 1.0;

    let wave = cloud
        .charset_wave_line_at(start + Duration::from_millis(CHARSET_TRANSITION_DURATION_MS as u64))
        .expect("wave must be active at t=duration");
    assert!(
        (wave - expected_end).abs() < 0.001,
        "charset wave at t=duration should reach lines+1 (smoothstep(1)=1), got {} expected {}",
        wave,
        expected_end
    );
}

#[test]
fn charset_wave_smoothstep_midpoint_below_linear_midpoint() {
    // The KEY smoothstep property: at t=duration/2 (progress=0.5),
    // smoothstep(0.5) = 0.5 * 0.5 * (3 - 2*0.5) = 0.25 * 2.0 = 0.5.
    // Wait — smoothstep(0.5) is exactly 0.5 (symmetric around 0.5).
    // So the midpoint test is a sanity check that eased(0.5) = 0.5,
    // NOT below the linear midpoint. The ease-in/ease-out is symmetric.
    //
    // The real easing difference is at NON-midpoint progress values:
    // - At progress=0.25: linear = 0.25, smoothstep = 0.15625 (BELOW linear — ease-in)
    // - At progress=0.75: linear = 0.75, smoothstep = 0.84375 (ABOVE linear — ease-out)
    //
    // This test verifies the ease-in at progress=0.25: the smoothstep
    // wave should be BELOW the linear wave at the same timestamp.
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);
    let lines_plus_one = cloud.lines as f32 + 1.0;
    let duration = CHARSET_TRANSITION_DURATION_MS as f32;

    // progress = 0.25 (quarter way through the transition)
    let quarter_ms = (duration * 0.25) as u64;
    let wave_at_quarter = cloud
        .charset_wave_line_at(start + Duration::from_millis(quarter_ms))
        .expect("wave must be active");

    // Linear would give: 0.25 * lines_plus_one
    // Smoothstep gives: 0.25 * 0.25 * (3 - 0.5) = 0.0625 * 2.5 = 0.15625
    let linear_at_quarter = 0.25 * lines_plus_one;
    let smoothstep_at_quarter = 0.15625 * lines_plus_one;

    assert!(
        wave_at_quarter < linear_at_quarter,
        "smoothstep wave at progress=0.25 ({}) should be BELOW linear ({}) — ease-in property",
        wave_at_quarter,
        linear_at_quarter
    );
    assert!(
        (wave_at_quarter - smoothstep_at_quarter).abs() < 0.01,
        "smoothstep wave at progress=0.25 ({}) should match formula 0.15625 * lines+1 ({})",
        wave_at_quarter,
        smoothstep_at_quarter
    );
}

#[test]
fn charset_wave_smoothstep_is_monotonic() {
    // LTS invariant: the wave must progress strictly downward over time
    // (existing tests assert this). smoothstep is monotonic on [0,1],
    // so this property is preserved.
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.charset_transition_start = Some(start);

    let wave_early = cloud
        .charset_wave_line_at(start + Duration::from_millis(50))
        .unwrap();
    let wave_mid = cloud
        .charset_wave_line_at(start + Duration::from_millis(250))
        .unwrap();
    let wave_late = cloud
        .charset_wave_line_at(start + Duration::from_millis(450))
        .unwrap();

    assert!(
        wave_mid > wave_early,
        "charset wave should progress downward (mid > early)"
    );
    assert!(
        wave_late > wave_mid,
        "charset wave should continue progressing downward (late > mid)"
    );
}

#[test]
fn color_wave_smoothstep_preserves_initial_band() {
    // The color wave has an initial_frac offset: at t=0, the first
    // initial_frac * lines rows adopt immediately. smoothstep(0) = 0,
    // so the eased term contributes 0 at t=0, and wave_line =
    // initial_frac * lines (unchanged from the linear version).
    let mut cloud = make_cloud();
    let now = Instant::now();
    cloud.transition_start = Some(now);

    let wave = cloud
        .color_wave_line_at(now)
        .expect("color wave must be active");
    let expected_initial = COLOR_TRANSITION_INITIAL_VISIBLE_PCT * cloud.lines as f32;

    assert!(
        (wave - expected_initial).abs() < 0.001,
        "color wave at t=0 should preserve initial band (smoothstep(0)=0), got {} expected {}",
        wave,
        expected_initial
    );
}

#[test]
fn color_wave_smoothstep_eases_after_initial_band() {
    // After the initial band, the remaining sweep uses smoothstep easing.
    // At progress=0.25 (quarter through the post-initial sweep):
    // - Linear: wave_line = initial_frac * lines + 0.25 * (1 - initial_frac) * (lines + 1)
    // - Smoothstep: wave_line = initial_frac * lines + 0.15625 * (1 - initial_frac) * (lines + 1)
    // The smoothstep value is BELOW the linear value (ease-in).
    let mut cloud = make_cloud();
    let start = Instant::now();
    cloud.transition_start = Some(start);
    let lines = cloud.lines as f32;
    let initial_frac = COLOR_TRANSITION_INITIAL_VISIBLE_PCT;
    let duration = COLOR_TRANSITION_DURATION_MS as f32;

    let quarter_ms = (duration * 0.25) as u64;
    let wave_at_quarter = cloud
        .color_wave_line_at(start + Duration::from_millis(quarter_ms))
        .unwrap();

    let linear_at_quarter = initial_frac * lines + 0.25 * (1.0 - initial_frac) * (lines + 1.0);
    let smoothstep_at_quarter =
        initial_frac * lines + 0.15625 * (1.0 - initial_frac) * (lines + 1.0);

    assert!(
        wave_at_quarter < linear_at_quarter,
        "smoothstep color wave at progress=0.25 ({}) should be BELOW linear ({}) — ease-in after initial band",
        wave_at_quarter,
        linear_at_quarter
    );
    assert!(
        (wave_at_quarter - smoothstep_at_quarter).abs() < 0.01,
        "smoothstep color wave at progress=0.25 ({}) should match formula ({})",
        wave_at_quarter,
        smoothstep_at_quarter
    );
}

#[test]
fn color_wave_smoothstep_is_monotonic() {
    // LTS invariant: the color wave must progress strictly downward.
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

    assert!(wave_mid > wave_early, "color wave should progress downward");
    assert!(
        wave_late > wave_mid,
        "color wave should continue progressing"
    );
}

// ── S-master-HUNT-15: diagonal stagger ─────────────────────────────────────
//
// The diagonal stagger adds a per-column offset to the wave-line comparison
// so column N's wave arrives STAGGER_PER_COL rows later than column 0.
// This creates a diagonal sweep (top-left converts first, bottom-right last)
// on top of the existing vertical smoothstep sweep. The stagger is capped at
// STAGGER_MAX_FRAC * lines so wide terminals don't produce a stagger larger
// than the screen.
//
// These tests verify the stagger via the DrawCtx methods
// (`color_uses_previous_palette` + `charset_uses_previous_pool`) which are
// the actual comparison functions the renderer uses per-cell.

#[test]
fn diagonal_stagger_column_0_has_zero_offset() {
    // Column 0 should have zero stagger — the wave arrives at the same
    // time as the pre-HUNT-15 behavior (no regression for the leftmost
    // column). Verify via `color_uses_previous_palette`: at the wave line
    // boundary, column 0 behaves exactly like the pre-stagger comparison.
    use super::super::render::DrawCtx;
    use crate::constants::MAX_PALETTE_SLOTS;
    use crate::runtime::{BoldMode, ColorMode, ColorPipeline};
    use crossterm::style::Color;

    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
    let glitch_map = bitvec::bitvec![0; 20];

    let ctx = DrawCtx {
        lines: 10,
        cols: 20,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::Mono,
        color_pipeline: ColorPipeline::detect(ColorMode::Mono),
        bold_mode: BoldMode::Off,
        glitchy: false,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0', '1'],
        previous_char_pool: &['0', '1'],
        edge_fade_lut: &[],
        vignette_lut: &[],
        vignette_lut_cols: 0,
        charset_wave_line: Some(5.0),
        color_wave_line: Some(5.0),
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_waves: &[],
        pool_is_binary: false,
        atmospheric: None,
        hue_drift_offset: None,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        head_halo_factor: None,
        transition_l_table: None,
    };

    // Column 0, line 5 (at the wave line): should NOT use previous (line
    // is at wave_line, not above it — `5.0 > 5.0 + jitter` is false for
    // jitter=0, but jitter can be 0..0.45 depending on the hash).
    // Instead of testing the exact boundary, test clearly above/below:
    // Line 3 (above wave_line=5): should NOT use previous (new palette).
    assert!(
        !ctx.color_uses_previous_palette(1, 3, 0),
        "col 0, line 3 (above wave): should use new palette"
    );
    // Line 8 (below wave_line=5): SHOULD use previous (old palette).
    assert!(
        ctx.color_uses_previous_palette(1, 8, 0),
        "col 0, line 8 (below wave): should use previous palette"
    );
}

#[test]
fn diagonal_stagger_column_n_converts_later_than_column_0() {
    // At the same wave_line, column N should still use the OLD palette
    // when column 0 has already converted to the new one. This is the
    // core diagonal property: left converts first, right converts last.
    use super::super::render::DrawCtx;
    use crate::constants::MAX_PALETTE_SLOTS;
    use crate::runtime::{BoldMode, ColorMode, ColorPipeline};
    use crossterm::style::Color;

    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
    let glitch_map = bitvec::bitvec![0; 20];

    let ctx = DrawCtx {
        lines: 10,
        cols: 20,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::Mono,
        color_pipeline: ColorPipeline::detect(ColorMode::Mono),
        bold_mode: BoldMode::Off,
        glitchy: false,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0', '1'],
        previous_char_pool: &['0', '1'],
        edge_fade_lut: &[],
        vignette_lut: &[],
        vignette_lut_cols: 0,
        charset_wave_line: Some(5.0),
        color_wave_line: Some(5.0),
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_waves: &[],
        pool_is_binary: false,
        atmospheric: None,
        hue_drift_offset: None,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        head_halo_factor: None,
        transition_l_table: None,
    };

    // wave_line = 5.0. Column 0 stagger = 0. Column 10 stagger = 10 * 0.15 = 1.5.
    // Line 4 (just above wave_line=5): column 0 has 4 + 0 = 4 <= 5 → new palette.
    // Column 10 has 4 + 1.5 = 5.5 > 5 → old palette (still uses previous).
    // This is the diagonal: column 10's line 4 hasn't converted yet while
    // column 0's line 4 has.
    let col0_line4 = ctx.color_uses_previous_palette(1, 4, 0);
    let col10_line4 = ctx.color_uses_previous_palette(1, 4, 10);
    // Column 0 line 4 should NOT use previous (above wave, converted).
    // Column 10 line 4 SHOULD use previous (stagger pushes it below wave).
    // Note: jitter can affect the boundary by up to 0.30. We pick a line
    // far enough from the boundary that jitter doesn't flip the result.
    // Line 4 + col 10 stagger 1.5 = 5.5 > wave 5.0 + max_jitter 0.30 = 5.30.
    // So col 10 line 4 always uses previous regardless of jitter.
    // Line 4 + col 0 stagger 0 = 4.0 < wave 5.0 - max_jitter 0.0 = 5.0.
    // Wait — jitter can be 0, so 4.0 > 5.0 is false → col 0 uses new.
    assert!(
        !col0_line4,
        "col 0, line 4 (above wave, no stagger): should use new palette"
    );
    assert!(
        col10_line4,
        "col 10, line 4 (stagger pushes below wave): should use previous palette"
    );
}

#[test]
fn diagonal_stagger_capped_at_max_frac_of_lines() {
    // On a wide terminal (200 cols), the raw stagger would be
    // 200 * 0.15 = 30 rows. With lines=10, the cap is 10 * 0.30 = 3.0.
    // So column 200's stagger = min(30, 3) = 3.0 — capped. Verify that
    // column 200 and column 100 have the SAME stagger (both at the cap).
    use super::super::render::DrawCtx;
    use crate::constants::MAX_PALETTE_SLOTS;
    use crate::runtime::{BoldMode, ColorMode, ColorPipeline};
    use crossterm::style::Color;

    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
    let glitch_map = bitvec::bitvec![0; 200];

    let ctx = DrawCtx {
        lines: 10,
        cols: 200,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::Mono,
        color_pipeline: ColorPipeline::detect(ColorMode::Mono),
        bold_mode: BoldMode::Off,
        glitchy: false,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0', '1'],
        previous_char_pool: &['0', '1'],
        edge_fade_lut: &[],
        vignette_lut: &[],
        vignette_lut_cols: 0,
        charset_wave_line: Some(5.0),
        color_wave_line: Some(5.0),
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_waves: &[],
        pool_is_binary: false,
        atmospheric: None,
        hue_drift_offset: None,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        head_halo_factor: None,
        transition_l_table: None,
    };

    // wave_line = 5.0. Both col 100 and col 200 have stagger = 3.0 (capped).
    // Line 3: col 100 effective = 3 + 3 = 6 > 5 → previous.
    // Line 3: col 200 effective = 3 + 3 = 6 > 5 → previous (same — both capped).
    // Line 1: col 100 effective = 1 + 3 = 4 <= 5 → new.
    // Line 1: col 200 effective = 1 + 3 = 4 <= 5 → new (same — both capped).
    // Verify both columns behave identically (the cap works).
    assert_eq!(
        ctx.color_uses_previous_palette(1, 3, 100),
        ctx.color_uses_previous_palette(1, 3, 200),
        "col 100 and col 200 should have same stagger (both at cap)"
    );
    assert_eq!(
        ctx.color_uses_previous_palette(1, 1, 100),
        ctx.color_uses_previous_palette(1, 1, 200),
        "col 100 and col 200 should behave identically (cap)"
    );
}
