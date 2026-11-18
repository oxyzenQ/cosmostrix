// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Mouse interaction tuning: the cursor hover glow and the wave-equation
//! click shockwave. Extracted from mod.rs (the v50.0.0-beta.7 LOC-refactor
//! pattern; night-frontier-Z4b grew the shockwave constants past the
//! 800-LOC cap) and re-exported so every `crate::constants::*` call site
//! keeps resolving.

// ─── Mouse interaction ─────────────────────────────────────────────────────

/// Radius (in columns) of the mouse hover glow.
pub(crate) const MOUSE_GLOW_RADIUS_COLS: f32 = 7.0;

/// Radius (in lines) of the mouse hover glow.
pub(crate) const MOUSE_GLOW_RADIUS_LINES: f32 = 5.0;

/// Intensity of the mouse hover glow (0.0 = disabled in default mode).
pub(crate) const MOUSE_GLOW_INTENSITY: f32 = 0.25;

/// Speed of the mouse-click shockwave (the wave equation's c, in
/// CELLS/second of horizontal travel).
///
/// Z4b: the click ripple is no longer a synthetic expanding ring —
/// the field solves the 2D wave equation (`u_tt + k*u_t = c^2*grad^2 u`)
/// on the cell grid, and this constant IS the wave speed `c` in the
/// horizontal physical direction. 32 cells/s preserves the synthetic
/// ring's expansion pace (visual continuity with the pre-Z4b era).
/// Vertical propagation covers half as many rows per second (rows are
/// ~2x taller than columns — `wave_field::CELL_ASPECT`), so the front
/// is isotropic in PIXEL space: the ripple renders as a circle.
pub(crate) const MOUSE_FLASH_SPEED: f32 = 32.0;

/// Peak intensity of the mouse-click shockwave (the brightness factor
/// a full-amplitude wave crest contributes, pre-clamp).
pub(crate) const MOUSE_FLASH_INTENSITY: f32 = 0.85;

/// Volume damping of the click shockwave field (the wave equation's
/// `k`, per second): the solver integrates the telegraph form
/// `u_tt + k*u_t + (k/2)^2*u = c^2*grad^2 u`.
///
/// The restoring term makes the field's life PROVABLY finite: in the
/// pure damped wave equation the spatial-mean mode converges to a
/// nonzero constant (a visible residue that never settles); the
/// telegraph form rides the same envelope `exp(-k*t/2)` on EVERY mode,
/// so k = 6.0/s takes a full-amplitude crest below the settle floor
/// (`MOUSE_FLASH_SETTLE_EPS` = 0.004, ~0.5% brightness — invisible)
/// in `2*ln(1/eps)/k` = 1.84 s, matching the synthetic era's
/// MOUSE_FLASH_DURATION_SECS = 1.8 s visual life. The field does not
/// EXPIRE by clock anymore — the physics decays it, and the solver
/// marks itself settled (zero-cost frames) at the floor.
pub(crate) const MOUSE_FLASH_WAVE_DAMPING: f32 = 6.0;

/// The shockwave solver's sub-step ceiling (seconds). The leapfrog
/// scheme's CFL bound is `rx^2 + ry^2 <= 1`; at DT_MAX = 1/120 s the
/// Courant numbers are rx = 32/120 = 0.267, ry = rx/2 = 0.133, giving
/// rx^2 + ry^2 = 0.089 — an 11x stability margin at ANY frame rate
/// (slow frames split into more sub-steps, capped at 8; a stall
/// advances the wave less instead of destabilizing it).
pub(crate) const MOUSE_FLASH_DT_MAX: f32 = 1.0 / 120.0;

/// The shockwave field's settle floor: when the peak |displacement|
/// falls below this, the solver clears the field and subsequent
/// frames cost zero work until the next click. 0.004 amplitude = a
/// brightness factor of ~0.0034 at MOUSE_FLASH_INTENSITY — far below
/// any visible threshold, so the settle is visually silent.
pub(crate) const MOUSE_FLASH_SETTLE_EPS: f32 = 0.004;
