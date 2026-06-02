// Copyright (c) 2026 rezky_nightky

//! Phase 3: Autonomous cinematic ecosystem types.

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::*;
use crate::runtime::ColorScheme;

// ---------------------------------------------------------------------------
// Phase 3: Autonomous cinematic ecosystem
// ---------------------------------------------------------------------------

/// Cinematic runtime behavior profiles — atmospheric identities that
/// fundamentally alter how the renderer feels, moves, and breathes.
/// These are NOT simple recolors; each profile defines a behavioral ecosystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BehaviorProfile {
    Monolith,
    Void,
    Neural,
    Decay,
    Eclipse,
    Static,
    Pulse,
}

impl BehaviorProfile {
    pub(super) fn name(self) -> &'static str {
        match self {
            BehaviorProfile::Monolith => "Monolith",
            BehaviorProfile::Void => "Void",
            BehaviorProfile::Neural => "Neural",
            BehaviorProfile::Decay => "Decay",
            BehaviorProfile::Eclipse => "Eclipse",
            BehaviorProfile::Static => "Static",
            BehaviorProfile::Pulse => "Pulse",
        }
    }

    pub(super) fn cycle(self) -> Self {
        match self {
            BehaviorProfile::Monolith => BehaviorProfile::Void,
            BehaviorProfile::Void => BehaviorProfile::Neural,
            BehaviorProfile::Neural => BehaviorProfile::Decay,
            BehaviorProfile::Decay => BehaviorProfile::Eclipse,
            BehaviorProfile::Eclipse => BehaviorProfile::Static,
            BehaviorProfile::Static => BehaviorProfile::Pulse,
            BehaviorProfile::Pulse => BehaviorProfile::Monolith,
        }
    }

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
            BehaviorProfile::Void => ProfileParams {
                speed_mult: 0.7,
                density_mult: 0.4,
                turbulence_mult: 0.1,
                phosphor_decay_mult: 1.5,
                anomaly_freq_mult: 0.2,
                luminance_offset: -0.1,
                persistence_boost: -0.1,
                entropy_rate: 0.2,
                short_pct: 0.7,
                linger_mult: 0.5,
            },
            BehaviorProfile::Neural => ProfileParams {
                speed_mult: 1.5,
                density_mult: 1.6,
                turbulence_mult: 2.0,
                phosphor_decay_mult: 0.8,
                anomaly_freq_mult: 2.0,
                luminance_offset: 0.1,
                persistence_boost: 0.1,
                entropy_rate: 1.5,
                short_pct: 0.5,
                linger_mult: 0.8,
            },
            BehaviorProfile::Decay => ProfileParams {
                speed_mult: 0.6,
                density_mult: 0.7,
                turbulence_mult: 0.5,
                phosphor_decay_mult: 0.3,
                anomaly_freq_mult: 0.6,
                luminance_offset: -0.05,
                persistence_boost: 0.4,
                entropy_rate: 0.5,
                short_pct: 0.6,
                linger_mult: 1.5,
            },
            BehaviorProfile::Eclipse => ProfileParams {
                speed_mult: 0.8,
                density_mult: 1.0,
                turbulence_mult: 1.0,
                phosphor_decay_mult: 0.6,
                anomaly_freq_mult: 1.5,
                luminance_offset: 0.0,
                persistence_boost: 0.2,
                entropy_rate: 0.8,
                short_pct: 0.4,
                linger_mult: 1.2,
            },
            BehaviorProfile::Static => ProfileParams {
                speed_mult: 0.2,
                density_mult: 0.3,
                turbulence_mult: 0.05,
                phosphor_decay_mult: 0.2,
                anomaly_freq_mult: 0.1,
                luminance_offset: -0.1,
                persistence_boost: 0.5,
                entropy_rate: 0.1,
                short_pct: 0.9,
                linger_mult: 3.0,
            },
            BehaviorProfile::Pulse => ProfileParams {
                speed_mult: 1.2,
                density_mult: 1.1,
                turbulence_mult: 1.3,
                phosphor_decay_mult: 0.9,
                anomaly_freq_mult: 1.0,
                luminance_offset: 0.05,
                persistence_boost: 0.15,
                entropy_rate: 1.2,
                short_pct: 0.3,
                linger_mult: 1.0,
            },
        }
    }
}

/// Tuning parameters for a cinematic behavior profile.
/// Each value is a multiplier (1.0 = default behavior).
#[derive(Clone, Copy, Debug)]
pub struct ProfileParams {
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

/// Returns 3-4 atmospherically related color schemes for autonomous palette drift.
fn related_schemes(scheme: ColorScheme) -> &'static [ColorScheme] {
    use ColorScheme::*;
    match scheme {
        Green => &[Green2, Green3, Aurora, Forest],
        Green2 => &[Green, Green3, Forest, Aurora],
        Green3 => &[Green, Green2, Forest],
        Gold => &[Yellow, Orange, Sun, Fire],
        Yellow => &[Gold, Orange, Sun],
        Orange => &[Gold, Fire, Sun, Yellow],
        Red => &[Fire, Orange, Supernova, Meteor],
        Blue => &[Ocean, DeepSpace, Neptune, Comet],
        Cyan => &[Aurora, Ocean, Neptune, Uranus],
        Purple => &[Nebula, Cosmos, Vaporwave, Galaxy],
        Neon => &[Vaporwave, Aurora, Cosmos, Nebula],
        Fire => &[Red, Orange, Supernova, Meteor],
        Ocean => &[Blue, DeepSpace, Neptune, Cyan],
        Forest => &[Green, Green2, Aurora, Green3],
        Vaporwave => &[Neon, Purple, Nebula, Cosmos],
        Gray => &[Mercury, Snow, Moon],
        Rainbow => &[Spectrum20, Neon, Vaporwave],
        Snow => &[Gray, Moon, Mercury, Stars],
        Aurora => &[Green, Cyan, Forest, Neon],
        FancyDiamond => &[Cyan, Snow, Nebula, Stardust],
        Cosmos => &[DeepSpace, Nebula, Galaxy, Purple],
        Nebula => &[Cosmos, Purple, Galaxy, Stardust],
        Spectrum20 => &[Rainbow, Neon, Vaporwave],
        Stars => &[DeepSpace, Cosmos, Galaxy, Comet],
        Mars => &[Red, Fire, Meteor, Supernova],
        Venus => &[Gold, Yellow, Sun, Orange],
        Mercury => &[Gray, Moon, Snow],
        Jupiter => &[Orange, Gold, Sun, Saturn],
        Saturn => &[Jupiter, Gold, Venus, Yellow],
        Uranus => &[Cyan, Neptune, Ocean, Aurora],
        Neptune => &[Blue, Ocean, DeepSpace, Uranus],
        Pluto => &[Mercury, Gray, Moon, DeepSpace],
        Moon => &[Gray, Mercury, Snow, Stars],
        Sun => &[Gold, Yellow, Venus, Fire],
        Comet => &[Blue, Stars, DeepSpace, Cyan],
        Galaxy => &[Cosmos, Nebula, DeepSpace, Stardust],
        Supernova => &[Fire, Red, Meteor, Mars],
        BlackHole => &[DeepSpace, Cosmos, Nebula, Pluto],
        Andromeda => &[Cosmos, Nebula, Galaxy, Stardust],
        Stardust => &[Galaxy, Nebula, Cosmos, FancyDiamond],
        Meteor => &[Fire, Red, Mars, Supernova],
        Eclipse => &[BlackHole, DeepSpace, Cosmos, Nebula],
        DeepSpace => &[Cosmos, BlackHole, Galaxy, Stars],
        // Catch-all for future variants
        #[allow(unreachable_patterns)]
        _ => &[Green, Blue, Cyan],
    }
}

/// Autonomous color ecosystem: slow palette drift, luminance climate shifts,
/// and tonal migration that makes the renderer feel atmospherically alive.
pub(super) struct ColorEcosystem {
    pub(super) luminance_climate: f32,
    pub(super) saturation_climate: f32,
    pub(super) hue_drift: f32,
    pub(super) luminance_direction: f32,
    pub(super) saturation_direction: f32,
    pub(super) hue_direction: f32,
    pub(super) last_tick: Instant,
}

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

    pub(super) fn tick(
        &mut self,
        now: Instant,
        mt: &mut StdRng,
        current_scheme: ColorScheme,
    ) -> Option<ColorScheme> {
        let elapsed = now.saturating_duration_since(self.last_tick).as_secs_f32();
        if elapsed < COLOR_ECOSYSTEM_TICK_SECS {
            return None;
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

        // Autonomous palette drift
        if chance_dist.sample(mt) < AUTONOMOUS_PALETTE_DRIFT_CHANCE {
            let related = related_schemes(current_scheme);
            if !related.is_empty() {
                let idx_dist = Uniform::new_inclusive(0usize, related.len().saturating_sub(1))
                    .expect("related_schemes idx always valid");
                let new_scheme = related[idx_dist.sample(mt)];
                return Some(new_scheme);
            }
        }

        None
    }
}

/// Autonomous atmospheric evolution: entropy cycles, density migration,
/// luminance shifts, anomaly pressure fluctuations. All slow, smooth, cinematic.
pub(super) struct AtmosphericEvolution {
    pub(super) entropy_phase: f32,
    pub(super) last_tick: Instant,
    pub(super) density_offset: f32,
    pub(super) luminance_offset: f32,
    pub(super) anomaly_offset: f32,
    pub(super) cycle_speed: f32,
}

impl AtmosphericEvolution {
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
        #[allow(unused_variables)] luminance: f32,
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
pub struct EmergentEffects {
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
        evolution: &AtmosphericEvolution,
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
