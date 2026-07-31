// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Tuning Constants
//!
//! Phase 4 (Dragon Awakening) activates two shader innovations that were
//! plumbed into `chroma::shaders::base::resolve_cell_color` during Phase 3
//! but left dormant (hardcoded to `None` in the `DrawCtx → ShaderCtx`
//! builder):
//!
//!   - Innovation C (temporal column hue coherence) — Phase 4-A
//!   - Innovation E (subpixel hue jitter)           — Phase 4-B
//!
//! Both are now always-on in production. The constants below tune their
//! amplitudes; see the doc comments on `ShaderCtx::column_coherence_phase`
//! and `ShaderCtx::subpixel_jitter_amplitude` for the full rationale.
//!
//! ## Why a separate module?
//!
//! These constants are Chroma Dragon-specific — they tune the coloring
//! engine, not the rendering engine or the cloud simulation. Keeping them
//! in `src/constants.rs` pushed that file over the 1500-LOC cap (1480 +
//! 51 = 1531). Moving them here keeps each file under the cap and groups
//! all chroma tuning in one auditable place. Future Chroma Dragon
//! innovations should add their tuning constants here too.

/// Phase 4-A: angular frequency (rad/s) of the temporal column hue
/// coherence oscillation.
///
/// The shader's `column_coherence_perturbation(phase, col)` computes
/// `sin(phase + col * 0.05)` and rounds to `{-1, 0, +1}`. The `phase`
/// argument advances at this rate so the per-column shimmer drifts
/// slowly over time.
///
/// `0.105` rad/s → period `2π / 0.105 ≈ 59.8 s` (~1 minute). Slow
/// enough to read as atmospheric rather than animated, fast enough
/// that a user watching for ~10 s perceives the columns breathing
/// through adjacent palette stops.
///
/// Spatial frequency is fixed at `0.05` rad/col (period ~125 cols)
/// inside the shader — that value is not exposed because it is
/// coupled to the `{-1, 0, +1}` rounding amplitude and changing it
/// in isolation would either quantize to 0 everywhere (too low) or
/// strobe per-cell (too high).
pub const COLUMN_COHERENCE_FREQ: f32 = 0.105;

/// Phase 4-B: amplitude of the per-cell subpixel hue jitter.
///
/// Each Middle cell's resolved RGB is perturbed by an independent
/// signed offset in `[-amp, +amp]` per channel, derived from a
/// deterministic FNV-1a hash of `(line, col)`. The same cell always
/// gets the same jitter (no strobing across frames); neighboring
/// cells get uncorrelated jitter (film-grain texture).
///
/// `3` is the conservative production default — at typical viewing
/// distance it reads as subtle organic texture rather than noise.
/// Higher values (6–8) produce a visible "static" effect; lower
/// values (1–2) are imperceptible on most terminals.
///
/// The jitter is applied AFTER the palette decision and BEFORE
/// atmospheric, so it does not interfere with the head→body→tail
/// hierarchy or the atmospheric luminance/saturation math.
pub const SUBPIXEL_JITTER_AMPLITUDE: u8 = 3;
