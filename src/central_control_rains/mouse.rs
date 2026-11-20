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
/// crest still visible on arrival under the water-ripple calibration
/// (see MOUSE_FLASH_WAVE_DAMPING for the envelope math and the measured
/// crest curve). CFL stays 5x inside the von Neumann bound
/// (rx = 48/120 = 0.4, rx^2 + ry^2 = 0.2 <= 1, also under the Z6
/// ledger law's 0.25 pin).
pub(crate) const MOUSE_FLASH_SPEED: f32 = 48.0;

/// Peak intensity of the mouse-click shockwave: the brightness factor
/// a wave crest of amplitude `u` contributes is `min(|u| * this, 1.0)`
/// (the clamp lives at the draw path's single read site).
///
/// Water-ripple retune (2026-11-20, v2): 0.85 -> 2.0. The 1D envelope
/// retune (v1) fixed the wave's REACH but ignored the 2D geometric
/// spreading — on a real solve the crest amplitude falls ~1/sqrt(r)
/// while it travels, so at the v1 gain the crest factor was ~0.018 at
/// half a 115-col screen and ~0.004 at the edge: physically correct,
/// perceptually invisible (the owner's "still not felt"). At 2.0 the
/// mapping is crest-SATURATING: any |u| >= 0.5 clamps to the full
/// palette-HEAD band, the 13-cell blast seed lands as a solid splash,
/// and the measured crest curve (exact solver replica, 115x24, seed
/// 2.0/1.0/0.5, k = 1.8) holds factor >= 0.15 out to ~85 columns and
/// ~0.05 at the horizontal edge — the owner's water analogy: touch a
/// calm surface, the ripple is FELT across the screen.
pub(crate) const MOUSE_FLASH_INTENSITY: f32 = 2.0;

/// Volume damping of the click shockwave field (the wave equation's
/// `k`, per second): the solver integrates the telegraph form
/// `u_tt + k*u_t + (k/2)^2*u = c^2*grad^2 u`.
///
/// The restoring term makes the field's life PROVABLY finite: in the
/// pure damped wave equation the spatial-mean mode converges to a
/// nonzero constant (a visible residue that never settles); the
/// telegraph form rides the same envelope `exp(-k*t/2)` on EVERY mode,
/// so the field provably crosses the settle floor
/// (`MOUSE_FLASH_SETTLE_EPS` = 0.004, a brightness factor of ~0.008 —
/// invisible) at `2*ln(A0/eps)/k` — since the water-ripple retune the
/// seed lands AT the A0 = 2 inject cap, so single click and storm cap
/// share the same ~6.9 s provable ceiling (real grids settle far
/// earlier; see below) — and the solver marks itself settled
/// (zero-cost frames) at the floor. The field does not EXPIRE by
/// clock anymore — the physics decays it.
///
/// Owner blast-radius retune (2026-11-20): Z4b calibrated k = 6.0 to
/// match the synthetic era's 1.8 s life, which decayed the crest
/// below visibility by ~20 columns (the "100 m" reach the owner
/// reported). The v1 retune (k = 3.0) fixed the reach but measured
/// the DAMPING envelope only — the 2D geometric spreading (~1/sqrt(r))
/// still collapsed the crest below visibility (see
/// MOUSE_FLASH_INTENSITY).
///
/// Water-ripple retune (2026-11-20, v2): k = 1.8/s. Measured on the
/// exact solver replica (115x24, blast seed): the crest factor stays
/// above 0.15 out to ~85 columns and ~0.05 at the 115-col edge (~1.0 s
/// of travel at c = 48), the field drops below the ~0.02-factor
/// visibility floor after ~1.5 s, and the settle still lands well
/// inside the provable ceiling `2*ln(A0/eps)/k` = ~6.9 s at A0 = 2
/// (the pure-envelope bound; on real grids the Mur edges and the
/// geometric spreading settle it far earlier — measured ~1.9 s at
/// 80x24). The tradeoff is honest: the sub-visibility tail is ~1 s
/// longer than v1's; the settle is still provable, visually silent,
/// and post-settle frames cost ZERO work.
pub(crate) const MOUSE_FLASH_WAVE_DAMPING: f32 = 1.8;

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
/// brightness factor of ~0.008 at the water-ripple
/// MOUSE_FLASH_INTENSITY — far below any visible threshold, so the
/// settle is visually silent.
pub(crate) const MOUSE_FLASH_SETTLE_EPS: f32 = 0.004;
