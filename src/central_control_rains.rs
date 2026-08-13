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
//! | Section                  | Controls                                            |
//! |--------------------------|-----------------------------------------------------|
//! | Parallax depth layers    | Layer count + per-layer speed/brightness/saturation |
//! | Phosphor persistence     | CRT afterglow decay, tail residual, glyph threshold |
//! | Parallax depth           | Density, contrast reduction, glyph dim              |
//! | Head bloom               | Gaussian glow sigma, intensity, cell radius         |
//! | Per-layer head bloom     | Depth-aware head pop multiplier per layer           |
//! | Depth fog vignette       | Top/bottom row dimming                              |
//! | CRT vignette             | Cinematic top/bottom edge dim                       |
//! | Radial vignette          | Edge darkening + layer masking                      |
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
//! | Living rain density      | Per-column density noise (spatial + temporal)       |
//! | Living rain wind gusts   | Gust attack/hold/decay envelope                     |
//! | Color transition         | Palette propagation wave duration/velocity          |
//! | Color ecosystems         | Long-timescale hue/sat/lum drift                    |
//! | Entropy drift            | Autonomous density/luminance/anomaly ranges         |
//! | Memory history           | Renderer long-timescale memory samples              |
//! | Emergent storytelling    | Rare emergent visual moments                        |
//! | Edge fade                | Viewport entry/exit taper                           |
//! | Anomaly events           | Rare visual corruption events                       |
//! | Quantum & ghost          | Quantum ripple + phosphor ghost spawn rates         |
//!
//! ## Calibration history (most recent first)
//!
//! - **v30.0.0 (peak masterclass cinematic lock + stabilization)**: visual
//!   test rated 10/10 perfect after silent override bug fix + front
//!   density restoration. No parameter changes — visual tuning is locked.
//!   Strengthened the bug fixes with permanent regression tests in
//!   `droplet.rs::silent_override_regression_tests`:
//!   1. `brightness_boost_above_one_actually_lightens` — front-layer
//!      boost 1.05 must produce r_out > r_in (catches Bug #1 regression).
//!   2. `brightness_dim_below_one_still_dims` — back-layer dim 0.48 must
//!      produce r_out < r_in (catches accidental gate widening).
//!   3. `saturation_boost_above_one_oversaturates_vivid_color` — front-
//!      layer oversaturation 1.05 must push vivid colors further from
//!      gray (catches Bug #2 regression).
//!   4. `saturation_boost_leaves_gray_unchanged` — mathematical fixed-
//!      point invariant (gray is unchanged by any saturation op).
//!   5. `selfbloom_fractional_multiplier_actually_applies` — all three
//!      selfbloom multipliers (0.38, 0.68, 1.15) must produce a non-zero
//!      boost (catches Bug #3 regression via `as i32` truncation).
//!   6. `per_layer_multipliers_are_monotically_nondecreasing` — depth
//!      cue invariant (front ≥ mid ≥ back for brightness/sat/selfbloom).
//!
//!   Also confirmed centralization is complete: every per-layer visual
//!   parameter (PARALLAX_*, PHOSPHOR_LAYER_DECAY, MONOLITH_LAYER_BRIGHTNESS,
//!   MONOLITH_BREATHING_AMPLITUDE) lives in this file. monolith.rs and
//!   cinematic.rs are pure consumers — they import from `crate::constants`
//!   and add zero hardcoded layer values. Editing any rain parameter
//!   requires touching only this single file.
//! - **v30.0.0 (silent override bug fix + centralization)**: user reported
//!   "the front layer feels dim, there's no glow" after differential tuning.
//!   Deep audit found 3 silent override bugs in droplet.rs that made boosts
//!   > 1.0 completely no-op:
//!   1. Brightness gate `if combined_layer < 1.0` skipped front boost 1.05
//!      (no effect). Fixed to `!= 1.0` so both dim and boost apply.
//!   2. Saturation gate `if saturation_mult < 1.0` skipped front boost 1.05
//!      (no effect). Fixed to `!= 1.0`; the `(color-lum)*(1-sat)` formula
//!      naturally extends to sat > 1.0 (push away from gray = oversaturate).
//!   3. Selfbloom `as i32` truncated 0.30→0, 0.65→0, 1.0→1, then integer
//!      division `(60 * 0_or_1) / 256` gave `wf = 0` for ALL layers —
//!      selfbloom was a 0% boost no-op since the constant was introduced.
//!      Switched to f32 math so fractional multipliers actually apply.
//!
//!   Also restored front spawn density 1.10 → 0.85 to compensate for the
//!   spawn-roll fix (commit 9080472) that gave front +40% more density
//!   rolls. Effective front spawn rate now matches 5571c0b level (sparse,
//!   crisp glow per droplet) while per-droplet boosts (now actually
//!   applied) make front read as more prominent than 5571c0b.
//!
//!   Centralization: moved `monolith.rs::layer_brightness` hardcoded
//!   values (0.48/0.78/1.0) and `cinematic.rs::monolith_breathing_factor`
//!   amplitudes (0.018/0.026/0.034) into this file as
//!   `MONOLITH_LAYER_BRIGHTNESS` and `MONOLITH_BREATHING_AMPLITUDE`.
//!   Now ALL layer-specific tuning lives in this single file — editing
//!   any rain parameter requires touching only central_control_rains.rs.
//! - **v30.0.0 (differential depth tuning)**: user requested sharper
//!   depth differential — back needs to be slightly more dim, mid needs
//!   reduced density + slight dim, front needs to read more prominent
//!   (no dimming). Tuned all three layers in opposite directions to
//!   widen the back→front depth gradient. Per-droplet visibility now:
//!   back 0.132, mid 0.551, front 1.103 (was 0.182/0.696/1.000). Field
//!   energy (×density): back 0.050, mid 0.342, front 1.213 (was
//!   0.082/0.522/1.000). Ratio back:mid:front widened from 1:6.4:12.2
//!   to 1:5.75:15.79 — front clearly dominant, back recedes into
//!   atmospheric haze, mid sits between as sparse vivid streaks.
//!   (Ratio reflects post-restoration front density 1.10; the earlier
//!   1:6.8:24.1 figure used the pre-restoration 1.00 density.)
//!   — *Front density later restored; per-droplet boosts fixed in silent
//!   override bug fix above.*
//! - **v30.0.0 (final — visual test locked)**: after A/B visual testing
//!   against option C (density-focused) and option D (haze-focused), the
//!   parameter set from commit 1e4e3fa (the initial visibility-floor
//!   raise) was confirmed as the optimal balance. Reverted option C's
//!   two mid tweaks: density 0.55 → 0.75, contrast_reduction 0.15 → 0.12.
//!   Mid layer now reads as vivid individual streaks at natural density —
//!   neither too sparse (C's 0.55 made the field feel empty) nor too
//!   hazy (D's 1.3 phosphor_decay muted trails). Effective mid energy:
//!   0.242 (the sweet spot between C's 0.174 and original-v30's 0.334).
//!   — *Superseded by differential depth tuning.*
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
pub(crate) const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
///
/// Back layer moves at 35% of base speed (parallax recession), front layer
/// at 170% (foreground whoosh). Mid matches base speed.
pub(crate) const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = far, 2 = near).
///
/// v30.0.0 differential depth tuning + silent override bug fix: back
/// dimmed 0.55→0.48 to push deeper into atmospheric haze; mid dimmed
/// 0.88→0.80 for slight presence reduction; front boosted 1.00→1.05
/// for more prominence. Bug fix in droplet.rs changed the brightness
/// gate from `< 1.0` to `!= 1.0` so the front boost 1.05 now actually
/// applies (was a silent no-op before).
///
/// Back effective visibility = 0.48 × 0.50 × (1−0.45) ≈ 0.132.
///   - Back  (0): 0.48 (dimmed — sits in soft fog)
///   - Mid   (1): 0.80 (slightly dim — fewer droplets, each vivid)
///   - Front (2): 1.10 (boosted — front reads as the hero layer, Option F)
pub(crate) const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.48, 0.80, 1.10];

/// Per-layer saturation multiplier (layer 0 = desaturated, 2 = full).
///
/// v30.0.0 differential + silent override bug fix: back desaturated
/// further 0.55→0.50 (more "rain in fog" feel); mid slightly
/// desaturated 0.90→0.84 to match the dimmer brightness; front pushed
/// 1.00→1.05 for richer neon. Bug fix in droplet.rs changed the
/// saturation gate from `< 1.0` to `!= 1.0` so the front boost 1.05
/// now actually applies (was a silent no-op before — oversaturation
/// pushes colors away from gray, making neon pop more).
///   - Back  (0): 0.50 (more haze blend)
///   - Mid   (1): 0.84 (slightly less vivid to match dimmer brightness)
///   - Front (2): 1.12 (richer neon — front pops as hero, Option F)
pub(crate) const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.50, 0.84, 1.12];

/// Per-layer head-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// v30.0.0 differential: back dimmed 0.55→0.48 (distant heads stay as
/// soft glow, never pop as bright pinpricks); mid reduced 0.82→0.74
/// (slightly less bloom to match lower density); front boosted 1.0→1.15
/// (more cinematic head pop, hero layer reads first).
///   - Back  (0): 0.48 (soft distant glow)
///   - Mid   (1): 0.74 (gentle pop, not noisy)
///   - Front (2): 1.30 (boosted — cinematic head pop, Option F)
pub(crate) const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.48, 0.74, 1.30];

/// Per-layer head self-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// v30.0.0 differential + silent override bug fix: back 0.45→0.38
/// (effective self-bloom ~9%, vs ~27% for front — distant heads read
/// as ambient glow); mid 0.78→0.68 (slightly less self-glow to match
/// lower density); front 1.0→1.15 (full cinematic self-bloom).
///
/// Bug fix in droplet.rs switched from `as i32` truncation (which made
/// ALL multipliers < 1.0 collapse to 0, giving 0% boost for every
/// layer) to f32 math. Now fractional multipliers actually apply:
/// back gets ~9% boost, mid gets ~16%, front gets ~27%.
///   - Back  (0): 0.38 (ambient distant glow, no pinprick)
///   - Mid   (1): 0.68 (clearly present, not flashy)
///   - Front (2): 1.20 (boosted self-glow, Option F)
pub(crate) const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.38, 0.68, 1.20];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
///
/// Back layer droplets are 50% of base length (brief streaks). Front layer
/// droplets are 140% (long cinematic rain streaks). Mid matches base.
pub(crate) const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];

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
pub(crate) const PHOSPHOR_DECAY_RATE: f32 = 5.0;

/// Energy level when a cell's tail passes (starts the phosphor glow).
///
/// At 160, trail brightness is ~63% of head — body cells clearly visible
/// as colored rain rather than dim ghosts.
pub(crate) const PHOSPHOR_TAIL_RESIDUAL: u8 = 160;

/// Below this energy, the cell is cleared to blank.
pub(crate) const PHOSPHOR_DEAD_THRESHOLD: u8 = 6;

/// Minimum phosphor energy for rendering the original character glyph in
/// ghost cells. Below this threshold, the ghost cell renders as a blank
/// space (or dim color-only patch). Prevents stale cells from filling
/// the background with dark charset glyphs.
pub(crate) const PHOSPHOR_GLYPH_THRESHOLD: u8 = 96;

/// Per-layer phosphor decay rate multiplier (far=fast, near=slow).
///
/// v30.0.0 differential: back raised 1.8→2.0 (trails fade even faster —
/// distant rain reads as brief flicker, not lingering streaks); mid
/// raised 1.0→1.2 (slightly faster fade to complement lower density —
/// fewer but cleaner streaks); front lowered 0.5→0.4 (slower fade —
/// trails linger longer for stronger cinematic presence).
///   - Back  (0): 2.0 (fast fade — brief distant flicker)
///   - Mid   (1): 1.2 (slightly faster fade — clean streaks)
///   - Front (2): 0.6 (slow fade — long cinematic trails, Option F)
pub(crate) const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [2.0, 1.2, 0.6];

/// Number of rows from the bottom of the screen where phosphor decay is
/// accelerated (prevents "concrete wall" residue buildup).
pub(crate) const PHOSPHOR_BOTTOM_ROWS: u16 = 12;

/// Phosphor decay rate multiplier applied to bottom rows (3.0× base).
pub(crate) const PHOSPHOR_BOTTOM_DECAY_MULT: f32 = 3.0;

// ─── Parallax depth layering ────────────────────────────────────────────
//
// Three independent controls stack to push the back layer into atmospheric
// depth: density (fewer spawns), contrast reduction (fg→bg blend = fog),
// and glyph dim (char-level dimming, currently 1.0 = no-op since brightness
// + saturation already cover it).

/// Per-layer spawn density multiplier (far = sparse, near = dense).
///
/// v30.0.0 silent override bug fix: front restored from 1.10 → 0.85 to
/// compensate for the spawn-roll fix (commit 9080472) that gave front
/// +40% more density rolls. At spawn_droplets distribution [0.35, 0.30,
/// 0.35] (post-fix), front effective spawn rate = 0.35 × 0.85 × col_mod
/// ≈ 0.208 — matching 5571c0b's 0.25 × 1.0 × col_mod ≈ 0.175 with a
/// slight +19% bump so front reads as more prominent (now that bug
/// fixes actually apply per-droplet boosts).
///
/// Mid reduced 0.75→0.62 (~17% fewer droplets — the primary lever for
/// reducing mid noise per user's request). Back kept at 0.45 (already
/// sparse, dimming handles the depth push instead).
///   - Back  (0): 0.45 (kept — sparse distant rain)
///   - Mid   (1): 0.62 (reduced — fewer but vivid streaks)
///   - Front (2): 0.85 (restored — sparse crisp glow, matches 5571c0b)
pub(crate) const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.62, 0.85];

/// Per-layer glyph simplicity (currently no-op — subsumed by brightness
/// + saturation). Kept as a tuning knob for future use.
pub(crate) const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];

/// Per-layer contrast reduction (depth-of-field perceptual blur).
///
/// Blends fg color toward black (background) by this fraction. The
/// terminal equivalent of DoF blur — back layer reads as "behind a
/// haze", front layer is sharp.
///
/// v30.0.0 differential: back raised 0.40→0.45 (slightly more fog
/// blend to push back deeper into atmospheric depth); mid raised
/// 0.12→0.18 (slight haze bump to complement lower density — mid
/// reads as sitting behind a thin veil); front kept at 0.0 (sharp,
/// hero layer has no haze).
///   - Back  (0): 0.55 (visible rain through soft fog, Option F)
///   - Mid   (1): 0.18 (slight veil — depth cue without milking out)
///   - Front (2): 0.0 (sharp foreground — hero pops clean)
pub(crate) const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.55, 0.18, 0.0];

// ─── Exponential trail fade & head bloom ───────────────────────────────────

/// Trail brightness exponential decay constant (higher = faster fade).
pub(crate) const TRAIL_EXPONENTIAL_K: f64 = 1.2;

/// Cap on accumulated spawn remainder per column (prevents burst spawns
/// after pause or long delta-time frames).
pub(crate) const SPAWN_REMAINDER_CAP: f32 = 4.0;

/// Cap on accumulated advance remainder per droplet (prevents position
/// jumps after pause or long delta-time frames).
pub(crate) const ADVANCE_REMAINDER_CAP: f32 = 3.0;

// ─── Warm start (initial rain seeding) ─────────────────────────────────────

/// Maximum head row for warm-start seeds (keeps initial heads near top).
pub(crate) const WARM_START_MAX_HEAD: u16 = 8;

/// Fraction of droplet pool to seed on warm start.
pub(crate) const WARM_START_SEED_FRACTION: f32 = 0.12;

/// Minimum number of warm-start seeds (prevents empty screen on tiny terms).
pub(crate) const WARM_START_SEED_MIN: usize = 3;

/// Maximum number of warm-start seeds (caps burst on huge terms).
pub(crate) const WARM_START_SEED_MAX: usize = 12;

/// Spawn debt carried into the first second after warm start (smooths
/// the transition from seed to steady-state spawn rate).
pub(crate) const WARM_START_SPAWN_DEBT: f32 = 0.5;

// ─── Glyph entry ramp (fresh droplet fade-in) ──────────────────────────────

/// Duration of the fresh-droplet brightness ramp-in (ms).
pub(crate) const GLYPH_ENTRY_RAMP_DURATION_MS: u32 = 700;

/// Minimum scale of the ramp (droplet starts at this brightness, ramps
/// to 1.0 over the duration above).
pub(crate) const GLYPH_ENTRY_RAMP_MIN_SCALE: f32 = 0.15;

// ─── Cinematic color transition ────────────────────────────────────────────

/// Maximum number of simultaneously-tracked palette slots (for
/// generation-based palette propagation during transitions).
pub(crate) const MAX_PALETTE_SLOTS: usize = 4;

/// Duration of the per-column color transition wave (ms).
pub(crate) const COLOR_TRANSITION_DURATION_MS: u16 = 300;

/// Fraction of columns initially visible during a transition (12% —
/// the rest propagate in over the duration above).
pub(crate) const COLOR_TRANSITION_INITIAL_VISIBLE_PCT: f32 = 0.12;

/// Duration of the per-column charset transition wave (ms).
pub(crate) const CHARSET_TRANSITION_DURATION_MS: u16 = 500;

/// Velocity boost applied to new-generation droplets during an active
/// transition (creates an incoming-wave feel).
pub(crate) const TRANSITION_VELOCITY_BOOST: f32 = 0.05;

/// Duration of the post-transition energy surge (sec).
pub(crate) const TRANSITION_ENERGY_DURATION_SECS: f32 = 1.5;

/// Saturation boost during the energy surge.
pub(crate) const TRANSITION_ENERGY_SATURATION_BOOST: f32 = 0.15;

/// Head glow boost during the energy surge.
pub(crate) const TRANSITION_HEAD_GLOW_BOOST: f32 = 0.2;

// ─── Droplet gravity & terminal velocity ───────────────────────────────────

/// Downward acceleration applied to droplet head position (cells/sec²).
pub(crate) const DROPLET_GRAVITY: f32 = 2.0;

/// Multiplier on base speed at which droplets stop accelerating
/// (terminal velocity).
pub(crate) const DROPLET_TERMINAL_VELOCITY_MULT: f32 = 1.8;

// ─── Cinematic startup easing ──────────────────────────────────────────────

/// Initial velocity as fraction of target velocity (3% — slow start).
pub(crate) const STARTUP_VELOCITY_FRACTION: f32 = 0.03;

/// Time constant for the startup velocity ramp (sec).
pub(crate) const STARTUP_EASE_TAU: f32 = 0.30;

// ─── Head bloom (exponential gaussian falloff) ─────────────────────────────

/// Sigma (standard deviation) of the gaussian head-bloom falloff.
pub(crate) const HEAD_BLOOM_SIGMA: f32 = 1.2;

/// Peak intensity of the head bloom glow (0.0 = none, 1.0 = full).
pub(crate) const HEAD_BLOOM_INTENSITY: f32 = 0.40;

/// Number of cells on each side of the head that receive bloom glow.
pub(crate) const HEAD_BLOOM_CELLS: u16 = 2;

// ─── Depth fog vignette (top/bottom row dim) ───────────────────────────────

/// Number of rows at top and bottom affected by depth fog.
/// v30 (visual mode): reduced from 4 → 3 per owner request — shorter
/// dimmer zones at top and bottom borders.
pub(crate) const FOG_ROWS: u16 = 3;

/// Minimum brightness factor at the extreme edge row.
/// v30 (visual mode): reduced from 0.65 → 0.45 per owner request —
/// darker top/bottom border (55% dim at extreme edge, was 35%).
pub(crate) const FOG_MIN_FACTOR: f32 = 0.45;

// ─── Cinematic CRT vignette (top & bottom edge dim) ────────────────────────

/// Height (in rows) of the CRT vignette band at top and bottom.
/// v30 (visual mode): reduced from 5 → 3 per owner request — shorter
/// dimmer zones at top and bottom borders.
pub(crate) const CRT_VIGNETTE_HEIGHT: u16 = 3;

/// Brightness factor at the extreme edge row of the CRT vignette.
///
/// ## v30.1 masterclass retune (2026-08-07)
/// The v30 value (0.5, 50% dim) compounded destructively with
/// `EDGE_FADE_TOP_MIN` (0.45) and `EDGE_FADE_BOTTOM_MIN` (0.20) — both
/// effects apply to the same top/bottom rows and their factors MULTIPLY,
/// producing compounded top brightness 0.5 × 0.45 = 0.225 (77.5% dim,
/// rain invisible) and bottom brightness 0.5 × 0.20 = 0.10 (90% dim,
/// rain invisible). The owner flagged v30 as "not happy — too aggressive".
///
/// The masterclass value 0.82 (18% dim) is calibrated against the
/// compounded brightness target of 0.50-0.55 at the top extreme (cinematic
/// but visible). At 0.82, the compounded top = 0.82 × 0.65 = 0.533 and
/// compounded bottom = 0.82 × 0.45 = 0.369 — both land in the "subtle dim,
/// rain clearly visible" zone (0.50-0.70 and 0.30-0.50 respectively per
/// the masterclass interpretation table in `docs/research/VISUAL_MODE_AUDIT.md`).
///
/// Reference points:
/// - 0.90 (pre-v30): 10% dim — barely-there, owner found too subtle
/// - 0.82 (masterclass): 18% dim — subtle CRT glow, doesn't compound destructively
/// - 0.50 (v30): 50% dim — destructive when compounded with edge fade
///
/// See `docs/research/VISUAL_MODE_AUDIT.md` for the full master audit
/// (compounding math, brightness curves, professional references).
pub(crate) const CRT_VIGNETTE_EDGE_FACTOR: f32 = 0.82;

/// Perf-pressure threshold below which the CRT vignette is skipped
/// (perf optimization — skip on slow systems).
pub(crate) const CRT_VIGNETTE_PERF_THRESHOLD: f32 = 0.5;

/// M1 (internal independent QA): phosphor decay pressure gate with hysteresis.
/// When pressure rises above `PHOSPHOR_SKIP_HIGH` (0.70), phosphor decay is
/// skipped entirely. It stays skipped until pressure drops below
/// `PHOSPHOR_SKIP_LOW` (0.50), preventing strobing when pressure fluctuates
/// around the threshold. The hysteresis gap (0.20) is wider than typical
/// run-to-run pressure noise, so the effect fades smoothly in/out rather
/// than hard-cutting.
pub(crate) const PHOSPHOR_SKIP_HIGH: f32 = 0.70;
pub(crate) const PHOSPHOR_SKIP_LOW: f32 = 0.50;

// ─── Cinematic radial vignette (edge darkening) ────────────────────────────

/// Intensity of the radial vignette (0.0 = none, 1.0 = full black at edges).
pub(crate) const VIGNETTE_INTENSITY: f32 = 0.30;

/// Inner radius (as fraction of half-screen) where vignette starts.
pub(crate) const VIGNETTE_INNER_RADIUS: f32 = 0.7;

/// Per-layer vignette multiplier (0.0 = no dimming, 1.0 = full dimming).
///
/// Front layer (2) is exempt — vignette is a depth effect that should
/// only push mid/back deeper into the background.
pub(crate) const VIGNETTE_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

// ─── Rain shadow (bottom quadratic fade-out) ───────────────────────────────

/// Percentage of screen height (from bottom) affected by rain shadow.
pub(crate) const RAIN_SHADOW_PCT: f32 = 0.15;

/// Per-layer rain shadow multiplier (front layer exempt, same as vignette).
pub(crate) const RAIN_SHADOW_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

/// Minimum brightness floor for the rain shadow quadratic. The fade curve
/// never drops below this value, even at the very last row.
///
/// ## v30.2 masterclass retune (2026-08-09)
/// Prior to v30.2, `rain_shadow_factor` faded quadratically to 0.0 (full
/// dark) at the bottom row. Compounded multiplicatively with the other
/// three dimming effects that hit the same row — `viewport_edge_fade`
/// (EDGE_FADE_BOTTOM_MIN = 0.45), `vignette_factor` (~0.71 at corners),
/// and `crt_vignette_factor` (CRT_VIGNETTE_EDGE_FACTOR = 0.82) — the
/// bottom row reached 0.08-0.11 brightness (89-92% dim) at the corners
/// of an 80x40 terminal. Rain was functionally invisible at the bottom
/// row, which is exactly the symptom the v30.1 retune (commit bfea09e)
/// was supposed to fix but could not — the v30.1 audit only modeled 2
/// of the 4 effects (CRT vignette x edge fade) and missed the rain
/// shadow + radial vignette contributions.
///
/// The 0.50 floor caps the shadow's contribution at 50% dim. Recomputing
/// the bottom-row compounded brightness with the floor in place:
///
/// ```text
/// rain_shadow_factor(line=39, lines=40)   = 0.50  (was 0.306)
/// viewport_edge_fade(line=39, lines=40)   = 0.45
/// vignette_factor(col=0, line=39, 80, 40) = 0.706 (corner)
/// crt_vignette_factor(line=39, lines=40) = 0.82
/// compounded = 0.50 * 0.45 * 0.706 * 0.82 = 0.130 (~13% brightness)
/// ```
///
/// 13% brightness is still dim, but rain is now visible (the prior
/// 0.08 = 8% was below the perceptual "rain visible" floor of ~10%).
/// The shadow's depth effect is preserved — the quadratic still
/// produces a clear top-to-bottom darkening gradient — only the
/// absolute floor changes.
///
/// Reference points:
/// - 0.00 (pre-v30.2): full quadratic to black — destructive when
///   compounded with the other 3 effects (bottom row at 8% brightness)
/// - 0.50 (v30.2 masterclass): 50% dim floor — visible depth, no
///   destruction (bottom row at 13% brightness, rain visible)
///
/// See `docs/research/VISUAL_MODE_AUDIT.md` for the full 4-effect
/// compounding model and the v30.2 retune rationale.
pub(crate) const RAIN_SHADOW_FLOOR: f32 = 0.50;

// ─── Front layer tail allocation ───────────────────────────────────────────
//
// Long front-layer droplets need proportional tails — otherwise they read
// as long head+body "lines" with invisible tails. Tail cell count is
// allocated as a percentage of droplet length, capped at a max.

/// Fraction of droplet length allocated to tail cells (45%).
pub(crate) const FRONT_LAYER_TAIL_PCT: f32 = 0.45;

/// Hard cap on tail cell count (prevents degenerate values on huge screens).
pub(crate) const FRONT_LAYER_TAIL_MAX_CELLS: u8 = 12;

/// Number of color stops used by long front-layer tails.
pub(crate) const FRONT_LAYER_MAX_TAIL_STOPS: u8 = 3;

// ─── Mouse interaction ─────────────────────────────────────────────────────

/// Radius (in columns) of the mouse hover glow.
pub(crate) const MOUSE_GLOW_RADIUS_COLS: f32 = 7.0;

/// Radius (in lines) of the mouse hover glow.
pub(crate) const MOUSE_GLOW_RADIUS_LINES: f32 = 5.0;

/// Intensity of the mouse hover glow (0.0 = disabled in default mode).
pub(crate) const MOUSE_GLOW_INTENSITY: f32 = 0.0;

/// Speed of the mouse-click flash ring expansion (cells/sec).
pub(crate) const MOUSE_FLASH_SPEED: f32 = 32.0;

/// Width of the mouse-click flash ring (in cells).
pub(crate) const MOUSE_FLASH_RING_WIDTH: f32 = 8.0;

/// Peak intensity of the mouse-click flash.
pub(crate) const MOUSE_FLASH_INTENSITY: f32 = 0.85;

/// Duration of the mouse-click flash (sec).
pub(crate) const MOUSE_FLASH_DURATION_SECS: f32 = 1.8;

/// Fraction of the primary ring intensity applied to the secondary echo ring.
pub(crate) const MOUSE_FLASH_SECONDARY_FRAC: f32 = 0.45;

/// Speed of the secondary ring as fraction of primary ring speed.
pub(crate) const MOUSE_FLASH_SECONDARY_SPEED_FRAC: f32 = 0.4;

/// Maximum simultaneous mouse-click flash waves (click pool size).
///
/// v30 fix: bounded pool so rapid double/triple-clicks spawn new waves
/// without resetting in-flight waves to zero. The previous single-slot
/// design (`flash_time: Option<Instant>`) silently overwrote the timer on
/// every click, restarting any in-flight wave from zero. With this pool,
/// up to `MOUSE_FLASH_POOL_SIZE` waves can coexist; the (POOL_SIZE+1)-th
/// click evicts the OLDEST active wave (smallest `birth`).
pub(crate) const MOUSE_FLASH_POOL_SIZE: usize = 4;

// ─── Velocity turbulence ───────────────────────────────────────────────────

/// Maximum velocity perturbation as fraction of base chars_per_sec.
pub(crate) const TURBULENCE_AMPLITUDE: f32 = 0.08;

/// Turbulence oscillation frequency (Hz).
pub(crate) const TURBULENCE_FREQ: f32 = 0.4;

// ─── Cinematic perceived smoothness ────────────────────────────────────────
//
// These constants control the per-frame "alive" feel — fractional head
// brightness pulses, head character shimmer, and spawn phase jitter break
// the robotic frame-locked cadence.

/// Fractional head brightness amplitude (head brightens up to 15% as it
/// approaches the next row).
pub(crate) const FRACTIONAL_HEAD_BRIGHTNESS_AMP: f32 = 0.15;

/// Fractional bloom modulation (bloom glow intensifies up to 10%).
pub(crate) const FRACTIONAL_BLOOM_AMP: f32 = 0.10;

/// Head character shimmer period (sec) — head cycles to a new char from
/// the pool at this interval.
pub(crate) const HEAD_SHIMMER_PERIOD_SECS: f32 = 0.10;

/// Whether to add random fractional phase offset when spawning (breaks
/// the synchronized "robotic march").
pub(crate) const SPAWN_PHASE_JITTER: bool = true;

/// Probability per frame that a mid-trail cell re-randomizes its
/// character (subtle "churn").
pub(crate) const TRAIL_CYCLE_PROBABILITY: f32 = 0.02;

// ─── Rare anomaly events ───────────────────────────────────────────────────

/// Chance per second of an anomaly event firing.
pub(crate) const ANOMALY_CHANCE_PER_SEC: f64 = 0.017;

/// Duration of an anomaly event (sec).
pub(crate) const ANOMALY_DURATION_SECS: f32 = 1.5;

/// Maximum simultaneously-active anomaly zones.
pub(crate) const ANOMALY_MAX_ZONES: usize = 3;

/// Luminance intensity boost during an anomaly.
pub(crate) const ANOMALY_LUMINANCE_INTENSITY: f32 = 0.3;

/// Chance that an anomaly corrupts (re-randomizes) trail characters.
pub(crate) const ANOMALY_CORRUPTION_CHANCE: f32 = 0.4;

// ─── Temporal color ecosystems ─────────────────────────────────────────────

/// Tick interval for color ecosystem evaluation (sec).
pub(crate) const COLOR_ECOSYSTEM_TICK_SECS: f32 = 3.0;

/// Climate luminance drift rate per tick.
pub(crate) const COLOR_CLIMATE_DRIFT_RATE: f32 = 0.008;

/// Saturation drift rate per tick.
pub(crate) const COLOR_SATURATION_DRIFT_RATE: f32 = 0.005;

/// Hue drift rate per tick.
pub(crate) const COLOR_HUE_DRIFT_RATE: f32 = 0.015;

/// Chance per tick of re-evaluating drift direction.
pub(crate) const COLOR_DRIFT_REEVAL_CHANCE: f32 = 0.15;

/// Minimum climate luminance multiplier.
pub(crate) const COLOR_LUMINANCE_CLIMATE_MIN: f32 = 0.75;

/// Maximum climate luminance multiplier.
pub(crate) const COLOR_LUMINANCE_CLIMATE_MAX: f32 = 1.0;

/// Minimum climate saturation multiplier.
pub(crate) const COLOR_SATURATION_CLIMATE_MIN: f32 = 0.7;

/// Maximum climate saturation multiplier.
pub(crate) const COLOR_SATURATION_CLIMATE_MAX: f32 = 1.0;

/// Chance per tick of an autonomous palette drift event.
pub(crate) const AUTONOMOUS_PALETTE_DRIFT_CHANCE: f32 = 0.03;

/// Phase D Bug #7 fix: minimum cooldown seconds between palette drift
/// events. Without this, drift can oscillate Green↔Green2 on consecutive
/// ticks (3% chance per tick × 2 schemes = 1.5% chance of immediate
/// reversal). Each transition triggers `apply_new_palette` (palette
/// rebuild + color_map regen + force_draw_everything) — expensive + visually
/// jarring. The cooldown prevents rapid oscillation while preserving the
/// long-term drift behavior. At 30s cooldown, drift fires at most once per
/// 10 ticks (3-sec tick × 10 = 30s), which is still frequent enough to be
/// noticeable over a 10-minute session.
pub(crate) const PALETTE_DRIFT_COOLDOWN_SECS: f32 = 30.0;

/// Whether autonomous color drift is enabled by default.
pub(crate) const AUTO_COLOR_DRIFT_DEFAULT: bool = false;

// ─── Cinematic runtime behavior profiles ───────────────────────────────────

/// Duration of the profile transition (sec).
pub(crate) const PROFILE_TRANSITION_SECS: f32 = 30.0;

/// Interpolation rate for profile parameter changes.
pub(crate) const PROFILE_INTERPOLATION_RATE: f32 = 0.02;

// ─── Autonomous atmospheric evolution ──────────────────────────────────────

/// Tick interval for atmospheric evolution (sec).
pub(crate) const ATMOSPHERE_TICK_SECS: f32 = 5.0;

/// Cycle period for entropy buildup and release (sec).
pub(crate) const ENTROPY_CYCLE_SECS: f32 = 300.0;

/// Range of density variation during atmospheric evolution.
pub(crate) const ATMOSPHERE_DENSITY_RANGE: f32 = 0.4;

/// Range of luminance variation during atmospheric evolution.
pub(crate) const ATMOSPHERE_LUMINANCE_RANGE: f32 = 0.2;

/// Range of anomaly probability variation during atmospheric evolution.
pub(crate) const ATMOSPHERE_ANOMALY_RANGE: f32 = 0.5;

// ─── Living rain: dynamic density noise ────────────────────────────────────
//
// Each column has a spatial density modifier in [MIN, MAX] that re-rolls
// every PERIOD_SECS. Kills the "uniform grid" feel without per-frame
// allocation — single O(1) hash per spawn.

/// Period at which the density noise field re-rolls (sec).
pub(crate) const DENSITY_NOISE_PERIOD_SECS: f64 = 10.0;

/// Minimum density noise modifier.
pub(crate) const DENSITY_NOISE_MIN: f32 = 0.6;

/// Maximum density noise modifier.
pub(crate) const DENSITY_NOISE_MAX: f32 = 1.4;

/// Hash multiplier for density noise (Knuth-style prime).
pub(crate) const DENSITY_NOISE_HASH_K: u32 = 2_654_435_761;

/// Hash seed multiplier for density noise.
pub(crate) const DENSITY_NOISE_HASH_SEED_K: u32 = 1_103_515_245;

// ─── Living rain: wind gusts ───────────────────────────────────────────────
//
// Gusts are an envelope: idle → attack → hold → decay → idle. Each phase
// has min/max duration; the actual duration is rolled uniformly. The peak
// multiplier scales droplet speed during the hold phase.

/// Idle phase min duration (sec) — time between gusts.
pub(crate) const GUST_IDLE_MIN_SECS: f64 = 30.0;

/// Idle phase max duration (sec).
pub(crate) const GUST_IDLE_MAX_SECS: f64 = 120.0;

/// Attack phase min duration (sec) — ramp-up time.
pub(crate) const GUST_ATTACK_MIN_SECS: f64 = 1.0;

/// Attack phase max duration (sec).
pub(crate) const GUST_ATTACK_MAX_SECS: f64 = 2.0;

/// Hold phase min duration (sec) — peak sustain time.
pub(crate) const GUST_HOLD_MIN_SECS: f64 = 0.5;

/// Hold phase max duration (sec).
pub(crate) const GUST_HOLD_MAX_SECS: f64 = 1.0;

/// Decay phase min duration (sec) — ramp-down time.
pub(crate) const GUST_DECAY_MIN_SECS: f64 = 3.0;

/// Decay phase max duration (sec).
pub(crate) const GUST_DECAY_MAX_SECS: f64 = 5.0;

/// Min peak speed multiplier during the hold phase.
pub(crate) const GUST_PEAK_MIN: f32 = 1.2;

/// Max peak speed multiplier during the hold phase.
pub(crate) const GUST_PEAK_MAX: f32 = 1.5;

// ─── Long-timescale renderer memory ────────────────────────────────────────
//
// The renderer samples recent state to detect anomalies and persistence
// patterns. These constants control the sampling cadence and how much
// weight is given to recent anomaly pressure.

/// Number of historical samples retained in renderer memory.
pub(crate) const MEMORY_HISTORY_SAMPLES: usize = 32;

/// Interval between memory samples (sec).
pub(crate) const MEMORY_SAMPLE_INTERVAL_SECS: f32 = 30.0;

/// Weight given to anomaly pressure in memory scoring.
pub(crate) const MEMORY_ANOMALY_PRESSURE_WEIGHT: f32 = 0.3;

/// Persistence boost applied during calm periods (rewards sustained calm).
pub(crate) const MEMORY_CALM_PERSISTENCE_BOOST: f32 = 0.15;

// ─── Emergent visual storytelling ──────────────────────────────────────────

/// Tick interval for emergent moment evaluation (sec).
pub(crate) const STORYTELLING_TICK_SECS: f32 = 10.0;

/// Chance per tick of an emergent moment firing.
pub(crate) const EMERGENT_MOMENT_CHANCE: f32 = 0.08;

/// Duration of an emergent moment (sec).
pub(crate) const EMERGENT_MOMENT_DURATION_SECS: f32 = 8.0;

/// Maximum simultaneously-active emergent moments.
pub(crate) const EMERGENT_MAX_MOMENTS: usize = 1;

/// Luminance intensity boost during an emergent moment.
pub(crate) const EMERGENT_LUMINANCE_INTENSITY: f32 = 0.12;

/// Density intensity boost during an emergent moment.
pub(crate) const EMERGENT_DENSITY_INTENSITY: f32 = 0.25;

/// Speed shift during an emergent moment (additive).
pub(crate) const EMERGENT_SPEED_SHIFT: f32 = 0.15;

// ─── Cinematic resume easing (pause → resume transition) ───────────────────

/// Duration of the resume easing ramp (sec). During this window, all
/// simulation parameters are scaled by a smootherstep curve from the
/// paused-state value to 1.0.
pub(crate) const RESUME_EASE_DURATION_SECS: f32 = 0.45;

/// Duration of the pause easing ramp (sec). Scales simulation parameters
/// from 1.0 down to the paused floor.
pub(crate) const PAUSE_EASE_DURATION_SECS: f32 = 0.30;

// ─── Viewport edge fade ────────────────────────────────────────────────────

/// Number of rows at the top affected by edge fade (smooth entry/exit
/// at terminal border).
/// v30 (visual mode): reduced from 3 → 2 per owner request — shorter
/// top dimmer zone.
pub(crate) const EDGE_FADE_ROWS: u16 = 2;

/// Number of rows at the bottom affected by edge fade.
/// v30 (visual mode): reduced from 12 → 8 per owner request — shorter
/// bottom dimmer zone. Must stay >= EDGE_FADE_ROWS for the Zone-1
/// pre-fade math to make sense.
/// v30.1 masterclass retune (2026-08-07): widened from 8 → 10 for a
/// smoother dissolve ramp — the v30 8-row zone produced a slightly
/// abrupt transition where the gentle pre-fade met the sharp lip.
/// 10 rows gives the smoothstep more room to ease in. See
/// `docs/research/VISUAL_MODE_AUDIT.md`.
pub(crate) const EDGE_FADE_BOTTOM_ROWS: u16 = 10;

/// Lip factor for the bottom edge fade (controls curvature).
/// v30.1 masterclass retune: lowered from 0.75 → 0.72 — slightly
/// smoother transition between Zone 1 (gentle pre-fade) and Zone 2
/// (sharp lip). The 0.03 reduction is barely perceptible on its own
/// but produces a more film-like dissolve when combined with the
/// widened EDGE_FADE_BOTTOM_ROWS.
pub(crate) const EDGE_FADE_BOTTOM_LIP: f32 = 0.72;

/// Minimum brightness factor at the top edge.
///
/// ## v30.1 masterclass retune (2026-08-07)
/// The v30 value (0.45, 55% dim) compounded destructively with
/// `CRT_VIGNETTE_EDGE_FACTOR` (0.5) — both effects apply to the top
/// rows and multiply, producing compounded top brightness 0.5 × 0.45 =
/// 0.225 (77.5% dim, rain invisible). The owner flagged v30 as "not
/// happy — too aggressive".
///
/// The masterclass value 0.65 (35% dim) is calibrated so the COMPOUNDED
/// top brightness = 0.82 × 0.65 = 0.533 lands in the "subtle dim, rain
/// clearly visible" zone (0.50-0.70). At 0.65, the top row rain is
/// clearly readable but visibly dimmer than the mid rows — the eye
/// reads it as "rain entering from above the screen", not as a dead
/// black bar.
///
/// Reference points:
/// - 0.70 (pre-v30): 30% dim — barely-there, owner found too subtle
/// - 0.65 (masterclass): 35% dim — visible cinematic fade-in
/// - 0.45 (v30): 55% dim — destructive when compounded with vignette
///
/// See `docs/research/VISUAL_MODE_AUDIT.md` for the compounding math.
pub(crate) const EDGE_FADE_TOP_MIN: f32 = 0.65;

/// Minimum brightness factor at the bottom edge.
///
/// ## v30.1 masterclass retune (2026-08-07)
/// The v30 value (0.20, 80% dim) compounded destructively with
/// `CRT_VIGNETTE_EDGE_FACTOR` (0.5) — both effects apply to the bottom
/// rows and multiply, producing compounded bottom brightness
/// 0.5 × 0.20 = 0.10 (90% dim, rain invisible). The owner flagged v30
/// as "not happy — too aggressive".
///
/// The bottom edge fade exists primarily to prevent the phosphor ghost
/// residue artifact where dying droplet heads "burn into" the bottom
/// row. It needs to be MORE aggressive than the top (asymmetric, per
/// the `viewport_edge_fade` doc), but v30 went too far. The masterclass
/// value 0.45 (55% dim) is calibrated so:
/// - Compounded bottom brightness = 0.82 × 0.45 = 0.369 — rain dissolves
///   into shadow but is NOT destroyed (zone 0.30-0.50, "cinematic dim").
/// - Still more aggressive than the top (0.65) — asymmetric fade
///   preserved, phosphor residue still prevented.
/// - Midpoint between pre-v30 (0.35) and v30 (0.20), leaning toward
///   v30's intent of "more aggressive bottom" but stopping short of
///   destruction.
///
/// Reference points:
/// - 0.35 (pre-v30): 65% dim — owner wanted more aggressive
/// - 0.45 (masterclass): 55% dim — dissolving, not destroyed
/// - 0.20 (v30): 80% dim — destructive when compounded with vignette
///
/// See `docs/research/VISUAL_MODE_AUDIT.md` for the compounding math.
pub(crate) const EDGE_FADE_BOTTOM_MIN: f32 = 0.45;

/// Brightness threshold below which bold attribute is suppressed at edges.
pub(crate) const EDGE_FADE_BOLD_THRESHOLD: f32 = 0.5;

/// Maximum phosphor energy at the viewport edge (caps edge glow).
pub(crate) const PHOSPHOR_EDGE_ENERGY_CAP: u8 = 64;

/// Taper rate (in rows) for phosphor energy at the viewport edge.
pub(crate) const PHOSPHOR_EDGE_ROW_TAPER: u8 = 8;

// ─── Cinematic Event Engine ──────────────────────────────────────────────

/// XOR mask applied to the event RNG seed (deterministic per-session).
pub(crate) const EVENT_RNG_XOR: u64 = 0xCAFE_BABE_1337_0420;

/// Perf-pressure gate below which atmospheric events are skipped.
pub(crate) const EVENT_PERF_GATE: f32 = 0.5;

// ─── Phosphor Ghost ────────────────────────────────────────────────────────

/// Chance per tick of a phosphor ghost spawning.
pub(crate) const GHOST_SPAWN_CHANCE_PER_TICK: f64 = 0.003;

/// Maximum simultaneously-active phosphor ghosts.
pub(crate) const GHOST_MAX_ACTIVE: usize = 1;

// ─── Cloud internals ───────────────────────────────────────────────────────
//
// These tune the droplet pool sizing and RNG behavior — affect rain
// density and continuity at the structural level.

/// Multiplier on (cols × density) for droplet pool sizing.
pub(crate) const DROPLET_COUNT_FACTOR: f32 = 1.5;

/// Minimum droplet length (cells) — guarantees a recognizable head→body→tail.
pub(crate) const MIN_DROPLET_LENGTH: u16 = 4;

/// Maximum droplet length cap (cells) — prevents degenerate values on
/// huge screens (8K UHD bench = 4320 lines).
pub(crate) const MAX_DROPLET_LENGTH_CAP: u16 = 200;

/// Size of the per-column character pool (pre-allocated, no per-spawn alloc).
pub(crate) const CHAR_POOL_SIZE: usize = 2048;

/// Size of the per-column glitch character pool.
pub(crate) const GLITCH_POOL_SIZE: usize = 1024;

/// Maximum character pool index (CHAR_POOL_SIZE - 1, used for fast mod).
pub(crate) const MAX_CHAR_POOL_IDX: u16 = 2047;

/// Interval at which the RNG is reseeded from system entropy (sec).
pub(crate) const RNG_RESEED_INTERVAL_SECS: u64 = 600;

/// Initial RNG seed (deterministic per-session, reseeded every interval above).
pub(crate) const RNG_INITIAL_SEED: u64 = 0x0123_4567;

/// Duration that a droplet's head lingers at peak brightness after
/// advancing (ms).
pub(crate) const HEAD_LINGER_BRIGHTNESS_MS: u64 = 300;

/// Interval between full-redraw forced refreshes (frames). Prevents
/// drift accumulation in long-running sessions.
pub(crate) const FULL_REDRAW_INTERVAL_FRAMES: u64 = 18000;

// ─── Performance tuning (rain-affecting subset) ────────────────────────────

/// Spawn-scale floor under perf pressure (don't go below 25% of target).
pub(crate) const PERF_SPAWN_SCALE_MIN: f32 = 0.25;

/// AB-11 (dragon power audit, option 2): aggressive spawn-scale floor used
/// when the self-healer has detected sustained high CPU pressure. Lower
/// floor (10% vs 25%) allows the engine to shed more load and recover
/// without touching the user's color/charset/density/speed/glitch_level.
pub(crate) const PERF_SPAWN_SCALE_MIN_AGGRESSIVE: f32 = 0.10;

/// Glitch activation threshold (fraction of cells).
pub(crate) const GLITCH_THRESHOLD: f32 = 0.35;

/// Ratio of the glitch bright phase to total glitch duration.
pub(crate) const GLITCH_BRIGHT_RATIO: f64 = 0.25;

/// Ratio of the glitch dim phase to total glitch duration.
pub(crate) const GLITCH_DIM_RATIO: f64 = 0.75;

/// Simulation pressure scaling factor (reduces sim load under pressure).
pub(crate) const SIM_PRESSURE_SCALE_FACTOR: f64 = 0.7;

/// Minimum simulation fraction (don't go below 50% of target).
pub(crate) const SIM_MIN_FRACTION: f64 = 0.5;

/// Maximum simulation delta-time cap (sec) — prevents huge sim steps
/// after pause or stall.
pub(crate) const SIM_MAX_CAP_SECS: f64 = 1.0 / 30.0;

/// Base multiplier for simulation step sizing.
pub(crate) const SIM_BASE_MULTIPLIER: f64 = 3.0;

/// Density step granularity for CLI/runtime density adjustments.
pub(crate) const DENSITY_STEP: f32 = 0.25;

/// Watchdog interval (sec) — checks for stuck droplets / state drift.
pub(crate) const WATCHDOG_INTERVAL_SECS: u64 = 1;

// ─── Frame timing budget (rain-affecting subset) ───────────────────────────

/// Frame spin budget — time the event loop will busy-wait before yielding.
pub(crate) const FRAME_SPIN_BUDGET: Duration = Duration::from_micros(500);

/// Frame spin limit — maximum time the event loop will busy-wait.
pub(crate) const FRAME_SPIN_LIMIT: Duration = Duration::from_micros(1000);

/// Minimum simulation factor under heavy load.
pub(crate) const SIM_FACTOR_MIN: f64 = 0.3;

// ─── Monolith scene — per-layer tuning ─────────────────────────────────────
//
// The Monolith scene (cloud/monolith.rs + cinematic.rs) has its own
// per-layer brightness and breathing multipliers that track the rain
// field's depth gradient. Previously these were hardcoded in monolith.rs
// and cinematic.rs as match arms — every rain parameter change required
// hunting down the matching hardcoded values across multiple files.
//
// Centralized here so editing any rain parameter requires touching only
// this single file. monolith.rs and cinematic.rs read from these
// constants directly.

/// Per-layer brightness multiplier for the Monolith scene glyph streams.
///
/// Tracks the rain field's visibility floor (PARALLAX_BRIGHTNESS_MULT).
/// Mid is set slightly under the rain's mid value so monolith glyph
/// streams read as half-a-step behind the rain front, preserving depth
/// cue without the rain "disappearing" behind a too-dim monolith. Back
/// matches the rain back value so the monolith's distant body sits in
/// the same atmospheric haze as the distant rain. Front kept at 1.0 —
/// the monolith hero pulse stays the brightest glyph element (front
/// rain at 1.05 is still slightly brighter per-cell but the monolith's
/// solid glyph mass keeps it visually dominant as the focal anchor).
///   - Back  (0): 0.48 (matches rain back — atmospheric haze)
///   - Mid   (1): 0.78 (half-step under rain mid 0.80 — depth cue)
///   - Front (2): 1.00 (monolith hero pulse — focal anchor)
pub(crate) const MONOLITH_LAYER_BRIGHTNESS: [f32; PARALLAX_LAYERS] = [0.48, 0.78, 1.0];

/// Per-layer breathing amplitude for the Monolith scene's subtle
/// triangle-wave brightness oscillation (cinematic.rs).
///
/// Back layer breathes slowest (0.018 amplitude = ±1.8% brightness
/// oscillation), front breathes most (0.034 = ±3.4%) — gives the
/// monolith wall a "living tissue" feel where the foreground pulses
/// more visibly than the distant background, reinforcing depth.
///   - Back  (0): 0.018 (±1.8% — subtle distant breath)
///   - Mid   (1): 0.026 (±2.6% — moderate)
///   - Front (2): 0.034 (±3.4% — visible foreground pulse)
pub(crate) const MONOLITH_BREATHING_AMPLITUDE: [f32; PARALLAX_LAYERS] = [0.018, 0.026, 0.034];
