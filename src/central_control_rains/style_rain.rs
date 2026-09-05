// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tuning constants for the vortex (third), flux (fourth) and
//! lorenz (fifth) rain styles — task-18/task-19 + NIGHT-research-4.
//! Split from `mod.rs` to respect the 800-LOC hard cap; re-exported
//! wholesale via the style_rain glob use so the `VORTEX_*`,
//! `FLUX_*` and `LORENZ_*` flat-namespace accesses keep working
//! like the monolith constants (single flat namespace by design).
//!
//! Catalog history: the original fourth style was `ripple`
//! (water-surface rings + splashes) — owner-rejected for not being
//! unique or masterpiece-grade. task-19 replaced it with `flux` (a
//! PIC/FLIP liquid solver); NIGHT-research-4 then added `lorenz`, a
//! real strange-attractor renderer (canonical Lorenz ODE integrated
//! via RK4), as the fifth style. The LORENZ_* constants below fully
//! replace the prior RIPPLE_ block.

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

// ── Lorenz (fifth rain style, NIGHT-research-4) ──────────────────────
// Strange-attractor motion model: motes follow trajectories of the
// canonical Lorenz ODE (sigma=10, rho=28, beta=8/3) integrated via
// classical RK4. Two-lobe "butterfly" projected to 2D, with z as
// depth/brightness cue. Joined the catalog via the NIGHT-research-4
// merge (task-19's flux had already removed the rejected ripple
// water-surface style) — the lorenz motion DNA (3D chaotic ODE) is
// fundamentally distinct from cascade/pillar/polar-orbit/fluid.

/// Lorenz system parameter sigma (Prandtl-like term in the original
/// atmospheric-convection model). Canonical value 10.0 — published by
/// Edward Lorenz in 1963 and used unchanged in every standard
/// reference visualization since.
pub(crate) const LORENZ_SIGMA: f32 = 10.0;

/// Lorenz system parameter rho (Rayleigh-like term). Canonical 28.0
/// — above the critical value (rho_critical ≈ 24.74) where the
/// steady-state equilibria lose stability and the system becomes
/// chaotic. Below 24.74 the system settles to a fixed point (no
/// butterfly); 28.0 gives the iconic two-lobe strange attractor.
pub(crate) const LORENZ_RHO: f32 = 28.0;

/// Lorenz system parameter beta (geometry-like term). Canonical
/// 8.0/3.0 — controls the z-axis damping. The non-integer value is
/// preserved here as-is to match the canonical literature (Lorenz
/// derived it from a Fourier-mode truncation where it falls out
/// naturally).
pub(crate) const LORENZ_BETA: f32 = 8.0 / 3.0;

/// Equilibrium x/y coordinate magnitude. NIGHT-research-4 tuning:
/// motes spawn at (±EQ_X, ±EQ_Y, EQ_Z) with a per-mote perturbation.
/// The canonical equilibrium coordinates are (±8.485, ±8.485, 27.0)
/// (derived from sqrt(beta*(rho-1))), but spawning there leaves motes
/// near an unstable fixed point where the local flow velocity is low —
/// motes drift slowly for many seconds before entering the chaotic
/// lobe interior. Instead, we use the classic textbook initial
/// condition (1, 1, 1) — well inside the saddle region where the
/// unstable manifold immediately accelerates motes outward into the
/// butterfly flow. Sign of x selects lobe (right C+ or left C-).
pub(crate) const LORENZ_EQ_X: f32 = 1.0;

/// Equilibrium y coordinate (mirrors EQ_X for the classic (1,1,1)
/// textbook initial condition).
pub(crate) const LORENZ_EQ_Y: f32 = 1.0;

/// Equilibrium z coordinate. Classic textbook value 1.0 — inside the
/// saddle region's unstable manifold, immediately entering the
/// chaotic flow.
pub(crate) const LORENZ_EQ_Z: f32 = 1.0;

/// Initial-state perturbation magnitude. Motes spawn ON the
/// equilibrium (an unstable fixed point of the deterministic flow)
/// plus a uniform ±perturb. At 2.0, the perturbation is large
/// enough to immediately kick motes off the equilibrium into the
/// chaotic flow (visual motion from frame 1), while still being
/// small enough that two motes seeded identically diverge visibly
/// over a few seconds — the butterfly effect (sensitive dependence
/// on initial conditions). Smaller perturbations (1e-3) leave motes
/// frozen at the equilibrium for many seconds (the exponential
/// divergence takes time to grow from a tiny seed); larger
/// perturbations (5+) break the demonstration (motes immediately
/// fly to one lobe, no visible divergence).
pub(crate) const LORENZ_SPAWN_PERTURB: f32 = 2.0;

/// Integration step per chars_per_sec: dt_lorenz = cps * this * dt_wall.
/// Tuned for RK4 stability on the canonical Lorenz attractor: at
/// speed-24 scene + 60 FPS, dt_wall ≈ 0.0167s, so dt_lorenz ≈
/// 24 * 0.005 * 0.0167 ≈ 0.002 per frame. RK4 is stable for Lorenz
/// up to dt ≈ 0.01, so we have ~5x headroom — the integration is
/// fast enough that motes traverse several cells per frame (visible
/// motion) without diverging from the true attractor. Raising this
/// speeds up the butterfly's wingbeat cadence.
pub(crate) const LORENZ_DT_PER_CPS: f32 = 0.005;

/// Mote lifetime cap (seconds). After this age, a mote is absorbed
/// and the slot respawns near a fresh equilibrium seed. 12s gives
/// each mote enough trajectory history to traverse both lobes at
/// least once before refresh — the butterfly structure reads
/// clearly. Shorter → constant motion blur (no lobe structure);
/// longer → motes pile up at saturation.
pub(crate) const LORENZ_MAX_AGE_SECS: f32 = 12.0;

/// Base active-mote ratio for lorenz density scaling.
pub(crate) const LORENZ_ACTIVE_BASE: f32 = 0.30;

/// Density multiplier for lorenz active-mote calculation.
pub(crate) const LORENZ_ACTIVE_DENSITY_MULT: f32 = 0.55;

/// Maximum active-mote ratio cap (of the one-mote-per-column pool).
pub(crate) const LORENZ_ACTIVE_MAX: f32 = 0.70;

/// Spawn rate multiplier for lorenz mote generation. Steady state
/// needs target/avg_lifetime ≈ target/12 per second; 0.35×target +
/// floor 1.5 reaches that with headroom for ramp-up after scene
/// entry (parity with vortex's tuning).
pub(crate) const LORENZ_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const LORENZ_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Matrix-style glyph mutation chance when a mote's head crosses
/// into a new cell (mutation tied to motion, like classic matrix
/// rain — parity with vortex's shimmer gate).
pub(crate) const LORENZ_SHIMMER_CHANCE: f32 = 0.4;

/// Viewport projection inset (fraction of half-width/half-height).
/// At 0.92 the attractor occupies 92% of the viewport's half-extent
/// in each direction — leaves an 8% margin so fast trajectories
/// near the lobe edges never clip the terminal border.
pub(crate) const LORENZ_VIEW_INSET: f32 = 0.92;

/// Lorenz x half-range used for projection (attractor x spans
/// approximately [-25, 25] in steady state). Half-range = 25.0.
pub(crate) const LORENZ_X_HALF_RANGE: f32 = 25.0;

/// Lorenz y half-range used for projection (attractor y spans
/// approximately [-30, 30] in steady state). Half-range = 30.0.
pub(crate) const LORENZ_Y_HALF_RANGE: f32 = 30.0;

/// Brightness zone boundary: z above this → Core (lobe peak hot).
/// The attractor's z range is approximately [0, 50]; lobe peaks
/// cluster near z=40+, so 38.0 marks the hot zone cleanly.
pub(crate) const LORENZ_Z_HOT: f32 = 38.0;

/// Brightness zone boundary: z above this → Hot (lobe body).
/// Set at 28.0 — the equilibrium z value (rho - 1). Below this,
/// motes are typically transiting the saddle region.
pub(crate) const LORENZ_Z_MID: f32 = 28.0;

/// Brightness zone boundary: z above this → Mid; below → Ghost.
/// Set at 13.0 — the saddle-region z where trajectories cross
/// between lobes (visible as the dim "bridge" between wings).
pub(crate) const LORENZ_Z_DIM: f32 = 13.0;
