// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Living rain phenomena: per-column density noise + wind gusts.
//!
//! Two independent, zero-allocation subsystems that make the rain feel
//! like weather rather than a uniform grid:
//!
//! ## Dynamic Density Noise
//!
//! A spatial variety layer: each column has a density modifier in
//! `[DENSITY_NOISE_MIN, DENSITY_NOISE_MAX]` that re-rolls every
//! `DENSITY_NOISE_PERIOD_SECS` seconds. Columns with modifier 1.4 spawn
//! ~40% more droplets; columns with 0.6 spawn ~40% fewer. The hash is
//! O(1) and branchless — a single `wrapping_mul` + `xor` per query.
//!
//! The pattern is **stable within each 10-second window** so the eye
//! can perceive "this column is dense, that one is sparse" instead of
//! seeing per-frame flicker. At each window boundary the pattern shifts
//! smoothly because the seed changes — no fade, just a new arrangement
//! that itself persists for the next 10 seconds.
//!
//! ## Wind Gusts
//!
//! A four-state machine that periodically surges the global spawn
//! density + speed multiplier:
//!
//! ```text
//! IDLE  ──(30-120s)──▶ ATTACK ──(1-2s)──▶ HOLD ──(0.5-1s)──▶ DECAY ──(3-5s)──▶ IDLE
//!  1.0                 1.0 → peak          peak               peak → 1.0         1.0
//! ```
//!
//! All ramps are **linear** (no sin/cos). The state machine runs once
//! per `rain_at` call, costs ~5ns when IDLE, and stores zero heap state.

use std::time::{Duration, Instant};

use rand::distr::{Distribution, Uniform};
use rand::Rng;

use crate::constants::*;

/// Wind-gust state machine. Lives as a single field on Cloud; advanced
/// once per frame in `rain_at`. All durations are sampled from the
/// `GUST_*_MIN_SECS` / `GUST_*_MAX_SECS` ranges when a transition fires.
#[derive(Debug, Clone)]
pub(super) struct GustState {
    /// Current phase: Idle / Attack / Hold / Decay.
    phase: GustPhase,
    /// Wall-clock timestamp at which the current phase started.
    phase_start: Instant,
    /// Sampled duration of the current phase. Pre-computed on phase entry
    /// so the per-frame update is a single `elapsed >= duration` check.
    phase_duration: Duration,
    /// Peak multiplier for the active gust (sampled in
    /// `[GUST_PEAK_MIN, GUST_PEAK_MAX]` when ATTACK starts). 1.0 when IDLE.
    peak: f32,
    /// Current output multiplier. Updated by `tick()` each frame via
    /// linear interpolation. Always in `[1.0, peak]`.
    multiplier: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GustPhase {
    /// Waiting for the next gust. Multiplier pinned at 1.0.
    Idle,
    /// Ramp-up: multiplier goes 1.0 → peak over `phase_duration`.
    Attack,
    /// Sustained peak: multiplier pinned at `peak` for `phase_duration`.
    Hold,
    /// Ramp-down: multiplier goes peak → 1.0 over `phase_duration`.
    Decay,
}

impl GustState {
    /// Build a fresh state machine in the IDLE phase. The first IDLE
    /// window samples from `[GUST_IDLE_MIN_SECS, GUST_IDLE_MAX_SECS]` so
    /// the first gust doesn't always arrive at the same time after launch.
    pub(super) fn new(now: Instant) -> Self {
        let idle_dur = sample_idle_duration(&mut rand::rng());
        Self {
            phase: GustPhase::Idle,
            phase_start: now,
            phase_duration: idle_dur,
            peak: 1.0,
            multiplier: 1.0,
        }
    }

    /// Advance the state machine to `now` and return the current
    /// multiplier. Called exactly once per `rain_at` frame.
    ///
    /// All transitions are deterministic once `phase_duration` is set:
    /// we just compare `now - phase_start >= phase_duration`. When a
    /// phase completes, we sample the next phase's duration from its
    /// configured range and continue.
    pub(super) fn tick<R: Rng>(&mut self, now: Instant, rng: &mut R) -> f32 {
        let elapsed = now.saturating_duration_since(self.phase_start);
        if elapsed < self.phase_duration {
            // Same phase — interpolate or pin the multiplier.
            self.multiplier = self.interpolate_within_phase(elapsed);
            return self.multiplier;
        }

        // Phase complete — advance and re-sample.
        match self.phase {
            GustPhase::Idle => {
                self.transition_to_attack(now, rng);
            }
            GustPhase::Attack => {
                self.transition_to_hold(now, rng);
            }
            GustPhase::Hold => {
                self.transition_to_decay(now, rng);
            }
            GustPhase::Decay => {
                self.transition_to_idle(now, rng);
            }
        }
        self.multiplier
    }

    /// Read-only access to the current multiplier. Use this from draw
    /// paths that must not advance the state machine.
    #[allow(dead_code)]
    pub(super) fn multiplier(&self) -> f32 {
        self.multiplier
    }

    // ── Phase transitions ──

    fn transition_to_attack<R: Rng>(&mut self, now: Instant, rng: &mut R) {
        self.phase = GustPhase::Attack;
        self.phase_start = now;
        self.phase_duration =
            sample_range_duration(rng, GUST_ATTACK_MIN_SECS, GUST_ATTACK_MAX_SECS);
        self.peak = rng.random_range(GUST_PEAK_MIN..GUST_PEAK_MAX);
        self.multiplier = 1.0;
    }

    fn transition_to_hold<R: Rng>(&mut self, now: Instant, rng: &mut R) {
        self.phase = GustPhase::Hold;
        self.phase_start = now;
        self.phase_duration = sample_range_duration(rng, GUST_HOLD_MIN_SECS, GUST_HOLD_MAX_SECS);
        self.multiplier = self.peak;
    }

    fn transition_to_decay<R: Rng>(&mut self, now: Instant, rng: &mut R) {
        self.phase = GustPhase::Decay;
        self.phase_start = now;
        self.phase_duration = sample_range_duration(rng, GUST_DECAY_MIN_SECS, GUST_DECAY_MAX_SECS);
        self.multiplier = self.peak;
    }

    fn transition_to_idle<R: Rng>(&mut self, now: Instant, rng: &mut R) {
        self.phase = GustPhase::Idle;
        self.phase_start = now;
        self.phase_duration = sample_idle_duration(rng);
        self.peak = 1.0;
        self.multiplier = 1.0;
    }

    // ── Linear interpolation within the current phase ──

    fn interpolate_within_phase(&self, elapsed: Duration) -> f32 {
        let t = (elapsed.as_secs_f32() / self.phase_duration.as_secs_f32()).clamp(0.0, 1.0);
        match self.phase {
            GustPhase::Idle | GustPhase::Hold => self.multiplier,
            GustPhase::Attack => 1.0 + (self.peak - 1.0) * t,
            GustPhase::Decay => self.peak + (1.0 - self.peak) * t,
        }
    }
}

/// Sample an IDLE duration in `[GUST_IDLE_MIN_SECS, GUST_IDLE_MAX_SECS]`.
fn sample_idle_duration<R: Rng>(rng: &mut R) -> Duration {
    sample_range_duration(rng, GUST_IDLE_MIN_SECS, GUST_IDLE_MAX_SECS)
}

/// Sample a `Duration` in `[min_secs, max_secs]`. Uses Uniform so the
/// distribution is uniform rather than the biased `random()` float cast.
fn sample_range_duration<R: Rng>(rng: &mut R, min_secs: f64, max_secs: f64) -> Duration {
    let dist = Uniform::new_inclusive(min_secs, max_secs)
        .expect("gust duration: min <= max (validated constants)");
    let secs = dist.sample(rng);
    Duration::from_secs_f64(secs)
}

// ── Dynamic density noise ──

/// Compute the per-column density modifier for the given column at the
/// given elapsed time (seconds since process start).
///
/// O(1) and branchless: two `wrapping_mul` + one `xor` + one
/// `wrapping_add` + a divide-by-u32-max normalization. Called once per
/// spawn attempt in `spawn_droplets`.
///
/// The modifier is stable within each `DENSITY_NOISE_PERIOD_SECS` window
/// (the seed is `floor(elapsed / period)`), then re-rolls across
/// windows. Output is always in `[DENSITY_NOISE_MIN, DENSITY_NOISE_MAX]`.
#[inline]
pub(super) fn column_density_modifier(col: u16, elapsed_secs: f64) -> f32 {
    let seed = (elapsed_secs / DENSITY_NOISE_PERIOD_SECS).floor() as u32;
    let mixed = (col as u32).wrapping_mul(DENSITY_NOISE_HASH_K)
        ^ seed.wrapping_mul(DENSITY_NOISE_HASH_SEED_K);
    // Map u32 → [0, 1] via divide by u32::MAX. Adding 1 to the divisor
    // avoids a div-by-zero and gives an inclusive upper bound.
    let normalized = mixed as f32 / (u32::MAX as f32 + 1.0);
    DENSITY_NOISE_MIN + normalized * (DENSITY_NOISE_MAX - DENSITY_NOISE_MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn fixed_rng() -> StdRng {
        StdRng::seed_from_u64(0xC0DE_C0FFEE)
    }

    // ── Density noise ──

    #[test]
    fn density_modifier_is_bounded() {
        for col in 0u16..256 {
            for secs in [0.0, 1.5, 9.99, 10.0, 10.01, 120.0, 3600.0] {
                let m = column_density_modifier(col, secs);
                assert!(
                    (DENSITY_NOISE_MIN..=DENSITY_NOISE_MAX).contains(&m),
                    "col={col} secs={secs}: modifier {m} out of [{DENSITY_NOISE_MIN}, {DENSITY_NOISE_MAX}]"
                );
            }
        }
    }

    #[test]
    fn density_modifier_is_stable_within_period() {
        // Same column, same period → same modifier.
        for col in 0u16..50 {
            let m1 = column_density_modifier(col, 3.0);
            let m2 = column_density_modifier(col, 5.0);
            let m3 = column_density_modifier(col, 9.999);
            assert_eq!(m1, m2, "col {col}: same period must give same modifier");
            assert_eq!(m1, m3, "col {col}: same period must give same modifier");
        }
    }

    #[test]
    fn density_modifier_changes_across_periods() {
        // At least one column should change modifier between period 0
        // and period 1. (Statistically nearly all will, but we only need
        // one to prove the seed is actually being mixed in.)
        let any_change = (0u16..100)
            .any(|col| column_density_modifier(col, 5.0) != column_density_modifier(col, 15.0));
        assert!(any_change, "modifier must change across period boundary");
    }

    #[test]
    fn density_modifier_varies_across_columns() {
        // At least 10 distinct values across 100 columns in the same
        // period — proves the column index is being mixed in. f32 is not
        // Hash, so we collect to a Vec and dedup manually after sorting
        // by bit-pattern (NaN-safe: bit-equality implies value-equality
        // for finite floats, which these always are).
        let mut values: Vec<f32> = (0u16..100)
            .map(|col| column_density_modifier(col, 5.0))
            .collect();
        values.sort_by(|a, b| a.total_cmp(b));
        values.dedup();
        assert!(
            values.len() >= 10,
            "expected >=10 distinct modifiers across 100 columns, got {}",
            values.len()
        );
    }

    // ── Gust state machine ──

    #[test]
    fn gust_starts_idle_with_multiplier_one() {
        let now = Instant::now();
        let mut g = GustState::new(now);
        let m = g.tick(now, &mut fixed_rng());
        assert!(
            (m - 1.0).abs() < 1e-6,
            "fresh GustState must report multiplier 1.0, got {m}"
        );
    }

    #[test]
    fn gust_idle_stays_at_one_until_duration_elapses() {
        let now = Instant::now();
        let mut rng = fixed_rng();
        let mut g = GustState::new(now);
        // Sample several frames inside IDLE — should never leave 1.0.
        for ms in [0, 100, 500, 1_000, 5_000, 10_000] {
            let t = now + Duration::from_millis(ms);
            let m = g.tick(t, &mut rng);
            assert!(
                (m - 1.0).abs() < 1e-6,
                "IDLE phase must keep multiplier at 1.0 (t={ms}ms, got {m})"
            );
        }
    }

    #[test]
    fn gust_transitions_idle_to_attack_after_idle_duration() {
        let now = Instant::now();
        let mut rng = fixed_rng();
        // Construct directly so we know the exact IDLE duration.
        let mut g = GustState {
            phase: GustPhase::Idle,
            phase_start: now,
            phase_duration: Duration::from_secs_f64(30.0),
            peak: 1.0,
            multiplier: 1.0,
        };
        // Just past IDLE duration → should enter ATTACK.
        let m = g.tick(now + Duration::from_millis(30_100), &mut rng);
        assert_eq!(g.phase, GustPhase::Attack);
        assert!(
            (m - 1.0).abs() < 1e-6,
            "first frame of ATTACK must start at 1.0, got {m}"
        );
        assert!(
            g.peak >= GUST_PEAK_MIN && g.peak <= GUST_PEAK_MAX,
            "peak must be sampled into [GUST_PEAK_MIN, GUST_PEAK_MAX], got {}",
            g.peak
        );
    }

    #[test]
    fn gust_attack_ramps_linearly_to_peak() {
        let now = Instant::now();
        let mut rng = fixed_rng();
        let peak = 1.35_f32;
        let mut g = GustState {
            phase: GustPhase::Attack,
            phase_start: now,
            phase_duration: Duration::from_secs_f64(1.0),
            peak,
            multiplier: 1.0,
        };
        // 0% — start.
        assert!(
            (g.tick(now, &mut rng) - 1.0).abs() < 1e-6,
            "attack at t=0 must be 1.0"
        );
        // 50% — midpoint.
        let mid = g.tick(now + Duration::from_millis(500), &mut rng);
        let expected_mid = 1.0 + (peak - 1.0) * 0.5;
        assert!(
            (mid - expected_mid).abs() < 1e-3,
            "attack midpoint must be ~{expected_mid}, got {mid}"
        );
        // 100% (just under) — approaches peak.
        let near = g.tick(now + Duration::from_millis(990), &mut rng);
        assert!(
            (near - peak).abs() < 0.01,
            "attack near-end must approach peak {peak}, got {near}"
        );
    }

    #[test]
    fn gust_full_cycle_returns_to_idle() {
        let now = Instant::now();
        let mut rng = fixed_rng();
        // Pre-set short durations so the test runs fast.
        let mut g = GustState {
            phase: GustPhase::Idle,
            phase_start: now,
            phase_duration: Duration::from_millis(1),
            peak: 1.0,
            multiplier: 1.0,
        };
        // Walk through all 4 phases.
        let mut t = now;
        // IDLE → ATTACK
        t += Duration::from_millis(2);
        g.tick(t, &mut rng);
        assert_eq!(g.phase, GustPhase::Attack);
        // Force ATTACK to complete quickly by overwriting its duration.
        g.phase_duration = Duration::from_millis(1);
        t += Duration::from_millis(2);
        g.tick(t, &mut rng);
        assert_eq!(g.phase, GustPhase::Hold);
        g.phase_duration = Duration::from_millis(1);
        t += Duration::from_millis(2);
        g.tick(t, &mut rng);
        assert_eq!(g.phase, GustPhase::Decay);
        g.phase_duration = Duration::from_millis(1);
        t += Duration::from_millis(2);
        g.tick(t, &mut rng);
        assert_eq!(g.phase, GustPhase::Idle);
        // After full cycle, multiplier is back at 1.0.
        assert!(
            (g.multiplier - 1.0).abs() < 1e-6,
            "multiplier must return to 1.0 after full cycle, got {}",
            g.multiplier
        );
    }

    #[test]
    fn gust_decay_phase_ramps_back_to_one() {
        let now = Instant::now();
        let mut rng = fixed_rng();
        let peak = 1.4_f32;
        let mut g = GustState {
            phase: GustPhase::Decay,
            phase_start: now,
            phase_duration: Duration::from_secs_f64(4.0),
            peak,
            multiplier: peak,
        };
        // Start of decay: should be at peak.
        assert!(
            (g.tick(now, &mut rng) - peak).abs() < 1e-6,
            "decay at t=0 must be at peak"
        );
        // Mid-decay.
        let mid = g.tick(now + Duration::from_secs(2), &mut rng);
        let expected = peak + (1.0 - peak) * 0.5;
        assert!(
            (mid - expected).abs() < 1e-3,
            "decay midpoint must be ~{expected}, got {mid}"
        );
    }
}
