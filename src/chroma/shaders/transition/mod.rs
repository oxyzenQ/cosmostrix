// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Perceptual Lab Smoothing at Palette Transition Wave
//!
//! Chroma Dragon Phase 5 — kills the brightness step at the palette
//! transition wave line (L channel smoothing).
//!
//! Chroma Dragon Phase 8 — extends smoothing to the OKLab a/b chroma
//! channels using polar (chroma magnitude + hue angle) interpolation,
//! so hue rotates through the natural chroma ring instead of cutting
//! through the desaturated gray center. This is the "hue-preserving
//! variant for transitions" called out as next-move 2.
//!
//! ## Problem (Phase 5)
//!
//! When a palette switches (theme change), `color_wave_line` sweeps
//! top-to-bottom over `COLOR_TRANSITION_DURATION_MS` (300 ms). Cells
//! above the wave use the new palette; cells below use the old. If the
//! two palettes have different perceptual luminance (OKLab L) at
//! corresponding stop indices, the wave line becomes a visible brightness
//! discontinuity — a hard horizontal stripe where the scene's overall
//! brightness steps.
//!
//! ## Problem (Phase 8)
//!
//! Phase 5 only smoothed L, leaving a/b chroma to hard-snap at the wave
//! line. When two palettes have different hues at the same stop index
//! (e.g. Green palette → Red palette at stop 5), the wave line shows a
//! hard hue step: bright-green-line-above, bright-red-line-below, no
//! transitional hue between them. The eye reads this as a color seam.
//!
//! Naively extending Phase 5 by linearly interpolating (a, b) in
//! Cartesian coordinates would interpolate through the OKLab chroma
//! plane's interior — but for opposing hues (red ↔ cyan, blue ↔ yellow)
//! the straight-line path passes near (a=0, b=0), producing a desaturated
//! gray midpoint. The wave line would dissolve into gray before
//! resolving to the new hue — a "washed-out blink" effect.
//!
//! ## Solution (Phase 8)
//!
//! Extend the per-stop table to carry the full OKLab triple `(L, a, b)`
//! for both old and new palettes (not just L). In the shader, for each
//! cell within ±`TRANSITION_L_SMOOTHING_WINDOW` lines of the wave,
//! smooth BOTH L and chroma:
//!
//! 1. L: linear interpolation (existing Phase 5 behavior, unchanged).
//! 2. Chroma magnitude `c = sqrt(a^2 + b^2)`: linear interpolation.
//! 3. Hue angle `h = atan2(b, a)`: shortest-arc angular interpolation.
//! 4. Reconstruct (a, b) from `(c_smoothed, h_smoothed)`.
//!
//! Shortest-arc hue interpolation picks the direction (clockwise or
//! counter-clockwise around the chroma ring) that gives the smaller
//! angular delta. This ensures red → cyan rotates through either
//! magenta or yellow (whichever is shorter) rather than cutting through
//! gray.
//!
//! ### Special case: zero chroma
//!
//! When either palette's stop has chroma = 0 (a grayscale color),
//! `atan2(0, 0) = 0` is undefined as a hue. Phase 8 falls back to
//! Cartesian (a, b) lerp for these stops — the gray midpoint is the
//! correct answer anyway, since rotating hue from "no hue" to any hue
//! is meaningless.
//!
//! ## Cost
//!
//! Per smoothed cell (Phase 5): ~20 multiplies + 6 `cbrt()`.
//! Per smoothed cell (Phase 8): same as Phase 5 plus 2 `atan2()`, 2
//! `sqrt()`, 2 `sin()`, 2 `cos()`. The trig functions are the expensive
//! part — ~80 ns each on modern x86_64. Total per smoothed cell: ~400 ns
//! vs ~200 ns for Phase 5. With window = 3 lines on an 80×50 display,
//! that's ≤480 cells per frame during the 300 ms transition: ≤192
//! µs/frame extra. Still negligible.
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
//! - `L_old == L_new AND a_old == a_new AND b_old == b_new` (no
//!   perceptual difference — no smoothing needed)

use crossterm::style::Color;

use crate::chroma::gradient::{oklab_to_srgb, polar_chroma_lerp, srgb_to_oklab};
use crate::chroma::palette::color_to_rgb;

/// One stop's OKLab values in both the old and new palettes.
///
/// Phase 8 extended this from `(L_old, L_new)` to the full OKLab triple
/// per side, so the shader can apply polar chroma smoothing in addition
/// to L smoothing. The struct makes the field names self-documenting
/// and avoids the readability cliff of a 6-tuple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransitionLabEntry {
    /// OKLab L for this stop in the OLD palette.
    pub l_old: f32,
    /// OKLab a (green↔red axis) for this stop in the OLD palette.
    pub a_old: f32,
    /// OKLab b (blue↔yellow axis) for this stop in the OLD palette.
    pub b_old: f32,
    /// OKLab L for this stop in the NEW palette.
    pub l_new: f32,
    /// OKLab a (green↔red axis) for this stop in the NEW palette.
    pub a_new: f32,
    /// OKLab b (blue↔yellow axis) for this stop in the NEW palette.
    pub b_new: f32,
}

/// Pre-computed OKLab values for each stop index in both the old and
/// new palettes, plus the current wave line position and smoothing window.
///
/// Built once per frame in `rain.rs` when a palette transition is active,
/// then borrowed through `DrawCtx → ShaderCtx` so the shader can apply
/// per-cell L + chroma smoothing without recomputing the table.
///
/// `entries[i]` corresponds to stop index `i` in BOTH palettes (matched
/// by index). If the palettes have different lengths, the table is sized
/// to the smaller one — stops beyond the smaller palette's range have no
/// counterpart and are not smoothed. Stops that are `Color::Reset` in
/// either palette are skipped (no RGB to derive OKLab from).
///
/// The struct name `TransitionLTable` is preserved for backward
/// compatibility with the Phase 5 plumbing (DrawCtx, ShaderCtx,
/// rain.rs builder). The actual content is now full OKLab per stop
/// (Phase 8).
#[derive(Debug, Clone)]
pub(crate) struct TransitionLTable {
    /// Per-stop OKLab values for both palettes. Sparse — entries are only
    /// present for indices where BOTH palettes had a non-Reset Color.
    /// Indexed by the shader's `color_idx` (the resolved palette stop
    /// index).
    pub entries: Vec<TransitionLabEntry>,

    /// Current wave line position (in lines from top of screen). Cells
    /// above this line use the new palette; cells below use the old.
    pub wave_line: f32,

    /// Smoothing window in lines. Cells within ±`window` of `wave_line`
    /// get L + chroma smoothing applied; cells outside are untouched.
    pub window: f32,
}

impl TransitionLTable {
    /// Build a transition Lab table from the old and new palettes.
    ///
    /// Computes the full OKLab triple `(L, a, b)` for each stop index in
    /// both palettes (matched by index). Stops that are `Color::Reset`
    /// in either palette are skipped — they have no RGB to derive OKLab
    /// from, and the shader's `apply_l_smoothing` will early-return for
    /// those indices.
    ///
    /// Returns `None` if:
    /// - Either palette is empty
    /// - `window <= 0.0`
    /// - No stop index had a non-Reset color in BOTH palettes
    #[must_use]
    pub(crate) fn build(
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
            let (l_old, a_old, b_old) = srgb_to_oklab(or_, og, ob);
            let (l_new, a_new, b_new) = srgb_to_oklab(nr, ng, nb);
            entries.push(TransitionLabEntry {
                l_old,
                a_old,
                b_old,
                l_new,
                a_new,
                b_new,
            });
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

    /// Look up `(L_old, L_new)` for a given stop index. Phase 5 shim —
    /// returns only the L fields of the full OKLab entry. The shader
    /// itself reads `entries.get(idx)` directly to access the Phase 8
    /// chroma fields, but this shim is kept for any external callers
    /// that only need L (e.g. diagnostic tools, future shader
    /// innovations that only consume L).
    ///
    /// Returns `None` if the index is out of range (the stop was skipped
    /// during build due to `Color::Reset`, or the index exceeds the
    /// smaller palette's length).
    #[inline]
    #[cfg(test)]
    pub(crate) fn get(&self, stop_idx: usize) -> Option<(f32, f32)> {
        self.entries.get(stop_idx).map(|e| (e.l_old, e.l_new))
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
pub(crate) fn apply_l_smoothing(
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
    let Some(entry) = table.entries.get(stop_idx_u32 as usize) else {
        return color;
    };

    // No smoothing needed if the two palettes have the same L AND the same
    // chroma for this stop. The OKLab round-trip would be a no-op anyway.
    // Phase 8 widened this check from L-only to full OKLab equality.
    let l_same = (entry.l_old - entry.l_new).abs() < 0.001;
    let a_same = (entry.a_old - entry.a_new).abs() < 0.001;
    let b_same = (entry.b_old - entry.b_new).abs() < 0.001;
    if l_same && a_same && b_same {
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
    let target_l = if distance >= 0.0 {
        entry.l_old
    } else {
        entry.l_new
    };

    // Phase 8: target (a, b) is the OPPOSITE palette's chroma — same logic
    // as L. Above wave (cell uses new palette) blend toward old's (a, b).
    // Below wave blend toward new's (a, b). The blend factor is the same
    // `blend` used for L, so L and chroma smooth in lockstep.
    let (target_a, target_b) = if distance >= 0.0 {
        (entry.a_old, entry.b_old)
    } else {
        (entry.a_new, entry.b_new)
    };

    // sRGB → OKLab. We adjust L (Phase 5) and (a, b) (Phase 8) before
    // converting back.
    let (r, g, b) = color_to_rgb(color);
    let (current_l, current_a, current_b) = srgb_to_oklab(r, g, b);

    // Phase 5: L linear interpolation (perceptual brightness).
    let smoothed_l = current_l + (target_l - current_l) * blend;

    // Phase 8: chroma smoothing. Use polar (chroma, hue) interpolation
    // so hue rotates through the natural chroma ring instead of cutting
    // through the desaturated gray center.
    //
    // The cell's CURRENT (a, b) is on the active palette's chroma arc
    // (possibly already adjusted by L smoothing — but L adjustment
    // preserves a/b exactly, so current_(a,b) == the active palette's
    // (a, b) for this stop). The TARGET (a, b) is the opposite palette's
    // chroma. We interpolate between them via polar coords.
    let (smoothed_a, smoothed_b) =
        polar_chroma_lerp(current_a, current_b, target_a, target_b, blend);

    let (r, g, b) = oklab_to_srgb(smoothed_l, smoothed_a, smoothed_b);
    Color::Rgb { r, g, b }
}

// Phase 8 polar chroma lerping is implemented in `chroma::gradient::polar_chroma_lerp`
// and imported above. See that function for the full rationale (shortest-arc
// hue rotation, grayscale fallback) — the same hue-rotation logic is used
// by the production gradient builder `gradient_from_stops_oklab` (sole path
// since v30 — Cartesian variant removed).

#[cfg(test)]
mod tests;
