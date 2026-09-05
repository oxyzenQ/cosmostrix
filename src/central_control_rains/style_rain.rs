// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tuning constants for the vortex (third) and ripple (fourth) rain
//! styles — task-18. Split from `mod.rs` to respect the 800-LOC hard
//! cap; re-exported wholesale via `pub(crate) use style_rain::*;` so
//! `crate::constants::VORTEX_*` / `RIPPLE_*` keep working like the
//! monolith constants (single flat namespace by design).

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

// ── Ripple (fourth rain style, task-18) ───────────────────────────────
// Water-surface model: glyph rain stops just above a virtual surface;
// impacts open expanding edge-on wavefront rings + short splash hops.

/// Water line depth: rows above the bottom edge.
pub(crate) const RIPPLE_SURFACE_ROWS: u16 = 3;

/// Droplet clearance: falling rain stops this many rows above the water
/// line, keeping the splash zone free of droplet cells (region contract).
pub(crate) const RIPPLE_DROPLET_CLEAR_ROWS: u16 = 3;

/// Ripple ring pool size (max concurrent rings).
pub(crate) const RIPPLE_RING_POOL: usize = 48;

/// Splash particle pool size.
pub(crate) const RIPPLE_SPLASH_POOL: usize = 32;

/// Base ripple ring lifetime (seconds, ±15% variance per impact).
pub(crate) const RIPPLE_RING_LIFETIME: f32 = 1.6;

/// Base ring max horizontal spread (cells, ±20-45% variance).
pub(crate) const RIPPLE_RING_MAX_RADIUS: f32 = 11.0;

/// Base splash lifetime (seconds, ±20% variance).
pub(crate) const RIPPLE_SPLASH_LIFETIME: f32 = 0.45;

/// Initial splash hop speed (rows/s upward).
pub(crate) const RIPPLE_SPLASH_SPEED: f32 = 11.0;

/// Splash gravity (rows/s²). With speed 11: apex ≈ 2.2 rows.
pub(crate) const RIPPLE_SPLASH_GRAVITY: f32 = 28.0;

/// Splash rise cap: rows above the water line a particle may reach
/// (= droplet clearance, so zones never overlap).
pub(crate) const RIPPLE_SPLASH_MAX_RISE: u16 = 2;

/// Surface shimmer spacing: every Nth column shows a dim surface glyph.
pub(crate) const RIPPLE_SHIMMER_SPACING: u16 = 8;

/// Speed normalization: ring/splash rate scale = chars_per_sec / this,
/// clamped 0.25..3.0 (the vortex/ripple default scenes run speed 18-24).
pub(crate) const RIPPLE_SPEED_REF_CPS: f32 = 20.0;

// ── Dragon (fifth rain style, NIGHT-research-5) ──────────────────────
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

// ── Physarum (sixth rain style, NIGHT-research-6) ────────────────────
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
