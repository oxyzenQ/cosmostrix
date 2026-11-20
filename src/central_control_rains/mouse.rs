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
/// horizontal physical direction. Vertical propagation covers half as
/// many rows per second (rows are ~2x taller than columns —
/// `wave_field::CELL_ASPECT`), so the front is isotropic in PIXEL
/// space: the ripple renders as a circle.
///
/// Owner blast-radius retune (2026-11-20): Z4b kept the synthetic
/// ring's 32 cells/s pace, but on the real solver the crest decays
/// while it travels, so the visible blast died ~20 columns out (the
/// owner's "100 m" reach). c = 48 cells/s puts the front on the
/// horizontal edge of a 115-col terminal ~1.2 s after the click and
/// the vertical edge (20.5 rows at 24 rows/s) at ~0.85 s — with the
/// crest still clearly visible on arrival (see MOUSE_FLASH_WAVE_DAMPING
/// for the envelope math). CFL stays 5x inside the von Neumann bound
/// (rx = 48/120 = 0.4, rx^2 + ry^2 = 0.2 <= 1, also under the Z6
/// ledger law's 0.25 pin).
pub(crate) const MOUSE_FLASH_SPEED: f32 = 48.0;

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
/// so the field provably crosses the settle floor
/// (`MOUSE_FLASH_SETTLE_EPS` = 0.004, ~0.5% brightness — invisible)
/// at `2*ln(A0/eps)/k` — ~3.7 s for a single click (A0 = 1), ~4.1 s
/// for the A0 = 2 storm cap — and the solver marks itself settled
/// (zero-cost frames) at the floor. The field does not EXPIRE by
/// clock anymore — the physics decays it.
///
/// Owner blast-radius retune (2026-11-20): Z4b calibrated k = 6.0 to
/// match the synthetic era's 1.8 s life, which decayed the crest
/// below visibility by ~20 columns (the "100 m" reach the owner
/// reported). k = 3.0/s recalibrates to the reach law instead: at the
/// 115-col screen edge (~1.2 s of travel at c = 48) the crest still
/// carries `exp(-1.5*1.2)` = 16.5% amplitude — a ~14% brightness
/// factor at MOUSE_FLASH_INTENSITY, clearly visible. The tradeoff is
/// honest: ~1 s of extra sub-1% tail per click; the settle is still
/// provable, visually silent, and post-settle frames cost ZERO work.
pub(crate) const MOUSE_FLASH_WAVE_DAMPING: f32 = 3.0;

/// The shockwave solver's sub-step ceiling (seconds). The leapfrog
/// scheme's CFL bound is `rx^2 + ry^2 <= 1`; at DT_MAX = 1/120 s the
/// Courant numbers are rx = 48/120 = 0.4, ry = rx/2 = 0.2, giving
/// rx^2 + ry^2 = 0.2 — a 5x stability margin at ANY frame rate (also
/// under the Z6 ledger law's 0.25 pin). Slow frames split into more
/// sub-steps, capped at 8; a stall advances the wave less instead of
/// destabilizing it.
pub(crate) const MOUSE_FLASH_DT_MAX: f32 = 1.0 / 120.0;

/// The shockwave field's settle floor: when the peak |displacement|
/// falls below this, the solver clears the field and subsequent
/// frames cost zero work until the next click. 0.004 amplitude = a
/// brightness factor of ~0.0034 at MOUSE_FLASH_INTENSITY — far below
/// any visible threshold, so the settle is visually silent.
pub(crate) const MOUSE_FLASH_SETTLE_EPS: f32 = 0.004;
