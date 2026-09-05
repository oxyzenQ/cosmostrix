// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tuning constants for the vortex (third) and flux (fourth) rain
//! styles — task-18/task-19. Split from `mod.rs` to respect the
//! 800-LOC hard cap; re-exported wholesale via the style_rain glob
//! use so the `VORTEX_*` and `FLUX_*` flat-namespace accesses keep
//! working like the monolith constants (single flat namespace by
//! design).

// ── Vortex (third rain style, task-18) ────────────────────────────────
// Polar motion model: motes spiral inward on Keplerian orbits
// (angular speed ∝ 1/radius → constant cells/sec along the orbit).
// All values tuned for the "galaxy drain" read: slow majestic rim,
// accelerating core, 3 precessing arms sheared into spirals.

/// Base active-mote ratio for vortex density scaling.
pub(crate) const VORTEX_ACTIVE_BASE: f32 = 0.25;

/// Density multiplier for vortex active-mote calculation.
pub(crate) const VORTEX_ACTIVE_DENSITY_MULT: f32 = 0.60;

/// Maximum active-mote ratio cap (of the one-mote-per-column pool).
pub(crate) const VORTEX_ACTIVE_MAX: f32 = 0.75;

/// Spawn rate multiplier for vortex mote generation. Steady state needs
/// target/avg_journey ≈ target/3 per second; 0.35×target + floor 1.5
/// reaches that with headroom for ramp-up after scene entry.
pub(crate) const VORTEX_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const VORTEX_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Number of spiral arms (spawn-angle concentrations).
pub(crate) const VORTEX_ARMS: u8 = 3;

/// Spawn spread around an arm center (radians, ± this value).
pub(crate) const VORTEX_ARM_SPREAD: f32 = 0.55;

/// Arm precession rate (rad/s) — arms drift around the rim; full
/// revolution ≈ 139 s at 0.045.
pub(crate) const VORTEX_ARM_PRECESSION: f32 = 0.045;

/// Rim-entry jitter: spawn radius = 1.0 + roll × this (motes clip into
/// view as they drift inward — no pop-in).
pub(crate) const VORTEX_RIM_JITTER: f32 = 0.08;

/// Event-horizon radius: motes below this normalized radius are absorbed.
pub(crate) const VORTEX_CORE_R: f32 = 0.075;

/// Radius floor for the angular-speed divisor (bounds core spin rate).
pub(crate) const VORTEX_MIN_R: f32 = 0.08;

/// Kepler constant K: orbital cells/sec = K × (cols/2). At 0.75 and 120
/// cols → 45 cells/s along every orbit (rim orbit ≈ 8.4 s, visibly
/// majestic; near-core ≈ 1 rev/s).
pub(crate) const VORTEX_KEPLER_K: f32 = 0.75;

/// Global speed headroom multiplier (1.0 = neutral; tuning reserve).
pub(crate) const VORTEX_SPEED_SCALE: f32 = 1.0;

/// Radial journey reference: at chars_per_sec = 1, a rim→core trip
/// takes this many seconds (mirrors rows/sec semantics of falling
/// styles; the vortex scene's speed 24 → ~2.9 s journey).
pub(crate) const VORTEX_JOURNEY_ROWS: f32 = 70.0;

/// Inward drift base factor (before core acceleration).
pub(crate) const VORTEX_FALL_BASE: f32 = 1.0;

/// Extra inward drift near the core (added as ×(1-r) weight).
pub(crate) const VORTEX_FALL_CORE_BOOST: f32 = 0.55;

/// Matrix-style glyph mutation chance when a mote's head crosses into
/// a new cell (mutation tied to motion, like classic matrix rain).
pub(crate) const VORTEX_SHIMMER_CHANCE: f32 = 0.4;

// ── Flux (fourth rain style, task-19 — supersedes the rejected
// task-18 ripple style) ─────────────────────────────────────────────
// Liquid rain model: glyphs are fluid particles in a PIC/FLIP
// hybrid solver (see cloud/flux_field.rs). All values in screen
// units — one unit equals one terminal column width on both axes
// (one vertical unit spans two cell lines); velocities are units
// per second, gravity units per second squared.

/// Base active-mote ratio for flux density scaling.
pub(crate) const FLUX_ACTIVE_BASE: f32 = 0.30;

/// Density multiplier for flux active-mote calculation.
pub(crate) const FLUX_ACTIVE_DENSITY_MULT: f32 = 0.55;

/// Maximum active-mote ratio cap (of the one-mote-per-column pool).
pub(crate) const FLUX_ACTIVE_MAX: f32 = 0.80;

/// Spawn rate multiplier for flux mote generation. Steady state
/// needs target/avg_journey; 0.30×target + floor 1.5 reaches it
/// with headroom for ramp-up after scene entry (fall through a
/// 40-line screen at scene speed 18 takes ~2.5 s including swirl
/// detours).
pub(crate) const FLUX_SPAWN_RATE_MULT: f32 = 0.30;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const FLUX_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Fixed solver timestep in simulated seconds — the game-physics
/// fixed-step value. 1/60 matches the bench uniform stepping exactly
/// (one solver step per frame) and the standard terminal refresh.
pub(crate) const FLUX_SIM_DT: f32 = 1.0 / 60.0;

/// Solver-step cap per rendered frame. Two steps let a 120+ Hz
/// terminal keep the liquid at its native 60 Hz cadence while the
/// render loop runs faster; slower terminals drop backlog instead
/// of bursting (anti-teleport, HUNT-22 lineage).
pub(crate) const FLUX_MAX_STEPS_PER_FRAME: u32 = 2;

/// Gravity in screen units per second squared, applied to
/// weight-carrying grid nodes. Balanced against the FLIP/PIC blend
/// so falling jets settle in the 20-30 units/s band (Hot
/// brightness) and eddy-trapped motes swirl in the 3-8 band
/// (Mid/Ghost).
pub(crate) const FLUX_GRAVITY: f32 = 55.0;

/// Gravity speed normalization: the gravity scale is
/// chars_per_sec / this, clamped 0.25..3.0 (scene speed 18 = 1.0).
pub(crate) const FLUX_SPEED_REF_CPS: f32 = 18.0;

/// PIC blend fraction of the G2P readback (1 - this = FLIP). The
/// industry hybrid: FLIP preserves particle energy and detail, PIC
/// damps numerical instability. 0.10 keeps the liquid lively with
/// bounded growth.
pub(crate) const FLUX_PIC_BLEND: f32 = 0.10;

/// Jacobi iterations for the pressure Poisson solve. Four sweeps at
/// terminal grid sizes remove the large-scale divergence (the shear
/// structure that reads as eddies); the sub-grid residual is a
/// disclosed approximation.
pub(crate) const FLUX_JACOBI_ITERATIONS: u32 = 4;

/// Mote base lifetime in simulated seconds (±30% variance per
/// mote). Guarantees churn for eddy-trapped motes so the pool keeps
/// flowing; falling motes exit by the bottom boundary long before.
pub(crate) const FLUX_MOTE_LIFETIME: f32 = 14.0;

/// Entry lateral jitter cap (screen units per second, ± this).
pub(crate) const FLUX_ENTRY_VX: f32 = 0.9;

/// Particle velocity damping per second (exponential decay factor
/// 1 - damping*dt). Half-life ~23 s — gentle enough to keep eddies
/// alive for their lifetime, strong enough to bleed numerical
/// energy growth.
pub(crate) const FLUX_PARTICLE_DAMPING: f32 = 0.03;

/// Hard velocity clamp in screen units per second — numerical
/// safety far above the visual range.
pub(crate) const FLUX_MAX_SPEED: f32 = 60.0;

/// Bottom exit margin in screen units past the viewport bottom
/// before a mote recycles (open boundary).
pub(crate) const FLUX_EXIT_MARGIN: f32 = 1.0;

/// Speed-brightness thresholds in screen units per second: Hot
/// above FLUX_BRIGHT_HOT (falling jets), Mid above
/// FLUX_BRIGHT_MID (swirls), Ghost below (calm drift).
pub(crate) const FLUX_BRIGHT_HOT: f32 = 12.0;

/// Mid-brightness speed threshold (see FLUX_BRIGHT_HOT).
pub(crate) const FLUX_BRIGHT_MID: f32 = 4.0;

/// Matrix-style glyph mutation chance when a mote's head crosses
/// into a new cell (mutation tied to motion, like the vortex).
pub(crate) const FLUX_SHIMMER_CHANCE: f32 = 0.4;
