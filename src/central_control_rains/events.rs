// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Atmospheric event + ecosystem constants — extracted from `mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns: rare anomaly events, temporal color ecosystems, runtime
//! behavior profiles, autonomous atmospheric evolution, living rain
//! density noise, wind gusts, renderer memory, emergent storytelling,
//! pause/resume easing. These are the "living world" constants that
//! make the rain feel alive over long viewing sessions.

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

// ─── Cinematic pause/resume easing (exponential decay) ────────────────────
//
// Masterclass: pause/resume easing uses exponential decay
// (`blend = exp(-k·t)` for decel, `blend = 1 - exp(-k·t)` for accel)
// — physically motivated drag with a long tail, matching the README's
// "exponential deceleration (~3s coast-down)" promise.
//
// Previously (v17–v50): smootherstep S-curve (6t⁵ - 15t⁴ + 10t³, C2
// continuous) over fixed 0.30s decel / 0.45s resume windows. That was
// perceptually smooth at the S-curve endpoints but the bounded
// duration felt abrupt at the end-snap, and the README's "exponential
// deceleration ~3s" wording was stale (smootherstep is not exponential).
//
// Exp decay has a long tail that never quite reaches 0/1, so we snap
// at the settle thresholds below. This trades asymptotic smoothness
// for a hard "fully paused" / "full speed" terminal state — required
// so other subsystems (spawn_remainder reset, monolith stream shift,
// phosphor LUT) see clean state transitions.
//
// Asymmetric rates: k_decel (1.2) > k_resume (0.9) — pause feels snappy
// (~2.5s settle), resume feels like a "wake up" (~3.3s settle). This
// preserves the prior 0.30/0.45 ratio's asymmetric feel.

/// Per-second decay rate for the pause deceleration ramp. At k=1.2,
/// the blend reaches 5% (`PAUSE_EASE_SETTLE_FRAC`) at t ≈ 2.5s — the
/// documented "~3s coast-down" feel with a touch of head-room.
pub(crate) const PAUSE_EASE_DECAY_RATE: f32 = 1.2;

/// Per-second decay rate for the resume acceleration ramp. Slightly
/// slower than pause (k=0.9 vs 1.2) so resume feels more "wake up" —
/// the asymmetric rate preserves the prior 0.30s/0.45s duration ratio's
/// feel where the resume ramp is perceptibly longer than the pause ramp.
pub(crate) const RESUME_EASE_DECAY_RATE: f32 = 0.9;

/// Per-second decay rate for the ABORT-resume ramp (NIGHT-hunter-8).
///
/// When the user presses `p` mid-deceleration (cancelling the pause),
/// the resume ramp starts from the CURRENT decel blend, not from 0.
/// The slow wake-up rate (0.9) would drag that recovery out to ~3s —
/// the old "rain looks stuck" bug — while the pre-NIGHT-hunter-8 code
/// snapped to 1.0 instantly, a visible velocity jump (the owner's
/// "little jump" on rapid p-taps). The fast abort rate recovers 95%
/// within ~0.5-0.6s from ANY starting blend: smooth enough to read as
/// inertia recovery, fast enough to feel like a cancel.
///
/// From blend 0.30: reach 0.95 at t = -ln(1 - 0.65/0.70)/5.0 ≈ 0.52s.
/// From blend 0.05: reach 0.95 at t = -ln(1 - 0.90/0.95)/5.0 ≈ 0.59s.
pub(crate) const RESUME_ABORT_EASE_DECAY_RATE: f32 = 5.0;

/// Settle threshold for pause decel — when `pause_blend` drops below
/// this, snap to fully paused. 5% matches the README's "~3s coast-down"
/// promise and is well below the perceptual motion floor.
pub(crate) const PAUSE_EASE_SETTLE_FRAC: f32 = 0.05;

/// Settle threshold for resume accel — when `resume_blend` rises above
/// this, snap to full speed. 95% is the symmetric counterpart to the
/// pause settle (95% speed is perceptually indistinguishable from 100%
/// at typical 16–33ms frame rates).
pub(crate) const RESUME_EASE_SETTLE_FRAC: f32 = 0.95;
