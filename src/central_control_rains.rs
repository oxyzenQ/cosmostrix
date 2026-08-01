// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Central Control — Rains
//!
//! Single source of truth for every rain, parallax, phosphor, and
//! depth-layer tuning parameter. This is the **plug-and-play control
//! file** for the entire rain visual stack — modeled after
//! `chroma/catalog.rs` for color themes.
//!
//! ## Scope
//!
//! Every constant in this file directly controls *how the rain looks*.
//! Anything that affects droplet motion, trail persistence, layer depth,
//! head/body/tail brightness, fog, vignette, or atmospheric noise lives
//! here. Non-visual constants (terminal limits, perf counters, buffer
//! caps, watchdog intervals) stay in `constants.rs`.
//!
//! ## How to fine-tune
//!
//! 1. Find the section you want to adjust below.
//! 2. Change the value(s).
//! 3. `cargo build --release` — that's it.
//!
//! All consumers reference `crate::constants::*` which re-exports
//! everything from this module via `pub use central_control_rains::*;`.
//! No call-site changes needed when tuning.
//!
//! ## Section map
//!
//! | Section                  | Controls                                           |
//! |--------------------------|----------------------------------------------------|
//! | Parallax depth layers    | Layer count + per-layer speed/brightness/saturation |
//! | Phosphor persistence     | CRT afterglow decay, tail residual, glyph threshold |
//! | Atmospheric depth        | Density, contrast reduction, glyph dim              |
//! | Head bloom               | Gaussian glow sigma, intensity, cell radius         |
//! | Per-layer head bloom     | Depth-aware head pop multiplier per layer           |
//! | Depth fog vignette       | Top/bottom row dimming                             |
//! | CRT vignette             | Cinematic top/bottom edge dim                       |
//! | Radial vignette          | Edge darkening + layer masking                     |
//! | Rain shadow              | Bottom quadratic fade-out + layer masking           |
//! | Front layer tail         | Long-stream proportional tail allocation            |
//! | Velocity turbulence      | Drift oscillation amplitude/frequency               |
//! | Cinematic smoothness     | Fractional head brightness, shimmer, spawn jitter   |
//! | Trail cycle              | Probability of mid-trail character re-randomization |
//! | Droplet gravity          | Acceleration + terminal velocity                    |
//! | Startup easing           | Cinematic ramp-up from idle                         |
//! | Resume easing            | Pause → resume transition smoothing                 |
//! | Spawn pacing             | Spawn remainder cap, advance remainder cap          |
//! | Warm start               | Initial seed count, head bias, spawn debt           |
//! | Glyph entry ramp         | Fresh-droplet brightness ramp-in                    |
//! | Living rain density      | Per-column density noise (spatial + temporal)        |
//! | Living rain wind gusts   | Gust attack/hold/decay envelope                     |
//! | Color transition         | Palette propagation wave duration/velocity           |
//! | Color ecosystems         | Long-timescale hue/sat/lum drift                    |
//! | Atmospheric evolution    | Autonomous density/luminance/anomaly ranges         |
//! | Memory history           | Renderer long-timescale memory samples              |
//! | Emergent storytelling    | Rare emergent visual moments                        |
//! | Edge fade                | Viewport entry/exit taper                           |
//! | Anomaly events           | Rare visual corruption events                       |
//! | Quantum & ghost          | Quantum ripple + phosphor ghost spawn rates         |
//!
//! ## Calibration history (most recent first)
//!
//! - **v30.0.0 (final — visual test locked)**: after A/B visual testing
//!   against option C (density-focused) and option D (haze-focused), the
//!   parameter set from commit 1e4e3fa (the initial visibility-floor
//!   raise) was confirmed as the optimal balance. Reverted option C's
//!   two mid tweaks: density 0.55 → 0.75, contrast_reduction 0.15 → 0.12.
//!   Mid layer now reads as vivid individual streaks at natural density —
//!   neither too sparse (C's 0.55 made the field feel empty) nor too
//!   hazy (D's 1.3 phosphor_decay muted trails). Effective mid energy:
//!   0.242 (the sweet spot between C's 0.174 and original-v30's 0.334).
//! - **v30 (option C — density-focused)**: mid layer density dropped
//!   from 0.75 → 0.55 to remove noise via fewer droplets rather than
//!   dimming. Reverted Option D's mid rollbacks (head_bloom 0.70 → 0.82,
//!   phosphor_decay 1.3 → 1.0, contrast_red 0.25 → 0.12) since the
//!   density drop alone is sufficient. Slight contrast_red bump to 0.15
//!   keeps a hint of haze. Effective mid energy: 0.174 (vs D's 0.187,
//!   pre-v30's 0.109, original v30's 0.334). — *Reverted by final lock.*
//! - **v30 (option D — haze-focused)**: rolled back mid head-bloom,
//!   raised mid contrast reduction + phosphor decay. Worked but user
//!   preferred density-focused approach. — *Reverted by option C.*
//! - **v30 (initial raise)**: lifted back + mid visibility floor to
//!   fix "too quiet/dim darkness" complaint. Effective back visibility
//!   raised 2.5x; effective mid raised 3.06x. — *Confirmed optimal by
//!   final visual lock; restored as v30.0.0 baseline.*

use std::time::Duration;

// ─── Parallax depth layers ─────────────────────────────────────────────────
//
// The rain is rendered in 3 parallax layers (back/mid/front = far/mid/near).
// Each droplet is assigned to a layer at spawn time. Layer controls depth
// via compound multiplicative factors: speed (parallax), brightness, haze,
// glow — all stack to push back-layer rain into atmospheric depth while
// keeping the front layer as the vivid neon focal point.
//
// The layer index is also used to gate per-layer effects in droplet.rs,
// phosphor.rs, monolith.rs, and the spawn path.

/// Number of parallax depth layers.
pub const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
///
/// Back layer moves at 35% of base speed (parallax recession), front layer
/// at 170% (foreground whoosh). Mid matches base speed.
pub const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = far, 2 = near).
///
/// v30.0.0 final lock — confirmed optimal by visual A/B test against
/// option C and option D variants. Back effective visibility
/// = 0.55 × 0.55 × (1−0.40) ≈ 0.182 (2.5× pre-v30 — fixes the original
/// "too quiet/dim darkness" complaint without overshooting).
///   - Back  (0): 0.55 (v30 floor — visible atmospheric depth)
///   - Mid   (1): 0.88 (v30 floor — clearly present, not "too quiet")
///   - Front (2): 1.00 (kept — full neon brightness)
pub const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.55, 0.88, 1.00];

/// Per-layer saturation multiplier (layer 0 = desaturated, 2 = full).
///
/// v30 calibration: back retains 55% saturation (45% haze — "rain in fog"
/// feel). Mid at 90% keeps color identity vivid. Front at 100% full neon.
pub const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.55, 0.90, 1.00];

/// Per-layer head-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// v30.0.0 final lock: mid at 0.82 gives the head a soft pop without
/// spilling into noisy bloom. Back at 0.55 keeps distant head glow as
/// soft haze rather than invisible pinprick. (Option D briefly rolled
/// mid back to 0.70 — too muted; option C restored 0.82 — confirmed.)
///   - Back  (0): 0.55 (v30 floor)
///   - Mid   (1): 0.82 (v30 floor)
///   - Front (2): 1.00 (full cinematic head pop)
pub const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.55, 0.82, 1.0];

/// Per-layer head self-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// v30: back at 0.45 means effective self-bloom is ~25% (vs 55% for
/// front), keeping back heads visible without popping as "white dots".
///   - Back  (0): 0.45 (v30 floor — visible without popping)
///   - Mid   (1): 0.78 (v30 floor — clearly present)
///   - Front (2): 1.0 (kept — full cinematic head pop)
pub const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.78, 1.0];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
///
/// Back layer droplets are 50% of base length (brief streaks). Front layer
/// droplets are 140% (long cinematic rain streaks). Mid matches base.
pub const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];

// ─── Phosphor persistence (CRT afterglow) ──────────────────────────────────
//
// Phosphor is what gives trails their "fading glow" — when a droplet's
// head passes a cell, that cell gets phosphor energy that decays over
// time, producing the characteristic Matrix afterglow. Per-layer decay
// lets back-layer trails fade fast (don't linger as bright spots) while
// front-layer trails fade slow (smooth body→tail gradient).

/// Per-cell phosphor energy decay rate (higher = faster fade).
///
/// At 5.0, afterglow lasts ~400ms (still 2× film Matrix's ~200ms, but
/// 2.7× faster than the old 1094ms afterglow).
pub const PHOSPHOR_DECAY_RATE: f32 = 5.0;

/// Energy level when a cell's tail passes (starts the phosphor glow).
///
/// At 160, trail brightness is ~63% of head — body cells clearly visible
/// as colored rain rather than dim ghosts.
pub const PHOSPHOR_TAIL_RESIDUAL: u8 = 160;

/// Below this energy, the cell is cleared to blank.
pub const PHOSPHOR_DEAD_THRESHOLD: u8 = 6;

/// Minimum phosphor energy for rendering the original character glyph in
/// ghost cells. Below this threshold, the ghost cell renders as a blank
/// space (or dim color-only patch). Prevents stale cells from filling
/// the background with dark charset glyphs.
pub const PHOSPHOR_GLYPH_THRESHOLD: u8 = 96;

/// Per-layer phosphor decay rate multiplier (far=fast, near=slow).
///
/// v30.0.0 final lock: mid at 1.0 — base decay, trails persist long
/// enough to read as rain streaks without lingering as bright spots.
/// Back at 1.8 keeps distant trails brief (no flicker). Front at 0.5 —
/// slow fade for smooth body→tail gradient. (Option D briefly raised
/// mid to 1.3 — too muted; option C restored 1.0 — confirmed.)
///   - Back  (0): 1.8 (trails fade quick, no bright-spot linger)
///   - Mid   (1): 1.0 (base decay — natural trail persistence)
///   - Front (2): 0.5 (slow fade for body→tail smoothness)
pub const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.8, 1.0, 0.5];

/// Number of rows from the bottom of the screen where phosphor decay is
/// accelerated (prevents "concrete wall" residue buildup).
pub const PHOSPHOR_BOTTOM_ROWS: u16 = 12;

/// Phosphor decay rate multiplier applied to bottom rows (3.0× base).
pub const PHOSPHOR_BOTTOM_DECAY_MULT: f32 = 3.0;

// ─── Atmospheric depth layering ────────────────────────────────────────────
//
// Three independent controls stack to push the back layer into atmospheric
// depth: density (fewer spawns), contrast reduction (fg→bg blend = fog),
// and glyph dim (char-level dimming, currently 1.0 = no-op since brightness
// + saturation already cover it).

/// Per-layer spawn density multiplier (far = sparse, near = dense).
///
/// v30.0.0 final lock: mid at 0.75 — the natural density that visual
/// A/B testing confirmed as the sweet spot. Option C tried 0.55 (too
/// sparse — mid layer felt empty, lost depth cues); 0.75 keeps enough
/// droplets to read as a steady rain field without crossing into noise.
/// Each mid droplet stays vivid (brightness 0.88, saturation 0.90,
/// head_bloom 0.82) at natural density.
///   - Back  (0): 0.45 (v30 floor — visible distant rain)
///   - Mid   (1): 0.75 (v30 floor — confirmed optimal by visual test)
///   - Front (2): 1.00 (kept — natural base rate)
pub const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.75, 1.0];

/// Per-layer glyph simplicity (currently no-op — subsumed by brightness
/// + saturation). Kept as a tuning knob for future use.
pub const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];

/// Per-layer contrast reduction (depth-of-field perceptual blur).
///
/// Blends fg color toward black (background) by this fraction. The
/// terminal equivalent of DoF blur — back layer reads as "behind a
/// haze", front layer is sharp.
///
/// v30.0.0 final lock: mid at 0.12 — minimal haze, lets mid-layer color
/// identity (saturation 0.90) read cleanly. Option C tried 0.15 (too
/// milky — mid felt washed out at the new lower density); 0.12 keeps
/// mid crisp while still sitting behind the front layer in depth.
///   - Back  (0): 0.40 (v30 floor — visible rain through soft fog)
///   - Mid   (1): 0.12 (v30 floor — confirmed optimal by visual test)
///   - Front (2): 0.0 (kept — sharp foreground)
pub const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.40, 0.12, 0.0];

// ─── Exponential trail fade & head bloom ───────────────────────────────────

/// Trail brightness exponential decay constant (higher = faster fade).
pub const TRAIL_EXPONENTIAL_K: f64 = 1.2;

/// Cap on accumulated spawn remainder per column (prevents burst spawns
/// after pause or long delta-time frames).
pub const SPAWN_REMAINDER_CAP: f32 = 4.0;

/// Cap on accumulated advance remainder per droplet (prevents position
/// jumps after pause or long delta-time frames).
pub const ADVANCE_REMAINDER_CAP: f32 = 3.0;

// ─── Warm start (initial rain seeding) ─────────────────────────────────────

/// Maximum head row for warm-start seeds (keeps initial heads near top).
pub const WARM_START_MAX_HEAD: u16 = 8;

/// Fraction of droplet pool to seed on warm start.
pub const WARM_START_SEED_FRACTION: f32 = 0.12;

/// Minimum number of warm-start seeds (prevents empty screen on tiny terms).
pub const WARM_START_SEED_MIN: usize = 3;

/// Maximum number of warm-start seeds (caps burst on huge terms).
pub const WARM_START_SEED_MAX: usize = 12;

/// Spawn debt carried into the first second after warm start (smooths
/// the transition from seed to steady-state spawn rate).
pub const WARM_START_SPAWN_DEBT: f32 = 0.5;

// ─── Glyph entry ramp (fresh droplet fade-in) ──────────────────────────────

/// Duration of the fresh-droplet brightness ramp-in (ms).
pub const GLYPH_ENTRY_RAMP_DURATION_MS: u32 = 700;

/// Minimum scale of the ramp (droplet starts at this brightness, ramps
/// to 1.0 over the duration above).
pub const GLYPH_ENTRY_RAMP_MIN_SCALE: f32 = 0.15;

// ─── Cinematic color transition ────────────────────────────────────────────

/// Maximum number of simultaneously-tracked palette slots (for
/// generation-based palette propagation during transitions).
pub const MAX_PALETTE_SLOTS: usize = 4;

/// Duration of the per-column color transition wave (ms).
pub const COLOR_TRANSITION_DURATION_MS: u16 = 300;

/// Fraction of columns initially visible during a transition (12% —
/// the rest propagate in over the duration above).
pub const COLOR_TRANSITION_INITIAL_VISIBLE_PCT: f32 = 0.12;

/// Duration of the per-column charset transition wave (ms).
pub const CHARSET_TRANSITION_DURATION_MS: u16 = 500;

/// Velocity boost applied to new-generation droplets during an active
/// transition (creates an incoming-wave feel).
pub const TRANSITION_VELOCITY_BOOST: f32 = 0.05;

/// Duration of the post-transition energy surge (sec).
pub const TRANSITION_ENERGY_DURATION_SECS: f32 = 1.5;

/// Saturation boost during the energy surge.
pub const TRANSITION_ENERGY_SATURATION_BOOST: f32 = 0.15;

/// Head glow boost during the energy surge.
pub const TRANSITION_HEAD_GLOW_BOOST: f32 = 0.2;

// ─── Droplet gravity & terminal velocity ───────────────────────────────────

/// Downward acceleration applied to droplet head position (cells/sec²).
pub const DROPLET_GRAVITY: f32 = 2.0;

/// Multiplier on base speed at which droplets stop accelerating
/// (terminal velocity).
pub const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 1.8;

// ─── Cinematic startup easing ──────────────────────────────────────────────

/// Initial velocity as fraction of target velocity (3% — slow start).
pub const STARTUP_VELOCITY_FRACTION: f32 = 0.03;

/// Time constant for the startup velocity ramp (sec).
pub const STARTUP_EASE_TAU: f32 = 0.30;

// ─── Head bloom (exponential gaussian falloff) ─────────────────────────────

/// Sigma (standard deviation) of the gaussian head-bloom falloff.
pub const HEAD_BLOOM_SIGMA: f32 = 1.2;

/// Peak intensity of the head bloom glow (0.0 = none, 1.0 = full).
pub const HEAD_BLOOM_INTENSITY: f32 = 0.40;

/// Number of cells on each side of the head that receive bloom glow.
pub const HEAD_BLOOM_CELLS: u16 = 2;

// ─── Depth fog vignette (top/bottom row dim) ───────────────────────────────

/// Number of rows at top and bottom affected by depth fog.
pub const FOG_ROWS: u16 = 4;

/// Minimum brightness factor at the extreme edge row (0.65 = 35% dim).
pub const FOG_MIN_FACTOR: f32 = 0.65;

// ─── Cinematic CRT vignette (top & bottom edge dim) ────────────────────────

/// Height (in rows) of the CRT vignette band at top and bottom.
pub const CRT_VIGNETTE_HEIGHT: u16 = 5;

/// Brightness factor at the extreme edge row of the CRT vignette.
pub const CRT_VIGNETTE_EDGE_FACTOR: f32 = 0.9;

/// Perf-pressure threshold below which the CRT vignette is skipped
/// (perf optimization — skip on slow systems).
pub const CRT_VIGNETTE_PERF_THRESHOLD: f32 = 0.5;

// ─── Cinematic radial vignette (edge darkening) ────────────────────────────

/// Intensity of the radial vignette (0.0 = none, 1.0 = full black at edges).
pub const VIGNETTE_INTENSITY: f32 = 0.30;

/// Inner radius (as fraction of half-screen) where vignette starts.
pub const VIGNETTE_INNER_RADIUS: f32 = 0.7;

/// Per-layer vignette multiplier (0.0 = no dimming, 1.0 = full dimming).
///
/// Front layer (2) is exempt — vignette is a depth effect that should
/// only push mid/back deeper into the background.
pub const VIGNETTE_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

// ─── Rain shadow (bottom quadratic fade-out) ───────────────────────────────

/// Percentage of screen height (from bottom) affected by rain shadow.
pub const RAIN_SHADOW_PCT: f32 = 0.15;

/// Per-layer rain shadow multiplier (front layer exempt, same as vignette).
pub const RAIN_SHADOW_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

// ─── Front layer tail allocation ───────────────────────────────────────────
//
// Long front-layer droplets need proportional tails — otherwise they read
// as long head+body "lines" with invisible tails. Tail cell count is
// allocated as a percentage of droplet length, capped at a max.

/// Fraction of droplet length allocated to tail cells (45%).
pub const FRONT_LAYER_TAIL_PCT: f32 = 0.45;

/// Hard cap on tail cell count (prevents degenerate values on huge screens).
pub const FRONT_LAYER_TAIL_MAX_CELLS: u8 = 12;

/// Number of color stops used by long front-layer tails.
pub const FRONT_LAYER_MAX_TAIL_STOPS: u8 = 3;

// ─── Mouse interaction ─────────────────────────────────────────────────────

/// Radius (in columns) of the mouse hover glow.
pub const MOUSE_GLOW_RADIUS_COLS: f32 = 7.0;

/// Radius (in lines) of the mouse hover glow.
pub const MOUSE_GLOW_RADIUS_LINES: f32 = 5.0;

/// Intensity of the mouse hover glow (0.0 = disabled in default mode).
pub const MOUSE_GLOW_INTENSITY: f32 = 0.0;

/// Speed of the mouse-click flash ring expansion (cells/sec).
pub const MOUSE_FLASH_SPEED: f32 = 32.0;

/// Width of the mouse-click flash ring (in cells).
pub const MOUSE_FLASH_RING_WIDTH: f32 = 8.0;

/// Peak intensity of the mouse-click flash.
pub const MOUSE_FLASH_INTENSITY: f32 = 0.85;

/// Duration of the mouse-click flash (sec).
pub const MOUSE_FLASH_DURATION_SECS: f32 = 1.8;

/// Fraction of the primary ring intensity applied to the secondary echo ring.
pub const MOUSE_FLASH_SECONDARY_FRAC: f32 = 0.45;

/// Speed of the secondary ring as fraction of primary ring speed.
pub const MOUSE_FLASH_SECONDARY_SPEED_FRAC: f32 = 0.4;

// ─── Velocity turbulence ───────────────────────────────────────────────────

/// Maximum velocity perturbation as fraction of base chars_per_sec.
pub const TURBULENCE_AMPLITUDE: f32 = 0.08;

/// Turbulence oscillation frequency (Hz).
pub const TURBULENCE_FREQ: f32 = 0.4;

// ─── Cinematic perceived smoothness ────────────────────────────────────────
//
// These constants control the per-frame "alive" feel — fractional head
// brightness pulses, head character shimmer, and spawn phase jitter break
// the robotic frame-locked cadence.

/// Fractional head brightness amplitude (head brightens up to 15% as it
/// approaches the next row).
pub const FRACTIONAL_HEAD_BRIGHTNESS_AMP: f32 = 0.15;

/// Fractional bloom modulation (bloom glow intensifies up to 10%).
pub const FRACTIONAL_BLOOM_AMP: f32 = 0.10;

/// Head character shimmer period (sec) — head cycles to a new char from
/// the pool at this interval.
pub const HEAD_SHIMMER_PERIOD_SECS: f32 = 0.10;

/// Whether to add random fractional phase offset when spawning (breaks
/// the synchronized "robotic march").
pub const SPAWN_PHASE_JITTER: bool = true;

/// Probability per frame that a mid-trail cell re-randomizes its
/// character (subtle "churn").
pub const TRAIL_CYCLE_PROBABILITY: f32 = 0.02;

// ─── Rare anomaly events ───────────────────────────────────────────────────

/// Chance per second of an anomaly event firing.
pub const ANOMALY_CHANCE_PER_SEC: f64 = 0.017;

/// Duration of an anomaly event (sec).
pub const ANOMALY_DURATION_SECS: f32 = 1.5;

/// Maximum simultaneously-active anomaly zones.
pub const ANOMALY_MAX_ZONES: usize = 3;

/// Luminance intensity boost during an anomaly.
pub const ANOMALY_LUMINANCE_INTENSITY: f32 = 0.3;

/// Chance that an anomaly corrupts (re-randomizes) trail characters.
pub const ANOMALY_CORRUPTION_CHANCE: f32 = 0.4;

// ─── Temporal color ecosystems ─────────────────────────────────────────────

/// Tick interval for color ecosystem evaluation (sec).
pub const COLOR_ECOSYSTEM_TICK_SECS: f32 = 3.0;

/// Climate luminance drift rate per tick.
pub const COLOR_CLIMATE_DRIFT_RATE: f32 = 0.008;

/// Saturation drift rate per tick.
pub const COLOR_SATURATION_DRIFT_RATE: f32 = 0.005;

/// Hue drift rate per tick.
pub const COLOR_HUE_DRIFT_RATE: f32 = 0.015;

/// Chance per tick of re-evaluating drift direction.
pub const COLOR_DRIFT_REEVAL_CHANCE: f32 = 0.15;

/// Minimum climate luminance multiplier.
pub const COLOR_LUMINANCE_CLIMATE_MIN: f32 = 0.75;

/// Maximum climate luminance multiplier.
pub const COLOR_LUMINANCE_CLIMATE_MAX: f32 = 1.0;

/// Minimum climate saturation multiplier.
pub const COLOR_SATURATION_CLIMATE_MIN: f32 = 0.7;

/// Maximum climate saturation multiplier.
pub const COLOR_SATURATION_CLIMATE_MAX: f32 = 1.0;

/// Chance per tick of an autonomous palette drift event.
pub const AUTONOMOUS_PALETTE_DRIFT_CHANCE: f32 = 0.03;

/// Whether autonomous color drift is enabled by default.
pub const AUTO_COLOR_DRIFT_DEFAULT: bool = false;

// ─── Cinematic runtime behavior profiles ───────────────────────────────────

/// Duration of the profile transition (sec).
pub const PROFILE_TRANSITION_SECS: f32 = 30.0;

/// Interpolation rate for profile parameter changes.
pub const PROFILE_INTERPOLATION_RATE: f32 = 0.02;

// ─── Autonomous atmospheric evolution ──────────────────────────────────────

/// Tick interval for atmospheric evolution (sec).
pub const ATMOSPHERE_TICK_SECS: f32 = 5.0;

/// Cycle period for entropy buildup and release (sec).
pub const ENTROPY_CYCLE_SECS: f32 = 300.0;

/// Range of density variation during atmospheric evolution.
pub const ATMOSPHERE_DENSITY_RANGE: f32 = 0.4;

/// Range of luminance variation during atmospheric evolution.
pub const ATMOSPHERE_LUMINANCE_RANGE: f32 = 0.2;

/// Range of anomaly probability variation during atmospheric evolution.
pub const ATMOSPHERE_ANOMALY_RANGE: f32 = 0.5;

// ─── Living rain: dynamic density noise ────────────────────────────────────
//
// Each column has a spatial density modifier in [MIN, MAX] that re-rolls
// every PERIOD_SECS. Kills the "uniform grid" feel without per-frame
// allocation — single O(1) hash per spawn.

/// Period at which the density noise field re-rolls (sec).
pub const DENSITY_NOISE_PERIOD_SECS: f64 = 10.0;

/// Minimum density noise modifier.
pub const DENSITY_NOISE_MIN: f32 = 0.6;

/// Maximum density noise modifier.
pub const DENSITY_NOISE_MAX: f32 = 1.4;

/// Hash multiplier for density noise (Knuth-style prime).
pub const DENSITY_NOISE_HASH_K: u32 = 2_654_435_761;

/// Hash seed multiplier for density noise.
pub const DENSITY_NOISE_HASH_SEED_K: u32 = 1_103_515_245;

// ─── Living rain: wind gusts ───────────────────────────────────────────────
//
// Gusts are an envelope: idle → attack → hold → decay → idle. Each phase
// has min/max duration; the actual duration is rolled uniformly. The peak
// multiplier scales droplet speed during the hold phase.

/// Idle phase min duration (sec) — time between gusts.
pub const GUST_IDLE_MIN_SECS: f64 = 30.0;

/// Idle phase max duration (sec).
pub const GUST_IDLE_MAX_SECS: f64 = 120.0;

/// Attack phase min duration (sec) — ramp-up time.
pub const GUST_ATTACK_MIN_SECS: f64 = 1.0;

/// Attack phase max duration (sec).
pub const GUST_ATTACK_MAX_SECS: f64 = 2.0;

/// Hold phase min duration (sec) — peak sustain time.
pub const GUST_HOLD_MIN_SECS: f64 = 0.5;

/// Hold phase max duration (sec).
pub const GUST_HOLD_MAX_SECS: f64 = 1.0;

/// Decay phase min duration (sec) — ramp-down time.
pub const GUST_DECAY_MIN_SECS: f64 = 3.0;

/// Decay phase max duration (sec).
pub const GUST_DECAY_MAX_SECS: f64 = 5.0;

/// Min peak speed multiplier during the hold phase.
pub const GUST_PEAK_MIN: f32 = 1.2;

/// Max peak speed multiplier during the hold phase.
pub const GUST_PEAK_MAX: f32 = 1.5;

// ─── Long-timescale renderer memory ────────────────────────────────────────
//
// The renderer samples recent state to detect anomalies and persistence
// patterns. These constants control the sampling cadence and how much
// weight is given to recent anomaly pressure.

/// Number of historical samples retained in renderer memory.
pub const MEMORY_HISTORY_SAMPLES: usize = 32;

/// Interval between memory samples (sec).
pub const MEMORY_SAMPLE_INTERVAL_SECS: f32 = 30.0;

/// Weight given to anomaly pressure in memory scoring.
pub const MEMORY_ANOMALY_PRESSURE_WEIGHT: f32 = 0.3;

/// Persistence boost applied during calm periods (rewards sustained calm).
pub const MEMORY_CALM_PERSISTENCE_BOOST: f32 = 0.15;

// ─── Emergent visual storytelling ──────────────────────────────────────────

/// Tick interval for emergent moment evaluation (sec).
pub const STORYTELLING_TICK_SECS: f32 = 10.0;

/// Chance per tick of an emergent moment firing.
pub const EMERGENT_MOMENT_CHANCE: f32 = 0.08;

/// Duration of an emergent moment (sec).
pub const EMERGENT_MOMENT_DURATION_SECS: f32 = 8.0;

/// Maximum simultaneously-active emergent moments.
pub const EMERGENT_MAX_MOMENTS: usize = 1;

/// Luminance intensity boost during an emergent moment.
pub const EMERGENT_LUMINANCE_INTENSITY: f32 = 0.12;

/// Density intensity boost during an emergent moment.
pub const EMERGENT_DENSITY_INTENSITY: f32 = 0.25;

/// Speed shift during an emergent moment (additive).
pub const EMERGENT_SPEED_SHIFT: f32 = 0.15;

// ─── Cinematic resume easing (pause → resume transition) ───────────────────

/// Duration of the resume easing ramp (sec). During this window, all
/// simulation parameters are scaled by a smootherstep curve from the
/// paused-state value to 1.0.
pub const RESUME_EASE_DURATION_SECS: f32 = 0.45;

/// Duration of the pause easing ramp (sec). Scales simulation parameters
/// from 1.0 down to the paused floor.
pub const PAUSE_EASE_DURATION_SECS: f32 = 0.30;

// ─── Viewport edge fade ────────────────────────────────────────────────────

/// Number of rows at the top affected by edge fade (smooth entry/exit
/// at terminal border).
pub const EDGE_FADE_ROWS: u16 = 3;

/// Number of rows at the bottom affected by edge fade.
pub const EDGE_FADE_BOTTOM_ROWS: u16 = 12;

/// Lip factor for the bottom edge fade (controls curvature).
pub const EDGE_FADE_BOTTOM_LIP: f32 = 0.75;

/// Minimum brightness factor at the top edge.
pub const EDGE_FADE_TOP_MIN: f32 = 0.70;

/// Minimum brightness factor at the bottom edge.
pub const EDGE_FADE_BOTTOM_MIN: f32 = 0.35;

/// Brightness threshold below which bold attribute is suppressed at edges.
pub const EDGE_FADE_BOLD_THRESHOLD: f32 = 0.5;

/// Maximum phosphor energy at the viewport edge (caps edge glow).
pub const PHOSPHOR_EDGE_ENERGY_CAP: u8 = 64;

/// Taper rate (in rows) for phosphor energy at the viewport edge.
pub const PHOSPHOR_EDGE_ROW_TAPER: u8 = 8;

// ─── Atmospheric Event Engine ──────────────────────────────────────────────

/// XOR mask applied to the event RNG seed (deterministic per-session).
pub const EVENT_RNG_XOR: u64 = 0xCAFE_BABE_1337_0420;

/// Perf-pressure gate below which atmospheric events are skipped.
pub const EVENT_PERF_GATE: f32 = 0.5;

// ─── Phosphor Ghost ────────────────────────────────────────────────────────

/// Chance per tick of a phosphor ghost spawning.
pub const GHOST_SPAWN_CHANCE_PER_TICK: f64 = 0.003;

/// Maximum simultaneously-active phosphor ghosts.
pub const GHOST_MAX_ACTIVE: usize = 1;

// ─── Cloud internals ───────────────────────────────────────────────────────
//
// These tune the droplet pool sizing and RNG behavior — affect rain
// density and continuity at the structural level.

/// Multiplier on (cols × density) for droplet pool sizing.
pub const DROPLET_COUNT_FACTOR: f32 = 1.5;

/// Minimum droplet length (cells) — guarantees a recognizable head→body→tail.
pub const MIN_DROPLET_LENGTH: u16 = 4;

/// Maximum droplet length cap (cells) — prevents degenerate values on
/// huge screens (8K UHD bench = 4320 lines).
pub const MAX_DROPLET_LENGTH_CAP: u16 = 200;

/// Size of the per-column character pool (pre-allocated, no per-spawn alloc).
pub const CHAR_POOL_SIZE: usize = 2048;

/// Size of the per-column glitch character pool.
pub const GLITCH_POOL_SIZE: usize = 1024;

/// Maximum character pool index (CHAR_POOL_SIZE - 1, used for fast mod).
pub const MAX_CHAR_POOL_IDX: u16 = 2047;

/// Interval at which the RNG is reseeded from system entropy (sec).
pub const RNG_RESEED_INTERVAL_SECS: u64 = 600;

/// Initial RNG seed (deterministic per-session, reseeded every interval above).
pub const RNG_INITIAL_SEED: u64 = 0x0123_4567;

/// Duration that a droplet's head lingers at peak brightness after
/// advancing (ms).
pub const HEAD_LINGER_BRIGHTNESS_MS: u64 = 300;

/// Interval between full-redraw forced refreshes (frames). Prevents
/// drift accumulation in long-running sessions.
pub const FULL_REDRAW_INTERVAL_FRAMES: u64 = 18000;

// ─── Performance tuning (rain-affecting subset) ────────────────────────────

/// Spawn-scale floor under perf pressure (don't go below 25% of target).
pub const PERF_SPAWN_SCALE_MIN: f32 = 0.25;

/// Glitch activation threshold (fraction of cells).
pub const GLITCH_THRESHOLD: f32 = 0.35;

/// Ratio of the glitch bright phase to total glitch duration.
pub const GLITCH_BRIGHT_RATIO: f64 = 0.25;

/// Ratio of the glitch dim phase to total glitch duration.
pub const GLITCH_DIM_RATIO: f64 = 0.75;

/// Simulation pressure scaling factor (reduces sim load under pressure).
pub const SIM_PRESSURE_SCALE_FACTOR: f64 = 0.7;

/// Minimum simulation fraction (don't go below 50% of target).
pub const SIM_MIN_FRACTION: f64 = 0.5;

/// Maximum simulation delta-time cap (sec) — prevents huge sim steps
/// after pause or stall.
pub const SIM_MAX_CAP_SECS: f64 = 1.0 / 30.0;

/// Base multiplier for simulation step sizing.
pub const SIM_BASE_MULTIPLIER: f64 = 3.0;

/// Density step granularity for CLI/runtime density adjustments.
pub const DENSITY_STEP: f32 = 0.25;

/// Watchdog interval (sec) — checks for stuck droplets / state drift.
pub const WATCHDOG_INTERVAL_SECS: u64 = 1;

// ─── Frame timing budget (rain-affecting subset) ───────────────────────────

/// Frame spin budget — time the event loop will busy-wait before yielding.
pub const FRAME_SPIN_BUDGET: Duration = Duration::from_micros(500);

/// Frame spin limit — maximum time the event loop will busy-wait.
pub const FRAME_SPIN_LIMIT: Duration = Duration::from_micros(1000);

/// Minimum simulation factor under heavy load.
pub const SIM_FACTOR_MIN: f64 = 0.3;
