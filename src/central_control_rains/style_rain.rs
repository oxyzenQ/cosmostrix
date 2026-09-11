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

/// Brightness-zone boundary: normalized radii above this read Ghost
/// (the dim rim zone of the drain's luminance gradient).
pub(crate) const VORTEX_ZONE_RIM: f32 = 0.66;

/// Brightness-zone boundary: radii above this (and at or below the rim
/// boundary) read Mid; below it down to the core radius, Hot.
pub(crate) const VORTEX_ZONE_MID: f32 = 0.33;

/// Event-horizon radius: motes below this normalized radius are absorbed.
pub(crate) const VORTEX_CORE_R: f32 = 0.075;

/// Radius floor for the angular-speed divisor (bounds core spin rate).
pub(crate) const VORTEX_MIN_R: f32 = 0.08;

/// Rotation-curve constant K: orbital cells/sec = K × (cols/2).
/// NIGHT-research-16 label fix: omega = K / r is a FLAT rotation
/// curve (tangential speed constant along every orbit, the galaxy
/// rotation-curve read — rim ≈ 8.4 s per lap, near-core ≈ 1 rev/s),
/// NOT Kepler's third law (that would be omega ∝ r^-1.5, which the
/// quasar's disk implements). At 0.75 and 120 cols → 45 cells/s
/// along every orbit (rim orbit ≈ 8.4 s, visibly majestic).
pub(crate) const VORTEX_ROTATION_K: f32 = 0.75;

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
/// accumulate into the Hot warm ceiling (the visible vein
/// signature; NIGHT-research-18 — the retired Core rung once sent
/// the saturated 4+-particle veins — equilibrium above ~0.30 —
/// into the white blend; they read Hot like every vein now).
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

/// Brightness zone boundary: trail value above this → Hot — the
/// vein warm ceiling (NIGHT-research-18 soft-light ruling: the
/// saturated-vein zone above the retired ~0.30 saturation bound
/// merged with this zone; every vein cell, sustained or saturated,
/// reads the same warm Hot stop, never the Core white blend).
/// Cells visited by 2+ particles reach this brightness
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
/// 400x100, matching the owner's "medium size ball" spec. The
/// effective ball is additionally capped by
/// `BLACK_HOLE_BALL_WIDTH_MAX` of the viewport half-width
/// (NIGHT-research-9): on narrow terminals the width cap wins, the
/// shadow shrinks, and the disk dominates the composition.
pub(crate) const BLACK_HOLE_BALL_FRACTION: f32 = 0.55;

/// Ball radius cap as a share of the viewport half-width (in
/// line-height units), NIGHT-research-9: the owner's narrow-screen
/// report — "when the terminal width is narrow the ball reads too
/// big; it should shrink so the disk reads long". Wherever the cap
/// binds (every viewport up to roughly 1.8:1 aspect — the common
/// terminal classes), the ball's diameter spans 30% of the terminal
/// width, putting the ball-to-disk diameter ratio near 0.33 against
/// the disk's 92%-of-half-width reach — the Gargantua/EHT read
/// where the disk stretches far beyond the shadow. On the widest
/// viewports (aspect beyond ~1.8:1) the ball fraction of the
/// limiting half-extent drops below the cap and the height bound
/// takes over — the disk then stretches proportionally further
/// still (the wide-terminal majestic read paired with
/// `BLACK_HOLE_DISK_WIDTH_FRACTION`).
pub(crate) const BLACK_HOLE_BALL_WIDTH_MAX: f32 = 0.30;

/// Disk reach floor as a share of the viewport half-width (in
/// line-height units), NIGHT-research-9: the disk's scale unit is
/// the larger of the viewport's limiting half-extent and this
/// fraction of the half-width. On wide terminals (width more than
/// ~2.8x the height) the height-limited unit leaves large empty
/// side margins — the disk covers only ~72% of the width at 4:1.
/// The 0.72 floor pushes the tier-0 semi-major to the projection's
/// 92%-of-half-width clamp instead, so the disk fills the width
/// and the wide terminal reads majestic, per the owner's ask
/// ("a big terminal should perform and look masterclass"). The
/// per-mote clamp stays the hard guard: the disk can never exceed
/// 92% of the half-width on any viewport. On width-limited
/// viewports the floor never engages and geometry is unchanged.
pub(crate) const BLACK_HOLE_DISK_WIDTH_FRACTION: f32 = 0.72;

/// Event-horizon (empty core) radius as a fraction of the ball outer
/// radius. The core is never drawn — it shows the background, the
/// hole itself. 0.58 leaves a visible annulus on small terminals
/// while keeping the empty middle unmistakably dominant, per the
/// owner's "core is black/empty" spec.
pub(crate) const BLACK_HOLE_CORE_FRACTION: f32 = 0.58;

/// Rim photon line band as a share of the annulus width
/// (NIGHT-research-10, the owner's Interstellar/NASA imagery read:
/// "inside the ball there is a thin ring like a line, shaped like
/// the ball"). The annulus's outer band flips to Core — the thin
/// bright photon line hugging the shadow's edge, the trademark of
/// the EHT photographs and Gargantua's render. The line is thin by
/// construction: the band width is this share of the annulus, never
/// the whole outer half.
pub(crate) const BLACK_HOLE_PHOTON_RIM_FRACTION: f32 = 0.12;

/// Minimum rim photon line width in line-height units — the floor
/// that keeps the thin line ALIVE on small terminals: the annulus
/// itself can shrink to a single cell of width, and a purely
/// proportional band would drop to a sub-cell sliver that the
/// raster never samples (a line that vanishes exactly on the
/// viewports where the shadow reads smallest). One full cell of
/// width guarantees the outermost ring of annulus cells always
/// lands inside the band on every terminal class.
pub(crate) const BLACK_HOLE_PHOTON_RIM_MIN_CELLS: f32 = 1.0;

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
/// line). The NIGHT-research-9 contract (owner mandate 2026-09-11):
/// the whole 70-110 degree near-vertical window is excluded — the
/// owner's report, "the disk gets cut off by the terminal screen
/// limit at those attitudes and reads ugly". The menu therefore
/// tops out at 60 degrees, comfortably below the window's 70-degree
/// edge: the lever sweeps the shallow 15/30/45-degree tilts and the
/// mid 50/60, the sign alternating every excursion so left-up and
/// right-up tilts take turns. The steep 85-degree rung of the
/// stage-2.5 menu is retired; the dynamic tilt cap
/// (see `RingRoll::set_tilt_cap`) can lower the effective ceiling
/// further on viewports whose vertical budget cannot host even the
/// 60-degree rung, but the menu never arms an attitude inside the
/// excluded window on any viewport.
pub(crate) const BLACK_HOLE_ROLL_TILT_DEGS: [f32; 5] = [60.0, 50.0, 45.0, 30.0, 15.0];

/// Chance (percent) that a tilted excursion chains directly into the
/// next tilted excursion instead of returning to the flat rest line
/// first — the disk sweeps through horizontal and keeps going, the
/// continuous lever wave of the owner's example sequence.
pub(crate) const BLACK_HOLE_ROLL_CHAIN_PCT: u32 = 35;

// ── Black hole halo streams (stage 2.6, re-weighted stage 2.7, five lanes NIGHT-research-10, unified NIGHT-research-11, crown-dominant NIGHT-research-12 — NIGHT-special-1) ──
// The arc-riding companion streams of the disk stack. Stage 2.6
// (owner 9.9/10 feedback) shipped the pool: stream motes ride the
// ARC CIRCLE around the shadow instead of the flat ellipse — same
// motion DNA as the ring motes (RK4 Lorenz turbulence, Keplerian
// mean motion, entry-spiral drift-in, proximity brightness, comet
// trails) — each mote riding the full circle and drawing only on
// its own semicircle, handing off at the extremes where the arcs
// meet the equatorial band, the read of plasma sweeping over the
// top and under the bottom of the shadow in the disk's rotational
// sense. Stage 2.7 (owner 9.95/10 feedback) re-weighted the split
// into the DOUBLE UPWARD STREAM. NIGHT-research-10 (the
// Interstellar/NASA imagery round) grew the family to five lanes:
// three crowns over the shadow, two mirrored arcs under it.
// NIGHT-research-11 (owner verdict 9.1/10) unified speed (the
// lockstep pace) and the soft warm head ceiling across the family.
// NIGHT-research-12 (owner verdict 9.8/10, the final polish round)
// re-cuts the density and the concentration: the CROWNS carry the
// population (the owner's read: the upper arcs read scattered, the
// lower arcs read spammy — the crowns grow denser and tighter, the
// lower family drops to a rare elegant echo, "few but substantive"),
// and the whole family concentrates onto its arcs (a halo-specific
// tight entry spiral plus a thinner wobble band — the riders stop
// flying in from far beyond and stop smearing across a wide band).
// Geometry is fractions of the ball outer radius, so the streams
// scale with any screen size.

/// Inner upper halo stream arc radius as a multiple of the ball
/// outer radius (the INNER crown of the upper family). 1.30
/// co-rides the lensing halo circle — the inner stream's riders
/// share the road with the far-side lensed image, the white-hot
/// crown of the iconic images.
pub(crate) const BLACK_HOLE_HALO_ARC_FRACTION: f32 = 1.30;

/// Lower halo stream arc radius as a multiple of the ball outer
/// radius. 1.30 mirrors the upper circle under the shadow (the
/// owner's wording: same as the above, just the opposite position
/// below) — the inner of the two lower arcs, the mirrored echo
/// that completes the photon-ring read around the hole.
pub(crate) const BLACK_HOLE_HALO_LOWER_ARC_FRACTION: f32 = 1.30;

/// Outer lower halo stream arc radius as a multiple of the ball
/// outer radius (NIGHT-research-10: the second lower arc). 1.48
/// mirrors the mid crown's circle under the shadow — the outer of
/// the two lower arcs, riding the same clear-of-the-inner-band
/// spacing the upper family uses so the lower family reads as two
/// distinct mirrored arcs, never one thickened smear.
pub(crate) const BLACK_HOLE_HALO_LOWER_OUTER_ARC_FRACTION: f32 = 1.48;

/// Outer upper halo stream arc radius as a multiple of the ball
/// outer radius (the MID crown of the upper family — the second
/// lane of the stage-2.7 double upward stream). 1.48 rides clear
/// of the inner crown's wobble band (1.30 ± 0.10), so the crowns
/// read as distinct arcs over the shadow — the double crown of the
/// owner's 9.95/10 ruling — while the apex still fits comfortably
/// inside the viewport's half-height on every terminal class from
/// 80x24 up (the dynamic-screen-size contract).
pub(crate) const BLACK_HOLE_HALO_OUTER_ARC_FRACTION: f32 = 1.48;

/// Top upper halo stream arc radius as a multiple of the ball
/// outer radius (the TOP crown — the third and outermost lane of
/// the NIGHT-research-10 triple crown). 1.66 rides clear of the
/// mid crown's wobble band (1.48 ± 0.10, settling band 1.56-1.78
/// outer radii at the wobble extremes), so the three upward arcs
/// read as three distinct crowns — the thick upper band the owner
/// asked to match the disk stack's three-tier read. The apex fits
/// by construction on every viewport: the ball outer radius never
/// exceeds 0.55 of the half-height (the width cap only shrinks it
/// further), and 1.78 x 0.55 = 0.98 stays inside the vertical
/// budget even at the wobble extremes.
pub(crate) const BLACK_HOLE_HALO_TOP_ARC_FRACTION: f32 = 1.66;

/// Radial turbulence amplitude of the halo streams (multiple of the
/// ball outer radius, driven by the attractor's radial coordinate —
/// the same wobble source the ring motes use). NIGHT-research-12
/// (the owner's concentration ruling: the crowns read scattered
/// across a wide band, "I want dense and concentrated, absolutely
/// not spread out"): 0.055 halves the plasma band to roughly one
/// cell of thickness at the standard terminal classes — the arcs
/// read as thin dense lines hugging their circles while keeping the
/// living wobble (the r_norm normalization still swings every rider
/// through the band, so the lanes stay alive, not rigid hoops). The
/// band extremes still clear the ball silhouette (the inner crowns
/// bottom out at 1.30 - 0.055 = 1.245, the mid crown at 1.48 -
/// 0.055 = 1.425, the top crown at 1.66 - 0.055 = 1.605 — no
/// occlusion rule needed for any stream rider) and the three crowns
/// still read as distinct arcs (the 0.18 lane spacings dwarf the
/// 0.11 full band widths).
pub(crate) const BLACK_HOLE_HALO_WOBBLE_FRACTION: f32 = 0.055;

/// Halo entry-spiral radius excess for freshly spawned riders
/// (NIGHT-research-12, the concentration ruling's second half): new
/// riders materialize only 4 percent beyond their arc and settle
/// with the halo's own short time constant — the ring's 0.55
/// drift-in read as particles flying in from far outside the
/// system (the owner's "still flying outward" report), so the halo
/// family gets its own tight spiral: a rider appears at the arc's
/// outer fringe and melts into the band over ~1.8 s, igniting at
/// the warm ceiling from its first frame (even the top crown's
/// fresh distance, 1.66 x 1.04, stays inside the lensed warm zone
/// — the accretion read survives as a subtle settling, without a
/// single wandering glyph or a dim drift-in).
pub(crate) const BLACK_HOLE_HALO_ENTRY_BOOST: f32 = 0.04;

/// Halo entry-spiral decay time constant in seconds (the halo
/// twin of the ring's entry tau — shorter, so the tight spawn
/// excess resolves before the eye reads it as scatter).
pub(crate) const BLACK_HOLE_HALO_ENTRY_TAU: f32 = 0.6;

/// Halo pool size as a multiple of the terminal's column count
/// (NIGHT-research-11, the owner's consistency ruling: every ring
/// above and below must match the center ring's speed, density and
/// smoothness). The pool is the five-lane family's shared lane
/// model — one rider per lane per every `POOL_PER_COL` columns. 2
/// seats the crown family's visible population (the semicircle
/// filter halves every lane's riders) at roughly the tier-0 main
/// line's own linear cell density: the arcs read as SOLID as the
/// center ring at every density setting (both pools' active ratios
/// track the same base/mult/max constants, so the proportion holds
/// when the density key scales either pool). NIGHT-research-12
/// re-cuts the lane shares inside the fixed pool: the crowns take
/// the CROWN_SHARE family slice split three ways by round robin
/// (denser than the retired even split — the concentrated crown
/// read), the lower arcs take the LOWER_SHARE slice split two ways
/// by toggle (a rare elegant echo — few riders, each well-formed:
/// the warm head, the comet trail, the same lockstep ride).
pub(crate) const BLACK_HOLE_HALO_POOL_PER_COL: usize = 2;

const _: () = assert!(BLACK_HOLE_HALO_POOL_PER_COL >= 1);

/// Share of every halo pool fill that seats the THREE crowns
/// (NIGHT-research-12, the owner's density ruling: the crowns must
/// read dense and concentrated while the lower family reads "only a
/// few, not spam"). 0.84 of the pool split three ways by the strict
/// crown round robin seats each crown at 0.28 of every fill (vs the
/// retired even split's 0.20 — each crown grows ~40 percent denser),
/// and the family's shared Bresenham accumulator keeps the split
/// exact over every fill window (25 spawns seat 21 crown riders,
/// seven per crown, and four lower riders, two per mirrored arc).
pub(crate) const BLACK_HOLE_HALO_CROWN_SHARE: f32 = 0.84;

/// Share of every halo pool fill that seats the TWO mirrored lower
/// arcs (NIGHT-research-12, the owner's sparse-lower ruling: "the
/// bottom ring's particles must be fewer, only a few, few but
/// substantive and elegant"). 0.16 of the pool split two ways by
/// the alternating toggle seats each lower lane at 0.08 of every
/// fill (vs the retired even split's 0.20 — a 60 percent cut): at
/// the standard 120-column terminal each lower arc shows roughly
/// seven riders at the steady state, each burning the same soft
/// warm head and comet trail as the crowns — a rare echo, not a
/// spam band. The shares sum to 1.0 by the compile-time contract
/// below.
pub(crate) const BLACK_HOLE_HALO_LOWER_SHARE: f32 = 0.16;

const _: () =
    assert!((BLACK_HOLE_HALO_CROWN_SHARE + BLACK_HOLE_HALO_LOWER_SHARE - 1.0).abs() < 1.0e-6);

/// The lensed-image brightness gain (NIGHT-research-10's head-white
/// ruling, re-pinned NIGHT-research-11 for the soft-light round):
/// the distance input to the shared proximity ladder is pulled
/// inward by this many outer radii for EVERY stream rider — both
/// the crowns and the lower arcs are lensed images of the disk
/// (the same light-path compression that makes the photon ring the
/// brightest structure in the iconic images), so every lane's
/// settled riders sit inside the ladder's hot zone. 0.34 seats the
/// 1.30 lanes at an effective 0.96, the 1.48 lanes at 1.14 and the
/// top crown (1.66) at 1.32 — inside or at the hot radius — so the
/// floored ladder burns every settled rider at the soft warm
/// ceiling (Hot, after the NIGHT-research-11 soft-head cap; the
/// Core head-white that strained the owner's eyes is retired from
/// the glyph heads), while the entry-spiral drift-in still reads
/// dim and ignites as the rider settles (the accretion read
/// survives the gain).
pub(crate) const BLACK_HOLE_HALO_CROWN_GAIN: f32 = 0.34;

/// Base active-mote ratio of the halo pool (pool = one stream mote
/// per lane per `BLACK_HOLE_HALO_POOL_PER_COL` columns, the family
/// lane model). Mirrors the ring pool's own base exactly: the halo
/// pool fills to the same fraction of its lanes as the ring does at
/// every density setting, so the crown family's visible share (the
/// semicircle-filtered half of its CROWN_SHARE slice) tracks the
/// tier-0 main line's population proportionally — the
/// crowns-as-dense-as-the-center-ring read holds across the whole
/// density range, not just at one setting, and the sparse lower
/// echo scales with it (few at every density, never spam).
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
/// variance the ring motes carry). NIGHT-research-11 pins it at the
/// ring's own 14 s: the lanes now ride the lockstep pace (the
/// ring's tier-0 mean motion), so a rider's lap is the ring's own
/// ~11.6 s at the scene default speed and the lifetimes match the
/// disk motes' one-lap-plus recycling cadence — the whole halo
/// family turns over on the same clock as the ring (the
/// consistency ruling extends to the spawn/absorb rhythm).
pub(crate) const BLACK_HOLE_HALO_MAX_AGE_SECS: f32 = 14.0;

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

/// Corotation impact-parameter range (in ball outer radii),
/// NIGHT-research-9: the ambient rain shares the disk's angular
/// momentum axis, so every infalling glyph is born corotating with
/// the accretion disk (the owner's report: captures whipping around
/// the shadow against the disk's rotation read as the disk
/// "counter-rotating"; a real black hole's infalling material
/// corotates — the disk exists because the captured material's net
/// angular momentum is coherent). The spawn samples a specific
/// angular momentum ell uniform in [MIN, MAX] and derives the
/// horizontal drift from it, so a glyph's ballistic crossing of the
/// hole's latitude lands at |x| = ell — the impact-parameter spread
/// that keeps every capture unique, now with a guaranteed
/// corotating sign: the specific angular momentum (x times vy minus
/// y times vx) is positive (in the y-down screen convention) for
/// every spawn, the
/// same sign the ring motes carry. 0.30 keeps the tight plunges
/// (near-radial dives); 1.60 keeps the wide fly-bys that bend
/// gently past the field's edge.
pub(crate) const BLACK_HOLE_INFALL_COROTATION_MIN: f32 = 0.30;
pub(crate) const BLACK_HOLE_INFALL_COROTATION_MAX: f32 = 1.60;

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
// kinetic-heat grade climbs monotonically). NIGHT-research-9 adds:
// the corotation impact-parameter range must be positive and
// ordered (every spawn carries the disk's rotational sign), the
// ball width cap and the disk width floor must stay inside (0, 1)
// (shares of the viewport half-width), and the tilt menu must stay
// below the 70-degree edge of the excluded near-vertical window.
const _: () = assert!(BLACK_HOLE_INFALL_CAPTURE_FRACTION < BLACK_HOLE_INFALL_INFLUENCE_FRACTION);
const _: () = assert!(BLACK_HOLE_INFALL_INFLUENCE_FRACTION > 2.62);
const _: () = assert!(BLACK_HOLE_INFALL_SPEED_GHOST < BLACK_HOLE_INFALL_SPEED_MID);
const _: () = assert!(BLACK_HOLE_INFALL_COROTATION_MIN > 0.0);
const _: () = assert!(BLACK_HOLE_INFALL_COROTATION_MIN < BLACK_HOLE_INFALL_COROTATION_MAX);
const _: () = assert!(BLACK_HOLE_BALL_WIDTH_MAX > 0.0 && BLACK_HOLE_BALL_WIDTH_MAX < 1.0);
const _: () = assert!(BLACK_HOLE_DISK_WIDTH_FRACTION > 0.0 && BLACK_HOLE_DISK_WIDTH_FRACTION < 1.0);
const _: () = assert!(BLACK_HOLE_BALL_WIDTH_MAX < BLACK_HOLE_DISK_WIDTH_FRACTION);
const _: () = assert!(BLACK_HOLE_ROLL_TILT_DEGS[0] < 70.0);
const _: () = assert!(BLACK_HOLE_ROLL_TILT_DEGS[BLACK_HOLE_ROLL_TILT_DEGS.len() - 1] >= 15.0);
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

/// Lateral bend speed limit, in cells per sim-second, clamped
/// symmetric: the seeking stays a bend, never a slide — vx is held
/// to a fraction of the fall so a homing drop still reads as rain.
pub(crate) const AEOLIAN_SEEK_VX_LIMIT: f32 = 3.0;

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
/// Hot — the accelerating fall's band and the surfed streak, both
/// the warm ceiling now. (NIGHT-research-19 soft-light ruling: the
/// retired rung-3 threshold 3.6 sent the drop Core above this
/// speed, but gravity alone drives every free fall past 3.6 within
/// ~1.5 s from the 1.2 calm entry — terminal 4.0 — so most of
/// every drop's visible flight stood white-blended. The rain half
/// now matches the instrument half's knots-only-Core policy.)
pub(crate) const AEOLIAN_SPEED_MID: f32 = 2.4;

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
const _: () = assert!(AEOLIAN_SPEED_MID < AEOLIAN_DROP_TERMINAL);
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

/// Mid rung: granule heat above this reads Mid on the granulation
/// ladder (warm convection grit).
pub(crate) const SOLAR_GRANULE_LEVEL_MID: f32 = 0.50;

/// Hot rung: granule heat above this reads Hot (the hottest
/// convection cells).
pub(crate) const SOLAR_GRANULE_LEVEL_HOT: f32 = 0.80;

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
/// Hot fresh — the warm ceiling, NIGHT-research-20 — and Core the
/// fresh-write blink: the ~0.35 s flash at the write moment itself,
/// while a freshly-stamped charge of CHARGE_MAX decays down to this
/// bound. The retired replication-window rung (1.5) kept every
/// written rung Core-white for ~1.6 s — the masterclass audit's
/// standing-Core finding; the wake now reads the warm Hot ceiling).
pub(crate) const DNA_CHARGE_LEVEL_MID: f32 = 0.25;
pub(crate) const DNA_CHARGE_LEVEL_HOT: f32 = 0.7;
pub(crate) const DNA_CHARGE_LEVEL_BLINK: f32 = 2.3;

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

/// Spawn rate multiplier + floor (the trickle equilibrium). Part 3
/// hunt-find: the multiplier was calibrated at 0.30 under the
/// active-counter leak (every absorption permanently ate one unit
/// of the spawn budget, so the gate read a population the pool no
/// longer carried and the 0.30 trickle was never asked to actually
/// sustain the target). With the leak fixed the true drain is
/// visible — an absorbed nucleotide lives ~1.7 sim-s, not the
/// 16-s lifetime backstop — and the rate must double to hold the
/// lane target (rate x mean-life = target: 6.85 x 1.7 ~ 11.6 on
/// the 120-lane reference dial). The equilibrium still reads as a
/// trickle: the population holds the sparse-minority band the
/// calm-sky dial promises, it just no longer decays to zero over a
/// long session.
pub(crate) const DNA_SPAWN_RATE_MULT: f32 = 0.60;
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

// ── DNA genesis (NIGHT-research-7 part 3, law 0) ──────────────────
//
// The birth sequence the entry replays: soup -> ladder -> windup
// -> steady (see type_rain/dna_helix/genesis.rs for the phase
// math). The whole timeline rides the molecule's sim clock (the
// family speed contract — the speed keys scale the birth with
// the molecule), and every window is expressed in sim-seconds so
// the choreography is viewport-invariant (the fronts travel as a
// fraction of the height, not at an absolute rate — the fork's
// absolute rate is a steady-state law, the genesis a one-shot
// presentation).

/// Primordial-soup dwell (sim-seconds): the sky carries the
/// nucleotide rain alone — no molecule, the broth before the
/// genome. The spawn dial runs its genesis multiplier through
/// the dwell and the ladder window because the soup IS the scene
/// while the molecule is absent or assembling.
pub(crate) const DNA_GENESIS_SOUP_SECS: f32 = 1.8;

/// Ladder-assembly window (sim-seconds): the assembly wave writes
/// the rungs top-down (each crossed rung stamped to max charge
/// with a rolled pair — the fork's fresh-write economy borrowed
/// for the birth) while the strand radius grows from the axis
/// (the spine splits into the two strands, cubic ease-out). The
/// rotation is held through the window so the flat ladder stays
/// face-on (a rotating flat ladder periodically collapses
/// edge-on to a single line).
pub(crate) const DNA_GENESIS_LADDER_SECS: f32 = 2.6;

/// Wind-up window (sim-seconds): the twist front zips from the
/// top — above the front the strands carry the full steady law,
/// below it the flat ladder extends at the front's angle (the
/// wound top drags the flat tail around the axis as it descends,
/// the physical read of winding a ribbon from one end). The
/// rotation resumes with the windup; at its end the geometry
/// evaluates exactly to the steady law (the final front is the
/// full height — no seam, no pop).
pub(crate) const DNA_GENESIS_WINDUP_SECS: f32 = 2.4;

/// The primordial-soup thickness multiplier on the spawn dial
/// while the molecule is absent or assembling: the broth carries
/// multiples of the steady sparse target. The steady dial hands
/// the sky back to the molecule as the hero once the genome
/// exists.
pub(crate) const DNA_GENESIS_SOUP_MULT: f32 = 2.5;

/// The genesis-phase active ceiling (the calm-sky ACTIVE_MAX is
/// the steady contract; the broth's ceiling is its own — while
/// the molecule is absent a denser rain is the composition). The
/// excess thins through the floor expiry and the lifetime
/// backstop when the steady dial returns, never a mass kill.
pub(crate) const DNA_GENESIS_ACTIVE_MAX: f32 = 0.28;

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
const _: () = assert!(DNA_CHARGE_LEVEL_HOT < DNA_CHARGE_LEVEL_BLINK);
const _: () = assert!(DNA_CHARGE_LEVEL_BLINK < DNA_CHARGE_MAX);
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
const _: () = assert!(DNA_GENESIS_SOUP_SECS > 0.0);
const _: () = assert!(DNA_GENESIS_LADDER_SECS > 0.0);
const _: () = assert!(DNA_GENESIS_WINDUP_SECS > 0.0);
const _: () = assert!(DNA_GENESIS_SOUP_MULT > 1.0);
const _: () = assert!(DNA_GENESIS_ACTIVE_MAX > DNA_ACTIVE_MAX);

// ── Murmuration (NIGHT-research-7, the twelfth style) ─────────────
//
// The rain is a flock: a starling murmuration wheeling over a
// dark sky — Reynolds 1987 boids (separation, alignment,
// cohesion) mapped to the terminal grid through a spatial hash,
// with a roaming anchor (the flock's thought), a breathing
// cohesion weight (the signature tighten/loosen shape cycles)
// and a clocked predator startle (the scatter-and-regather
// drama). The complete derivation and the five laws of the flock
// live in type_rain/murmuration/mod.rs; the constants here are
// the shipped calibration.

// Population dials (the flock IS the scene — the hero dial).

/// The minimum flock (a narrow terminal still reads as a flock).
pub(crate) const MURM_MIN_BIRDS: usize = 24;

/// The maximum flock (the O(n) hash keeps 200 birds cheap, but
/// the dirty-cell budget caps the visual density).
pub(crate) const MURM_MAX_BIRDS: usize = 220;

/// The neighbor window (law 2): the alignment/cohesion radius in
/// cells — the hash bucket size. Sized so the average bird scans
/// the starling topological number (~7 neighbors) at the shipped
/// population dial.
pub(crate) const MURM_NEIGHBOR_R: f32 = 8.0;

/// The separation radius (law 1): birds inside this push apart —
/// the minimum spacing IS the visual bird density.
pub(crate) const MURM_SEP_R: f32 = 3.0;

// The triad weights (law 1, cells per sim-second squared).

/// Separation: the strongest local rule (birds never overlap).
pub(crate) const MURM_SEP_W: f32 = 90.0;

/// Alignment: steer toward the neighbors' mean heading.
pub(crate) const MURM_ALIGN_W: f32 = 26.0;

/// Cohesion base weight: the weak spring toward the local
/// centroid (the breathing oscillator modulates it, law 4).
pub(crate) const MURM_COH_W: f32 = 1.6;

/// The flight band (law 1's speed clamps): a starling never
/// hovers, never teleports.
pub(crate) const MURM_SPEED_MIN: f32 = 7.0;
pub(crate) const MURM_SPEED_MAX: f32 = 26.0;

// The kinetic ladder (the draw read).

pub(crate) const MURM_SPEED_GHOST: f32 = 10.0;
pub(crate) const MURM_SPEED_MID: f32 = 15.0;
pub(crate) const MURM_SPEED_CORE: f32 = 21.0;

// The thought (law 3, the roaming anchor).

/// The anchor attraction weight (weak against the triad — the
/// macro intent, never the collapse).
pub(crate) const MURM_ANCHOR_W: f32 = 0.05;

/// The anchor's roam speed in cells per sim-second.
pub(crate) const MURM_ANCHOR_SPEED: f32 = 7.0;

/// Mean sim-seconds between the anchor's target re-rolls.
pub(crate) const MURM_ANCHOR_HOLD: f32 = 5.0;

/// The wall banking margin (cells) and weight: birds near the
/// margins steer inward — they curve along the edge, never hit it.
pub(crate) const MURM_WALL_MARGIN: f32 = 9.0;
pub(crate) const MURM_WALL_W: f32 = 50.0;

// The breathing (law 4, the shape cycle).

/// The cohesion multiplier's base + amplitude: the flock cycles
/// between (base - amp) loose and (base + amp) tight — the
/// signature murmuration shape-shift.
pub(crate) const MURM_BREATH_BASE: f32 = 1.0;
pub(crate) const MURM_BREATH_AMP: f32 = 0.75;

/// The breathing rate in radians per sim-second (a full
/// tighten/loosen cycle every ~14 sim-seconds).
pub(crate) const MURM_BREATH_RATE: f32 = 0.45;

// The startle (law 5, the predator).

/// Mean sim-seconds between startles (variance banded at fire).
pub(crate) const MURM_STARTLE_CLOCK_MEAN: f32 = 11.0;

/// The panic radius in cells (the scatter's reach).
pub(crate) const MURM_PANIC_R: f32 = 14.0;

/// The panic impulse: the velocity kick's magnitude (an impulse,
/// not a force — the scatter is instant; the speed clamp
/// saturates it at V_MAX).
pub(crate) const MURM_PANIC_IMPULSE: f32 = 34.0;

/// The predator flash window in sim-seconds (the raptor's glyph
/// burns at Core while the scatter blooms).
pub(crate) const MURM_PANIC_FLASH_SECS: f32 = 0.8;

/// The panic speed floor's window: a startled bird flies floored
/// near max for this long after the kick.
pub(crate) const MURM_PANIC_FLOOR_SECS: f32 = 1.2;

// The organic wobble + the render contract.

/// The jitter acceleration's magnitude (the clamped random walk
/// that keeps the flock organic — no dead-locked symmetric
/// configurations).
pub(crate) const MURM_JITTER_W: f32 = 16.0;

/// Comet trail length in cells (the flight's wake).
pub(crate) const MURM_TRAIL_LEN: usize = 2;

/// The bird glyph's motion-gated shimmer chance (the family
/// contract — mutation tied to motion, deterministic under the
/// bench's uniform stepping).
pub(crate) const MURM_SHIMMER_CHANCE: f32 = 0.12;

/// The staggered entry's spawn rate (the accumulator contract —
/// the flock assembles over the first seconds).
pub(crate) const MURM_SPAWN_RATE_MULT: f32 = 0.45;
pub(crate) const MURM_SPAWN_RATE_FLOOR: f32 = 0.8;

/// Sim-time coupling to the speed keys (the family contract — see
/// AEOLIAN_SIM_TIME_PER_CPS; the reference scene speed is 18 cps).
pub(crate) const MURM_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

// Compile-time contracts on the flock calibration: the flight
// band is strictly ordered with the kinetic ladder inside it, the
// radii are ordered (separation inside the neighbor window), the
// breathing never goes negative (a repulsive cohesion would tear
// the flock), the population band is ordered, the panic impulse
// is positive, the anchor weight is a small fraction of the
// cohesion weight (the thought steers, it never collapses), and
// the wall weight dominates the anchor pull (banking wins).
const _: () = assert!(MURM_MIN_BIRDS < MURM_MAX_BIRDS);
const _: () = assert!(MURM_SEP_R < MURM_NEIGHBOR_R);
const _: () = assert!(MURM_SPEED_MIN < MURM_SPEED_GHOST);
const _: () = assert!(MURM_SPEED_GHOST < MURM_SPEED_MID);
const _: () = assert!(MURM_SPEED_MID < MURM_SPEED_CORE);
const _: () = assert!(MURM_SPEED_CORE < MURM_SPEED_MAX);
const _: () = assert!(MURM_BREATH_BASE - MURM_BREATH_AMP > 0.0);
const _: () = assert!(MURM_BREATH_BASE + MURM_BREATH_AMP < 3.0);
const _: () = assert!(MURM_ANCHOR_W > 0.0);
const _: () = assert!(MURM_ANCHOR_W < MURM_COH_W);
const _: () = assert!(MURM_PANIC_IMPULSE > 0.0);
const _: () = assert!(MURM_PANIC_R > MURM_NEIGHBOR_R);
const _: () = assert!(MURM_STARTLE_CLOCK_MEAN > 0.0);
const _: () = assert!(MURM_BREATH_RATE > 0.0);
const _: () = assert!(MURM_TRAIL_LEN >= 1);
const _: () = assert!(MURM_SHIMMER_CHANCE > 0.0 && MURM_SHIMMER_CHANCE <= 1.0);

// ── Quasar (NIGHT-research-8, the thirteenth style) ──────────────
//
// The rain feeds the engine: a quasar is a supermassive black hole
// eating ferociously — a cold gas cloud falls in, spirals into a
// Keplerian accretion disk (the inner laps lapping the outer, one
// limb doppler-brightened), the core ignites white-hot, and the
// poles fire relativistic jets. The black hole rain style is the
// SAME engine silent; the quasar is it running at full power. The
// complete derivation and the five laws of the engine live in
// type_rain/quasar/mod.rs; the constants here are the shipped
// calibration.

// Population dials (the engine IS the scene — the hero dial, the
// murmuration precedent).

/// The minimum engine (a narrow terminal still reads as a quasar).
pub(crate) const QUAS_MIN_DISK: usize = 24;

/// The maximum disk (the dirty-cell budget caps the visual density;
/// the disk is the brightest surface, so its cap leads).
pub(crate) const QUAS_MAX_DISK: usize = 96;

/// Jet particles per beam (two beams — the recycling stream that
/// reads as the collimated outflow). Sized so the beam reads as a
/// near-continuous column: the particles bunch at the slow base
/// (the energy density) and spread at the fast tip, and 22 over a
/// 16-row beam still holds a solid line through the gaps.
pub(crate) const QUAS_JET_PER_BEAM: usize = 22;

/// The halo population band (the host glow).
pub(crate) const QUAS_MIN_HALO: usize = 8;
pub(crate) const QUAS_MAX_HALO: usize = 22;

/// The steady infall dial (the calm-sky feeder — a sparse ambient
/// drizzle, never a downpour; the murmuration calm-sky dial
/// family) and the ignition multiplier (the thick cold cloud: the
/// broth runs thicker while the engine is dark, the DNA genesis
/// soup precedent).
pub(crate) const QUAS_INFALL_MIN: usize = 6;
pub(crate) const QUAS_INFALL_MAX: usize = 20;
pub(crate) const QUAS_IGNITION_INFALL_MULT: f32 = 2.2;

// The disk (law 2 — Kepler + doppler).

/// Disk tilt: the vertical squash of the orbit ellipse (a disk seen
/// at ~76 degrees inclination — the classic thin-ellipse quasar
/// read, wide enough that the shear is legible on the cell grid).
pub(crate) const QUAS_DISK_TILT: f32 = 0.24;

/// The inner disk edge (the ISCO homage): orbit fractions run
/// [QUAS_DISK_INNER, 1.0] of the disk semi-major axis.
pub(crate) const QUAS_DISK_INNER: f32 = 0.30;

/// Kepler's constant: omega = K / f^1.5 (the third law — the inner
/// edge laps the outer ~6x, the shear IS the rotation read).
pub(crate) const QUAS_KEPLER_K: f32 = 0.16;

/// The doppler beaming weight: the brightness factor swings
/// [1 - W, 1 + W] across the disk (the approaching limb brightens,
/// the receding dims — the M87 photograph's signature asymmetry).
pub(crate) const QUAS_DOPPLER_W: f32 = 0.45;

/// The doppler rung threshold: |los| beyond this shifts the
/// brightness rung one step (mono-safe — the asymmetry survives
/// colorless terminals).
pub(crate) const QUAS_DOPPLER_RUNG: f32 = 0.5;

/// The circularization damping time constant (sim-seconds): a
/// captured gas streamer eases onto its target orbit (the disk
/// assembles without a pop).
pub(crate) const QUAS_CIRC_TAU: f32 = 0.9;

/// The halo circularization damping time constant (sim-seconds):
/// the host halo's riders glide onto their annulus (the same
/// exponential ease the disk uses, on the halo's slower clock).
/// NIGHT-lts-1 stage 1 promoted the inline 1.6 magic number of
/// `halo_step` to this named constant — the value is unchanged.
pub(crate) const QUAS_HALO_CIRC_TAU: f32 = 1.6;

// The fuel (law 3 — the infall rain).

/// The infall base rate in orbit-fraction per sim-second (scaled by
/// the inward acceleration below — the plunge accelerates as the
/// gravity tightens). Calibrated so a streamer's transit runs
/// ~3 sim-seconds: the disk is ~90 percent condensed when the
/// core lights, and completes over the first breath of the
/// steady state.
pub(crate) const QUAS_INFALL_RATE: f32 = 0.22;

/// The infall spiral wind (radians per sim-second at the outer
/// edge; tightens with 1/f — the streamer coils as it falls).
pub(crate) const QUAS_INFALL_SPIN: f32 = 0.50;

/// The fresh-feed charge decay time constant (sim-seconds): the
/// light that shows where the engine has been recently fed (the
/// DNA rung-charge economy, the disk's heir).
pub(crate) const QUAS_CHARGE_TAU: f32 = 3.2;

/// The charge rung threshold: a freshly-fed disk cell reads one
/// rung hotter while its charge is above this.
pub(crate) const QUAS_CHARGE_RUNG: f32 = 0.55;

// The jets (law 4 — the exhaust).

/// The jet base speed in beam-fraction per sim-second (s advances
/// on v(s) = V0 (1 + ACC s) — the relativistic acceleration, the
/// tip outraces the collar).
pub(crate) const QUAS_JET_V0: f32 = 0.16;
pub(crate) const QUAS_JET_ACC: f32 = 2.4;

/// The jet precession rate (radians per sim-second — the beams'
/// slow conical wobble; a full sweep every ~28 sim-seconds).
pub(crate) const QUAS_PREC_RATE: f32 = 0.22;

/// The jet helix: the beam's particles ride a helix of this
/// amplitude (cells, at the tip — the amplitude grows with s), one
/// full turn every 1/QUAS_HELIX_TURNS of the beam.
pub(crate) const QUAS_HELIX_AMP: f32 = 1.6;
pub(crate) const QUAS_HELIX_TURNS: f32 = 1.2;

/// The knot window (beam-fraction): the flare-launched shock
/// brightens every jet particle inside this band of the knot's
/// position (the traveling pulse — the engine's heartbeat read up
/// the beam).
pub(crate) const QUAS_KNOT_W: f32 = 0.14;

/// The knot's travel speed (beam-fraction per sim-second —
/// slower than the beam's fastest particles: the knot rides the
/// stream, it never outruns it).
pub(crate) const QUAS_KNOT_V: f32 = 0.50;

// The core and the glow (laws 1 and 5).

/// The core pulse rate (radians per sim-second — the luminosity
/// breathing, a full cycle every ~7 sim-seconds).
pub(crate) const QUAS_PULSE_RATE: f32 = 0.9;

/// The feed-flare clock mean (sim-seconds between flares, variance
/// banded at fire — the drama event, the murmuration startle's
/// heir): a gas clump arrives, the core flares, the infall surges
/// and a knot climbs each beam.
pub(crate) const QUAS_FLARE_CLOCK_MEAN: f32 = 9.0;

/// The feed-flare window (sim-seconds): the core's lock above its
/// pulse peak while the clump burns in.
pub(crate) const QUAS_FLARE_WINDOW: f32 = 2.0;

/// The feed-flare infall surge multiplier (the clump: the rain
/// itself thickens for the window).
pub(crate) const QUAS_FLARE_SURGE: f32 = 2.5;

// The ignition (law 0 — the birth, the DNA genesis contract).

/// The dark cloud window (sim-seconds): cold gas falls, nothing
/// burns.
pub(crate) const QUAS_IGNITION_DARK_SECS: f32 = 2.0;

/// The disk assembly window (sim-seconds): captured streamers
/// circularize, the disk condenses.
pub(crate) const QUAS_IGNITION_DISK_SECS: f32 = 2.6;

/// The first light window (sim-seconds): the core ignites and the
/// luminosity ramps to full.
pub(crate) const QUAS_IGNITION_LIGHT_SECS: f32 = 1.6;

/// The jet extension window (sim-seconds): the beams push out to
/// full length.
pub(crate) const QUAS_IGNITION_JET_SECS: f32 = 1.9;

// The render contract.

/// The disk/jet/infall glyph's motion-gated shimmer chance (the
/// family contract — mutation tied to motion, deterministic under
/// the bench's uniform stepping).
pub(crate) const QUAS_SHIMMER_CHANCE: f32 = 0.12;

/// The staggered infall spawn rate (the accumulator contract — the
/// cloud assembles over the first seconds).
pub(crate) const QUAS_SPAWN_RATE_MULT: f32 = 0.50;
pub(crate) const QUAS_SPAWN_RATE_FLOOR: f32 = 1.2;

/// Sim-time coupling to the speed keys (the family contract — see
/// AEOLIAN_SIM_TIME_PER_CPS; the reference scene speed is 18 cps).
pub(crate) const QUAS_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

// Compile-time contracts on the engine calibration: the ignition
// windows are strictly positive (the phase classifier's ordering
// depends on every window being reachable), the ignition
// multiplier surges (a thinner broth would starve the birth), the
// doppler swing stays inside the factor's brightening band and the
// rung threshold sits inside the |los| unit range, the Kepler
// constant keeps the inner edge turning slower than a blur and the
// outer edge faster than a still life, the fuel falls inward
// (rate positive) and the charge decays (tau positive), the jets
// accelerate (ACC positive) and outrun their own knots (the knot
// rides the beam slower than the beam's fastest particles — the
// pulse reads as a wave, not a teleport), the population bands are
// ordered, the pulse beats, the flare clock ticks, the surge
// thickens, and the halo band brackets the disk's outer edge.
const _: () = assert!(QUAS_IGNITION_DARK_SECS > 0.0);
const _: () = assert!(QUAS_IGNITION_DISK_SECS > 0.0);
const _: () = assert!(QUAS_IGNITION_LIGHT_SECS > 0.0);
const _: () = assert!(QUAS_IGNITION_JET_SECS > 0.0);
const _: () = assert!(QUAS_IGNITION_INFALL_MULT > 1.0);
const _: () = assert!(QUAS_FLARE_SURGE > 1.0);
const _: () = assert!(QUAS_DOPPLER_W > 0.0 && QUAS_DOPPLER_W < 1.0);
const _: () = assert!(QUAS_DOPPLER_RUNG > 0.0 && QUAS_DOPPLER_RUNG < 1.0);
const _: () = assert!(QUAS_DISK_INNER > 0.0 && QUAS_DISK_INNER < 1.0);
const _: () = assert!(QUAS_DISK_TILT > 0.0 && QUAS_DISK_TILT < 1.0);
const _: () = assert!(QUAS_KEPLER_K > 0.0);
const _: () = assert!(QUAS_KEPLER_K / (QUAS_DISK_INNER * QUAS_DISK_INNER) < 2.0);
const _: () = assert!(QUAS_KEPLER_K > 0.05);
const _: () = assert!(QUAS_INFALL_RATE > 0.0);
const _: () = assert!(QUAS_CHARGE_TAU > 0.0);
const _: () = assert!(QUAS_JET_V0 > 0.0 && QUAS_JET_ACC > 0.0);
const _: () = assert!(QUAS_KNOT_V > 0.0);
const _: () = assert!(QUAS_JET_V0 * (1.0 + QUAS_JET_ACC) > QUAS_KNOT_V);
const _: () = assert!(QUAS_KNOT_W > 0.0 && QUAS_KNOT_W < 1.0);
const _: () = assert!(QUAS_MIN_DISK < QUAS_MAX_DISK);
const _: () = assert!(QUAS_MIN_HALO < QUAS_MAX_HALO);
const _: () = assert!(QUAS_MIN_HALO < QUAS_MAX_DISK);
const _: () = assert!(QUAS_PULSE_RATE > 0.0);
const _: () = assert!(QUAS_FLARE_CLOCK_MEAN > 0.0);
const _: () = assert!(QUAS_FLARE_WINDOW > 0.0);
const _: () = assert!(QUAS_SHIMMER_CHANCE > 0.0 && QUAS_SHIMMER_CHANCE <= 1.0);
const _: () = assert!(QUAS_INFALL_MIN < QUAS_INFALL_MAX);

// ── Neural (NIGHT-research-9, the fourteenth style) ───────────────

/// The input band's population bounds (the widest layer — the
/// data needs bandwidth; the viewport derivation clamps inside).
pub(crate) const NEUR_INPUT_MIN: usize = 4;
pub(crate) const NEUR_INPUT_MAX: usize = 14;

/// The hidden/output layers' population floor (a layer exists
/// even on degenerate viewports — the machine stays valid).
pub(crate) const NEUR_HIDDEN_FLOOR: usize = 2;

/// The streamer (data) population bounds for the accumulator.
pub(crate) const NEUR_MIN_STREAMER: usize = 5;
pub(crate) const NEUR_MAX_STREAMER: usize = 16;

/// The burst surge's hard cap on the live streamer target (the
/// clump never floods the dirty-cell budget).
pub(crate) const NEUR_STREAMER_SURGE_CAP: usize = 26;

/// The fixed pulse pool size (the signal traffic bound — a
/// saturated volley sheds load, never grows the pool).
pub(crate) const NEUR_PULSE_POOL: usize = 64;

/// The per-node fanout (the wiring density — every source reaches
/// FANOUT targets in the next layer).
pub(crate) const NEUR_FANOUT: usize = 3;

/// The input band's anchor (fraction of the frame's height — the
/// data lands near the top; the composition leaves the rain its
/// share of the sky).
pub(crate) const NEUR_INPUT_Y_FRAC: f32 = 0.26;

/// The output band's anchor (fraction of the height — the answer
/// lands near the floor).
pub(crate) const NEUR_OUTPUT_Y_FRAC: f32 = 0.86;

/// The layer grid's horizontal margin (fraction of the width).
pub(crate) const NEUR_X_MARGIN_FRAC: f32 = 0.08;

/// The golden-angle y-jitter's amplitude (cells — the lattice
/// must not read as a machine grid).
pub(crate) const NEUR_Y_JITTER: f32 = 1.5;

/// The firing threshold (the all-or-nothing spike's gate).
pub(crate) const NEUR_THRESHOLD: f32 = 1.0;

/// The potential's hard cap (bounded by construction, not by
/// hope — a saturated cell clamps, it never explodes).
pub(crate) const NEUR_POT_CAP: f32 = 2.0;

/// The membrane leak's time constant (sim-seconds — the
/// forgetting window: kicks that do not sum to a thought inside
/// it decay away).
pub(crate) const NEUR_LEAK_TAU: f32 = 1.8;

/// The refractory window (sim-seconds — no fire while it burns;
/// a saturated cell sheds load like the real substrate).
pub(crate) const NEUR_REFRACTORY: f32 = 0.5;

/// The fired flash's decay constant (sim-seconds — the spike's
/// afterglow).
pub(crate) const NEUR_FLASH_TAU: f32 = 0.35;

/// The wire weight's floor and span (the delivery charge in
/// [MIN, MIN + SPAN] — one arrival is sub-threshold by design:
/// a single meal is not a thought).
pub(crate) const NEUR_WEIGHT_MIN: f32 = 0.55;
pub(crate) const NEUR_WEIGHT_SPAN: f32 = 0.30;

/// The pulse travel speed (cells per sim-second) and its per-
/// pulse spread share (the rolled speed keeps waves from reading
/// as a grid — the jet energy-share's heir).
pub(crate) const NEUR_PULSE_V: f32 = 7.0;
pub(crate) const NEUR_PULSE_SPREAD: f32 = 0.25;

/// The wire's signal-glow decay (sim-seconds — the light shows
/// where signals have recently passed).
pub(crate) const NEUR_GLOW_TAU: f32 = 1.4;

/// The spontaneous input kick's Poisson rate (per input neuron
/// per sim-second — the machine idles alive between meals) and
/// the kick's size.
pub(crate) const NEUR_SPONT_RATE: f32 = 0.22;
pub(crate) const NEUR_SPONT_KICK: f32 = 0.38;

/// The streamer capture's kick (the steady feed's charge — the
/// data feeds the machine until it thinks).
pub(crate) const NEUR_CAPTURE_KICK: f32 = 0.55;

/// The thought-burst clock's mean (sim-seconds — the drama
/// event's cadence; re-armed with the family's 0.6-1.4 roll),
/// the burst window (the surge's duration), the volley size
/// (inputs force-fired together) and the streamer surge (the
/// data thickens while the machine thinks).
pub(crate) const NEUR_BURST_CLOCK_MEAN: f32 = 8.0;
pub(crate) const NEUR_BURST_WINDOW: f32 = 1.8;
pub(crate) const NEUR_BURST_INPUTS: usize = 4;
pub(crate) const NEUR_BURST_SURGE: f32 = 2.2;

/// The plasticity clock's mean (sim-seconds — at most one
/// rewire in flight, the bounded learning), the retire fade's
/// duration (the wire dims out while its riding pulses land)
/// and the growth duration (the successor's reach-out, and the
/// genesis wires' growth rate).
pub(crate) const NEUR_REWIRE_CLOCK_MEAN: f32 = 7.5;
pub(crate) const NEUR_REWIRE_FADE_SECS: f32 = 1.1;
pub(crate) const NEUR_GROW_SECS: f32 = 1.4;

/// The wire sweep's stagger window (sim-seconds from the wire
/// phase's start — the growth sweeps the machine layer-pair by
/// layer-pair, finishing inside the thought window).
pub(crate) const NEUR_WIRE_STAGGER_WINDOW: f32 = 1.0;

/// The genesis timeline (sim-seconds per phase: the data falls,
/// the layers build, the dendrites reach out, the first thought
/// fires — the DNA genesis contract's heir).
pub(crate) const NEUR_GENESIS_SIGNAL_SECS: f32 = 2.2;
pub(crate) const NEUR_GENESIS_LAYERS_SECS: f32 = 2.6;
pub(crate) const NEUR_GENESIS_WIRE_SECS: f32 = 1.8;
pub(crate) const NEUR_GENESIS_THOUGHT_SECS: f32 = 1.6;

/// The streamer fall speed's floor and span (cells per
/// sim-second — the rain does not fall in lockstep) and the
/// sway's amplitude (cells).
pub(crate) const NEUR_FALL_MIN: f32 = 2.8;
pub(crate) const NEUR_FALL_SPAN: f32 = 1.6;
pub(crate) const NEUR_DRIFT_AMP: f32 = 0.8;

/// The moving cells' shimmer chance (the family contract).
pub(crate) const NEUR_SHIMMER_CHANCE: f32 = 0.12;

/// The staggered streamer spawn rate (the accumulator contract —
/// the data cloud assembles over the first seconds).
pub(crate) const NEUR_SPAWN_RATE_MULT: f32 = 0.50;
pub(crate) const NEUR_SPAWN_RATE_FLOOR: f32 = 1.2;

/// The genesis's thick data cloud (while the machine is unbuilt
/// the rain IS the scene — the DNA soup precedent).
pub(crate) const NEUR_GENESIS_SIGNAL_MULT: f32 = 2.2;

/// Sim-time coupling to the speed keys (the family contract — see
/// AEOLIAN_SIM_TIME_PER_CPS; the reference scene speed is 16 cps).
pub(crate) const NEUR_SIM_TIME_PER_CPS: f32 = 1.0 / 12.0;

// Compile-time contracts on the machine's calibration: the
// genesis windows are strictly positive (the phase classifier's
// ordering depends on every window being reachable), the signal
// multiplier surges (a thinner cloud would starve the birth),
// the potential's cap sits above the threshold (a clamped cell
// still reads as saturating, not as dead), the leak forgets and
// the refractory ends (positive taus), the wires carry weight
// (positive floor and span, one arrival sub-threshold by
// design), the pulses travel (positive speed, bounded spread),
// the glow fades, the clocks tick, the volley fits the input
// band and the pool absorbs a full fanout volley (the burst
// never sheds load on a healthy machine), the population bands
// are ordered, the layer anchors bracket the frame, the streamers
// fall (positive floor and span), the surge thickens, and the
// shimmer stays a chance.
const _: () = assert!(NEUR_GENESIS_SIGNAL_SECS > 0.0);
const _: () = assert!(NEUR_GENESIS_LAYERS_SECS > 0.0);
const _: () = assert!(NEUR_GENESIS_WIRE_SECS > 0.0);
const _: () = assert!(NEUR_GENESIS_THOUGHT_SECS > 0.0);
const _: () = assert!(NEUR_GENESIS_SIGNAL_MULT > 1.0);
const _: () = assert!(NEUR_BURST_SURGE > 1.0);
const _: () = assert!(NEUR_THRESHOLD > 0.0);
const _: () = assert!(NEUR_POT_CAP > NEUR_THRESHOLD);
const _: () = assert!(NEUR_LEAK_TAU > 0.0);
const _: () = assert!(NEUR_REFRACTORY > 0.0);
const _: () = assert!(NEUR_FLASH_TAU > 0.0);
const _: () = assert!(NEUR_PULSE_V > 0.0);
const _: () = assert!(NEUR_PULSE_SPREAD > 0.0 && NEUR_PULSE_SPREAD < 1.0);
const _: () = assert!(NEUR_GLOW_TAU > 0.0);
const _: () = assert!(NEUR_WEIGHT_MIN > 0.0 && NEUR_WEIGHT_SPAN > 0.0);
const _: () = assert!(NEUR_WEIGHT_MIN + NEUR_WEIGHT_SPAN < NEUR_THRESHOLD);
const _: () = assert!(NEUR_MIN_STREAMER < NEUR_MAX_STREAMER);
const _: () = assert!(NEUR_MAX_STREAMER < NEUR_STREAMER_SURGE_CAP);
const _: () = assert!(NEUR_INPUT_MIN < NEUR_INPUT_MAX);
const _: () = assert!(NEUR_HIDDEN_FLOOR >= 2);
const _: () = assert!(NEUR_FANOUT > 0);
const _: () = assert!(NEUR_PULSE_POOL > NEUR_INPUT_MAX * NEUR_FANOUT);
const _: () = assert!(NEUR_BURST_CLOCK_MEAN > 0.0);
const _: () = assert!(NEUR_BURST_WINDOW > 0.0);
const _: () = assert!(NEUR_BURST_INPUTS > 0 && NEUR_BURST_INPUTS <= NEUR_INPUT_MIN);
const _: () = assert!(NEUR_REWIRE_CLOCK_MEAN > 0.0);
const _: () = assert!(NEUR_REWIRE_FADE_SECS > 0.0);
const _: () = assert!(NEUR_GROW_SECS > 0.0);
const _: () = assert!(NEUR_WIRE_STAGGER_WINDOW > 0.0);
const _: () = assert!(
    NEUR_WIRE_STAGGER_WINDOW + NEUR_GROW_SECS < NEUR_GENESIS_WIRE_SECS + NEUR_GENESIS_THOUGHT_SECS
);
const _: () = assert!(NEUR_SHIMMER_CHANCE > 0.0 && NEUR_SHIMMER_CHANCE <= 1.0);
const _: () = assert!(NEUR_INPUT_Y_FRAC > 0.0 && NEUR_INPUT_Y_FRAC < NEUR_OUTPUT_Y_FRAC);
const _: () = assert!(NEUR_OUTPUT_Y_FRAC < 1.0);
const _: () = assert!(NEUR_FALL_MIN > 0.0 && NEUR_FALL_SPAN > 0.0);
const _: () = assert!(NEUR_Y_JITTER > 0.0);
