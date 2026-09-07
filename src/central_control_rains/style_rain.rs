// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: pure tuning-constants file — every style's motion/density
// constants with their mandatory why-docs, no logic (same data-file class
// as themes.rs). NIGHT-special-1 stage 2.4 added the three-tier disk
// table, the see-saw roll schedule and the proximity-brightness zones
// on top of the stage-2.3 equatorial-disk constants; splitting the
// single tuning surface by style family would scatter the owner's
// visual-feedback rationale across files (RULES_LOC.md "When NOT to
// Split").

//! Tuning constants for the vortex (third), flux (fourth), lorenz
//! (fifth), dragon (sixth), physarum (seventh) and black hole
//! (eighth, NIGHT-special-1) rain styles — task-18/task-19 +
//! NIGHT-research-4/5/6.
//! Split from `mod.rs` to respect the 800-LOC hard cap; re-exported
//! wholesale via the style_rain glob use so the `VORTEX_*`,
//! `FLUX_*`, `LORENZ_*`, `DRAGON_*` and `PHYSARUM_*` flat-namespace
//! accesses keep working like the monolith constants (single flat
//! namespace by design).
//!
//! Catalog history: the original fourth style was `ripple`
//! (water-surface rings + splashes) — owner-rejected for not being
//! unique or masterpiece-grade. task-19 replaced it with `flux` (a
//! PIC/FLIP liquid solver); NIGHT-research-4 then added `lorenz`, a
//! real strange-attractor renderer (canonical Lorenz ODE integrated
//! via RK4), as the fifth style; NIGHT-research-5 added `dragon`
//! (Chinese-mythology serpentine chain via FABRIK) as the sixth
//! style; NIGHT-research-6 added `physarum` (Jeff Jones 2010
//! slime-mold emergent networks) as the seventh style. The
//! `LORENZ_*`, `DRAGON_*` and `PHYSARUM_*` constants below fully
//! replace the prior `RIPPLE_*` block.

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
// hybrid solver (see cloud/type_rain/flux/flux_field.rs). All values in screen
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

// ── Dragon (sixth rain style, NIGHT-research-5) ──────────────────────
// Chinese-mythology serpentine dragon motion model: each dragon is a
// chain of segments (head + body + tail) following a path-generating
// head via FABRIK distance constraints (snake kinematics). The head
// runs a two-state machine — Soar (smooth random-walk turn rate from
// layered sine noise) and Circle (constant turn rate producing a
// circular orbit). Wall bounce reflects velocity and snaps to Soar.
// Brightness fades along the body (head Core, tail Ghost) — the
// signature serpentine fade of the Chinese dragon's sinuous body.

/// Body length (segments per dragon, including head). 20 gives a
/// long, sinuous body — the Chinese-dragon silhouette. Each segment
/// is one cell; at spacing 1.4 the body spans ~28 cells when
/// stretched straight.
pub(crate) const DRAGON_BODY_LEN: usize = 20;

/// Spacing between consecutive body segments (cells). At 1.4 the
/// body has visible curvature without bunching; smaller values
/// crowd segments onto the same cell, larger values create gaps.
pub(crate) const DRAGON_SEGMENT_SPACING: f32 = 1.4;

/// Pool size cap (max concurrent dragons). NIGHT-research-5 owner
/// tune: fixed at 3 to match the three dragon engines in
/// cosmostrix (cosmic_dragon_engine, crystal_dragon_engine,
/// chroma_dragon_engine). The active target is also fixed at 3
/// regardless of density — the dragon count is a deliberate
/// signature, not a tunable.
pub(crate) const DRAGON_POOL_MAX: usize = 3;

/// Fixed active-dragon count. NIGHT-research-5 owner directive:
/// always 3 dragons on screen — matches the 3 dragon engines in
/// cosmostrix (cosmic_dragon_engine, crystal_dragon_engine,
/// chroma_dragon_engine). Density no longer affects the count;
/// density only influences spawn timing (which is already
/// deficit-bounded by the spawn accumulator).
pub(crate) const DRAGON_FIXED_ACTIVE: usize = 3;

/// Base active-dragon ratio (DEPRECATED by NIGHT-research-5 owner
/// directive — kept for compatibility but the active count is now
/// fixed at DRAGON_FIXED_ACTIVE regardless of density).
#[allow(dead_code)]
pub(crate) const DRAGON_ACTIVE_BASE: f32 = 0.005;

/// Density multiplier (DEPRECATED by NIGHT-research-5 owner
/// directive — kept for compatibility but the active count is now
/// fixed at DRAGON_FIXED_ACTIVE regardless of density).
#[allow(dead_code)]
pub(crate) const DRAGON_ACTIVE_DENSITY_MULT: f32 = 0.030;

/// Maximum active-dragon ratio cap (DEPRECATED by NIGHT-research-5
/// owner directive — kept for compatibility).
#[allow(dead_code)]
pub(crate) const DRAGON_ACTIVE_MAX: f32 = 0.030;

/// Spawn rate multiplier for dragon generation. Steady state needs
/// target/avg_lifetime dragons per second; 0.35x target + floor 1.5
/// reaches that with headroom for ramp-up after scene entry (parity
/// with vortex/lorenz tuning).
pub(crate) const DRAGON_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const DRAGON_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Mote lifetime cap (seconds). 20s gives each dragon a long
/// majestic flight — at speed 18 (default scene), the dragon
/// traverses ~360 cells before refresh. Shorter → constant respawn
/// chatter; longer → motes pile up.
pub(crate) const DRAGON_LIFETIME_SECS: f32 = 20.0;

/// NIGHT-enhanced-4: entry-reveal duration (seconds). When a dragon
/// spawns, it enters from the top of the viewport with a graceful
/// head-down heading and the body chain unfurls segment-by-segment
/// from head to tail over this duration. 1.5s is long enough to read
/// as an elegant, majestic entrance (not an instant pop) but short
/// enough that the dragon reaches full body before the viewer's
/// attention drifts. The entry uses an ease-out curve (smooth start,
/// gentle settle) so the reveal decelerates naturally — the last few
/// tail segments arrive softly, like a brush stroke settling.
pub(crate) const DRAGON_ENTRY_DURATION_SECS: f32 = 1.5;

/// Head speed scale (cells/sec per chars_per_sec unit). At 1.0 the
/// dragon's head moves at the same rate as droplet rain. Lower
/// values make the dragon more majestic; higher values make it
/// frantic (out of character for Chinese mythology).
pub(crate) const DRAGON_SPEED_SCALE: f32 = 1.0;

/// SOAR state turn rate (radians/sec, max). The layered sine
/// noise in the advance pass scales this — actual turn rate
/// oscillates between -0.7x and +0.7x of this value, producing
/// organic free-flight curves.
pub(crate) const DRAGON_SOAR_TURN_RATE: f32 = 1.5;

/// CIRCLE state turn rate (radians/sec, constant). Combined with
/// the head speed, produces a circular orbit of radius
/// speed / turn_rate ≈ 12 cells at speed 18 — visible but not
/// screen-filling.
pub(crate) const DRAGON_CIRCLE_TURN_RATE: f32 = 1.5;

/// SOAR state minimum duration (seconds).
pub(crate) const DRAGON_SOAR_MIN_DURATION: f32 = 4.0;

/// SOAR state maximum duration (seconds).
pub(crate) const DRAGON_SOAR_MAX_DURATION: f32 = 8.0;

/// CIRCLE state minimum duration (seconds).
pub(crate) const DRAGON_CIRCLE_MIN_DURATION: f32 = 3.0;

/// CIRCLE state maximum duration (seconds).
pub(crate) const DRAGON_CIRCLE_MAX_DURATION: f32 = 6.0;

/// Matrix-style glyph mutation chance when a segment crosses into a
/// new cell (mutation tied to motion, like classic matrix rain —
/// parity with vortex/lorenz shimmer gates).
pub(crate) const DRAGON_SHIMMER_CHANCE: f32 = 0.4;

// ── Physarum (seventh rain style, NIGHT-research-6) ────────────────────
// Bio-inspired emergent network model (Jeff Jones 2010): particles
// follow sense-decide-move-deposit rules on a stigmergic trail field.
// Three sensor samples steer each particle toward the strongest
// signal; positive feedback between deposition and sensing creates
// emergent vein-like networks. Trail decays exponentially (negative
// feedback) so unused paths fade. The terminal's discrete cell grid
// IS the substrate — perfect medium match (masterpiece contract).

/// Sensor angle offset (radians, left and right of heading). At
/// PI/4 (45 degrees), particles sense a 90-degree cone ahead —
/// the standard Jeff Jones value, produces branching network
/// patterns. Smaller angles give tighter veining; larger gives
/// more diffuse coverage.
pub(crate) const PHYSARUM_SENSOR_ANGLE: f32 = std::f32::consts::FRAC_PI_4;

/// Sensor sample distance (cells ahead of the particle). At 3.0,
/// particles sense 3 cells in front of them — far enough to detect
/// oncoming trails but close enough to keep local steering. Larger
/// values produce global network convergence; smaller produces
/// local maze-like patterns.
pub(crate) const PHYSARUM_SENSOR_DISTANCE: f32 = 3.0;

/// Step size per chars_per_sec: step_dist = cps * this * dt. At
/// 2.0 + speed 18 + 60 FPS, particles move 18 * 2.0 * 0.0167 =
/// 0.6 cells per frame — visible motion that visits multiple cells
/// per second (essential for network emergence — too slow and
/// particles pile up on single cells, too fast and they skip the
/// sensor sampling window).
pub(crate) const PHYSARUM_STEP_PER_CPS: f32 = 2.0;

/// Trail deposit amount per particle per second. At 0.5, a cell
/// visited by one particle for one second accumulates 0.5 trail
/// value. Against the rate-independent decay (0.90 per 60 Hz
/// reference step), the steady-state trail at a continuously
/// visited cell is deposit-per-step / (1 - decay) = (0.5/60)/0.10
/// ~ 0.083 — above the PHYSARUM_BRIGHTNESS_DIM threshold, so
/// single-particle cells reach Mid zone, and multi-particle cells
/// accumulate into Hot/Core (the visible vein signature).
pub(crate) const PHYSARUM_DEPOSIT_AMOUNT: f32 = 0.5;

/// Trail decay rate (per-step multiplier, quoted at the 60 Hz
/// reference cadence). The advance pass raises this constant to
/// (dt × 60) — the per-second decay is frame-rate independent, so a
/// 144 Hz terminal and a 30 Hz terminal produce the same trail
/// equilibrium and the same vein brightness grading against the
/// absolute PHYSARUM_BRIGHTNESS_* thresholds (NIGHT-hunter-10; the
/// former per-frame multiply made the equilibrium scale with the
/// frame rate). At 0.90, the trail loses 10% of its value per
/// reference step — at 60 FPS, an unvisited cell fades to 1% of its
/// peak value in ~0.44 seconds. This is the negative feedback that
/// lets unused paths fade so the network stays alive (without decay,
/// every cell saturates and the network disappears). Tuned higher
/// than the original 0.92 so fresh trails are brighter relative to
/// old ones (more visible vein distinction).
pub(crate) const PHYSARUM_TRAIL_DECAY: f32 = 0.90;

/// Max turn rate (radians/sec). At 1.0, particles can curve up to
/// 1 radian per second — produces the organic curved vein signature
/// (sharp turns would break the network pattern into straight
/// segments). The actual turn per frame is rate-bounded: turn_rate
/// * dt_p (where dt_p is per-particle pace-adjusted dt).
pub(crate) const PHYSARUM_TURN_RATE: f32 = 1.0;

/// Particle lifetime cap (seconds). At 15s, each particle gets
/// enough time to contribute to multiple network paths before
/// refresh — the continuous absorption/respawn keeps the
/// simulation alive without saturating. Shorter → constant respawn
/// chatter (no network emergence); longer → patterns freeze.
pub(crate) const PHYSARUM_LIFETIME_SECS: f32 = 15.0;

/// Base active-particle ratio for density scaling. Combined with
/// PHYSARUM_ACTIVE_DENSITY_MULT, yields 30-60 particles at typical
/// densities (0.40-0.85). More particles produce denser networks;
/// fewer produce sparser branching.
pub(crate) const PHYSARUM_ACTIVE_BASE: f32 = 0.30;

/// Density multiplier for physarum active-count calculation.
pub(crate) const PHYSARUM_ACTIVE_DENSITY_MULT: f32 = 0.40;

/// Maximum active-particle ratio cap (of the one-particle-per-column
/// pool). Bounds the active count so very high density settings
/// don't oversaturate the trail field.
pub(crate) const PHYSARUM_ACTIVE_MAX: f32 = 0.75;

/// Spawn rate multiplier for particle generation. Steady state
/// needs target/avg_lifetime particles per second; 0.35x target +
/// floor 1.5 reaches that with headroom for ramp-up after scene
/// entry (parity with vortex/lorenz/dragon tuning).
pub(crate) const PHYSARUM_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const PHYSARUM_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Matrix-style glyph mutation chance when a particle crosses into
/// a new cell (mutation tied to motion — parity with the other
/// structured styles' shimmer gates).
pub(crate) const PHYSARUM_SHIMMER_CHANCE: f32 = 0.4;

/// Brightness zone boundary: trail value above this → Core (hot
/// vein). Tuned so cells visited by 4+ particles reach this
/// brightness (the visible network signature).
pub(crate) const PHYSARUM_BRIGHTNESS_HOT: f32 = 0.30;

/// Brightness zone boundary: trail value above this → Hot.
/// Cells visited by 2-3 particles reach this brightness
/// (sustained vein paths).
pub(crate) const PHYSARUM_BRIGHTNESS_MID: f32 = 0.15;

/// Brightness zone boundary: trail value above this → Mid; below
/// → Ghost (exploring new territory, low trail accumulation).
/// Single-particle-visited cells typically reach this brightness
/// after sustained deposition (steady-state ~0.10).
pub(crate) const PHYSARUM_BRIGHTNESS_DIM: f32 = 0.03;

// ── Black hole (eighth rain style, NIGHT-special-1) ────────────────────
// A gravitating body, not a particle field: a centered medium ball
// with a black empty event-horizon core and a photon-ring rim that
// fades outward into the dark (the sorgonemous_intrascals scene).
// Staged rollout — stage 1 ships the ball; stage 2 adds the RK4
// orbital ring, stage 3 the glyph infall. All radius math runs in
// line-height units so the geometry scales with any screen size.

/// Ball outer radius as a fraction of the viewport's limiting
/// half-extent (in line-height units: min(cols / 4, lines / 2)).
/// 0.55 reads as a medium ball on every terminal class — roughly a
/// third of the short axis at 80x24 and the same proportion at
/// 400x100, matching the owner's "medium size ball" spec.
pub(crate) const BLACK_HOLE_BALL_FRACTION: f32 = 0.55;

/// Event-horizon (empty core) radius as a fraction of the ball outer
/// radius. The core is never drawn — it shows the background, the
/// hole itself. 0.58 leaves a visible annulus on small terminals
/// while keeping the empty middle unmistakably dominant, per the
/// owner's "core is black/empty" spec.
pub(crate) const BLACK_HOLE_CORE_FRACTION: f32 = 0.58;

/// Per-cell per-frame glyph re-roll chance for the ball surface (the
/// matrix-shimmer life sign every style carries). Far below the
/// motion-gated shimmer of the particle styles because the ball is
/// static: 0.02 at 60 FPS re-rolls each cell about once per 0.8 s —
/// a calm surface flicker at the event horizon, not a chaotic storm.
pub(crate) const BLACK_HOLE_SHIMMER_CHANCE: f32 = 0.02;

// ── Black hole orbital ring (stage 2, NIGHT-special-1) ─────────────────
// RK4-Lorenz-turbulent motes orbiting the ball on a wide tilted
// ellipse — the accretion-disk read of the iconic imagery (EHT M87*,
// Gargantua): a disk that stretches far left and right of the
// shadow, near side crossing in front of the hole, far side lensed
// up and over the top (gravitational lensing bends the far-side
// light into the halo arc above the shadow). The mean motion is
// Keplerian (inner motes orbit faster, the differential rotation of
// a real disk); the canonical Lorenz attractor (sigma 10, rho 28,
// beta 8/3 — the same system the lorenz style renders) drives the
// turbulence: its radial coordinate wobbles the orbital radius, its
// z displaces the mote out of the ring plane and grades the glyph
// brightness. Ellipse radii are fractions of the viewport unit (the
// same unit the ball scales from), so the disk scales with any
// screen size like the ball does.

/// Orbital ellipse semi-major axis (the horizontal reach) as a
/// fraction of the viewport unit. 1.05 stretches the disk to about
/// twice the ball's radius left and right of the shadow — the wide
/// left-right read of every real accretion-disk image, per the
/// owner's stage-2 feedback ("the ring should be long left-right").
pub(crate) const BLACK_HOLE_RING_MAJOR_FRACTION: f32 = 1.05;

/// Per-tier disk geometry (stage 2.4, the owner's Interstellar stack:
/// "the stage-1 ring is the longest, stage 2 sits above it a bit
/// shorter, stage 3 is the shortest and closest to the black hole").
/// The ring pool carries three concentric bands: tier 0 is the
/// 9.7/10 approved equatorial main disk (its values mirror the
/// stage-2.3 scalars — the table is now the single source of truth,
/// the scalars it references stay for the test pins); tier 1 is a
/// shorter band a small step above the equatorial line; tier 2 is
/// the shortest band one more small step up — the tight
/// stacked-lines family of the Gargantua imagery. Stage 2.5, the
/// owner's snug-gap ruling: the upper two lines must sit close to
/// the main line and to each other (his analogy measures two
/// objects ten meters apart when they should read one meter apart).
/// Stage 2.6 (owner 9.9/10 feedback) closed the gap to the
/// near-merged read. Stage 2.7 (owner 9.95/10 feedback) drops the
/// WHOLE family below the viewport center (his ruling: all three
/// snug stacks reduce their position so the light reads below
/// center) and stretches the main band's reach by exactly a quarter
/// (his length analogy: the default 1 cm line must read 1.25 cm).
/// All lengths are fractions (the semi-major scale multiplies the
/// main disk's major fraction, the minor multiplies the viewport
/// unit, offsets and tilts multiply the ball outer radius) so the
/// stack scales with any screen size exactly like the ball.
pub(crate) struct BlackHoleRingTier {
    /// Semi-major scale of this tier (times the main disk's
    /// MAJOR_FRACTION reach): 1.375 / 0.80 / 0.52 — the descending
    /// staircase of the stacked lines, the main band stretched by a
    /// quarter at stage 2.7 (the owner's 1 cm -> 1.25 cm length
    /// ruling; the stage-2.6 value was 1.10).
    pub(crate) major_scale: f32,
    /// Semi-minor axis of this tier's orbit (fraction of the
    /// viewport unit — the vertical thickness of the band).
    pub(crate) minor_fraction: f32,
    /// Band center offset from the ball's equator, in ball outer
    /// radii: -0.30 / -0.17 / -0.07 — the stage-2.7 whole-family
    /// descent (the owner's 9.95/10 ruling: all three snug stacks
    /// reduce their position so the light reads BELOW the center).
    /// The near-merged center steps of the approved stage-2.6 read
    /// stay untouched (0.13 / 0.10 — the family keeps its dense
    /// braided grouping, now hanging under the equator like the real
    /// Gargantua composition: crown above, disk line below).
    pub(crate) center_offset: f32,
    /// Near-strand squash factor: the in-front half maps its sine
    /// onto this fraction of the minor axis (the strand separation
    /// that keeps the two flow directions of a band apart).
    pub(crate) near_squash: f32,
    /// Radial turbulence amplitude of this tier (multiple of the
    /// ball outer radius) — scaled down on the shorter bands so the
    /// tight lines do not wobble past their own reach.
    pub(crate) wobble_fraction: f32,
    /// Out-of-plane displacement amplitude of this tier (multiple of
    /// the ball outer radius) — the breathing thickness, reduced per
    /// tier so the upper lines stay crisp.
    pub(crate) z_tilt: f32,
    /// Keplerian pace multiplier of this tier: the inner bands orbit
    /// visibly faster (1.6x / 2.4x) — the differential rotation of a
    /// real multi-ring disk, per Kepler's third law scaling.
    pub(crate) pace: f32,
    /// Share of the mote pool that spawns onto this tier (the
    /// weights sum to 1.0). Tier 0 keeps the lion's share because
    /// the equatorial line must read as the solid white band.
    pub(crate) spawn_weight: f32,
}

/// The three-tier Interstellar stack (stage 2.4, tightened stage
/// 2.5, closed to the near-merged read stage 2.6, descended below
/// center and stretched stage 2.7). Tier 0 carries the approved
/// equatorial main disk scalars plus the stage-2.7 reads: the band
/// centers drop to -0.30 / -0.17 / -0.07 outer radii — the whole
/// family hangs BELOW the viewport center (the owner's 9.95/10
/// ruling) while the near-merged 0.13/0.10 center steps keep the
/// dense braided grouping — and the main band's reach widens to
/// 1.375x the major fraction (the 1 cm -> 1.25 cm expansion). The
/// vertical budget: tier 2's band center sits 0.07 outer radii
/// below the ball center, tier 1's 0.17, the main band's 0.30 — the
/// family crosses the shadow's lower face, with the 1.30 lensing
/// arc crown and the double halo streams well above it.
pub(crate) const BLACK_HOLE_RING_TIERS: [BlackHoleRingTier; 3] = [
    BlackHoleRingTier {
        major_scale: 1.375,
        minor_fraction: BLACK_HOLE_RING_MINOR_FRACTION,
        center_offset: -0.30,
        near_squash: BLACK_HOLE_RING_NEAR_SQUASH,
        wobble_fraction: BLACK_HOLE_RING_WOBBLE_FRACTION,
        z_tilt: BLACK_HOLE_RING_Z_TILT,
        pace: 1.0,
        spawn_weight: 0.52,
    },
    BlackHoleRingTier {
        major_scale: 0.80,
        minor_fraction: 0.055,
        center_offset: -0.17,
        near_squash: 0.42,
        wobble_fraction: 0.20,
        z_tilt: 0.05,
        pace: 1.6,
        spawn_weight: 0.30,
    },
    BlackHoleRingTier {
        major_scale: 0.52,
        minor_fraction: 0.04,
        center_offset: -0.07,
        near_squash: 0.38,
        wobble_fraction: 0.14,
        z_tilt: 0.035,
        pace: 2.4,
        spawn_weight: 0.18,
    },
];

/// Orbital ellipse semi-minor axis (the vertical squeeze) as a
/// fraction of the viewport unit. 0.14 renders the disk almost
/// exactly edge-on — the Gargantua read of the owner's stage-2.3
/// feedback (9.5/10): the Interstellar disk reads as one thin
/// horizontal line through the shadow's middle, not a fat band
/// ("dense enough to read as a horizontal line"). Halved from 0.28 at
/// the same time the near side gained its vertical squash and the
/// density floor tripled — thinner geometry, more glyphs, solid
/// line. Tier 0's value in the stage-2.4 tier table (the upper
/// bands carry their own thinner fractions).
pub(crate) const BLACK_HOLE_RING_MINOR_FRACTION: f32 = 0.14;

/// Near-side vertical squash factor: the in-front half of the orbit
/// maps its sine onto this fraction of the semi-minor axis, so the
/// crossing band hugs the equator instead of dipping a full minor
/// axis below it. Stage 2.3 owner feedback: the solid line must sit
/// at the vertical MIDDLE of the core ("the line should sit at the
/// middle of the black hole core — right now it sits below it")
/// — with 0.5 the near side spans only half a minor axis below the
/// center, and together with the z-tilt breathing the band reads
/// centered on the shadow. The far side keeps the full factor: it
/// belongs to the arms rising into the lensing halo. Tier 0's value
/// in the stage-2.4 tier table.
pub(crate) const BLACK_HOLE_RING_NEAR_SQUASH: f32 = 0.5;

/// Gravitational-lensing halo radius as a multiple of the ball outer
/// radius. Far-side motes are re-projected onto an arc of this
/// radius over the top of the shadow: 1.30 hugs just above the
/// photon ring, the bright halo of the iconic images — the
/// "particles curve upward as they approach the hole's edge" read
/// from the owner's stage-2 feedback.
pub(crate) const BLACK_HOLE_RING_LENS_ARC_FRACTION: f32 = 1.30;

/// Entry spiral radius excess for freshly spawned motes: new motes
/// materialize 55% beyond the disk and settle onto the ring
/// exponentially — accretion from outside, not pop-in on the orbit.
/// Also gives steady-state respawns their drift-in read.
pub(crate) const BLACK_HOLE_RING_ENTRY_BOOST: f32 = 0.55;

/// Entry spiral decay time constant in seconds (the excess radius
/// decays to 5% within ~3 tau ≈ 2.7 s).
pub(crate) const BLACK_HOLE_RING_ENTRY_TAU: f32 = 0.9;

/// Ball rim co-rotation rate as a multiple of the ring's mean
/// angular rate. 1.0 locks the hole's rim to the disk's phase — the
/// glyphs and the Doppler-style bright lobe circulate left-to-right
/// around the event horizon in lockstep with the orbiting stream
/// (the owner's stage-2 feedback: the hole must visibly spin,
/// synchronized with the ring).
pub(crate) const BLACK_HOLE_RING_SPIN_RATE: f32 = 1.0;

/// Angular width of one rim conveyor bucket (radians). The rim's
/// glyph pattern is bucketed at this granularity and the buckets
/// slide around the annulus with the spin phase — the visible
/// surface rotation of the ball.
pub(crate) const BLACK_HOLE_RING_CONVEYOR_ARC: f32 = 0.26;

/// Cosine threshold of the rotating Doppler-style brightness lobe on
/// the rim: cells within ±53 degrees of the lobe peak brighten one
/// ladder rung, cells near the opposite point dim one — a hot spot
/// sweeping around the event horizon with the spin.
pub(crate) const BLACK_HOLE_RING_LOBE_GAIN: f32 = 0.6;

/// Radial turbulence amplitude as a multiple of the ball outer
/// radius, driven by the attractor's radial coordinate (the lobe
/// distance). 0.34 swings the band across roughly a third of the
/// ball radius — a living plasma stream, not a rigid hoop. Tier 0's
/// value in the stage-2.4 tier table (the upper bands wobble less).
pub(crate) const BLACK_HOLE_RING_WOBBLE_FRACTION: f32 = 0.34;

/// Out-of-plane displacement amplitude (the attractor z mapped onto
/// the screen vertical), as a multiple of the ball outer radius.
/// Tightened 0.18 -> 0.12 for the stage-2.3 thin-disk read: the band
/// breathes around the equatorial plane (the thickness cue of a real
/// disk) without inflating the solid line into a ribbon — at the
/// scene default the excursions stay under ~1.2 lines at 120x40.
/// Tier 0's value in the stage-2.4 tier table (the upper bands
/// breathe shallower so the stacked lines stay crisp).
pub(crate) const BLACK_HOLE_RING_Z_TILT: f32 = 0.12;

/// Proximity-brightness hot radius (stage 2.4, the owner's 9.7/10
/// feedback: "particles still near the black hole must be bright —
/// simply use the white head"). Motes whose projected screen distance
/// from the hole's center falls below this multiple of the ball
/// outer radius step UP two ladder rungs at draw time — Mid and Hot
/// bases land at Core, the white-hot plasma: the crossing band
/// across the shadow, and (at 1.30, just inside the bound) the whole
/// lensing halo arc. Two rungs instead of the stage-2.3 one because
/// the distance is now the key: the near-hole read must be
/// unmistakably the brightest thing on screen.
pub(crate) const BLACK_HOLE_RING_HOT_RADIUS: f32 = 1.32;

/// Proximity-brightness fade start: beyond this multiple of the ball
/// outer radius the disk brightness steps down toward Ghost rung by
/// rung — the outer disk thins out with distance from the hole. The
/// owner's Interstellar reference: the dense white line ends in a
/// few sparse particles, a smooth transition into the dark
/// ("the end of the white line carries a few sparse particles so
/// the transition reads smooth"), and his stage-2.4 wording: "the
/// ones moving away fade, less bright".
/// 1.40 keeps the flat main line bright out to ~70% of its reach
/// before the dissolve begins.
pub(crate) const BLACK_HOLE_RING_FADE_START: f32 = 1.40;

/// Proximity-brightness fade span: the distance (in ball radii)
/// over which the fade ladder descends its full rung count past the
/// fade start — from 1.40 to 1.95, which brackets the flat line's
/// extremes (~1.9 with wobble). Fresh entry-spiral motes drift in
/// from beyond the span at Ghost brightness and ignite as they
/// approach — the accretion read for free.
pub(crate) const BLACK_HOLE_RING_FADE_SPAN: f32 = 0.55;

/// Edge-fade depth: the maximum ladder rungs stepped down past the
/// fade start. 3 rungs lands the farthest-dwell motes at Ghost — dim
/// wisps — while the inner band stays untouched, the smooth density
/// falloff of a real disk fading with radius.
pub(crate) const BLACK_HOLE_RING_EDGE_FADE_RUNGS: u8 = 3;

/// Trail depth per mote (comet streak length in cells). Four cells
/// matches the vortex drain streak: at the ring's tangential speed
/// the head crosses a new cell every few frames, so four cells show
/// a short luminous arc trailing each mote.
pub(crate) const BLACK_HOLE_RING_TRAIL_LEN: usize = 4;

/// Mote lifetime cap in seconds (with ±15% per-mote variance at
/// spawn, so absorption is staggered — no rhythmic mass respawn).
/// 14 s is slightly calmer than lorenz's 12 s: one orbit at the
/// scene default speed takes ~11.6 s, so most motes complete a
/// full lap before recycling.
pub(crate) const BLACK_HOLE_RING_MAX_AGE_SECS: f32 = 14.0;

/// Base active-mote ratio for density scaling (pool = one mote per
/// column, the family lane model). History: 0.22 -> 0.55 at stage
/// 2.3 (the solid-band read), 0.55 -> 0.75 at stage 2.4 — the pool
/// now feeds THREE bands (the Interstellar stack), and tier 0's
/// 52% share of 0.94 target must still cover the equatorial line
/// with comet-trail overlap (the 9.7/10 solidity verdict must not
/// regress when the upper bands light up).
pub(crate) const BLACK_HOLE_RING_ACTIVE_BASE: f32 = 0.75;

/// Density multiplier for the ring active-count target.
pub(crate) const BLACK_HOLE_RING_ACTIVE_DENSITY_MULT: f32 = 0.35;

/// Maximum active-mote ratio cap of the pool — bounds the stream
/// density so extreme settings don't saturate the band. Raised
/// 0.55 -> 1.0 for stage 2.3: a full pool is the honest ceiling for
/// the solid-line read (every column hosts a mote), and the spawn
/// accumulator's deficit bound already keeps the fill gradual.
pub(crate) const BLACK_HOLE_RING_ACTIVE_MAX: f32 = 1.0;

/// Spawn rate multiplier (steady state needs target/avg_lifetime
/// motes per second; 0.35x target + floor 1.5 reaches it with
/// ramp-up headroom — parity with vortex/lorenz/physarum).
pub(crate) const BLACK_HOLE_RING_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum spawns per second).
pub(crate) const BLACK_HOLE_RING_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Matrix-style glyph mutation chance when a mote head crosses into
/// a new cell (mutation tied to motion — the family shimmer gate).
pub(crate) const BLACK_HOLE_RING_SHIMMER_CHANCE: f32 = 0.4;

/// RK4 integration step per cell of speed: chars_per_sec mapped onto
/// attractor time (parity with LORENZ_DT_PER_CPS — the same
/// canonical system, the same stability regime; RK4 stays stable
/// for dt well under 0.01).
pub(crate) const BLACK_HOLE_RING_DT_PER_CPS: f32 = 0.005;

/// Mean orbital angular rate per cell of speed (radians per
/// second). At the scene default speed 12 the ring completes one
/// lap in ~11.6 s — a majestic pace; the up/down speed keys scale
/// it linearly like every other style.
pub(crate) const BLACK_HOLE_RING_OMEGA_PER_CPS: f32 = 0.045;

/// Keplerian shear exponent (Kepler's third law: omega scales with
/// r to the minus three-halves). Motes wobbled inward orbit
/// visibly faster than ones wobbled outward — the differential
/// rotation signature of a real accretion disk.
pub(crate) const BLACK_HOLE_RING_KEPLER_EXP: f32 = 1.5;

/// Attractor radial center for the wobble normalization: the lobe
/// radius sqrt(beta*(rho-1)) of the canonical system (about 8.485).
/// The mote's distance from the attractor z-axis, normalized around
/// this center, drives the orbital-radius turbulence.
pub(crate) const BLACK_HOLE_RING_R_NORM_CENTER: f32 = 8.5;

/// Attractor radial normalization gain (1 / R_NORM_CENTER).
pub(crate) const BLACK_HOLE_RING_R_NORM_GAIN: f32 = 1.0 / 8.5;

/// Attractor z center for the out-of-plane normalization: the lobe
/// altitude rho - 1 = 27 of the canonical system.
pub(crate) const BLACK_HOLE_RING_Z_NORM_CENTER: f32 = 27.0;

/// Attractor z normalization gain (the canonical attractor z range
/// [0, 50] spans ±23 around the center).
pub(crate) const BLACK_HOLE_RING_Z_NORM_GAIN: f32 = 1.0 / 23.0;

// ── Black hole see-saw roll (stage 2.4, NIGHT-special-1) ────────────────
// The owner's lever motion ("the ring can rise and fall left and
// right like a lever"): the whole disk stack pivots around the hole
// in the screen plane. The flat horizontal line (180 degrees in the
// owner's wording) is the REST mode; the excursions tilt the stack
// — left end rising while the right end descends — through the
// 15-180 degree attitude window with exactly the vertical
// 90-degree attitude excluded (the stage-2.5 owner ruling), every
// attitude parked at a LONG hold (30 s or more — the improved,
// more special long duration), then eased onward or chained into
// the next tilt. The angle is the deviation from horizontal in
// radians, positive = left side up.

/// Hold duration of the flat rest mode (seconds). 36 s keeps the
/// flat line the single longest pose on the timeline — the owner's
/// spec that the 180-degree horizontal mode holds the longest
/// (originally ~30 s), now sitting above the 30 s tilt holds so the
/// Gargantua read stays the identity of the scene while every
/// attitude enjoys a 30 s-or-more dwell.
pub(crate) const BLACK_HOLE_ROLL_FLAT_HOLD: f32 = 36.0;

/// Hold duration of a tilted excursion (seconds). 30 s per the
/// stage-2.5 owner ruling — the long duration now applies across
/// the whole 15-180 degree attitude window (not only the flat
/// line), making each tilted pose a deliberate, special vista
/// instead of a quick punctuation. The sweeps between attitudes
/// still resolve within a few seconds, so the lever always reads
/// alive.
pub(crate) const BLACK_HOLE_ROLL_TILT_HOLD: f32 = 30.0;

/// Roll sweep rate (radians per second). 0.42 carries a full
/// 85-degree sweep in ~3.5 s — the "up and down within a few
/// seconds" read: deliberate, gravitational, never a snap.
pub(crate) const BLACK_HOLE_ROLL_RATE: f32 = 0.42;

/// Minimum roll-turn duration (seconds) — small corrections near the
/// rest line still read as motion, not teleport.
pub(crate) const BLACK_HOLE_ROLL_TURN_MIN_SECS: f32 = 1.2;

/// Maximum roll-turn duration (seconds) — the widest chained swing
/// (from +85 to -85 degrees through the flat line, 170 degrees)
/// clamps here so even the biggest lever sweep resolves within
/// ~5.5 s.
pub(crate) const BLACK_HOLE_ROLL_TURN_MAX_SECS: f32 = 5.5;

/// The excursion menu (degrees of tilt from the horizontal rest
/// line). The stage-2.5 contract: the attitude window spans
/// 15-180 degrees in the owner's convention (180 = the flat rest
/// line, 90 = vertical) with exactly 90 excluded — the lever
/// sweeps the shallow 15/30/45/50-degree tilts, the mid 60, and
/// the steep 85 (a near-vertical diagonal that keeps the drama of
/// the old vertical pose without ever parking on the excluded
/// attitude), the sign alternating every excursion so left-up and
/// right-up tilts take turns.
pub(crate) const BLACK_HOLE_ROLL_TILT_DEGS: [f32; 6] = [85.0, 60.0, 50.0, 45.0, 30.0, 15.0];

/// Chance (percent) that a tilted excursion chains directly into the
/// next tilted excursion instead of returning to the flat rest line
/// first — the disk sweeps through horizontal and keeps going, the
/// continuous lever wave of the owner's example sequence.
pub(crate) const BLACK_HOLE_ROLL_CHAIN_PCT: u32 = 35;

// ── Black hole halo streams (stage 2.6, re-weighted stage 2.7 — NIGHT-special-1) ────────────────
// The arc-riding companion streams of the disk stack. Stage 2.6
// (owner 9.9/10 feedback) shipped the pool: stream motes ride the
// ARC CIRCLE around the shadow instead of the flat ellipse — same
// motion DNA as the ring motes (RK4 Lorenz turbulence, Keplerian
// mean motion, entry-spiral drift-in, proximity brightness, comet
// trails) — each mote riding the full circle and drawing only on
// its own semicircle, handing off at the extremes where the arcs
// meet the equatorial band, the read of plasma sweeping over the
// top and under the bottom of the shadow in the disk's rotational
// sense. Stage 2.7 (owner 9.95/10 feedback) re-weights the split
// into the DOUBLE UPWARD STREAM: a second upper lane joins the
// first on a wider arc (two distinct crowns of the same rider
// population — the upward-curving read doubles through two arcs,
// not one thickened band), while the lower stream drops to a RARE
// echo (a sparse particle trickle under the shadow). Geometry is
// fractions of the ball outer radius, so the streams scale with
// any screen size.

/// Inner upper halo stream arc radius as a multiple of the ball
/// outer radius (the INNER crown of the stage-2.7 double upward
/// stream). 1.30 co-rides the lensing halo circle — the inner
/// stream's riders share the road with the far-side lensed image,
/// the white-hot crown of the iconic images.
pub(crate) const BLACK_HOLE_HALO_ARC_FRACTION: f32 = 1.30;

/// Lower halo stream arc radius as a multiple of the ball outer
/// radius. 1.30 mirrors the upper circle under the shadow (the
/// owner's wording: same as the above, just the opposite position
/// below) — the rare stage-2.7 echo that completes the photon-ring
/// read around the hole.
pub(crate) const BLACK_HOLE_HALO_LOWER_ARC_FRACTION: f32 = 1.30;

/// Outer upper halo stream arc radius as a multiple of the ball
/// outer radius (the OUTER crown of the stage-2.7 double upward
/// stream). 1.48 rides clear of the inner crown's wobble band
/// (1.30 ± 0.10), so the two upward streams read as two distinct
/// arcs over the shadow — the double crown of the owner's 9.95/10
/// ruling — while the apex still fits comfortably inside the
/// viewport's half-height on every terminal class from 80x24 up
/// (the dynamic-screen-size contract).
pub(crate) const BLACK_HOLE_HALO_OUTER_ARC_FRACTION: f32 = 1.48;

/// Radial turbulence amplitude of the halo streams (multiple of the
/// ball outer radius, driven by the attractor's radial coordinate —
/// the same wobble source the ring motes use). 0.10 gives each arc a
/// thin plasma thickness without ever dipping inside the ball
/// silhouette (the inner crowns bottom out at 1.30 - 0.10 = 1.20
/// outer radii, the outer crown at 1.48 - 0.10 = 1.38 — no
/// occlusion rule needed for any stream rider).
pub(crate) const BLACK_HOLE_HALO_WOBBLE_FRACTION: f32 = 0.10;

/// Spawn share of the inner upper halo stream (the three stream
/// weights sum to 1.0): 0.41 keeps the lensing-circle lane exactly
/// as populated per arc as the old single upper stream was — the
/// stage-2.7 doubling comes from running TWO crowns at that
/// population, not from thickening one band (the ring pool's tier
/// shares are untouched).
pub(crate) const BLACK_HOLE_HALO_UPPER_WEIGHT: f32 = 0.41;

/// Spawn share of the outer upper halo stream — the second lane of
/// the stage-2.7 double upward stream (the owner's 9.95/10 ruling:
/// the upward stream now carries a double upward stream). 0.41
/// mirrors the inner share exactly, so the two crowns carry the
/// same rider population: two arcs of the old single stream's
/// density, the honest reading of "double".
pub(crate) const BLACK_HOLE_HALO_OUTER_WEIGHT: f32 = 0.41;

/// Spawn share of the lower halo stream — 0.18 leaves the mirrored
/// stream RARE under the shadow (the owner's stage-2.7 ruling: the
/// lower stream keeps only sparse particles), a faint echo of the
/// double crown while the composition stays anchored on the sky
/// above the hole.
pub(crate) const BLACK_HOLE_HALO_LOWER_WEIGHT: f32 = 0.18;

// Compile-time contracts on the stream weights: the three shares
// partition the pool (sum to one — the halo activation walks the
// upper family's combined bound and falls through to the lower),
// the two upper crowns carry EQUAL shares (the spawn pass splits
// the upper family with a strict inner/outer lane toggle, so the
// constants must agree with it), and the lower stream stays the
// rare one (the owner's stage-2.7 wording: the lower curve reads
// sparse).
const _: () = assert!(
    BLACK_HOLE_HALO_UPPER_WEIGHT + BLACK_HOLE_HALO_OUTER_WEIGHT + BLACK_HOLE_HALO_LOWER_WEIGHT
        > 0.99
        && BLACK_HOLE_HALO_UPPER_WEIGHT
            + BLACK_HOLE_HALO_OUTER_WEIGHT
            + BLACK_HOLE_HALO_LOWER_WEIGHT
            < 1.01
);
const _: () = assert!(BLACK_HOLE_HALO_UPPER_WEIGHT == BLACK_HOLE_HALO_OUTER_WEIGHT);
const _: () = assert!(BLACK_HOLE_HALO_LOWER_WEIGHT < BLACK_HOLE_HALO_UPPER_WEIGHT);

/// Keplerian pace multiplier of the inner halo streams at their arc
/// radius. (1.30 / 1.05)^(-3/2) is about 0.74: the inner crowns sit
/// beyond the disk's mean radius, so their riders orbit visibly
/// slower — Kepler's third law across the whole system (and the
/// upper sweep still flows left-to-right over the top, matching the
/// far-side lensing direction — the rotation follows the disk).
pub(crate) const BLACK_HOLE_HALO_PACE: f32 = 0.74;

/// Keplerian pace multiplier of the outer upper stream at its arc
/// radius. (1.48 / 1.05)^(-3/2) is about 0.60: the outer crown
/// orbits visibly slower than the inner one — Kepler's third law
/// across the two crowns of the double upward stream, the layered
/// outer-lane read.
pub(crate) const BLACK_HOLE_HALO_OUTER_PACE: f32 = 0.60;

/// Base active-mote ratio of the halo pool (pool = one stream mote
/// per column, the family lane model). Mirrors the ring pool's own
/// base exactly: the halo pool fills to the same fraction of its
/// lanes as the ring does at every density setting, so the double
/// upper family's visible share (roughly 0.41 of the pool — the
/// 0.82 tag split times the visible half-lap, split evenly across
/// the two crowns) tracks the far-side lensing population
/// proportionally — the double-crown read holds across the whole
/// density range, not just at one setting.
pub(crate) const BLACK_HOLE_HALO_ACTIVE_BASE: f32 = 0.75;

/// Density multiplier for the halo active-count target (mirrors the
/// ring's multiplier — same reason as the base: proportional
/// doubling at every density).
pub(crate) const BLACK_HOLE_HALO_ACTIVE_DENSITY_MULT: f32 = 0.35;

/// Maximum active-mote ratio cap of the halo pool — the full pool is
/// the honest ceiling (parity with the ring's stage-2.3 cap).
pub(crate) const BLACK_HOLE_HALO_ACTIVE_MAX: f32 = 1.0;

/// Spawn rate multiplier for the halo pool (parity with the ring's
/// accumulator arithmetic: 0.35x target + the floor reaches the
/// steady target with ramp-up headroom).
pub(crate) const BLACK_HOLE_HALO_SPAWN_RATE_MULT: f32 = 0.35;

/// Spawn rate floor (minimum halo spawns per second).
pub(crate) const BLACK_HOLE_HALO_SPAWN_RATE_FLOOR: f32 = 1.5;

/// Halo mote lifetime cap in seconds (with the same ±15% per-mote
/// variance the ring motes carry). 16 s covers one full inner-crown
/// lap at the scene default speed (the halo pace 0.74 stretches the
/// ~11.6 s base lap to ~15.7 s); the outer crown's slower lane
/// (pace 0.60, ~19.3 s lap) means its riders complete their one
/// visible sweep and recycle mid-transit — each rider's longer
/// visibility offsets the fewer laps, so both crowns stay evenly
/// populated in the steady state.
pub(crate) const BLACK_HOLE_HALO_MAX_AGE_SECS: f32 = 16.0;

// ── Black hole glyph infall (stage 3, NIGHT-special-1) ────────────────
// The third act: the rain itself becomes the accretion material.
// Glyphs spawn above the viewport and fall through the hole's
// gravitational field — straight ambient rain far from the system,
// elegant arcs near it, a decaying spiral into the shadow for the
// captured. Motion DNA: inverse-square gravity toward the hole with
// the field's magnitude blended to zero at the influence edge (just
// past the disk's reach), plus an accretion brake inside the capture
// radius (infalling material shocks against the disk and radiates
// angular momentum away — the drag that turns hyperbolic fly-bys
// into tightening inspirals). Motes that cross the event horizon are
// eaten: never drawn inside the empty core, their final flash on the
// photon ring. Brightness is speed-graded (kinetic heat — the
// faster the mote, the brighter the base) composed with the shared
// proximity ladder, so the whip-around reads Core white while the
// far ambient rain reads Ghost. Every length is a fraction of the
// ball outer radius and every speed scales with the viewport unit,
// so the infall reads identically on any screen size.

/// Base active-mote ratio of the infall pool (pool = one infall mote
/// per column, the family lane model). 0.18 keeps the rain an AMBIENT
/// layer — sparse streaks over a dark sky, never a downpour: the hole
/// stays the hero of the composition, the rain the weather around it
/// (the density slider still scales it through the multiplier below).
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_BASE: f32 = 0.18;

/// Density multiplier for the infall active-count target: the same
/// sensitivity as the ring's, so the slider moves both layers
/// proportionally (the ambient read holds across the range).
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_DENSITY_MULT: f32 = 0.35;

/// Maximum active-mote ratio cap of the infall pool. 0.55 keeps even
/// max-density rain below the disk's population — the accretion
/// material must read subordinate to the approved stack.
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_MAX: f32 = 0.55;

/// Spawn rate multiplier for the infall pool (parity with the ring's
/// accumulator arithmetic: fraction-of-target + floor reaches the
/// steady target with a gentle ramp-up — rain drifts in, it never
/// bursts in).
pub(crate) const BLACK_HOLE_INFALL_SPAWN_RATE_MULT: f32 = 0.30;

/// Spawn rate floor (minimum infall spawns per second).
pub(crate) const BLACK_HOLE_INFALL_SPAWN_RATE_FLOOR: f32 = 0.8;

/// Infall mote lifetime cap in seconds (±15% per-mote variance, the
/// family contract). 13 s is the backstop, not the rule: most motes
/// end earlier by absorption (the horizon eats them) or by exiting
/// the viewport — the lifetime only sweeps strays caught in a high
/// tangential orbit that the brake has not yet decayed.
pub(crate) const BLACK_HOLE_INFALL_MAX_AGE_SECS: f32 = 13.0;

/// Gravitational acceleration constant of the hole, in ball-outer-radii
/// cubed per sim-second squared (a = G / r^2 in outer-r units; one
/// sim-second is one wall-second at the scene's reference 12 cps, so
/// the speed keys scale the whole field uniformly and the trajectory
/// shapes survive them). 5.2 sets the circular speed at the photon ring
/// (r = 1.0 outer radii) to sqrt(5.2) = 2.28 outer radii/sim-s — a whip,
/// against the ~1.4 outer radii/sim-s orbital speed of the disk's inner
/// edge: material visibly accelerates as it falls the last stretch
/// (Kepler's second law, the exchange of height for speed).
pub(crate) const BLACK_HOLE_INFALL_GRAVITY: f32 = 5.2;

/// Field influence radius in ball outer radii: the smoothstep blend
/// that fades the gravitational pull to zero. 2.80 sits just past the
/// tier-0 disk's semi-major reach (1.375 x 1.05 / 0.55 = 2.62 outer
/// radii), so the bending zone and the disk read as one system — rain
/// crossing the disk's reach starts to curve, rain outside it falls
/// straight (the ambient matrix read the scene keeps at its edges).
pub(crate) const BLACK_HOLE_INFALL_INFLUENCE_FRACTION: f32 = 2.80;

/// Capture radius in ball outer radii where the accretion brake takes
/// hold (velocity bleeding off inside, full strength by the inner
/// blend). 1.95 sits inside the disk's reach but outside the halo
/// crowns' wobble band (1.48 + 0.10): glyphs crossing the strong
/// field get braked and spiral in, glyphs skimming wider keep their
/// speed and swing past — the deflection fly-by read of light
/// passing a deep well.
pub(crate) const BLACK_HOLE_INFALL_CAPTURE_FRACTION: f32 = 1.95;

/// Accretion brake rate: the exponential tangential-velocity decay per
/// sim-second at full capture strength (v_t *= exp(-DRAG_RATE x dt x
/// capture_blend) — the brake eats angular momentum, never the radial
/// plunge). 0.62 decays a captured mote's tangential speed by half every
/// ~1.1 sim-s — the inspiral tightens lap by lap (the read of material
/// shedding angular momentum into the disk), while gravity keeps feeding
/// the radial plunge, so capture always resolves into the horizon within
/// a few seconds.
pub(crate) const BLACK_HOLE_INFALL_DRAG_RATE: f32 = 0.62;

/// Fall speed of a fresh infall mote, in ball outer radii per
/// sim-second. 1.32 reads as a calm drift next to the disk's own pace
/// (the disk's inner edge orbits at ~1.4), so the rain's arrival at the
/// field feels like weather, not a volley — and the far ambient rain's
/// Ghost-dim straight lines stay calm against the bright stack.
pub(crate) const BLACK_HOLE_INFALL_FALL_SPEED: f32 = 1.32;

/// Sim-time coupling to the speed keys: sim-seconds per wall-second per
/// chars_per_sec (dt_sim = dt_wall x cps x SIM_TIME_PER_CPS). One
/// sim-second equals one wall-second at the scene's reference 12 cps;
/// the up/down speed keys scale positions, velocities AND gravity
/// together (the whole field runs on one clock), so trajectory shapes
/// are invariant under the speed setting — the family's speed contract,
/// expressed as one scalar.
pub(crate) const BLACK_HOLE_INFALL_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

/// Maximum horizontal drift of a fresh infall mote as a fraction of
/// its fall speed (uniform ±DRIFT). 0.45 spreads the impact
/// parameters: some glyphs fall dead-center (the plunge read), most
/// pass offset (the bend and whip reads) — the spread that keeps
/// every capture unique.
pub(crate) const BLACK_HOLE_INFALL_DRIFT_FRACTION: f32 = 0.45;

/// Speed ladder rung 1: below this mote speed (outer radii per
/// sim-second) the base brightness reads Ghost — slow distant rain, the
/// ambient sprinkle far from the field.
pub(crate) const BLACK_HOLE_INFALL_SPEED_GHOST: f32 = 1.10;

/// Speed ladder rung 2: below this the base reads Mid, above Hot —
/// the falling rain's typical band (the fresh fall speed of 1.32 reads
/// Mid; the fall through the inner field accelerates a mote past this
/// rung on approach).
pub(crate) const BLACK_HOLE_INFALL_SPEED_MID: f32 = 2.05;

/// Speed ladder rung 3: above this mote speed the base reads Core —
/// the whip (the kinetic-heat flash of the periapsis pass, composed
/// with the proximity grade's two-rung bump it lands deep white).
pub(crate) const BLACK_HOLE_INFALL_SPEED_CORE: f32 = 3.30;

/// Comet trail length of the infall motes (the streak behind the
/// falling glyph, one brightness rung dimmer per cell — the whip
/// arcs read as streaks, the calm fall as a soft tail).
pub(crate) const BLACK_HOLE_INFALL_TRAIL_LEN: usize = 4;

/// Motion-gated shimmer chance for the infall heads (mutation tied
/// to motion, the family life sign — a glyph re-rolls its character
/// when its head lands on a new cell, at the same rate the ring
/// heads do).
pub(crate) const BLACK_HOLE_INFALL_SHIMMER_CHANCE: f32 = 0.4;

// Compile-time contracts on the infall geometry: the capture radius
// must sit inside the influence edge (the brake lives inside the
// field), the influence edge must clear the disk's tier-0 reach
// (1.375 x 1.05 / 0.55 = 2.62 outer radii — the bending zone covers
// the system), and the speed ladder must be strictly ordered (the
// kinetic-heat grade climbs monotonically).
const _: () = assert!(BLACK_HOLE_INFALL_CAPTURE_FRACTION < BLACK_HOLE_INFALL_INFLUENCE_FRACTION);
const _: () = assert!(BLACK_HOLE_INFALL_INFLUENCE_FRACTION > 2.62);
const _: () = assert!(BLACK_HOLE_INFALL_SPEED_GHOST < BLACK_HOLE_INFALL_SPEED_MID);
const _: () = assert!(BLACK_HOLE_INFALL_SPEED_MID < BLACK_HOLE_INFALL_SPEED_CORE);

// ── Black hole formation intro (stage 2.2, NIGHT-special-1) ────────────
// The hole's birth sequence — stellar collapse as the intro: a tiny
// singularity seed fades in at the center, the collapse flares it
// to peak brightness, the event horizon blooms outward from the
// center, then the ring accretes in (motes drift in on the entry
// spiral). Durations in wall-clock seconds; the sequence rides the
// engine's shared dt clock so pause/resume and the speed keys feel
// native. Style entry replays it; a pure resize does not (the hole
// re-forms only when the scene is re-entered).

/// Seed phase duration: one glyph fading in at the viewport center
/// (Ghost -> Dim -> Mid up the brightness ladder). 1.4 s reads as
/// "appearing slowly" — the deliberate opposite of the pop-in the
/// owner flagged.
pub(crate) const BLACK_HOLE_FORM_SEED_SECS: f32 = 1.4;

/// Collapse phase duration: the seed brightens to Core and a
/// four-cell cross flares around it — the last light of the
/// collapsing star, the pre-explosion beat.
pub(crate) const BLACK_HOLE_FORM_COLLAPSE_SECS: f32 = 0.5;

/// Horizon bloom duration: the annulus grows from the inside out
/// (photon-ring cells first, outer rim last) on a cubic ease-out —
/// the "small dot explodes into the hole" read, ~1.2 s so the
/// expansion is unmistakable without strobing.
pub(crate) const BLACK_HOLE_FORM_HORIZON_SECS: f32 = 1.2;
