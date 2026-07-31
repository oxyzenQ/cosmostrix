// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Perceptual L Smoothing at Palette Transition Wave
//!
//! Chroma Dragon Phase 5 — kills the brightness step at the palette
//! transition wave line.
//!
//! ## Problem
//!
//! When a palette switches (theme change), `color_wave_line` sweeps
//! top-to-bottom over `COLOR_TRANSITION_DURATION_MS` (300 ms). Cells
//! above the wave use the new palette; cells below use the old. If the
//! two palettes have different perceptual luminance (OKLab L) at
//! corresponding stop indices, the wave line becomes a visible brightness
//! discontinuity — a hard horizontal stripe where the scene's overall
//! brightness steps.
//!
//! ## Solution
//!
//! During the transition window, build a `TransitionLTable` containing
//! the OKLab L for each stop index in both the old and new palettes
//! (pre-computed once per frame, not per cell). Pass it through
//! `DrawCtx → ShaderCtx → resolve_cell_color`. In the shader, for each
//! cell within ±`TRANSITION_L_SMOOTHING_WINDOW` lines of the wave:
//!
//! 1. Look up `(L_old, L_new)` for the cell's stop index.
//! 2. Compute the cell's distance from the wave line.
//! 3. Blend factor = `0.5 * (1 - |distance| / window)` — peaks at 0.5
//!    at the wave line, falls off linearly to 0 at ±window.
//! 4. Target L = opposite palette's L (above wave → L_old, below → L_new).
//! 5. Smoothed L = `current_L + (target_L - current_L) * blend`.
//! 6. Convert sRGB → OKLab → set L → back to sRGB.
//!
//! The 0.5 peak blend (not 1.0) avoids a palette swap at the wave line —
//! cells exactly at the wave get a 50/50 midpoint between L_old and L_new,
//! producing a smooth gradient instead of a hard step.
//!
//! ## Cost
//!
//! Per smoothed cell: ~20 multiplies + 6 `cbrt()` (sRGB → OKLab → sRGB
//! round-trip). With `window = 3` lines on an 80×50 display, that's
//! ≤480 cells per frame during the 300 ms transition (≤8640 round-trips
//! per transition total). Negligible.
//!
//! Per non-smoothed cell: 1 Option check + 1 Reset check + 1 distance
//! comparison — all early-return paths. The shader's hot path stays cheap.
//!
//! ## Skip Conditions
//!
//! `apply_l_smoothing` returns the color unchanged when:
//! - `table` is `None` (transition inactive, or palettes lack RGB)
//! - `color` is `Color::Reset` (no RGB to modify)
//! - Cell is outside the smoothing window (`|distance| >= window`)
//! - `stop_idx` is out of the table's range (stop beyond the smaller
//!   palette's length, or skipped due to `Color::Reset` entries)
//! - `L_old == L_new` (no luminance difference — no smoothing needed)

use crossterm::style::Color;

use crate::chroma::gradient::{oklab_to_srgb, srgb_to_oklab};
use crate::chroma::palette::color_to_rgb;

/// Pre-computed OKLab L values for each stop index in both the old and
/// new palettes, plus the current wave line position and smoothing window.
///
/// Built once per frame in `rain.rs` when a palette transition is active,
/// then borrowed through `DrawCtx → ShaderCtx` so the shader can apply
/// per-cell L smoothing without recomputing the table.
///
/// `entries[i]` corresponds to stop index `i` in BOTH palettes (matched
/// by index). If the palettes have different lengths, the table is sized
/// to the smaller one — stops beyond the smaller palette's range have no
/// counterpart and are not smoothed. Stops that are `Color::Reset` in
/// either palette are skipped (no RGB to derive L from).
#[derive(Debug, Clone)]
pub struct TransitionLTable {
    /// `(L_old, L_new)` per stop index. Sparse — entries are only present
    /// for indices where BOTH palettes had a non-Reset Color. Indexed by
    /// the shader's `color_idx` (the resolved palette stop index).
    pub entries: Vec<(f32, f32)>,

    /// Current wave line position (in lines from top of screen). Cells
    /// above this line use the new palette; cells below use the old.
    pub wave_line: f32,

    /// Smoothing window in lines. Cells within ±`window` of `wave_line`
    /// get L smoothing applied; cells outside are untouched.
    pub window: f32,
}

impl TransitionLTable {
    /// Build a transition L table from the old and new palettes.
    ///
    /// Computes the OKLab L for each stop index in both palettes (matched
    /// by index). Stops that are `Color::Reset` in either palette are
    /// skipped — they have no RGB to derive L from, and the shader's
    /// `apply_l_smoothing` will early-return for those indices.
    ///
    /// Returns `None` if:
    /// - Either palette is empty
    /// - `window <= 0.0`
    /// - No stop index had a non-Reset color in BOTH palettes
    #[must_use]
    pub fn build(
        old_palette: &[Color],
        new_palette: &[Color],
        wave_line: f32,
        window: f32,
    ) -> Option<Self> {
        if old_palette.is_empty() || new_palette.is_empty() || window <= 0.0 {
            return None;
        }
        let min_len = old_palette.len().min(new_palette.len());
        let mut entries = Vec::with_capacity(min_len);
        for i in 0..min_len {
            let old_c = old_palette[i];
            let new_c = new_palette[i];
            if matches!(old_c, Color::Reset) || matches!(new_c, Color::Reset) {
                continue;
            }
            let (or_, og, ob) = color_to_rgb(old_c);
            let (nr, ng, nb) = color_to_rgb(new_c);
            let (l_old, _, _) = srgb_to_oklab(or_, og, ob);
            let (l_new, _, _) = srgb_to_oklab(nr, ng, nb);
            entries.push((l_old, l_new));
        }
        if entries.is_empty() {
            return None;
        }
        Some(Self {
            entries,
            wave_line,
            window,
        })
    }

    /// Look up `(L_old, L_new)` for a given stop index.
    ///
    /// Returns `None` if the index is out of range (the stop was skipped
    /// during build due to `Color::Reset`, or the index exceeds the
    /// smaller palette's length).
    #[inline]
    #[must_use]
    pub fn get(&self, stop_idx: usize) -> Option<(f32, f32)> {
        self.entries.get(stop_idx).copied()
    }
}

/// Apply perceptual L smoothing to a resolved cell color during a palette
/// transition.
///
/// See the module-level docs for the full rationale. In short: within
/// ±`window` lines of `wave_line`, blend the cell's OKLab L channel
/// toward the opposite palette's L for that stop index. The blend peaks
/// at 0.5 at the wave line (50% midpoint, no palette swap) and falls off
/// linearly to 0 at ±window.
///
/// # Arguments
///
/// - `color` — the resolved cell color (after palette lookup + head halo).
/// - `table` — the precomputed transition L table, or `None` if no
///   transition is active.
/// - `stop_idx` — the cell's resolved palette stop index (`color_idx`).
///   Negative values (shouldn't occur post-clamp) early-return.
/// - `line` — the cell's line number (row index from top).
///
/// # Returns
///
/// The smoothed color, or the original color if any skip condition
/// applies (see module-level docs).
#[inline]
pub fn apply_l_smoothing(
    color: Color,
    table: Option<&TransitionLTable>,
    stop_idx: i32,
    line: u16,
) -> Color {
    let Some(table) = table else {
        return color;
    };
    if matches!(color, Color::Reset) {
        return color;
    }

    // Distance check (cheap) before the more expensive table lookup +
    // OKLab conversion. Most cells in most frames are outside the window
    // — this early-return keeps the hot path cheap.
    let distance = f32::from(line) - table.wave_line;
    let abs_distance = distance.abs();
    if abs_distance >= table.window {
        return color;
    }

    // Bounds-check the stop index. Negative indices shouldn't occur
    // (the shader clamps color_idx to [0, last] before this point),
    // but defending against an i32 underflow is cheap.
    let Ok(stop_idx_u32) = u32::try_from(stop_idx) else {
        return color;
    };
    let Some((l_old, l_new)) = table.get(stop_idx_u32 as usize) else {
        return color;
    };

    // No smoothing needed if the two palettes have the same L for this
    // stop. The OKLab round-trip would be a no-op anyway — skip it.
    if (l_old - l_new).abs() < 0.001 {
        return color;
    }

    // Blend factor: 0.5 at the wave line (midpoint, no palette swap),
    // 0 at ±window (no change). Linear falloff — smoothstep is overkill
    // for a 3-line window.
    let blend = 0.5 * (1.0 - abs_distance / table.window);

    // Target L: above wave (distance > 0) the cell uses the new palette,
    // so blend toward the OLD palette's L. Below wave (distance < 0) the
    // cell uses the old palette, so blend toward the NEW palette's L.
    // This produces a smooth gradient from L_new (far above) → midpoint
    // (at wave) → L_old (far below).
    let target_l = if distance >= 0.0 { l_old } else { l_new };

    // sRGB → OKLab → adjust L → sRGB. The a/b chroma channels are
    // preserved, so only the perceptual lightness changes — hue and
    // saturation stay intact.
    let (r, g, b) = color_to_rgb(color);
    let (current_l, a, b_chroma) = srgb_to_oklab(r, g, b);
    let smoothed_l = current_l + (target_l - current_l) * blend;
    let (r, g, b) = oklab_to_srgb(smoothed_l, a, b_chroma);
    Color::Rgb { r, g, b }
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
