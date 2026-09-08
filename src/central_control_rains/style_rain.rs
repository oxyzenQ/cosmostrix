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

// ── Black hole glyph infall (stage 3, NIGHT-special-1; stage 4 calm) ───
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
// so the infall reads identically on any screen size. Stage 4 (the
// calm sky, owner verdict on stage 3: 9/10, "too much rain") keeps
// the motion DNA untouched and tunes only the WEATHER: a sparse
// lane population, a trickle spawn cadence, and a dim calm entry —
// the clean, uncrowded read.

/// Base active-mote ratio of the infall pool (pool = one infall mote
/// per column, the family lane model). 0.05 keeps the rain a SPARSE
/// ambient layer — occasional streaks over a dark sky, never a
/// downpour: the hole stays the hero of the composition, the rain
/// the weather around it (the density slider still scales it through
/// the multiplier below). Stage 4 (owner verdict 9/10: "too much
/// rain, spammy from the top") cut this from 0.18 — at the engine's
/// default density the steady population dropped from roughly half
/// the screen's columns to about one in seven, the calm the owner
/// asked for.
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_BASE: f32 = 0.05;

/// Density multiplier for the infall active-count target: the same
/// sensitivity family as the ring's (softened in stage 4 from 0.35 to
/// 0.09 so the slider stays proportional at the new sparse base —
/// sliding density up no longer floods the sky, it thickens the
/// drizzle gently), so the ambient read holds across the range.
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_DENSITY_MULT: f32 = 0.09;

/// Maximum active-mote ratio cap of the infall pool. 0.16 keeps even
/// max-density rain a clear minority of the lanes — the accretion
/// material must read subordinate to the approved stack (stage 4:
/// cut from 0.55, which at full slider let the rain rival the disk
/// itself — the crowd the owner rejected).
pub(crate) const BLACK_HOLE_INFALL_ACTIVE_MAX: f32 = 0.16;

/// Spawn rate multiplier for the infall pool (parity with the ring's
/// accumulator arithmetic: fraction-of-target + floor reaches the
/// steady target with a gentle ramp-up — rain drifts in, it never
/// bursts in). Kept from stage 3: the stage-4 cut landed on the
/// TARGET (the crowd was the population, not the pace), so the same
/// multiplier now fills a lane budget roughly four times smaller —
/// appearances slow with the crowd and the equilibrium settles
/// well below the cap, a gentle drizzle that never floods the sky.
pub(crate) const BLACK_HOLE_INFALL_SPAWN_RATE_MULT: f32 = 0.30;

/// Spawn rate floor (minimum infall spawns per second). Stage 4 cut
/// it from 0.8 to 0.25 — a fresh glyph at most every four seconds on
/// the quietest pools, so even a long watch never reads a rhythm of
/// pops from the top edge (the elegance the owner asked for).
pub(crate) const BLACK_HOLE_INFALL_SPAWN_RATE_FLOOR: f32 = 0.25;

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
/// ambient sprinkle far from the field. 1.45 sits ABOVE the fresh fall
/// speed (1.32): a glyph entering at the top edge reads dim and quiet,
/// and only the accelerating fall through the field lifts it up the
/// ladder — the calm entry is the stage-4 elegance read (previously
/// 1.10, which lit every fresh drop Mid-bright the moment it appeared
/// — part of the spammy read the owner rejected).
pub(crate) const BLACK_HOLE_INFALL_SPEED_GHOST: f32 = 1.45;

/// Speed ladder rung 2: below this the base reads Mid, above Hot —
/// the accelerating fall's band (a mote that has fallen deep enough
/// into the field to pick up speed reads Mid; the approach past this
/// rung reads Hot). The fresh fall speed of 1.32 sits BELOW it since
/// stage 4 — the calm entry reads Ghost and only the fall's own
/// acceleration brightens the glyph.
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

// ── Aeolian weave (NIGHT-special-2, the ninth style) ────────────────────
// The invented system, born in this repo (owner directive: a rain
// type with no existing mathematical reference — original motion
// DNA, the LEAP-engine spirit: an equation set no textbook carries,
// derived from first principles FOR the terminal medium). The
// complete derivation and the six laws of the weave live in
// type_rain/aeolian/mod.rs; the constants here are the shipped
// calibration.

// Population dials (the stage-4 calm-sky DNA: the rain is an
// ambient minority layer — a drizzle over a dark sky, never a
// downpour — so the strings stay the hero of the composition).

/// Base active-drop ratio of the aeolian pool (pool = one drop per
/// column, the family lane model). 0.05 matches the infall's
/// stage-4 sparse base: occasional falling glyphs over the dark
/// sky, the strings revealed only where the rain plays them.
pub(crate) const AEOLIAN_ACTIVE_BASE: f32 = 0.05;

/// Density multiplier for the aeolian active-count target (the
/// slider thickens the drizzle gently, never floods the sky — the
/// stage-4 sensitivity family, 0.09).
pub(crate) const AEOLIAN_ACTIVE_DENSITY_MULT: f32 = 0.09;

/// Maximum active-drop ratio cap of the aeolian pool (even at full
/// slider the rain stays a clear minority of the lanes — the calm
/// sky the owner approved on the black hole's stage 4).
pub(crate) const AEOLIAN_ACTIVE_MAX: f32 = 0.16;

/// Spawn rate multiplier for the aeolian pool (accumulator
/// arithmetic identical to the family: fraction-of-target + floor,
/// deficit-bounded — rain drifts in one glyph at a time, never
/// bursts).
pub(crate) const AEOLIAN_SPAWN_RATE_MULT: f32 = 0.30;

/// Spawn rate floor (minimum aeolian spawns per second). 0.25 —
/// the stage-4 trickle cadence: a fresh glyph at most every four
/// seconds on the quietest pools, so the top edge never reads a
/// rhythm of pops.
pub(crate) const AEOLIAN_SPAWN_RATE_FLOOR: f32 = 0.25;

/// Drop lifetime cap in seconds (+-15% per-mote variance, the
/// family contract). 15 s is the backstop: most drops end earlier
/// (captured by a string, or absorbed at the bottom edge).
pub(crate) const AEOLIAN_MAX_AGE_SECS: f32 = 15.0;

// Drop physics (the rain half of the weave).

/// Gravitational acceleration of a falling drop, in lines per
/// sim-second squared. 1.6 keeps the fall a drift: from rest a
/// drop crosses a 40-line screen in ~7 s at terminal speed —
/// weather, not a volley.
pub(crate) const AEOLIAN_DROP_GRAVITY: f32 = 1.6;

/// Terminal fall speed cap, in lines per sim-second. 4.0 is the
/// fastest a drop may fall: bright long streaks read as rain
/// sheets, not spam.
pub(crate) const AEOLIAN_DROP_TERMINAL: f32 = 4.0;

/// Fresh drop fall speed, in lines per sim-second. 1.2 is the calm
/// entry: a glyph appearing at the top edge reads as a slow dim
/// drift (Ghost on the kinetic ladder — the stage-4 dim-entry
/// elegance).
pub(crate) const AEOLIAN_DROP_FALL_BASE: f32 = 1.2;

/// Maximum horizontal drift of a fresh drop as a fraction of its
/// fall speed (uniform +-DRIFT): 0.35 spreads the landing sites so
/// consecutive plucks never ring the same column in rhythm.
pub(crate) const AEOLIAN_DROP_DRIFT_FRACTION: f32 = 0.35;

/// Resonance-seeking gain: lateral acceleration per unit field
/// slope, in cells per sim-second squared. 5.5 slides a drop a few
/// cells toward a passing packet's crest over its descent — the
/// bend that reads as the glyphs hearing the music.
pub(crate) const AEOLIAN_SEEK_GAIN: f32 = 5.5;

/// Lateral drag rate: exponential decay of a drop's lateral
/// velocity per sim-second (vx *= exp(-rate x dt)). 2.1 settles the
/// seeking without orbiting — a drop homes in and lands, it does
/// not circle.
pub(crate) const AEOLIAN_SEEK_DRAG: f32 = 2.1;

/// Vertical range around a string within which a drop feels the
/// field's slope, in lines. 4.5: the bend begins a comfortable
/// distance above the string and completes by impact.
pub(crate) const AEOLIAN_SEEK_RANGE: f32 = 4.5;

/// Base capture probability at a string when the local field is
/// dark (u = 0): 0.35 of falling glyphs slip through a silent
/// string — the sky below stays alive with through-rain.
pub(crate) const AEOLIAN_CAPTURE_BASE: f32 = 0.35;

/// Capture probability gain per unit field brightness (probability
/// = base + gain x min(1, u / level-hot)): bright antinodes eat
/// rain, silent strings let it pass — the feedback that
/// concentrates the weather onto the ringing zones (the
/// self-organization loop of the weave).
pub(crate) const AEOLIAN_CAPTURE_GAIN: f32 = 0.55;

/// Graze pluck fraction: a drop that PASSES a string still rings
/// it faintly (pluck gain x 0.25) — the through-rain keeps the
/// instrument murmuring between captures.
pub(crate) const AEOLIAN_PASS_GRAZE: f32 = 0.25;

/// Surf kick: vertical speed gained by crossing a bright packet
/// (lines per sim-second per unit amplitude). 1.4 makes a drop
/// flare and accelerate as it punches through a wavefront — the
/// interference streak.
pub(crate) const AEOLIAN_SURF_KICK: f32 = 1.4;

/// Charge accumulation rate: a drop's pluck charge grows with its
/// fall speed (charge += |vy| x rate x dt). 0.22: a full 40-line
/// descent at terminal speed charges ~2.3 — deep fast drops ring
/// the strings hard, shallow grazes softly.
pub(crate) const AEOLIAN_CHARGE_RATE: f32 = 0.22;

/// Fresh drop charge seed: every impact has a floor (0.4) so even
/// a top-string graze speaks audibly.
pub(crate) const AEOLIAN_CHARGE_SEED: f32 = 0.4;

/// Kinetic ladder rung 1: below this fall speed (lines per
/// sim-second) the drop reads Ghost. 1.45 sits ABOVE the fresh
/// fall speed (1.2) — the dim calm entry (the stage-4 elegance
/// read, identical intent to the infall's ladder).
pub(crate) const AEOLIAN_SPEED_GHOST: f32 = 1.45;

/// Kinetic ladder rung 2: below this the drop reads Mid, above it
/// Hot — the accelerating fall's band (a drop that has surfed a
/// wavefront or fallen deep reads bright).
pub(crate) const AEOLIAN_SPEED_MID: f32 = 2.4;

/// Kinetic ladder rung 3: above this fall speed the drop reads
/// Core — the white streak of a drop punching a bright packet at
/// full surf kick.
pub(crate) const AEOLIAN_SPEED_CORE: f32 = 3.6;

/// Comet trail length of the falling drops (cells behind the head,
/// one brightness rung dimmer per cell).
pub(crate) const AEOLIAN_TRAIL_LEN: usize = 3;

/// Motion-gated shimmer chance for the drop heads (mutation tied
/// to motion, the family life sign — a glyph re-rolls when its
/// head lands on a new cell).
pub(crate) const AEOLIAN_SHIMMER_CHANCE: f32 = 0.4;

// String physics (the instrument half of the weave).

/// The signal hop rate: a bright channel cell's clock speed, in
/// cells per sim-second (a struck packet's sprint — 50 cells/s
/// crosses a 120-col screen in ~2.4 s, a 200-col screen in ~4 s,
/// under one string-decay half-life). Kept under the 60-fps
/// saturation point (50 x 1/60 = 0.83 hops/tick < 1) so the hop
/// clock never accumulates phase debt at 60 fps — the speed
/// contract stays exactly rate-based there (debt only engages
/// below ~24 fps, a graceful slowdown).
pub(crate) const AEOLIAN_HOP_FAST: f32 = 50.0;

/// The residue crawl rate: a dim channel cell's clock speed (the
/// seep of the wake a racing signal sheds, and the slow fade of
/// the decayed field). 3 cells/s moves the residue ~4-6 cells over
/// its decay lifetime — the wake reads as a lingering shimmer
/// around the impact zone, not a second traveling packet.
pub(crate) const AEOLIAN_HOP_SLOW: f32 = 3.0;

/// The signal threshold: a channel cell whose mass sits at or
/// above this amplitude runs its hop clock at the sprint rate (the
/// racing signal); below it, at the residue crawl. The QUANTIZED
/// two-voice switch is deliberate — every bright cell of a pulse
/// runs at the same rate, so the pulse hops in lockstep and
/// translates rigidly (a smooth rate curve would shear the pulse
/// apart within a few ticks). 0.9 sits under typical pluck
/// amplitudes (a charged capture injects 1.0-3.0 per channel), so
/// a fresh strike sprints immediately and only its decayed wake
/// lingers. Decay carries a cell monotonically from the signal
/// voice to the residue voice (mass only shrinks between plucks),
/// so the switch never oscillates.
pub(crate) const AEOLIAN_URGENCY_SWITCH: f32 = 0.9;

/// Self-similar decay rate of the string field (amplitude x
/// exp(-rate x dt), the shape-preserving shrink). 0.35 gives a
/// packet a 2 s half-life: long enough to cross half a wide
/// screen, short enough that silence returns between plucks.
pub(crate) const AEOLIAN_STRING_DECAY: f32 = 0.35;

/// Wall reflection fraction: a packet reaching the screen edge
/// re-enters the opposite channel at 0.85 of its amplitude — the
/// instrument is closed, the walls are the bridge's nut. 15% of
/// the energy is absorbed per bounce (the walls also mute).
pub(crate) const AEOLIAN_WALL_REFLECT: f32 = 0.85;

/// Pluck gain: channel amplitude injected per unit drop charge
/// (symmetric split into BOTH channels — the classic pluck read:
/// light races away from the impact in both directions).
pub(crate) const AEOLIAN_PLUCK_GAIN: f32 = 0.85;

/// Inter-string resonance echo: probability that a captured drop's
/// impact also seeds a faint packet on the string BELOW (same
/// column, symmetric split) — the frame resonates, the cascade
/// read. 0.5: half the captures ring the neighbor.
pub(crate) const AEOLIAN_ECHO_CHANCE: f32 = 0.5;

/// Echo seed amplitude as a fraction of the parent pluck: 0.45 —
/// the aftershock reads as a dim murmur on the next string, never
/// a second voice.
pub(crate) const AEOLIAN_ECHO_GAIN: f32 = 0.45;

// Draw thresholds (field amplitude to brightness ladder).

/// Draw floor: a string cell below this combined amplitude is not
/// drawn at all — the instrument is INVISIBLE until played (the
/// dark sky stays dark where the strings are silent).
pub(crate) const AEOLIAN_DRAW_FLOOR: f32 = 0.14;

/// Mid rung: combined amplitude above this reads Mid on the
/// brightness ladder (the packet body).
pub(crate) const AEOLIAN_LEVEL_MID: f32 = 0.6;

/// Hot rung: combined amplitude above this reads Hot (the packet
/// crest).
pub(crate) const AEOLIAN_LEVEL_HOT: f32 = 1.4;

/// Knot threshold: when BOTH channels at a cell exceed this
/// amplitude, counter-propagating packets overlap there and the
/// cell reads Core white — the interference knot, the signature
/// read of the weave.
pub(crate) const AEOLIAN_KNOT_LEVEL: f32 = 0.5;

/// Sim-time coupling to the speed keys: sim-seconds per
/// wall-second per chars_per_sec (dt_sim = dt_wall x cps x
/// SIM_TIME_PER_CPS). One sim-second equals one wall-second at the
/// scene's reference 12 cps; the speed keys scale the rain, the
/// conduction and the decay on one clock — the family's speed
/// contract, trajectory shapes invariant.
pub(crate) const AEOLIAN_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

/// Maximum strings drawn, by viewport height (52 lines and up
/// draws 4, 34..=51 draws 3, 18..=33 draws 2, anything less 1):
/// one string per ~13-17 lines keeps the resonance bands separated
/// enough for the eye to read each instrument individually.
pub(crate) const AEOLIAN_STRING_LINES_PER: u16 = 17;

// Compile-time contracts on the aeolian calibration: the ladder
// must be strictly ordered (kinetic heat climbs monotonically),
// the draw thresholds must be ordered with the floor lowest (a
// cell can only climb the ladder), capture probability must stay
// a probability, the hop rates must be ordered (residue crawls
// slower than the signal sprints), and the sprint must stay under
// the 60-fps saturation point (one hop per tick at most — the
// no-tunneling guarantee; the signal threshold must sit under a
// typical pluck so strikes sprint from birth).
const _: () = assert!(AEOLIAN_SPEED_GHOST < AEOLIAN_SPEED_MID);
const _: () = assert!(AEOLIAN_SPEED_MID < AEOLIAN_SPEED_CORE);
const _: () = assert!(AEOLIAN_DRAW_FLOOR < AEOLIAN_LEVEL_MID);
const _: () = assert!(AEOLIAN_LEVEL_MID < AEOLIAN_LEVEL_HOT);
const _: () = assert!(AEOLIAN_KNOT_LEVEL > AEOLIAN_DRAW_FLOOR);
const _: () = assert!(AEOLIAN_CAPTURE_BASE + AEOLIAN_CAPTURE_GAIN <= 1.0);
const _: () = assert!(AEOLIAN_WALL_REFLECT <= 1.0);
const _: () = assert!(AEOLIAN_DROP_FALL_BASE < AEOLIAN_DROP_TERMINAL);
const _: () = assert!(AEOLIAN_URGENCY_SWITCH > 0.0);
const _: () = assert!(AEOLIAN_HOP_SLOW < AEOLIAN_HOP_FAST);
const _: () = assert!(AEOLIAN_HOP_FAST / 60.0 < 1.0);
// A charged capture (charge ~2 from a mid-fall drop) plucks a
// center cell of 2 x PLUCK_GAIN — that must sprint from birth.
const _: () = assert!(AEOLIAN_URGENCY_SWITCH < 2.0 * AEOLIAN_PLUCK_GAIN);

// ── Solar flare corona (NIGHT-special-4, the tenth style) ──────────────
// The third invented system, born in this repo (the NIGHT-special-2
// directive carried forward: motion DNA with no existing
// mathematical reference — original derivation, the LEAP-engine
// spirit). The complete derivation and the five laws of the corona
// live in type_rain/solar_flare/mod.rs; the constants here are the
// shipped calibration.

// The magnetic carpet (law 1).

/// Loop arcade spacing: one coronal loop per this many columns of
/// viewport width. 10 keeps the arcade airy — an arc, a gap, an
/// arc — while the loops still read as one connected corona.
pub(crate) const SOLAR_LOOP_SPACING_COLS: u16 = 10;

/// Hard cap on the loop count (beyond 8 arcs the repulsion pass and
/// the draw budget stop scaling; a 200-column terminal already
/// saturates the corona).
pub(crate) const SOLAR_LOOP_MAX: usize = 8;

/// Minimum footpoint span in columns (an arc narrower than 8 cells
/// stops reading as a loop and starts reading as a dash).
pub(crate) const SOLAR_W_MIN: f32 = 8.0;

/// Maximum footpoint span as a fraction of viewport width (0.30 —
/// a loop wider than a third of the screen dwarfs its neighbors).
pub(crate) const SOLAR_W_MAX_FRAC: f32 = 0.30;

/// Width breath glide rate: the span eases toward its anchor at
/// this rate per sim-second (an anchor resolve reads as the loop
/// slowly fattening or slimming, never snapping).
pub(crate) const SOLAR_W_RELAX: f32 = 0.8;

/// Mean breath dwell: average sim-seconds a loop holds its span and
/// height anchors before re-rolling (rolled with 0.5x-1.5x variance
/// — the breaths never march in rhythm).
pub(crate) const SOLAR_W_DWELL_MEAN: f32 = 9.0;

/// Minimum apex height in lines (a flatter arc stops reading as a
/// coronal loop).
pub(crate) const SOLAR_H_MIN: f32 = 3.0;

/// Quiet apex height band (fractions of the usable height above the
/// surface): Stable loops breathe between these.
pub(crate) const SOLAR_ARC_MIN_FRAC: f32 = 0.16;
pub(crate) const SOLAR_ARC_MAX_FRAC: f32 = 0.52;

/// Hard height cap (fraction of the usable height): the stretched
/// erupting arc never leaves the sky.
pub(crate) const SOLAR_H_CAP_FRAC: f32 = 0.78;

/// Height glide rate per sim-second (emergence grows over ~1 s; the
/// eruption stretch resolves over ~1 s — visible, never a snap).
pub(crate) const SOLAR_H_RELAX: f32 = 1.4;

/// Wind span: the global arcade drift target re-rolls within
/// plus-or-minus this many columns per sim-second. 0.5 is a slow
/// advect — the whole corona crosses the screen in tens of seconds,
/// the majestic differential-rotation read.
pub(crate) const SOLAR_WIND_SPAN: f32 = 0.5;

/// Wind relaxation rate per sim-second (the drift direction never
/// snaps — it eases).
pub(crate) const SOLAR_WIND_RELAX: f32 = 0.5;

/// Mean wind hold: average sim-seconds between drift target
/// re-rolls (rolled with 0.6x-1.4x variance — no rhythmic gusting).
pub(crate) const SOLAR_WIND_HOLD_MEAN: f32 = 8.0;

/// Wind coupling: each loop's drift relaxes toward the global wind
/// at this rate per sim-second (repulsion rides on top as the local
/// correction; erupting/detaching loops decouple — they are leaving
/// the carpet and hold their momentum).
pub(crate) const SOLAR_WIND_COUPLE: f32 = 0.8;

/// Footpoint repulsion gain: facing feet of adjacent loops push
/// apart with GAIN / floored-gap (columns per sim-second squared).
/// 7.0 with the 10-column spacing holds the equilibrium coverage
/// without packing the walls.
pub(crate) const SOLAR_REPULSION: f32 = 7.0;

/// Repulsion gap floor: the smallest gap the inverse-gap force sees
/// (prevents the singularity when two footpoints collide).
pub(crate) const SOLAR_GAP_FLOOR: f32 = 3.0;

/// Loop drift clamp (columns per sim-second) — the carpet's hard
/// speed bound (law 1).
pub(crate) const SOLAR_DRIFT_MAX: f32 = 2.0;

/// Wall bounce damping: a loop's feet hitting a screen edge keep
/// this fraction of the drift, reversed. 0.55 reads as a soft
/// deflect, not a mirror.
pub(crate) const SOLAR_WALL_DAMP: f32 = 0.55;

/// Emergence duration: a fresh arc grows out of the photosphere
/// over this many sim-seconds (flux emergence).
pub(crate) const SOLAR_EMERGE_SECS: f32 = 1.2;

/// Eruption duration: the flare stretch lasts this many sim-seconds
/// before the lift-off.
pub(crate) const SOLAR_ERUPT_SECS: f32 = 1.4;

/// Detach duration: the lifted arc rises and dissolves over this
/// many sim-seconds.
pub(crate) const SOLAR_DETACH_SECS: f32 = 1.8;

/// Detach lift rate (lines per sim-second): the translating arc's
/// rise speed during the lift-off.
pub(crate) const SOLAR_DETACH_LIFT_RATE: f32 = 6.0;

/// Eruption stretch factor: the erupting apex target grows to this
/// multiple of the loop's natural height (the star throws its loop
/// tall before it tears free).
pub(crate) const SOLAR_STRETCH: f32 = 1.9;

// The footpoint deposition (law 3).

/// Flux decay rate per sim-second: the corona cools at exp(-rate x
/// dt). 0.38 keeps a fed loop hot for a few seconds after its last
/// landing.
pub(crate) const SOLAR_FLUX_DECAY: f32 = 0.38;

/// Flux hard clamp (law 3b — bounded by construction). 2.6 sits
/// above the Core flare threshold so a heavy landing sequence can
/// hold the white read briefly.
pub(crate) const SOLAR_FLUX_MAX: f32 = 2.6;

/// Deposition gain: a landing's kinetic charge enters the flux
/// multiplied by this (the heating efficiency of the footpoint).
pub(crate) const SOLAR_FLUX_GAIN: f32 = 1.0;

/// Mid rung: flux above this reads Mid on the loop ladder (a
/// rained-on loop — the coronal read's working dim).
pub(crate) const SOLAR_FLUX_LEVEL_MID: f32 = 0.25;

/// Hot rung: flux above this reads Hot (a heavily-fed arcade
/// member) and arms the apex condensation glow.
pub(crate) const SOLAR_FLUX_LEVEL_HOT: f32 = 0.7;

/// Core rung: flux above this AND a fresh flash reads Core — the
/// flaring punch.
pub(crate) const SOLAR_FLUX_LEVEL_CORE: f32 = 1.5;

/// Flash window: sim-seconds after a landing during which the
/// footpoint cells read one rung hotter (the landing punch) and a
/// strong flux reads Core.
pub(crate) const SOLAR_FLASH_SECS: f32 = 0.6;

// The flare eruption (law 4).

/// Eruption threshold: a Stable loop's flux must cross this before
/// the flare gate will consider it (the corona stores its rain
/// before it detonates).
pub(crate) const SOLAR_FLUX_ERUPT_THRESHOLD: f32 = 1.1;

/// Mean flare cadence: sim-seconds between eruption opportunities
/// (the global gate — a flare is a singular event, never a chorus;
/// at most one erupting loop at a time). Rolled with 0.5x-1.5x
/// variance.
pub(crate) const SOLAR_FLARE_CLOCK_MEAN: f32 = 9.0;

/// The apex burst count: fresh ejecta spawned at the erupting loop's
/// top (the spray).
pub(crate) const SOLAR_EJECTA_BURST: usize = 5;

// The coronal condensation (law 2).

/// Thermal kick v0 (lines per sim-second): every descent starts
/// from this speed at the apex (the lazy departure — energy
/// conservation's baseline). Strictly positive by contract: the
/// riding s-motion is monotone because of it.
pub(crate) const SOLAR_RAIN_V0: f32 = 0.7;

/// Leg gravity (lines per sim-second squared): the energy budget the
/// closed-form speed draws from — v = sqrt(v0^2 + 2 G h (2|s-0.5|)^2).
pub(crate) const SOLAR_LEG_G: f32 = 7.5;

/// Condensation window: fresh riders spawn at s within
/// plus-or-minus this of the apex (condensation happens near the
/// loop top).
pub(crate) const SOLAR_RAIN_SPAN: f32 = 0.16;

/// Hot-loop tournament league: the spawn samples this many loops and
/// rains on the flux-richest (law 2's concentration arm — the
/// arcade members that have been rained on collect the next
/// condensations).
pub(crate) const SOLAR_TOURNAMENT: usize = 4;

/// Deposition rate: a landing's charge is its arrival speed times
/// this, plus the seed floor.
pub(crate) const SOLAR_DEPOSIT_RATE: f32 = 0.16;

/// Deposition seed: every landing carries this floor (a fresh drop
/// still murmurs the footpoint).
pub(crate) const SOLAR_DEPOSIT_SEED: f32 = 0.25;

// The ejecta ballistics (law 4).

/// Rider fling gain: a converted rider's along-arc speed becomes its
/// ejection speed times this.
pub(crate) const SOLAR_EJECTA_FLING: f32 = 1.2;

/// Upward kick added to every flung rider (the flare throws plasma
/// off the star, not along it).
pub(crate) const SOLAR_EJECTA_RISE: f32 = 1.5;

/// Apex-burst speed band (lines per sim-second): fresh spray leaves
/// the loop top within this range.
pub(crate) const SOLAR_EJECTA_SPEED_MIN: f32 = 2.0;
pub(crate) const SOLAR_EJECTA_SPEED_MAX: f32 = 4.5;

/// Ejecta lateral spread (columns per sim-second, plus-or-minus).
pub(crate) const SOLAR_EJECTA_SPREAD: f32 = 1.6;

/// Stellar gravity on the ejecta (lines per sim-second squared):
/// decelerates the rise, pulls the spent sparks back toward the
/// photosphere (the splash arm).
pub(crate) const SOLAR_EJECTA_G: f32 = 2.4;

/// Ejecta lifetime (sim-seconds, +-25% variance): the fade clock.
pub(crate) const SOLAR_EJECTA_LIFE: f32 = 3.0;

/// Splash heat: an ejecta landing back on the photosphere deposits
/// this into the granule it hits.
pub(crate) const SOLAR_SPLASH_HEAT: f32 = 0.45;

// The granulation surface (law 5).

/// Surface band height in lines (the loop feet sit on the top line).
pub(crate) const SOLAR_SURFACE_LINES: u16 = 2;

/// Granulation random-walk step per sim-second (the convection
/// grit's flicker speed).
pub(crate) const SOLAR_GRANULE_STEP: f32 = 0.9;

/// Granule heat bounds (the walk clamps inside this band).
pub(crate) const SOLAR_GRANULE_MIN: f32 = 0.10;
pub(crate) const SOLAR_GRANULE_MAX: f32 = 0.95;

// Drop pool dials (the calm-sky family contract).

/// Base active-drop ratio of the solar pool (pool = one drop per
/// column, the family lane model). 0.05 matches the stage-4
/// calm-sky DNA: a sparse drizzle of coronal rain over the arcade.
pub(crate) const SOLAR_ACTIVE_BASE: f32 = 0.05;

/// Density multiplier for the active-count target.
pub(crate) const SOLAR_ACTIVE_DENSITY_MULT: f32 = 0.06;

/// Maximum active-drop ratio cap of the solar pool (even at full
/// density the coronal rain stays a minority layer — the arcade is
/// the hero of the composition).
pub(crate) const SOLAR_ACTIVE_MAX: f32 = 0.14;

/// Spawn rate multiplier for the solar pool (accumulator contract).
pub(crate) const SOLAR_SPAWN_RATE_MULT: f32 = 0.28;

/// Spawn rate floor (minimum solar spawns per second).
pub(crate) const SOLAR_SPAWN_RATE_FLOOR: f32 = 0.22;

/// Lifetime backstop in sim-seconds.
pub(crate) const SOLAR_MAX_AGE_SECS: f32 = 18.0;

// The kinetic-heat ladder (drop head brightness).

/// Ghost rung ceiling: riding speeds at or below this read Ghost.
pub(crate) const SOLAR_SPEED_GHOST: f32 = 1.4;

/// Mid rung ceiling: riding speeds above GHOST up to this read Mid.
pub(crate) const SOLAR_SPEED_MID: f32 = 2.0;

/// Hot rung ceiling: riding speeds above MID up to this read Hot;
/// above it the full-speed footpoint arrival reads Core.
pub(crate) const SOLAR_SPEED_CORE: f32 = 3.0;

/// Comet trail length behind a drop head.
pub(crate) const SOLAR_TRAIL_LEN: usize = 2;

// The shimmer law (law 5).

/// Arc shimmer chance per frame: the quiet corona re-rolls rarely (a
/// slow coronal flicker).
pub(crate) const SOLAR_SHIMMER_ARC: f32 = 0.06;

/// Hot shimmer chance per frame: a flaring loop or a bright
/// footpoint flickers hard (also the drop-head re-roll rate — the
/// family contract).
pub(crate) const SOLAR_SHIMMER_HOT: f32 = 0.30;

/// Surface shimmer chance per frame: the granulation grit twinkles
/// at its own pace, between the quiet arcs and the flares.
pub(crate) const SOLAR_SHIMMER_SURFACE: f32 = 0.10;

/// Sim-time coupling to the speed keys (the family contract — see
/// AEOLIAN_SIM_TIME_PER_CPS; the reference scene speed is 14 cps).
pub(crate) const SOLAR_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

// Compile-time contracts on the corona calibration: the height band
// is strictly ordered with the cap above it, the stretch factor
// grows, the flux ladder is strictly ordered with the clamp above
// the Core rung, the eruption threshold sits between Hot and the
// clamp, the kinetic ladder is ordered, the population dials are
// ordered (base under the max cap), the thermal kick is strictly
// positive (the monotone-riding guarantee), the leg gravity and
// the ejecta speeds are positive, and the wall damping is a proper
// fraction.
const _: () = assert!(SOLAR_ARC_MIN_FRAC < SOLAR_ARC_MAX_FRAC);
const _: () = assert!(SOLAR_ARC_MAX_FRAC < SOLAR_H_CAP_FRAC);
const _: () = assert!(SOLAR_STRETCH > 1.0);
const _: () = assert!(SOLAR_FLUX_LEVEL_MID < SOLAR_FLUX_LEVEL_HOT);
const _: () = assert!(SOLAR_FLUX_LEVEL_HOT < SOLAR_FLUX_LEVEL_CORE);
const _: () = assert!(SOLAR_FLUX_LEVEL_CORE < SOLAR_FLUX_MAX);
const _: () = assert!(SOLAR_FLUX_LEVEL_HOT < SOLAR_FLUX_ERUPT_THRESHOLD);
const _: () = assert!(SOLAR_FLUX_ERUPT_THRESHOLD < SOLAR_FLUX_MAX);
const _: () = assert!(SOLAR_SPEED_GHOST < SOLAR_SPEED_MID);
const _: () = assert!(SOLAR_SPEED_MID < SOLAR_SPEED_CORE);
const _: () = assert!(SOLAR_ACTIVE_BASE < SOLAR_ACTIVE_MAX);
const _: () = assert!(SOLAR_RAIN_V0 > 0.0);
const _: () = assert!(SOLAR_LEG_G > 0.0);
const _: () = assert!(SOLAR_EJECTA_SPEED_MIN < SOLAR_EJECTA_SPEED_MAX);
const _: () = assert!(SOLAR_EJECTA_SPEED_MIN > 0.0);
const _: () = assert!(SOLAR_EJECTA_G > 0.0);
const _: () = assert!(SOLAR_EJECTA_LIFE > 0.0);
const _: () = assert!(SOLAR_GAP_FLOOR > 0.0);
const _: () = assert!(SOLAR_REPULSION > 0.0);
const _: () = assert!(SOLAR_WALL_DAMP <= 1.0);
const _: () = assert!(SOLAR_W_MIN > 0.0);
const _: () = assert!(SOLAR_SURFACE_LINES >= 1);
const _: () = assert!(SOLAR_TOURNAMENT >= 1);

// ── DNA helix (NIGHT-research-7, the eleventh style) ──────────────
//
// The rain writes the genome: a rotating double helix of glyph
// strands spanned by Watson-Crick base-pair rungs, fed by a
// nucleotide soup, periodically swept by a replication fork that
// dissolves, widens and re-synthesizes the ladder (the pair
// re-rolled — the visible mutation). The complete derivation and
// the five laws of the ladder live in
// type_rain/dna_helix/mod.rs; the constants here are the shipped
// calibration.

// The turn (law 1).

/// One full helical turn every this many lines. Real B-DNA packs
/// ~10.5 base pairs per turn; with a rung every 2 lines this
/// gives 11 — the biology honored at terminal legibility.
pub(crate) const DNA_TURN_LINES: u16 = 22;

/// Helix rotation rate in radians per sim-second. At the scene's
/// 14 cps the effective rate is ~0.58 rad/s — a full turn every
/// ~11 wall-seconds, the majestic corkscrew.
pub(crate) const DNA_ROT_RATE: f32 = 0.5;

/// The radius as a fraction of the viewport width.
pub(crate) const DNA_R_FRAC: f32 = 0.21;

/// The minimum radius in columns (narrow terminals keep a
/// readable helix).
pub(crate) const DNA_R_MIN: f32 = 3.5;

/// The maximum radius in columns (wide terminals: the icon stays
/// an icon — the single-body flagship aesthetic).
pub(crate) const DNA_R_MAX: f32 = 16.0;

// The pairing (law 2).

/// One base-pair rung every this many lines (the ladder's line
/// registry: rung i lives at line 1 + i x RUNG_STEP).
pub(crate) const DNA_RUNG_STEP: u16 = 2;

// The recency (law 3).

/// The synthesis charge's hard clamp (bounded by construction).
pub(crate) const DNA_CHARGE_MAX: f32 = 2.6;

/// Charge decay per sim-second (exponential — the genome cools).
pub(crate) const DNA_CHARGE_DECAY: f32 = 0.35;

/// The recency ladder rungs (Ghost the archive, Mid transcribed,
/// Hot fresh, Core the replication window).
pub(crate) const DNA_CHARGE_LEVEL_MID: f32 = 0.25;
pub(crate) const DNA_CHARGE_LEVEL_HOT: f32 = 0.7;
pub(crate) const DNA_CHARGE_LEVEL_CORE: f32 = 1.5;

// The replication fork (law 4).

/// The fork's travel rate in lines per sim-second (a 60-line
/// molecule sweeps in ~7.5 sim-seconds).
pub(crate) const DNA_FORK_RATE: f32 = 8.0;

/// The fork envelope's base sigma in lines (the bow width and the
/// dissolution window scale with it; clamped to 35% of the height
/// and floored at 2 — see `fork_sigma`).
pub(crate) const DNA_FORK_GAP: f32 = 7.0;

/// The strand bow amplitude at the fork center (the Y: the local
/// radius grows by up to +85%).
pub(crate) const DNA_BOW_MAX: f32 = 0.85;

/// Mean sim-seconds between replication sweeps (variance banded
/// 0.6-1.4x at re-arm).
pub(crate) const DNA_REPLICATION_CLOCK_MEAN: f32 = 12.0;

// The soup (law 5).

/// The capture band's half width as a radius multiple (the soup
/// falls a little past the strand extremes).
pub(crate) const DNA_CAPTURE_BAND_MULT: f32 = 1.6;

/// The capture margin beyond the rung span's ends, in cells.
pub(crate) const DNA_CAPTURE_MARGIN: f32 = 1.5;

/// The nucleotide's terminal fall speed in lines per sim-second
/// (scaled by the sim clock — the family speed contract).
pub(crate) const DNA_FALL_MULT: f32 = 8.0;

/// The fall speed's variance band (+- this fraction of the
/// speed).
pub(crate) const DNA_FALL_BAND: f32 = 0.25;

/// The brownian lateral drift's random-walk gain.
pub(crate) const DNA_DRIFT_RATE: f32 = 3.0;

/// The drift velocity's clamp in columns per sim-second.
pub(crate) const DNA_DRIFT_MAX: f32 = 0.8;

/// Charge deposited by one absorbed nucleotide.
pub(crate) const DNA_DEPOSIT_RATE: f32 = 0.35;

/// The chance an absorption re-rolls the rung's pair (the
/// substitution mutation — the rain visibly edits the genome).
pub(crate) const DNA_MUTATION_CHANCE: f32 = 0.35;

/// The nucleotide lifetime backstop in sim-seconds (+-15% at
/// spawn).
pub(crate) const DNA_MAX_AGE_SECS: f32 = 16.0;

// Population dials (the calm-sky family: the molecule is the
// hero, the soup the minority layer).

pub(crate) const DNA_ACTIVE_BASE: f32 = 0.06;
pub(crate) const DNA_ACTIVE_DENSITY_MULT: f32 = 0.05;
pub(crate) const DNA_ACTIVE_MAX: f32 = 0.16;

/// Spawn rate multiplier + floor (the trickle equilibrium).
pub(crate) const DNA_SPAWN_RATE_MULT: f32 = 0.30;
pub(crate) const DNA_SPAWN_RATE_FLOOR: f32 = 0.25;

/// Comet trail length in cells (the falling nucleotide's wake).
pub(crate) const DNA_TRAIL_LEN: usize = 2;

/// Quiet-genome shimmer chance per frame (the fabric identity's
/// re-roll rate — the archive holds its letters).
pub(crate) const DNA_SHIMMER_QUIET: f32 = 0.05;

/// Hot-genome shimmer chance per frame (fresh synthesis flickers
/// — the mutation rate reads off the recency).
pub(crate) const DNA_SHIMMER_HOT: f32 = 0.25;

/// Sim-time coupling to the speed keys (the family contract — see
/// AEOLIAN_SIM_TIME_PER_CPS; the reference scene speed is 14 cps).
pub(crate) const DNA_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

// Compile-time contracts on the ladder calibration: the radius
// band is strictly ordered, the recency ladder is strictly
// ordered with the clamp above the Core rung, the population dials
// are ordered (base under the max cap), the fork rate and the
// fall speed are strictly positive, the capture geometry is
// positive, the mutation chance is a proper fraction, and the
// twist/pair ratio honors the B-DNA turn (TURN_LINES over
// RUNG_STEP stays near 11 base pairs per turn).
const _: () = assert!(DNA_R_MIN < DNA_R_MAX);
const _: () = assert!(DNA_R_FRAC > 0.0);
const _: () = assert!(DNA_CHARGE_LEVEL_MID < DNA_CHARGE_LEVEL_HOT);
const _: () = assert!(DNA_CHARGE_LEVEL_HOT < DNA_CHARGE_LEVEL_CORE);
const _: () = assert!(DNA_CHARGE_LEVEL_CORE < DNA_CHARGE_MAX);
const _: () = assert!(DNA_ACTIVE_BASE < DNA_ACTIVE_MAX);
const _: () = assert!(DNA_FORK_RATE > 0.0);
const _: () = assert!(DNA_FALL_MULT > 0.0);
const _: () = assert!(DNA_CAPTURE_BAND_MULT > 0.0);
const _: () = assert!(DNA_CAPTURE_MARGIN >= 0.0);
const _: () = assert!(DNA_MUTATION_CHANCE >= 0.0 && DNA_MUTATION_CHANCE <= 1.0);
const _: () = assert!(DNA_RUNG_STEP >= 1);
const _: () = assert!(DNA_TURN_LINES > DNA_RUNG_STEP);
const _: () = assert!(DNA_TURN_LINES / DNA_RUNG_STEP >= 10);
const _: () = assert!(DNA_TURN_LINES / DNA_RUNG_STEP <= 12);
const _: () = assert!(DNA_BOW_MAX >= 0.0);
const _: () = assert!(DNA_DRIFT_MAX > 0.0);
const _: () = assert!(DNA_SHIMMER_QUIET < DNA_SHIMMER_HOT);
const _: () = assert!(DNA_SHIMMER_HOT <= 1.0);
