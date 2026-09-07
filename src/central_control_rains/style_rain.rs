// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: pure tuning-constants file — every style's motion/density
// constants with their mandatory why-docs, no logic (same data-file class
// as themes.rs). NIGHT-special-1 stage 2.3 added the equatorial-disk
// constants and pushed it 4 lines over; splitting the single tuning
// surface by style family would scatter the owner's visual-feedback
// rationale across files (RULES_LOC.md "When NOT to Split").

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

/// Orbital ellipse semi-minor axis (the vertical squeeze) as a
/// fraction of the viewport unit. 0.14 renders the disk almost
/// exactly edge-on — the Gargantua read of the owner's stage-2.3
/// feedback (9.5/10): the Interstellar disk reads as one thin
/// horizontal line through the shadow's middle, not a fat band
/// ("padat sampai terlihat garis horizontal"). Halved from 0.28 at
/// the same time the near side gained its vertical squash and the
/// density floor tripled — thinner geometry, more glyphs, solid
/// line.
pub(crate) const BLACK_HOLE_RING_MINOR_FRACTION: f32 = 0.14;

/// Near-side vertical squash factor: the in-front half of the orbit
/// maps its sine onto this fraction of the semi-minor axis, so the
/// crossing band hugs the equator instead of dipping a full minor
/// axis below it. Stage 2.3 owner feedback: the solid line must sit
/// at the vertical MIDDLE of the core ("garisnya harusnya berada di
/// tengah core blackhole, kalo yang sekarang malah berada di bawahnya")
/// — with 0.5 the near side spans only half a minor axis below the
/// center, and together with the z-tilt breathing the band reads
/// centered on the shadow. The far side keeps the full factor: it
/// belongs to the arms rising into the lensing halo.
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
/// ball radius — a living plasma stream, not a rigid hoop.
pub(crate) const BLACK_HOLE_RING_WOBBLE_FRACTION: f32 = 0.34;

/// Out-of-plane displacement amplitude (the attractor z mapped onto
/// the screen vertical), as a multiple of the ball outer radius.
/// Tightened 0.18 -> 0.12 for the stage-2.3 thin-disk read: the band
/// breathes around the equatorial plane (the thickness cue of a real
/// disk) without inflating the solid line into a ribbon — at the
/// scene default the excursions stay under ~1.2 lines at 120x40.
pub(crate) const BLACK_HOLE_RING_Z_TILT: f32 = 0.12;

/// Inner-disk brightness zone: motes whose |cos phi| (the horizontal
/// orbital position, 0 directly in front / behind, 1 at the line's
/// extremes) falls below this bound brighten one ladder rung at draw
/// time — the hot inner edge of a real accretion disk, and the
/// "solid white horizontal line" read of the owner's stage-2.3
/// feedback: the brightest plasma sits across the shadow and (lensed)
/// over the top, exactly where Gargantua glows hardest.
pub(crate) const BLACK_HOLE_RING_INNER_ZONE: f32 = 0.45;

/// Edge-fade start: beyond this |cos phi| the disk brightness steps
/// down toward Ghost rung by rung — the outer disk thins out. The
/// owner's Interstellar reference: the dense white line ends in a
/// few sparse particles, a smooth transition into the dark
/// ("di ujung garis putih itu ada sedikit beberapa partikel jadi
/// terlihat smooth transisi").
pub(crate) const BLACK_HOLE_RING_EDGE_FADE_START: f32 = 0.68;

/// Edge-fade depth: the maximum ladder rungs stepped down at the
/// line's extremes (|cos phi| -> 1). 3 rungs lands most extreme-dwell
/// motes at Ghost — dim wisps — while the mid-band stays untouched,
/// the smooth density falloff of a real disk fading with radius.
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
/// column, the family lane model). Raised 0.22 -> 0.55 for the
/// stage-2.3 solid-band read: at the scene density 0.55 the target
/// sits near 74% of the pool, and the comet trails knit the band
/// into the near-continuous horizontal line of the owner's
/// Interstellar reference (a stream of individuals reads as gaps,
/// not a line).
pub(crate) const BLACK_HOLE_RING_ACTIVE_BASE: f32 = 0.55;

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
