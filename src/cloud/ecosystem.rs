// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Autonomous cinematic ecosystem types.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::*;

// Autonomous cinematic ecosystem

/// Cinematic runtime behavior profiles — atmospheric identities that
/// fundamentally alter how the renderer feels, moves, and breathes.
/// These are NOT simple recolors; each profile defines a behavioral ecosystem.
///
/// Only Monolith is currently wired into the production render path.
/// #[non_exhaustive] reserves the right to add future profiles (Void,
/// Neural, Decay, Eclipse, Static, Pulse) without a semver break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BehaviorProfile {
    Monolith,
}

impl BehaviorProfile {
    pub(super) fn params(self) -> ProfileParams {
        match self {
            BehaviorProfile::Monolith => ProfileParams {
                speed_mult: 0.5,
                density_mult: 1.3,
                turbulence_mult: 0.3,
                phosphor_decay_mult: 0.4,
                anomaly_freq_mult: 0.4,
                luminance_offset: 0.0,
                persistence_boost: 0.3,
                entropy_rate: 0.3,
                short_pct: 0.2,
                linger_mult: 2.0,
            },
        }
    }
}

/// Tuning parameters for a cinematic behavior profile.
/// Each value is a multiplier (1.0 = default behavior).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProfileParams {
    pub speed_mult: f32,
    pub density_mult: f32,
    pub turbulence_mult: f32,
    pub phosphor_decay_mult: f32,
    pub anomaly_freq_mult: f32,
    pub luminance_offset: f32,
    pub persistence_boost: f32,
    pub entropy_rate: f32,
    pub short_pct: f32,
    pub linger_mult: f32,
}

#[inline]
pub(super) fn lerp_profile_params(a: ProfileParams, b: ProfileParams, t: f32) -> ProfileParams {
    ProfileParams {
        speed_mult: a.speed_mult + (b.speed_mult - a.speed_mult) * t,
        density_mult: a.density_mult + (b.density_mult - a.density_mult) * t,
        turbulence_mult: a.turbulence_mult + (b.turbulence_mult - a.turbulence_mult) * t,
        phosphor_decay_mult: a.phosphor_decay_mult
            + (b.phosphor_decay_mult - a.phosphor_decay_mult) * t,
        anomaly_freq_mult: a.anomaly_freq_mult + (b.anomaly_freq_mult - a.anomaly_freq_mult) * t,
        luminance_offset: a.luminance_offset + (b.luminance_offset - a.luminance_offset) * t,
        persistence_boost: a.persistence_boost + (b.persistence_boost - a.persistence_boost) * t,
        entropy_rate: a.entropy_rate + (b.entropy_rate - a.entropy_rate) * t,
        short_pct: a.short_pct + (b.short_pct - a.short_pct) * t,
        linger_mult: a.linger_mult + (b.linger_mult - a.linger_mult) * t,
    }
}

// ── Color family classification (removed — Crystal Dragon) ──────────
//
// The old FeelingState→ColorFamily→family_members pipeline was the
// auto-color-drift v1 engine. It has been replaced by the Crystal Dragon
// Engine (src/crystal_dragon_engine/) which uses a point-based temperature
// group system (Cold/Medium/Hot) with calc-v1 probabilistic weighted
// selection. All palette drift logic now lives in crystal_dragon_tick()
// in runtime_controls.rs. This file only handles climate drift
// (luminance/saturation/hue modulation) which is orthogonal to palette
// scheme selection.

/// Autonomous color ecosystem: slow palette drift, luminance climate shifts,
/// and tonal migration that makes the renderer feel atmospherically alive.
///
/// Derives `Clone, Copy` so the ecosystem state can be carried across
/// live-reload (Phase D Bug #9 fix in `Cloud::inherit_ecosystem_state`).
/// All fields are plain f32 / Instant — trivially Copy.
#[derive(Clone, Copy)]
pub(super) struct ColorEcosystem {
    pub(super) luminance_climate: f32,
    pub(super) saturation_climate: f32,
    pub(super) hue_drift: f32,
    // Drift directions are BINARY (-1.0 or +1.0), not continuous. The
    // audit (Bug #6 "continuous direction") considered making these
    // floating-point (e.g. -0.3, +0.7) for smoother drift curves.
    // DEFERRED — see the risk note below the field list.
    pub(super) luminance_direction: f32,
    pub(super) saturation_direction: f32,
    pub(super) hue_direction: f32,
    pub(super) last_tick: Instant,
}

// ── Bug #6 (continuous direction) — DEFERRED ──────────────────────────────
//
// The audit proposed making `luminance_direction` / `saturation_direction`
// / `hue_direction` continuous floating-point values (e.g. -0.3, +0.7)
// instead of binary -1.0/+1.0. The argument: smoother drift curves,
// fewer visible "step" transitions, more cinematic feel.
//
// WHY DEFERRED:
// 1. RISK TO TUNED BEHAVIOR — The current binary-direction drift has been
//    tuned alongside COLOR_CLIMATE_DRIFT_RATE, COLOR_DRIFT_REEVAL_CHANCE,
//    and the clamp bounds to produce a specific aesthetic. Switching to
//    continuous directions would require re-tuning all four constants,
//    and the new tuning would need extensive manual A/B viewing to
//    confirm it actually looks better. The current drift already feels
//    smooth at normal viewing distance.
//
// 2. NO MEASURABLE BENEFIT — The drift deltas are tiny per tick
//    (COLOR_CLIMATE_DRIFT_RATE is sub-1%). The visible difference between
//    "direction = -1.0" and "direction = -0.7" over a 30-second window
//    is below human perception threshold for most viewers. The aesthetic
//    win is theoretical, not measurable.
//
// 3. TEST INSTABILITY — Multiple tests in tests_color_stability.rs
//    exercise the ecosystem tick path with deterministic seeds. Changing
//    the direction algorithm would change the exact drift values
//    produced, requiring test fixture updates and reducing test
//    reliability as a regression guard.
//
// 4. SHADOW METRIC DRIFT — The drift directions feed into the renderer's
//    brightness/saturation/hue multipliers, which the bench_report
//    ATMOSPHERE section reports on. Changing the algorithm would change
//    the shadow metric distribution, possibly triggering false-positive
//    regression alerts in CI.
//
// IF UNBLOCKING IN THE FUTURE:
// - Add a config key `climate-drift-mode = binary | continuous` so users
//   can opt-in to the new algorithm without forcing a re-tune on everyone.
// - Run a 2-week A/B viewing test with the team before making continuous
//   the default.
// - Update tests to accept either algorithm via a fixture parameter.
// - Re-evaluate COLOR_CLIMATE_DRIFT_RATE — continuous directions may
//   require a higher rate to produce visible drift.

impl ColorEcosystem {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            luminance_climate: 0.85,
            saturation_climate: 0.85,
            hue_drift: 0.0,
            luminance_direction: 0.0,
            saturation_direction: 0.0,
            hue_direction: 0.0,
            last_tick: now,
        }
    }

    /// Shift all internal timestamps by `elapsed`. Called on resume
    /// from pause so the ecosystem doesn't interpret a long pause as
    /// a single long tick (which would trigger a massive climate drift).
    pub(super) fn shift_in_time(&mut self, elapsed: Duration) {
        self.last_tick += elapsed;
    }

    /// Climate-only tick: evolve luminance/saturation/hue climate drift.
    ///
    /// Palette scheme selection has been moved to the
    /// Crystal Dragon Engine — see `crystal_dragon_tick()` in
    /// `runtime_controls.rs`. This method only handles climate modulation
    /// (luminance/saturation/hue) which is orthogonal to palette scheme.
    pub(super) fn tick(&mut self, now: Instant, mt: &mut StdRng) {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < COLOR_ECOSYSTEM_TICK_SECS {
            return;
        }
        self.last_tick = now;

        // Randomly re-evaluate drift directions
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.luminance_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.saturation_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }
        if chance_dist.sample(mt) < COLOR_DRIFT_REEVAL_CHANCE {
            self.hue_direction = if chance_dist.sample(mt) < 0.5 {
                -1.0
            } else {
                1.0
            };
        }

        // Apply drift rates
        self.luminance_climate += self.luminance_direction * COLOR_CLIMATE_DRIFT_RATE;
        self.saturation_climate += self.saturation_direction * COLOR_SATURATION_DRIFT_RATE;
        self.hue_drift += self.hue_direction * COLOR_HUE_DRIFT_RATE;

        // Clamp values
        self.luminance_climate = self
            .luminance_climate
            .clamp(COLOR_LUMINANCE_CLIMATE_MIN, COLOR_LUMINANCE_CLIMATE_MAX);
        self.saturation_climate = self
            .saturation_climate
            .clamp(COLOR_SATURATION_CLIMATE_MIN, COLOR_SATURATION_CLIMATE_MAX);
        self.hue_drift = self
            .hue_drift
            .clamp(-std::f32::consts::PI, std::f32::consts::PI);
    }
}

/// Autonomous atmospheric evolution: entropy cycles, density migration,
/// luminance shifts, anomaly pressure fluctuations. All slow, smooth, cinematic.
///
/// Derives `Clone, Copy` so the evolution state can be carried across
/// live-reload (Phase D Bug #9 fix in `Cloud::inherit_ecosystem_state`).
#[derive(Clone, Copy)]
pub(super) struct EntropyDrift {
    pub(super) entropy_phase: f32,
    pub(super) last_tick: Instant,
    pub(super) density_offset: f32,
    pub(super) luminance_offset: f32,
    pub(super) anomaly_offset: f32,
    pub(super) cycle_speed: f32,
}

impl EntropyDrift {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            entropy_phase: 0.0,
            last_tick: now,
            density_offset: 0.0,
            luminance_offset: 0.0,
            anomaly_offset: 0.0,
            cycle_speed: 1.0,
        }
    }

    pub(super) fn tick(&mut self, now: Instant, profile_entropy_rate: f32) {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < ATMOSPHERE_TICK_SECS {
            return;
        }
        self.last_tick = now;
        self.cycle_speed = profile_entropy_rate;

        self.entropy_phase += (elapsed / ENTROPY_CYCLE_SECS) * self.cycle_speed;
        self.entropy_phase %= 1.0;

        let tau = std::f32::consts::TAU;
        self.density_offset = (self.entropy_phase * tau).sin() * ATMOSPHERE_DENSITY_RANGE;
        self.luminance_offset = (self.entropy_phase * tau + std::f32::consts::FRAC_PI_3).sin()
            * ATMOSPHERE_LUMINANCE_RANGE;
        self.anomaly_offset = (self.entropy_phase * tau + 2.0 * std::f32::consts::FRAC_PI_3).sin()
            * ATMOSPHERE_ANOMALY_RANGE;
    }
}

/// Long-timescale renderer memory: historical influence on current rendering.
/// Remembers anomaly history, density history, luminance pressure.
pub(super) struct RendererMemory {
    pub(super) anomaly_history: [f32; MEMORY_HISTORY_SAMPLES],
    pub(super) density_history: [f32; MEMORY_HISTORY_SAMPLES],
    pub(super) history_idx: usize,
    pub(super) last_sample: Instant,
    pub(super) instability_pressure: f32,
    pub(super) persistence_richness: f32,
}

impl RendererMemory {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            anomaly_history: [0.0; MEMORY_HISTORY_SAMPLES],
            density_history: [0.0; MEMORY_HISTORY_SAMPLES],
            history_idx: 0,
            last_sample: now,
            instability_pressure: 0.0,
            persistence_richness: 0.0,
        }
    }

    pub(super) fn record_sample(
        &mut self,
        now: Instant,
        anomaly_density: f32,
        rain_density: f32,
        _luminance: f32,
    ) {
        let elapsed = now
            .saturating_duration_since(self.last_sample)
            .as_secs_f32();
        if elapsed < MEMORY_SAMPLE_INTERVAL_SECS {
            return;
        }
        self.last_sample = now;
        self.anomaly_history[self.history_idx] = anomaly_density;
        self.density_history[self.history_idx] = rain_density;
        self.history_idx = (self.history_idx + 1) % MEMORY_HISTORY_SAMPLES;
    }

    pub(super) fn recompute_derived(&mut self) {
        let n = MEMORY_HISTORY_SAMPLES as f32;
        let avg_anomaly: f32 = self.anomaly_history.iter().sum::<f32>() / n;

        self.instability_pressure = avg_anomaly * MEMORY_ANOMALY_PRESSURE_WEIGHT;
        self.persistence_richness = (1.0 - avg_anomaly) * MEMORY_CALM_PERSISTENCE_BOOST;
    }
}

/// Kind of emergent visual moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EmergentKind {
    LuminanceSwell,
    DensityPulse,
    TemporalDilation,
}

/// An active emergent moment.
#[derive(Clone, Debug)]
pub(super) struct EmergentMoment {
    pub(super) kind: EmergentKind,
    pub(super) start_time: Instant,
    pub(super) duration: f32,
}

/// Current emergent effects applied to rendering.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EmergentEffects {
    pub luminance_boost: f32,
    pub density_boost: f32,
    pub speed_shift: f32,
}

/// Emergent visual storytelling system: watches for convergence conditions
/// across other systems and occasionally produces emotionally resonant moments.
pub(super) struct StorytellingState {
    pub(super) moments: Vec<EmergentMoment>,
    pub(super) last_tick: Instant,
    pub(super) cooldown_until: Option<Instant>,
}

impl StorytellingState {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            moments: Vec::new(),
            last_tick: now,
            cooldown_until: None,
        }
    }

    pub(super) fn tick(
        &mut self,
        now: Instant,
        mt: &mut StdRng,
        evolution: &EntropyDrift,
        memory: &RendererMemory,
        ecosystem: &ColorEcosystem,
    ) -> Option<EmergentKind> {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < STORYTELLING_TICK_SECS {
            return None;
        }
        self.last_tick = now;

        // Check cooldown
        if let Some(until) = self.cooldown_until {
            if now < until {
                return None;
            }
        }

        // Check max moments
        if self.moments.len() >= EMERGENT_MAX_MOMENTS {
            return None;
        }

        // Convergence conditions
        let entropy_near_peak = (evolution.entropy_phase - 0.5).abs() < 0.15;
        let has_instability = memory.instability_pressure > 0.1;
        let not_too_bright = ecosystem.luminance_climate < 0.85;

        if !entropy_near_peak || !has_instability || !not_too_bright {
            return None;
        }

        // Roll for emergent moment
        let chance_dist = Uniform::new(0.0f32, 1.0f32).expect("chance_dist always valid");
        if chance_dist.sample(mt) < EMERGENT_MOMENT_CHANCE {
            let kind_roll = chance_dist.sample(mt);
            let kind = if kind_roll < 0.33 {
                EmergentKind::LuminanceSwell
            } else if kind_roll < 0.66 {
                EmergentKind::DensityPulse
            } else {
                EmergentKind::TemporalDilation
            };
            self.cooldown_until =
                Some(now + Duration::from_secs_f32(EMERGENT_MOMENT_DURATION_SECS + 60.0));
            return Some(kind);
        }

        None
    }

    pub(super) fn active_effects(&self, now: Instant) -> EmergentEffects {
        let mut effects = EmergentEffects::default();
        for moment in &self.moments {
            let elapsed = now
                .saturating_duration_since(moment.start_time)
                .as_secs_f32();
            if elapsed >= moment.duration {
                continue;
            }
            let progress = elapsed / moment.duration;
            let sin_pi = (progress * std::f32::consts::PI).sin();
            match moment.kind {
                EmergentKind::LuminanceSwell => {
                    effects.luminance_boost += EMERGENT_LUMINANCE_INTENSITY * sin_pi;
                }
                EmergentKind::DensityPulse => {
                    effects.density_boost += EMERGENT_DENSITY_INTENSITY * sin_pi;
                }
                EmergentKind::TemporalDilation => {
                    effects.speed_shift -= EMERGENT_SPEED_SHIFT * sin_pi;
                }
            }
        }
        effects
    }

    /// Expire moments past their duration. Must be called separately since
    /// active_effects only borrows &self.
    pub(super) fn expire_moments(&mut self, now: Instant) {
        self.moments
            .retain(|m| now.saturating_duration_since(m.start_time).as_secs_f32() < m.duration);
    }
}
