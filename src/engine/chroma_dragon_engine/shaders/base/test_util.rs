// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Test-support helpers shared by the four shader test modules
//! (`tests.rs`, `tests_activation.rs`, `tests_bold_audit.rs`,
//! `tests_hue_drift.rs`).
//!
//! Extracted from `base/mod.rs` in the NIGHT-hunter-25 part 2 LOC split
//! (the bundle refactor pushed the file over the 800-line hard cap).
//! The re-export in `base/mod.rs` keeps the `super::` glob and the
//! explicit `crate::...::base::` paths in the test modules resolving
//! unchanged.

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::constants::MAX_PALETTE_SLOTS;
use crate::runtime::{BoldMode, ColorMode};

use super::{CellPaint, CharLoc, ShaderCtx};

/// Test helper: build a minimal ShaderCtx for testing resolve_cell_color.
/// Caller supplies the `palette_slices` array (so it outlives the
/// ShaderCtx borrow) and the color_map slice. color_map is initialized
/// to a constant value in the tests so we can detect when the remap
/// overrides it.
pub(crate) fn make_test_shader<'a>(
    palette_slices: &'a [&'a [Color]; MAX_PALETTE_SLOTS],
    color_map: &'a [u8],
    shading_distance: bool,
) -> ShaderCtx<'a> {
    ShaderCtx {
        palette_slices,
        active_palette_slot: 0,
        color_wave_line: None,
        bold_mode: BoldMode::Random,
        lines: 50,
        color_map,
        shading_distance,
        glitchy: false,
        glitch_map: <&BitSlice>::default(),
        glitch_bright: false,
        glitch_dim: false,
        color_mode: ColorMode::TrueColor,
        column_coherence_lut: None,
        subpixel_jitter_amplitude: None,
        atmospheric: None,
        hue_drift_offset: None,
        head_halo_factor: None,
        transition_l_table: None,
        bg: None,
    }
}

/// Test helper: build a `MAX_PALETTE_SLOTS`-sized palette_slices array with
/// slot 0 pointing to the given palette and all other slots empty.
pub(crate) fn slot_array(palette: &[Color]) -> [&[Color]; MAX_PALETTE_SLOTS] {
    let mut arr: [&[Color]; MAX_PALETTE_SLOTS] = [&[]; MAX_PALETTE_SLOTS];
    arr[0] = palette;
    arr
}

/// Test helper: build a `CellPaint` with the common test fixture values
/// (palette_slot 0, glyph 'x') and the five per-case variables. Keeps the
/// 47 shader test call sites one line each after the NIGHT-hunter-25 part 2
/// bundle refactor; call sites that need other slots/glyphs construct the
/// struct literal directly.
pub(crate) fn test_paint(
    line: u16,
    col: u16,
    loc: CharLoc,
    head_put_line: u16,
    length: u16,
) -> CellPaint {
    CellPaint {
        palette_slot: 0,
        line,
        col,
        val: 'x',
        loc,
        head_put_line,
        length,
    }
}
