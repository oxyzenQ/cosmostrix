// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Quantum Ripple constants
//!
//! Extracted from `src/constants.rs` in v30 to keep that file under the
//! 1500-LOC project cap. These constants tune the mouse-click particle
//! burst (Quantum Ripple) — a v25 masterclass feature where each click
//! spawns 20 outward-radiating glyphs that snapshot the active palette's
//! body color and fade out over 0.8s.
//!
//! ## v30 masterclass: render-time tone-down
//!
//! `QUANTUM_BODY_TONE_DOWN` is applied at render time in
//! `cloud::rain::apply_quantum_ripple` so the snapshot stored on each
//! particle stays equal to the palette body stop exactly (preserving
//! the crossfade and "snapshot matches body stop" regression-test
//! contracts), while the rendered pixel is dimmed to match the rain's
//! perceived average brightness rather than the saturated body stop
//! alone.

/// Maximum concurrent Quantum Ripple particles. Pre-allocated once at
/// Cloud init; reused via free-list. 32 covers the peak case of 2-3
/// rapid clicks (each spawns up to 25) with overlap.
pub const QUANTUM_RIPPLE_POOL_SIZE: usize = 64;

/// Particles spawned per click (fixed 20 for determinism).
pub const QUANTUM_RIPPLE_PARTICLE_COUNT: usize = 20;

/// Particle lifespan in seconds (0.8s midpoint).
pub const QUANTUM_RIPPLE_LIFETIME_SECS: f32 = 0.8;

/// Particle outward radial speed (cells/sec).
pub const QUANTUM_RIPPLE_SPEED: f32 = 18.0;

/// Brand purple RGB (same as logo color) for Quantum effects.
pub const QUANTUM_BRAND_PURPLE_R: u8 = 168;
pub const QUANTUM_BRAND_PURPLE_G: u8 = 85;
pub const QUANTUM_BRAND_PURPLE_B: u8 = 247;

/// v30 masterclass: render-time tone-down applied to each particle's
/// snapshot of the palette body color.
///
/// Owner visual testing reported that ripple particles read as "too
/// bright" — a click on the Green scheme produced saturated
/// `(0, 220, 0)`-class pixels that visually out-shone the surrounding
/// rain (whose droplets are mostly head→body→tail gradient, so the
/// average cell the eye sees is much dimmer than the body stop alone).
///
/// Rather than change the snapshot itself (the snapshot must stay
/// equal to `palette.colors[len/2]` because the crossfade tests assert
/// that exact invariant), this constant is applied at RENDER time in
/// `apply_quantum_ripple`: the per-particle RGB used for blending is
/// `p.r * QUANTUM_BODY_TONE_DOWN`, etc.
///
/// `0.72` was chosen empirically: on Green it dims `(0, 220, 0)` to
/// `(0, 158, 0)` — still clearly green and well above the trail floor
/// (~131), but no longer competing with the head stop for visual
/// dominance. On Red it dims `(220, 0, 0)` to `(158, 0, 0)`. On dark
/// themes like Cosmos the dimmed body still sits comfortably above the
/// Phase 7 floor, so no theme regresses on visibility.
///
/// The snapshot stored on the particle (`p.r/g/b`) is unchanged —
/// palette-switch crossfade and the "snapshot matches the body stop"
/// regression tests still hold. Only the rendered pixel is dimmed.
///
/// Lower values (0.5–0.65) make ripples read as ambient sparks rather
/// than a hue burst — fine on bright themes but too dim on dark themes
/// like Cosmos/Nebula where the body is already low-luminance. Higher
/// values (0.85–1.0) restore the "too bright" complaint.
pub const QUANTUM_BODY_TONE_DOWN: f32 = 0.72;
