// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Base Cell Shader — `resolve_cell_color()`
//!
//! This is the convergence point where palette, position, glyph, transition,
//! and head-state signals resolve to a single `(foreground, bold)` pair.
//!
//! Extracted verbatim from `cloud::render::DrawCtx::get_attr()` in Chroma
//! Dragon Phase 2. The body is identical; only the receiver changed from
//! `&DrawCtx` (which carries many non-color fields) to `&ShaderCtx` (which
//! carries only the inputs the shader actually reads).
//!
//! ## Performance
//!
//! Called 100–300× per frame on the hot path. `ShaderCtx` is a thin borrow
//! view — no allocation, no virtual dispatch. The function and its helper
//! `color_uses_previous_palette()` are marked `#[inline]` so LLVM can fold
//! the `DrawCtx → ShaderCtx → resolve_cell_color` chain at the call site,
//! yielding identical codegen to the pre-extraction monolith.

use bitvec::prelude::BitSlice;
use crossterm::style::Color;

use crate::constants::*;
use crate::runtime::{BoldMode, ColorMode};

/// Position of a cell within its droplet — determines which palette stop to
/// resolve. Moved here from `cloud::render` in Phase 2 because it is a pure
/// shader input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharLoc {
    Middle,
    Tail,
    /// Multi-cell tail segment for front-layer droplets. `seg` is the
    /// position within the tail region (0 = furthest from head = darkest,
    /// increasing toward body). `total` is the total number of tail cells
    /// for this droplet (1..=FRONT_LAYER_TAIL_MAX_CELLS).
    ///
    /// The seg index is scaled linearly across palette color stops
    /// 0..FRONT_LAYER_MAX_TAIL_STOPS so the tail fades smoothly from the
    /// darkest stop to the body transition regardless of how many tail
    /// cells the droplet has. This keeps the 3-stop tail gradient while
    /// allowing long front-layer droplets to have proportional tails.
    ///
    /// Only used for layer 2 (front). Mid/back layers use `CharLoc::Tail`
    /// (single cell, color_idx=0) to preserve the existing 3-2-2 distribution.
    TailN {
        seg: u8,
        total: u8,
    },
    Head,
}

/// Read-only borrow view of the per-frame inputs that `resolve_cell_color`
/// actually reads. Constructed on-the-fly from `DrawCtx` (or any future
/// shader data source) at the call site — no allocation, just copies of
/// references and cheap scalars.
///
/// Carrying only the shader-visible subset (not the full `DrawCtx`) keeps
/// the borrow footprint small and makes future shader innovations (OKLab
/// gradient, dither LUT, atmospheric state) easy to add as new fields
/// without touching the renderer.
pub struct ShaderCtx<'a> {
    /// Per-slot palette color arrays for generation-based rendering.
    /// Index by droplet's `palette_slot` to resolve its birth palette.
    pub palette_slices: &'a [&'a [Color]; MAX_PALETTE_SLOTS],

    /// Which palette slot is the currently active (latest) one.
    /// Used for transition glow effects on new-generation streams.
    pub active_palette_slot: u8,

    /// Color transition wave line: during a palette transition, rows above
    /// this value use the new (active) palette; rows below use their birth
    /// palette. Sweeps from 0 to lines+1 over COLOR_TRANSITION_DURATION_MS,
    /// creating a top-to-bottom wave that matches the charset transition.
    pub color_wave_line: Option<f32>,

    pub bold_mode: BoldMode,
    pub lines: u16,
    pub color_map: &'a [u8],

    pub shading_distance: bool,

    pub glitchy: bool,
    pub glitch_map: &'a BitSlice,

    /// Cached `is_bright(now)` snapshot — avoids per-cell Instant arithmetic
    /// in the glitch branch (called 100–300×/frame when glitchy).
    pub glitch_bright: bool,
    /// Cached `is_dim(now)` snapshot.
    pub glitch_dim: bool,

    pub color_mode: ColorMode,
}

/// Precomputed exponential decay lookup table for trail brightness.
/// Maps 256 normalized distances → exp(-TRAIL_EXPONENTIAL_K * t).
/// Eliminates ~3,000 exp() calls per frame in shading_distance mode.
///
/// Moved here from `cloud::render` in Phase 2 — it is a shader resource
/// owned by the chroma engine, not by the renderer.
pub(crate) static TRAIL_EXP_LUT: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *entry = (-(TRAIL_EXPONENTIAL_K as f32) * t).exp();
    }
    lut
});

/// Bayer 4×4 ordered dithering threshold matrix.
///
/// Each entry is in {0..=15}. The cell at `(line, col)` reads
/// `BAYER_4X4[line & 3][col & 3]`, divides by 16, and compares against the
/// fractional part of the continuous color value to decide whether to round
/// up or down. The matrix is laid out so the spatial average of the
/// up/down decisions equals undithered rounding — no brightness shift,
/// just banding broken into fine-grain texture.
///
/// Phase 3-B (Chroma Dragon Innovation B): eliminates visible banding on
/// long shading-distance droplets where many cells would otherwise share
/// the same `color_idx`.
const BAYER_4X4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Read the Bayer 4×4 threshold for a cell at `(line, col)`.
/// Returns a value in `{0..=15}`. Indexed by `line mod 4` and `col mod 4`
/// so the pattern tiles every 4×4 block.
#[inline]
fn bayer_threshold(line: u16, col: u16) -> u8 {
    // Bitwise AND with 3 is equivalent to % 4 but avoids the division.
    BAYER_4X4[(line as usize) & 3][(col as usize) & 3]
}

/// During a color transition, returns whether a cell at `(line, col)` should
/// use its birth (previous) palette rather than the new (active) palette.
/// Rows below the wave line use the old palette; rows above use the new.
/// This creates a top-to-bottom cascade matching the charset transition.
///
/// Extracted as a free function so both `DrawCtx::color_uses_previous_palette`
/// (called from `monolith.rs`) and `resolve_cell_color` share one source of
/// truth — previously the shader inlined its own copy of the wave test.
#[inline]
pub fn color_uses_previous_palette(
    color_wave_line: Option<f32>,
    active_palette_slot: u8,
    palette_slot: u8,
    line: u16,
    col: u16,
) -> bool {
    let Some(wave_line) = color_wave_line else {
        return false;
    };
    // Only applies to droplets that still carry the old palette slot
    if palette_slot == active_palette_slot {
        return false;
    }
    // Jitter for organic edge (same pattern as charset wave)
    let jitter =
        (((line as u32).wrapping_mul(13) ^ (col as u32).wrapping_mul(29)) % 3) as f32 * 0.15;
    (line as f32) > wave_line + jitter
}

/// Resolve a single cell's `(foreground, bold)` attribute pair.
///
/// This is the renderer's convergence point for palette, position, glyph,
/// transition, and head-state signals. Pure function — no hidden state,
/// no allocation, no side effects.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn resolve_cell_color(
    shader: &ShaderCtx<'_>,
    palette_slot: u8,
    line: u16,
    col: u16,
    val: char,
    loc: CharLoc,
    head_put_line: u16,
    length: u16,
) -> (Option<Color>, bool) {
    // Resolve this stream's palette from the generation table.
    // During a color transition, cells above the wave line adopt the new
    // (active) palette even if the droplet was born with the old one,
    // creating a visible top-to-bottom cascade.
    let effective_slot = if color_uses_previous_palette(
        shader.color_wave_line,
        shader.active_palette_slot,
        palette_slot,
        line,
        col,
    ) {
        palette_slot // Below wave: keep birth palette
    } else {
        shader.active_palette_slot // Above wave or no transition: use new palette
    };
    let palette_colors = if (effective_slot as usize) < MAX_PALETTE_SLOTS {
        shader.palette_slices[effective_slot as usize]
    } else {
        // Fallback: use active palette for invalid slots
        shader.palette_slices[shader.active_palette_slot as usize]
    };

    let mut bold = false;
    if shader.bold_mode == BoldMode::Random {
        bold = (((line as u32) ^ (val as u32)) % 2) == 1;
    }

    let idx = col as usize * shader.lines as usize + line as usize;
    // Cosmic Dragon egg #15: bounds-check + direct indexing for color_map.
    // color_map is sized cols*lines. Callers pass col < cols and
    // line < lines (from droplet iteration), but defensive check is cheap
    // and avoids Option alloc on the hot path.
    let mut color_idx = if idx < shader.color_map.len() {
        shader.color_map[idx] as i32
    } else {
        0
    };

    if shader.shading_distance {
        let last = palette_colors.len().saturating_sub(1) as u64;
        let dist = head_put_line.saturating_sub(line) as f64;
        let len = length.max(1) as f64;

        // Exponential decay: brightness = exp(-k * distance/length)
        let normalized_dist = (dist / len).clamp(0.0, 1.0);
        // OPTIMIZED: use precomputed LUT instead of exp() per cell
        let lut_idx = (normalized_dist * 255.0) as usize;
        let brightness = TRAIL_EXP_LUT[lut_idx.min(255)];
        let v_continuous = brightness * last as f32;

        // Phase 3-B (Chroma Dragon Innovation B): Bayer 4×4 ordered dithering.
        //
        // Rounding `v_continuous` to the nearest integer produces visible
        // banding — long droplets where many cells share the same distance
        // bucket all land on the same `color_idx`, so the trail shows hard
        // stair-steps instead of a smooth gradient. Bayer dithering splits
        // the rounding decision per-cell using a 4×4 threshold matrix:
        // neighboring cells alternate between adjacent palette stops, and
        // the eye perceives an intermediate color.
        //
        // The matrix is laid out so the spatial average of up/down decisions
        // equals undithered rounding — no brightness shift, just banding
        // broken into fine-grain texture. Threshold ∈ {0..15}/16, indexed
        // by (line mod 4, col mod 4) so the pattern tiles seamlessly.
        let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
        let frac = v_continuous - v_continuous.floor();
        let mut v = if frac > bayer_t {
            (v_continuous.floor() as u64 + 1).min(last)
        } else {
            v_continuous.floor() as u64
        };

        // Bloom: cells right behind head get extra brightness
        if dist < HEAD_BLOOM_CELLS as f64 {
            v = (v + 1).min(last);
        }

        color_idx = v as i32;
    }

    // Cosmic Dragon egg #16: bounds-check + direct indexing for glitch_map.
    // idx = col*lines + line, same as color_map above. Already computed.
    if shader.glitchy && idx < shader.glitch_map.len() && shader.glitch_map[idx] {
        // PERF: glitch_bright/glitch_dim are cached once per DrawCtx
        // construction (rain_at) — they depend only on `now`, not on
        // cell position, so recomputing per-cell was pure waste.
        if shader.glitch_bright {
            color_idx += 1;
            bold = true;
        } else if shader.glitch_dim {
            color_idx -= 1;
            bold = false;
        }
    }

    let last = palette_colors.len().saturating_sub(1) as i32;
    match loc {
        CharLoc::Tail => {
            color_idx = 0;
            bold = false;
        }
        CharLoc::TailN { seg, total } => {
            // Front-layer multi-cell tail: scale seg across palette color
            // stops 0..FRONT_LAYER_MAX_TAIL_STOPS. seg=0 → darkest
            // (furthest from head), seg=total-1 → brightest tail stop
            // (closest to body). When total > MAX_STOPS, multiple cells
            // share a stop, producing a smooth gradient even for long
            // proportional tails. Clamped to valid palette range.
            let max_stop = (crate::constants::FRONT_LAYER_MAX_TAIL_STOPS as i32).min(last);
            let total_cells = (total as i32).max(1);
            // Map seg [0, total-1] to color_idx [0, max_stop] linearly.
            // Using (max_stop + 1) as numerator ensures seg=total-1 maps
            // exactly to max_stop (no off-by-one at the bright end).
            let scaled = (seg as i32 * (max_stop + 1)) / total_cells;
            color_idx = scaled.min(max_stop).max(0);
            bold = false;
        }
        CharLoc::Head => {
            color_idx = last;
            bold = true;
        }
        CharLoc::Middle => {
            color_idx = color_idx.clamp(0, last.max(0));
        }
    }

    match shader.bold_mode {
        BoldMode::Off => bold = false,
        BoldMode::All => bold = true,
        BoldMode::Random => {}
    }

    let fg = if shader.color_mode == ColorMode::Mono {
        None
    } else {
        palette_colors.get(color_idx as usize).copied()
    };

    (fg, bold)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bayer matrix tiles every 4×4 block — same (line, col) mod 4 returns
    /// the same threshold.
    #[test]
    fn bayer_threshold_tiles_4x4() {
        for line in 0..16u16 {
            for col in 0..16u16 {
                let a = bayer_threshold(line, col);
                let b = bayer_threshold(line + 4, col + 4);
                assert_eq!(a, b, "Bayer threshold must tile every 4×4 block");
            }
        }
    }

    /// Bayer matrix is a permutation of {0..=15} — every threshold appears
    /// exactly once per 4×4 tile. This is what makes the spatial average
    /// equal undithered rounding.
    #[test]
    fn bayer_matrix_is_permutation_of_0_to_15() {
        let mut seen = [false; 16];
        for row in &BAYER_4X4 {
            for &v in row {
                let v = v as usize;
                assert!(v < 16, "Bayer entry out of range: {v}");
                assert!(!seen[v], "Bayer entry {v} appears more than once");
                seen[v] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "Not all thresholds 0..=15 present");
    }

    /// Bayer dithering preserves the spatial average: averaging the
    /// up/down decision across a full 4×4 tile equals undithered rounding
    /// of `v_continuous + 0.5/16` (a tiny bias from the threshold layout).
    /// In practice this means no brightness shift — just banding broken
    /// into fine texture.
    #[test]
    fn bayer_dither_preserves_spatial_average() {
        // Pick a continuous value whose fractional part is exactly 0.5 —
        // the worst case for rounding bias.
        let v_continuous = 4.5_f32;
        let frac = v_continuous - v_continuous.floor();
        assert!((frac - 0.5).abs() < 1e-6);

        // For each of the 16 cells in a 4×4 tile, compute the dithered v.
        let mut sum: u64 = 0;
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                sum += v;
            }
        }
        // Average should be ~4.5 (between 4 and 5). With 16 cells and
        // thresholds {0..15}/16, exactly 8 cells round up (frac > t when
        // t ∈ {0..7}/16) and 8 round down. So sum = 8*4 + 8*5 = 72,
        // average = 4.5 — exactly the continuous value.
        assert_eq!(sum, 72, "16-cell sum should be 8*4 + 8*5 = 72");
        let avg = sum as f32 / 16.0;
        assert!(
            (avg - v_continuous).abs() < 1e-6,
            "Spatial average {avg} should equal continuous value {v_continuous}"
        );
    }

    /// Bayer dithering rounds down to floor when frac=0 (no fractional part).
    #[test]
    fn bayer_dither_rounds_down_at_zero_frac() {
        let v_continuous = 4.0_f32; // no fractional part
        let frac = v_continuous - v_continuous.floor();
        assert!(frac < 1e-6);
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                assert_eq!(v, 4, "Zero frac should always round down");
            }
        }
    }

    /// Bayer dithering rounds up to ceil when frac ≥ 15/16 (nearly 1.0).
    #[test]
    fn bayer_dither_rounds_up_at_near_one_frac() {
        let v_continuous = 4.9375_f32; // frac = 15/16
        let frac = v_continuous - v_continuous.floor();
        // 15/16 = 0.9375; only the threshold 15/16 itself fails (frac > t
        // is false when frac == t). So 15 of 16 cells round up.
        let mut ups = 0;
        let mut downs = 0;
        for line in 0..4u16 {
            for col in 0..4u16 {
                let bayer_t = bayer_threshold(line, col) as f32 / 16.0;
                let v = if frac > bayer_t {
                    v_continuous.floor() as u64 + 1
                } else {
                    v_continuous.floor() as u64
                };
                if v == 5 {
                    ups += 1;
                } else {
                    downs += 1;
                }
            }
        }
        assert_eq!(ups, 15, "frac=15/16 should round up in 15 of 16 cells");
        assert_eq!(downs, 1, "frac=15/16 should round down in 1 of 16 cells");
    }
}
