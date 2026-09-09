// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Shader helper functions — extracted from `shaders/base/mod.rs` to
//! keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns 6 free helper functions used by the chroma dragon shader:
//! - `bayer_threshold`: ordered dithering threshold (4x4 Bayer matrix).
//! - `column_coherence_perturbation`: per-column hue phase offset.
//! - `hue_drift_offset`: maps ecosystem hue_drift to i32 offset.
//! - `cell_hash`: FNV-1a hash for per-cell deterministic jitter.
//! - `apply_subpixel_jitter`: RGB subpixel dithering for smooth gradients.
//! - `color_uses_previous_palette`: color transition wave test.
//!
//! Plus the three shader constants moved here in the NIGHT-hunter-25
//! part 2 LOC split (the CellPaint bundle pushed mod.rs over the cap):
//! `TRAIL_EXP_LUT`, `SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD`, and
//! `BAYER_4X4`.
//!
//! Re-exported from `shaders/base/mod.rs` via `pub(crate) use` so all
//! existing call sites resolve unchanged.

use crossterm::style::Color;

use crate::constants::TRAIL_EXPONENTIAL_K;

/// Precomputed exponential decay lookup table for trail brightness.
/// Maps 256 normalized distances → exp(-TRAIL_EXPONENTIAL_K * t).
/// Eliminates ~3,000 exp() calls per frame in shading_distance mode.
///
/// Moved from `cloud::render` in Phase 2 (it is a shader resource owned
/// by the chroma engine, not by the renderer); relocated to helpers.rs
/// in the NIGHT-hunter-25 part 2 LOC split. The `pub(crate) use`
/// re-export in mod.rs keeps the `crate::...::base::TRAIL_EXP_LUT`
/// path alive.
pub(crate) static TRAIL_EXP_LUT: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
    let mut lut = [0.0f32; 256];
    for (i, entry) in lut.iter_mut().enumerate() {
        let t = i as f32 / 255.0;
        *entry = (-(TRAIL_EXPONENTIAL_K as f32) * t).exp();
    }
    lut
});

/// Phase 3-F (Chroma Dragon Innovation F): luminance-remap threshold for
/// short droplets.
///
/// Droplets with `length <= SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD` get
/// their `CharLoc::Middle` cells remapped from the (random-uniform)
/// `color_map` value to a position-based ramp that spans the full palette
/// range — head-adjacent cells land on the brightest stop, tail-adjacent
/// cells on the darkest. Without this, short droplets (4–8 cells) sample
/// only 2–6 random `color_map` entries and look perceptually flat compared
/// to long droplets where the same random distribution produces visible
/// shimmering across many cells.
///
/// Threshold of 8 = 2× `MIN_DROPLET_LENGTH` (4). Below this, the visible
/// Middle range is too small for the random color_map to read as a
/// gradient. Above this, the existing color_map path produces enough
/// inter-cell variation to look natural.
///
/// Only applies when `!shading_distance` — that branch already has its
/// own length-aware exponential decay ramp. Also only applies to
/// `CharLoc::Middle` — Head and Tail stops are pinned by the shader
/// (`last` and `0` respectively) and should not be perturbed.
pub(super) const SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD: u16 = 8;

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
pub(super) const BAYER_4X4: [[u8; 4]; 4] =
    [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

pub(super) fn bayer_threshold(line: u16, col: u16) -> u8 {
    // Bitwise AND with 3 is equivalent to % 4 but avoids the division.
    BAYER_4X4[(line as usize) & 3][(col as usize) & 3]
}

/// Phase 3-C: compute the per-cell column-coherence hue perturbation.
///
/// Returns an integer offset in `{-1, 0, +1}` that nudges the cell's
/// `color_idx` based on its column position and the current time phase.
/// Neighboring columns get similar offsets (low spatial frequency:
/// 0.05 rad/col → period ~125 columns), and the offset drifts slowly
/// over time (caller advances `phase` by a small amount per frame).
///
/// Amplitude is ±0.5 before rounding, so the offset is 0 most of the
/// time and ±1 near the peaks of the sine. This produces a gentle
/// "shimmer" rather than a strong hue shift.
///
/// Phase D (hot-path): `pub(crate)` so `cloud::rain::rain_at` can call this
/// once per column per frame to build `ShaderCtx::column_coherence_lut`.
/// The shader hot path then reads the precomputed i32 from the LUT
/// instead of calling this fn per cell (~65-130M cycles/sec saved).
#[inline]
pub(crate) fn column_coherence_perturbation(phase: f32, col: u16) -> i32 {
    // Spatial frequency: 0.05 rad/col → period ~125 cols
    let spatial = (col as f32) * 0.05;
    // Amplitude: ±0.5 → rounds to {-1, 0, +1}
    ((phase + spatial).sin() * 0.5_f32).round() as i32
}

/// Phase 3-H: compute the global hue-drift palette-stop offset.
///
/// Maps `ColorEcosystem.hue_drift` (in radians, clamped to `[-π, π]`) to
/// an integer palette-stop offset in `{-2, -1, 0, +1, +2}`. The scaling
/// `drift / π * 2.0` means a full π rotation shifts by 2 stops — subtle
/// enough to feel atmospheric, visible enough to notice over the ~10-minute
/// drift cycle (COLOR_HUE_DRIFT_RATE = 0.015 rad/tick, 1 tick/3sec).
///
/// Unlike column_coherence (per-cell perturbation), hue_drift is a GLOBAL
/// offset: every Middle cell in every column shifts by the same amount.
/// The effect is that the entire scene's palette slowly cycles through
/// adjacent stops, so the same column looks slightly different each minute.
///
/// Returns 0 for `drift = 0.0` (no shift) and for very small drifts
/// (|drift| < π/4 ≈ 0.785 rad, which rounds to 0).
///
/// Phase C: now `pub(crate)` so `cloud/rain.rs` can call it once per
/// frame at `DrawCtx` construction. The per-cell hot path no longer
/// calls this — it reads the pre-computed `i32` from `ShaderCtx`.
#[inline]
pub(crate) fn hue_drift_offset(drift: f32) -> i32 {
    (drift / std::f32::consts::PI * 2.0_f32).round() as i32
}

/// Phase 3-E: deterministic per-cell hash for subpixel jitter.
///
/// Returns a u32 that varies pseudo-randomly with `(line, col)`. The same
/// input always produces the same output (deterministic), but different
/// inputs produce uncorrelated outputs (low collision rate). Used to drive
/// per-cell RGB perturbation so the film-grain texture is stable across
/// frames — the same cell always gets the same jitter, so it doesn't strobe.
///
/// Implementation: FNV-1a variant with line and col mixed in via XOR
/// after each multiply step. Cheap (3 multiplies + 2 XORs), no allocation.
#[inline]
pub(super) fn cell_hash(line: u16, col: u16) -> u32 {
    let mut h = 0x811C9DC5u32; // FNV offset basis
    h ^= line as u32;
    h = h.wrapping_mul(16777619); // FNV prime
    h ^= col as u32;
    h = h.wrapping_mul(16777619);
    h
}

/// Phase 3-E: apply per-cell RGB jitter to a color.
///
/// Perturbs each channel by an independent signed offset in `[-amp, +amp]`,
/// derived from three independent 4-bit slices of `hash`. The result is
/// clamped to `[0, 255]` per channel.
///
/// `amplitude = 0` or `Color::Reset` input returns the original unchanged.
/// Output is always `Color::Rgb` (normalized via `color_to_rgb`).
#[inline]
pub(super) fn apply_subpixel_jitter(color: Color, hash: u32, amplitude: u8) -> Color {
    if amplitude == 0 || matches!(color, Color::Reset) {
        return color;
    }
    let (r, g, b) = crate::chroma_dragon_engine::palette::color_to_rgb(color);
    let amp = i32::from(amplitude);
    // Three independent 4-bit signed offsets in [-8, +7].
    let dr_raw = (hash & 0xF) as i32 - 8;
    let dg_raw = ((hash >> 4) & 0xF) as i32 - 8;
    let db_raw = ((hash >> 8) & 0xF) as i32 - 8;
    // Scale [-8, +7] → [-amp, +amp*7/8]. Slight asymmetry is acceptable
    // for film-grain — the perceptual effect is symmetric.
    let dr = dr_raw * amp / 8;
    let dg = dg_raw * amp / 8;
    let db = db_raw * amp / 8;
    Color::Rgb {
        r: (i32::from(r) + dr).clamp(0, 255) as u8,
        g: (i32::from(g) + dg).clamp(0, 255) as u8,
        b: (i32::from(b) + db).clamp(0, 255) as u8,
    }
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
pub(crate) fn color_uses_previous_palette(
    color_wave_line: Option<f32>,
    active_palette_slot: u8,
    palette_slot: u8,
    line: u16,
    col: u16,
    lines: u16,
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
    // S-master-HUNT-15: diagonal stagger — each column's wave arrives
    // STAGGER_PER_COL rows later than the previous, creating a diagonal
    // sweep (top-left converts first, bottom-right last) on top of the
    // vertical smoothstep sweep. Capped at STAGGER_MAX_FRAC * lines so
    // wide terminals don't produce a stagger larger than the screen.
    let col_stagger = diagonal_stagger(col, lines);
    (line as f32 + col_stagger) > wave_line + jitter
}

/// Compute the per-column diagonal stagger for the transition wave
/// (S-master-HUNT-15). Returns the row offset added to `line` in the
/// wave-line comparison, so column N converts `stagger` rows later
/// than column 0. Capped at `STAGGER_MAX_FRAC * lines`.
#[inline]
fn diagonal_stagger(col: u16, lines: u16) -> f32 {
    let raw = col as f32 * crate::constants::WAVE_DIAGONAL_STAGGER_PER_COL;
    let cap = lines as f32 * crate::constants::WAVE_DIAGONAL_STAGGER_MAX_FRAC;
    raw.min(cap)
}
