// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! PaletteLadder contract tests (NIGHT-lts-5b).
//!
//! The ladder hoist moved the level-to-palette-stop index equations
//! out of `color_for_level`'s per-cell path into a per-frame
//! `PaletteLadder`. These tests pin the equations against an
//! independent re-derivation (the pre-lts-5b inline math, restated
//! here on purpose so a regression in `from_len` cannot hide behind
//! itself), the `from_slices` per-slot alignment, and the exact
//! stop each level resolves to through `color_for_level`.

use crate::cloud::monolith::BrightnessLevel;
use crate::cloud::render::{DrawCtx, PaletteLadder};
use crate::constants::MAX_PALETTE_SLOTS;
use crate::runtime::{BoldMode, ColorMode, ColorPipeline};
use crossterm::style::Color;

/// The pre-NIGHT-lts-5b inline math, restated verbatim as the oracle.
fn legacy_indices(len: usize) -> [usize; 4] {
    let last = len.saturating_sub(1);
    let first_visible = usize::from(last > 0);
    let ghost_idx = (last / 3).max(first_visible);
    let mid_idx = (last * 3) / 5;
    let hot_idx = (last * 17) / 20;
    [ghost_idx, mid_idx, hot_idx, last]
}

#[test]
fn ladder_matches_the_legacy_equations_across_lengths() {
    // Lengths that exercise every branch: 0/1 (degenerate, never
    // indexed in production but pinned for safety), 2-4 (small
    // palettes), 5/10/16 (common themes), 20 and 255 (extremes).
    for len in [0, 1, 2, 3, 4, 5, 10, 16, 20, 255] {
        let [ghost, mid, hot, core] = legacy_indices(len);
        let ladder = PaletteLadder::from_len(len);
        assert_eq!(
            (
                ladder.ghost_idx,
                ladder.mid_idx,
                ladder.hot_idx,
                ladder.core_idx
            ),
            (ghost, mid, hot, core),
            "len {len}: ladder must match the legacy equations"
        );
    }
}

#[test]
fn ladder_indices_stay_inside_the_palette() {
    for len in 1..=64usize {
        let ladder = PaletteLadder::from_len(len);
        for idx in [
            ladder.ghost_idx,
            ladder.mid_idx,
            ladder.hot_idx,
            ladder.core_idx,
        ] {
            assert!(idx < len, "len {len}: index {idx} out of bounds");
        }
    }
}

#[test]
fn ladder_monotonicity_ghost_mid_hot_core() {
    // The v17 mastery contract: brightness strictly orders
    // Ghost <= Mid <= Hot <= Core for every palette length from 3
    // stops up (the cinematic depth hierarchy the depth tests pin
    // visually). Note the legacy quirk preserved verbatim by the
    // hoist: at len 2 (last=1) the ghost stop pins to the visible
    // floor 1 while mid/hot sit at 0 — the pre-lts-5b equations
    // behave the same way, and lts-5b changes values nowhere.
    for len in 3..=64usize {
        let ladder = PaletteLadder::from_len(len);
        assert!(ladder.ghost_idx <= ladder.mid_idx, "len {len}");
        assert!(ladder.mid_idx <= ladder.hot_idx, "len {len}");
        assert!(ladder.hot_idx <= ladder.core_idx, "len {len}");
    }
    // The len-2 quirk itself, pinned as a preserved-behavior guard.
    let len2 = PaletteLadder::from_len(2);
    assert_eq!(
        (len2.ghost_idx, len2.mid_idx, len2.hot_idx, len2.core_idx),
        (1, 0, 0, 1)
    );
}

#[test]
fn from_slices_aligns_per_slot_lengths() {
    let a: Vec<Color> = (0..10)
        .map(|i| Color::Rgb {
            r: 0,
            g: (i * 20) as u8,
            b: 0,
        })
        .collect();
    let b: Vec<Color> = (0..3)
        .map(|i| Color::Rgb {
            r: 0,
            g: 0,
            b: (i * 60) as u8,
        })
        .collect();
    let empty: &[Color] = &[];
    let slices: [&[Color]; MAX_PALETTE_SLOTS] = [&a, &b, empty, &a];
    let ladders = PaletteLadder::from_slices(&slices);
    for (ladder, slice) in ladders.iter().zip(slices.iter()) {
        assert_eq!(*ladder, PaletteLadder::from_len(slice.len()));
    }
    // Empty slots carry the all-zero placeholder.
    assert_eq!(ladders[2], PaletteLadder::EMPTY);
}

#[test]
fn color_for_level_resolves_the_ladder_stops_exactly() {
    // A 10-stop palette with distinct values: the ladder maps
    // Ghost/Dim -> 3, Mid -> 5, Hot -> 7, Core -> 9 (the legacy
    // equations at len 10). factor 1.0 skips both blend stages, so
    // each level must resolve to the exact stop color.
    let colors: Vec<Color> = (0..10)
        .map(|i| Color::Rgb {
            r: (i * 25) as u8,
            g: (i * 25) as u8,
            b: 0,
        })
        .collect();
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [&colors, empty, empty, empty];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 10,
        cols: 10,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::TrueColor,
        color_pipeline: ColorPipeline::detect(ColorMode::TrueColor),
        bold_mode: BoldMode::Off,
        glitchy: false,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        palette_ladders: PaletteLadder::from_slices(&palette_slices),
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0'],
        previous_char_pool: &['0'],
        edge_fade_lut: &[],
        vignette_lut: &[],
        vignette_lut_cols: 0,
        charset_wave_line: None,
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_waves: &[],
        pool_is_binary: true,
        atmospheric: None,
        hue_drift_offset: None,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        head_halo_factor: None,
        transition_l_table: None,
    };

    let stop =
        |level: BrightnessLevel| crate::cloud::monolith::color_for_level(&ctx, 0, 0, 0, level, 1.0);
    // Ghost and Dim share the ghost stop.
    assert_eq!(stop(BrightnessLevel::Ghost), Some(colors[3]));
    assert_eq!(stop(BrightnessLevel::Dim), Some(colors[3]));
    assert_eq!(stop(BrightnessLevel::Mid), Some(colors[5]));
    assert_eq!(stop(BrightnessLevel::Hot), Some(colors[7]));
    // Core blends toward white (MONOLITH_CORE_WHITE_BLEND) on top of
    // stop 9, so it must differ from the raw stop while staying the
    // brightest level (pinned by the depth suite); here we pin the
    // exact base: a factor-1.0 Core must still be Some (never None
    // for a non-empty palette).
    assert!(stop(BrightnessLevel::Core).is_some());
}

#[test]
fn empty_palette_slots_still_resolve_to_none() {
    let empty: &[Color] = &[];
    let palette_slices: [&[Color]; MAX_PALETTE_SLOTS] = [empty; MAX_PALETTE_SLOTS];
    let glitch_map = bitvec::bitvec![0; 100];
    let ctx = DrawCtx {
        lines: 10,
        cols: 10,
        shading_distance: false,
        bg: None,
        color_mode: ColorMode::TrueColor,
        color_pipeline: ColorPipeline::detect(ColorMode::TrueColor),
        bold_mode: BoldMode::Off,
        glitchy: false,
        glitch_bright: false,
        glitch_dim: true,
        palette_slices,
        palette_ladders: PaletteLadder::from_slices(&palette_slices),
        active_palette_slot: 0,
        transitioning: false,
        color_map: &[],
        glitch_map: glitch_map.as_bitslice(),
        char_pool: &['0'],
        previous_char_pool: &['0'],
        edge_fade_lut: &[],
        vignette_lut: &[],
        vignette_lut_cols: 0,
        charset_wave_line: None,
        color_wave_line: None,
        mouse_col: u16::MAX,
        mouse_line: u16::MAX,
        flash_waves: &[],
        pool_is_binary: true,
        atmospheric: None,
        hue_drift_offset: None,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        head_halo_factor: None,
        transition_l_table: None,
    };
    assert_eq!(
        crate::cloud::monolith::color_for_level(&ctx, 0, 0, 0, BrightnessLevel::Mid, 1.0),
        None
    );
}
