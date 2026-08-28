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
//! Re-exported from `shaders/base/mod.rs` via `pub(crate) use` so all
//! existing call sites resolve unchanged.

use crossterm::style::Color;

use super::BAYER_4X4;

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
