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
