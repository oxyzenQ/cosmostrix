// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tuning constants for the vortex (third), flux (fourth), lorenz
//! (fifth), dragon (sixth) and physarum (seventh) rain styles —
//! task-18/task-19 + NIGHT-research-4/5/6.
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
/// value. Combined with the decay rate (0.90/frame at 60 FPS =
/// ~0.5/sec effective decay), steady-state trail value at a cell
/// visited continuously = deposit / decay = 0.10 — above the
/// PHYSARUM_BRIGHTNESS_DIM threshold, so single-particle cells
/// reach Mid zone, and multi-particle cells accumulate into
/// Hot/Core (the visible vein signature).
pub(crate) const PHYSARUM_DEPOSIT_AMOUNT: f32 = 0.5;

/// Trail decay rate per frame (multiplier). At 0.90, the trail
/// loses 10% of its value per frame — at 60 FPS, an unvisited cell
/// fades to 1% of its peak value in ~0.44 seconds. This is the
/// negative feedback that lets unused paths fade so the network
/// stays alive (without decay, every cell saturates and the
/// network disappears). Tuned higher than the original 0.92 so
/// fresh trails are brighter relative to old ones (more visible
/// vein distinction).
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
