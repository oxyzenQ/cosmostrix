// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Central Control — Rains
//!
//! Single source of truth for every rain, parallax, phosphor, and
//! depth-layer tuning parameter. This is the plug-and-play control
//! file for the entire rain visual stack — modeled after
//! `chroma/catalog.rs` for color themes.
//!
//! ## Scope
//!
//! Every constant in this file directly controls how the rain looks.
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
//! | Head self-bloom base     | Base white-blend for head self-bloom intensity      |
//! | Monolith structure       | Core white-blend, boost cap, stream geometry        |
//!
//! ## Easing family policy (v50.0.0-beta.5 masterclass consolidation)
//!
//! All temporal easing in the rain simulation uses exponential
//! decay (`blend = exp(-kt)` for decel, `blend = 1 - exp(-kt)` for
//! accel) — physically motivated drag with a long tail, snap-settled at
//! a threshold to avoid the asymptotic tail. This is the unified
//! "masterclass" easing family across:
//!
//! - Pause decel (`PAUSE_EASE_DECAY_RATE` = 1.2/s, settle 5% @ ~2.5s)
//! - Resume accel (`RESUME_EASE_DECAY_RATE` = 0.9/s, settle 95% @ ~3.3s)
//! - Glyph entry ramp (`GLYPH_ENTRY_RAMP_DECAY_RATE` = 4.28/s, settle 95% @ ~700ms)
//!
//! Asymmetric k_decel > k_resume (1.2 vs 0.9) preserves the prior
//! 0.30s/0.45s "pause snappy / resume wake-up" feel. Glyph entry uses
//! a much higher k (4.28) because the 700ms settle target is much
//! shorter than pause/resume's multi-second windows.
//!
//! ### What is NOT exp decay (intentionally)
//!
//! - Spatial fades (edge fade, vignette, brightness bands in
//!   `cloud/brightness_factors.rs`, `cloud/rain_post.rs`) use
//!   smoothstep (3t^2-2t^3, C1). These are position-based, not
//!   time-based — the "blend" parameter is a cell's row/col, not
//!   elapsed time. Smoothstep's bounded [0,1] domain is correct for
//!   spatial gradients; exp decay would be inappropriate (the cell at
//!   row 0 has factor 0 regardless of "elapsed time").
//!
//! - Profile interpolation (cloud/rain.rs:1024, 30s morph) uses a
//!   smoothstep-shaped per-frame lerp rate that ramps from
//!   PROFILE_INTERPOLATION_RATE (0.02) to 1.0 over PROFILE_TRANSITION_SECS
//!   (30s). This is a "slow drift then accelerate then snap" feel —
//!   intentionally different from exp approach's "fast start then settle"
//!   feel. Profile morphs are 30s atmospheric transitions, not motion
//!   easing, so the different family is correct.
//!
//! - Chroma color transition falloff (chroma shaders/transition/
//!   mod.rs:288) uses linear falloff for a 3-row spatial window.
//!   Smoothstep was deliberately rejected as "overkill for 3 lines".
//!
//! - Intro logo Phase 3 fade (intro_style/logo.rs, phase-3
//!   `base_brightness`)
//!   uses smoothstep (1 - 3t^2+2t^3). This is intro animation, not
//!   pause/resume lifecycle — outside the easing consolidation scope.
//!
//! ### Why exp decay for the temporal paths
//!
//! - Long tail gives genuine inertia coast-down (no abrupt end-snap).
//! - Physically motivated (drag = real-world motion model).
//! - Matches the README's "exponential deceleration (~3s coast-down)"
//!   promise (which was stale under smootherstep).
//! - Settle-snap at threshold trades asymptotic smoothness for clean
//!   terminal state — required so other subsystems (spawn_remainder
//!   reset, monolith stream shift, phosphor LUT) see unambiguous
//!   state transitions.
//! - exp() is already used in the cosmic locked path
//!   (`cloud/phosphor.rs:307` LUT build) and chroma shaders/base LUT
//!   (`shaders/base/mod.rs:237`), so no new math primitive is
//!   introduced.
//!
//! ## Calibration history (most recent first)
//!
//! - (peak masterclass cinematic lock + stabilization): visual
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
//! - (silent override bug fix + centralization): user reported
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
//! - (differential depth tuning): user requested sharper
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
//! - (final — visual test locked): after A/B visual testing
//!   against option C (density-focused) and option D (haze-focused), the
//!   parameter set from commit 1e4e3fa (the initial visibility-floor
//!   raise) was confirmed as the optimal balance. Reverted option C's
//!   two mid tweaks: density 0.55 → 0.75, contrast_reduction 0.15 → 0.12.
//!   Mid layer now reads as vivid individual streaks at natural density —
//!   neither too sparse (C's 0.55 made the field feel empty) nor too
//!   hazy (D's 1.3 phosphor_decay muted trails). Effective mid energy:
//!   0.242 (the sweet spot between C's 0.174 and original-v30's 0.334).
//!   — Superseded by differential depth tuning.
//! - v30 (option C — density-focused): mid layer density dropped
//!   from 0.75 → 0.55 to remove noise via fewer droplets rather than
//!   dimming. Reverted Option D's mid rollbacks (head_bloom 0.70 → 0.82,
//!   phosphor_decay 1.3 → 1.0, contrast_red 0.25 → 0.12) since the
//!   density drop alone is sufficient. Slight contrast_red bump to 0.15
//!   keeps a hint of haze. Effective mid energy: 0.174 (vs D's 0.187,
//!   pre-v30's 0.109, original v30's 0.334). — Reverted by final lock.
//! - v30 (option D — haze-focused): rolled back mid head-bloom,
//!   raised mid contrast reduction + phosphor decay. Worked but user
//!   preferred density-focused approach. — Reverted by option C.
//! - v30 (initial raise): lifted back + mid visibility floor to
//!   fix "too quiet/dim darkness" complaint. Effective back visibility
//!   raised 2.5x; effective mid raised 3.06x. — *Confirmed optimal by
//!   final visual lock; restored as baseline.*

use std::time::Duration;

// v50.0.0-beta.7 LOC refactor: parallax + atmosphere + events sections
// extracted to sibling files to keep mod.rs under the 800-LOC hard cap.
// Re-exported here so all existing 'crate::central_control_rains::*' +
// 'use crate::constants::*' (which re-exports this module) call sites
// continue to resolve unchanged.
mod atmosphere;
mod density_throttle;
mod events;
mod parallax;
pub(crate) use atmosphere::*;
pub(crate) use density_throttle::*;
pub(crate) use events::*;
pub(crate) use parallax::*;

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

// ─── Glyph entry ramp (fresh-droplet fade-in, exp approach) ───────────────
//
// Masterclass: glyph entry ramp uses exp approach
// (`blend = 1 - exp(-k*t)`) — consistent with the pause/resume easing
// family. Replaces the prior smoothstep S-curve (3t^2 - 2t^3) over a
// fixed 700ms window. The exp approach gives an instant cascade that
// asymptotes to full speed (cinematic "top-entry cascade" feel), then
// snaps at the settle threshold for clean state transitions.
//
// Derived constants: GLYPH_ENTRY_RAMP_DURATION_MS (700ms) is now the
// expected SETTLE time (when blend reaches SETTLE_FRAC = 95%), not a
// fixed animation window. k = -ln(1 - 0.95) / 0.7 = 4.28/sec.

/// Settle time for the glyph entry ramp (ms). At k =
/// `GLYPH_ENTRY_RAMP_DECAY_RATE` (4.28/s), the blend reaches
/// `GLYPH_ENTRY_RAMP_SETTLE_FRAC` (95%) at this elapsed time, then
/// snaps to 1.0. Documents the prior 700ms smoothstep window's
/// perceived duration - preserved as the new settle target. Used by
/// the regression test that verifies k was derived correctly from
/// this settle time + threshold.
#[allow(dead_code)] // referenced by tests + doc-comments only
pub(crate) const GLYPH_ENTRY_RAMP_DURATION_MS: u32 = 700;

/// Per-second exp approach rate for the glyph entry ramp. Derived:
/// k = -ln(1 - 0.95) / (700/1000) = 4.28/s. Set so that the blend
/// reaches 95% at the documented 700ms settle time.
pub(crate) const GLYPH_ENTRY_RAMP_DECAY_RATE: f32 = 4.28;

/// Settle threshold for the glyph entry ramp - when the blend rises
/// above this, snap to full speed (1.0) and clear `glyph_entry_time`.
/// Symmetric with `RESUME_EASE_SETTLE_FRAC` (95%).
pub(crate) const GLYPH_ENTRY_RAMP_SETTLE_FRAC: f32 = 0.95;

/// Minimum scale of the ramp (droplet starts at this brightness, ramps
/// to 1.0 over the duration above). Preserved from prior smoothstep
/// implementation - the exp approach interpolates from this floor to
/// 1.0 in the same way.
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

/// Per-column diagonal stagger for the color + charset transition waves
/// (S-master-HUNT-15). Each column's wave arrives `STAGGER_PER_COL` rows
/// later than the previous column, creating a diagonal sweep (top-left
/// converts first, bottom-right converts last) on top of the existing
/// vertical smoothstep sweep. The stagger is capped at
/// `STAGGER_MAX_FRAC * lines` so wide terminals (200+ cols) don't
/// produce a stagger larger than the screen — the cap ensures the
/// transition still completes within the duration window.
pub(crate) const WAVE_DIAGONAL_STAGGER_PER_COL: f32 = 0.15;

/// Maximum stagger as a fraction of lines (30% — the rightmost column
/// converts at most 30% of lines later than the leftmost). Keeps the
/// diagonal subtle enough to not delay the transition completion
/// (the rain_at.rs completion logic clears transition_start after
/// DURATION_MS regardless, but the VISUAL stagger should stay within
/// the sweep window for a clean look).
pub(crate) const WAVE_DIAGONAL_STAGGER_MAX_FRAC: f32 = 0.30;

/// Velocity boost applied to new-generation droplets during an active
/// transition (creates an incoming-wave feel).
pub(crate) const TRANSITION_VELOCITY_BOOST: f32 = 0.05;

/// Duration of the post-transition energy surge (sec).
pub(crate) const TRANSITION_ENERGY_DURATION_SECS: f32 = 1.5;

/// Saturation boost during the energy surge.
pub(crate) const TRANSITION_ENERGY_SATURATION_BOOST: f32 = 0.25;

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
///
/// Deep Focus preset: 0.36 — glare-controlled bloom for
/// cinematic head glow (battle round 2 champion, locked 2026-08-23).
/// See `docs/VISUAL_IDENTITY.md`.
pub(crate) const HEAD_BLOOM_INTENSITY: f32 = 0.36;

/// Number of cells on each side of the head that receive bloom glow.
pub(crate) const HEAD_BLOOM_CELLS: u16 = 2;

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
pub(crate) const MOUSE_GLOW_INTENSITY: f32 = 0.25;

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

// ─── Viewport edge fade ────────────────────────────────────────────────────

/// Number of rows at the top affected by edge fade (smooth entry/exit
/// at terminal border).
/// v30 (visual mode): reduced from 3 → 2 per owner request — shorter
/// top dimmer zone.
pub(crate) const EDGE_FADE_ROWS: u16 = 2;

/// Number of rows at the bottom affected by edge fade.
///
/// Deep Focus preset: 12 — wider dissolve zone (battle round 2
/// champion). Gentle cinematic exit.
/// Soft fade — rain trails off into shadow at the bottom edge.
pub(crate) const EDGE_FADE_BOTTOM_ROWS: u16 = 12;

/// Lip factor for the bottom edge fade (controls curvature).
///
/// Deep Focus preset: 0.82 — smoother junction (battle round 2
/// champion).
/// The Zone 1↔Zone 2 junction creates a gentle curve,
/// fading perceptibly in the last rows — deep-focus dissolve.
pub(crate) const EDGE_FADE_BOTTOM_LIP: f32 = 0.82;

/// Minimum brightness factor at the top edge.
///
/// Deep Focus preset: 0.48 (52% dim) — gentler dark entry than
/// noir (battle round 2 champion, locked 2026-08-23). Rain fades
/// in from shadow at the top border — deep-focus fade-in. Rain
/// emerges from darkness.
///
/// Reference points:
/// - 0.48 (Deep Focus): 52% dim — gentler than noir entry
/// - 0.45 (Cinema Noir, superseded): 55% dim — dramatic noir entry
/// - 0.65 (v50 alpha.2): 35% dim — visible cinematic fade-in
///
/// See `docs/VISUAL_IDENTITY.md` for the preset lineage and
/// `docs/research/VISUAL_MODE_AUDIT.md` for the compounding math.
pub(crate) const EDGE_FADE_TOP_MIN: f32 = 0.48;

/// Minimum brightness factor at the bottom edge.
///
/// Deep Focus preset: 0.68 (32% dim) — later dissolve than noir
/// (battle round 2 champion). Rain fades softly toward the bottom.
/// The moderate dim prevents harsh cutoff while keeping ghost
/// residue manageable — deep-focus aesthetic.
///
/// Reference points:
/// - 0.68 (Deep Focus): 32% dim — later dissolve
/// - 0.65 (Cinema Noir, superseded): 35% dim — gentle dissolve
/// - 0.45 (masterclass): 55% dim — calibrated when fog was active
///
/// See `docs/VISUAL_IDENTITY.md` for the preset lineage and
/// `docs/research/VISUAL_MODE_AUDIT.md` for the compounding math.
pub(crate) const EDGE_FADE_BOTTOM_MIN: f32 = 0.68;

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
//
// v80.0.0-beta.1: the adaptive density throttle (banded spawn-scale curve) moved
// to `density_throttle.rs` — same section family, extracted to keep this
// file under the 800-LOC cap.

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

// ─── Head self-bloom base intensity ───────────────────────────────────────
//
// The base white-blend factor for head self-bloom. Combined with
// per-layer PARALLAX_HEAD_SELFBLOOM_MULT, this determines how much
// the head cell is boosted toward white (glow effect).
//
// Previously hardcoded in droplet.rs as `60.0 / 256.0`. Centralized
// here so all rain visual tuning lives in a single file.

/// Base head self-bloom intensity (fraction of white-blend applied to
/// head cells). Per-layer scaling via PARALLAX_HEAD_SELFBLOOM_MULT.
///
/// Effective per-layer white-blend:
/// - Back  (0): 0.234 × 0.38 = 0.089 (9% — ambient distant glow)
/// - Mid   (1): 0.234 × 0.68 = 0.159 (16% — clearly present)
/// - Front (2): 0.234 × 1.20 = 0.281 (28% — cinematic hero glow)
pub(crate) const HEAD_SELFBLOOM_BASE: f32 = 60.0 / 256.0; // ~0.234

// ─── Monolith scene brightness & structure ─────────────────────────────────
//
// Core/head cell white-blend, boost cap, and stream geometry for the
// Monolith scene. Previously hardcoded in monolith.rs. Centralized
// here per the single-source-of-truth principle.

/// Core cell white-blend factor for the Monolith scene.
///
/// Applied to Core-level cells (brightest tier) as an extra blend toward
/// white on top of the normal brightness scaling. Produces the dramatic
/// head/core brightness that makes the monolith's focal glyphs pop.
///
/// v17 mastery: raised from 115/256 (0.45) → 140/256 (0.55) for
/// higher-contrast vivid hierarchy.
pub(crate) const MONOLITH_CORE_WHITE_BLEND: f32 = 140.0 / 256.0; // ~0.547

/// Maximum white-blend cap for Monolith pulse/breathing boost.
///
/// When the monolith's brightness factor exceeds 1.0 (pulse/breathing),
/// the excess is blended toward white but capped at this value to prevent
/// clipping into pure white noise.
///
/// v17 mastery: raised from 0.12 → 0.20 for stronger pulse visibility.
pub(crate) const MONOLITH_WHITE_BOOST_CAP: f32 = 0.20;

/// Minimum monolith stream span (rows).
pub(crate) const MONOLITH_MIN_STREAM_SPAN: u16 = 14;

/// Maximum monolith stream span (rows).
pub(crate) const MONOLITH_MAX_STREAM_SPAN: u16 = 30;

/// Base active-lane ratio for monolith density scaling.
pub(crate) const MONOLITH_ACTIVE_BASE: f32 = 0.06;

/// Density multiplier for monolith active-lane calculation.
pub(crate) const MONOLITH_ACTIVE_DENSITY_MULT: f32 = 0.28;

/// Maximum active-lane ratio cap.
pub(crate) const MONOLITH_ACTIVE_MAX: f32 = 0.35;

/// Spawn rate multiplier for monolith stream generation.
pub(crate) const MONOLITH_SPAWN_RATE_MULT: f32 = 1.4;

/// Spawn rate floor (minimum spawns per tick).
pub(crate) const MONOLITH_SPAWN_RATE_FLOOR: f32 = 2.0;

/// Spine repeat period (rows between spine characters).
pub(crate) const MONOLITH_SPINE_PERIOD: u16 = 3;

/// Spine brightness relative to surrounding segments.
pub(crate) const MONOLITH_SPINE_BRIGHTNESS: f32 = 0.07;

/// Reserved drawn-cell capacity per monolith lane.
pub(crate) const MONOLITH_DRAWN_CELLS_PER_LANE_RESERVE: usize = 32;

#[cfg(test)]
#[path = "../../test/central_control_rains/tests.rs"]
mod tests;
