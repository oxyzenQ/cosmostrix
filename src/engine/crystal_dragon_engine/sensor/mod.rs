// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon sensor: CPU polling + CLOCK fallback.
//!
//! Reads the system state and produces a point (1–99) that the
//! point system maps to a temperature group and color theme.
//!
//! ## CPU mode (primary)
//!
//! Samples process CPU% via `cpustat::current_cpu_ns()`, smooths with
//! an EMA, and maps to 1–99 using a sqrt curve:
//!
//! ```text
//! point = clamp(1, 99, round(sqrt(cpu_ema) * 9.9))
//! ```
//!
//! The sqrt curve spreads the distribution across all 3 temperature
//! groups at realistic CPU usage levels. cosmostrix is a highly
//! optimized single-threaded renderer — typical interactive CPU usage
//! is 0.5–8%. With the old linear mapping (`cpu * 0.99`), this
//! produced points 1–8 (always Cold group → blues/cyans/whites only).
//! The sqrt mapping produces:
//!
//! | CPU% | Linear point | Sqrt point | Group |
//! |------|-------------|------------|-------|
//! | 0.5  | 1           | 7          | Cold  |
//! | 1    | 1           | 10         | Cold  |
//! | 2    | 2           | 14         | Cold  |
//! | 5    | 5           | 22         | Cold  |
//! | 8    | 8           | 28         | Cold  |
//! | 12   | 12          | 34         | Medium|
//! | 20   | 20          | 44         | Medium|
//! | 34   | 34          | 58         | Medium|
//! | 50   | 50          | 70         | Hot   |
//! | 67   | 66          | 81         | Hot   |
//! | 100  | 98          | 99         | Hot   |
//!
//! Now Medium group (greens/purples) is reachable at ~12% CPU, and Hot
//! group (yellows/reds/fire) at ~50% CPU — matching the owner's design
//! intent of full color variety across the day.
//!
//! ## CLOCK mode (fallback)
//!
//! When CPU sampling is unsupported (Windows, some sandboxes), derives
//! a point from UTC time-of-day:
//!
//! ```text
//! hour_frac = hour + minute / 60.0
//! point = clamp(1, 99, round(1.0 + hour_frac * 4.083))
//! ```
//!
//! This produces point ~1 at 00:00 (midnight → Cold) and point ~99
//! at 23:59 (late evening → Hot), with a smooth ramp through Medium
//! during the day. The mapping is intentionally monotonic — time of
//! day directly maps to color temperature, giving a natural day/night
//! cycle without CPU dependency.

use std::time::Instant;

use crate::cpustat;
use crate::crystal_dragon_engine::crystal_dragon_control::{
    CrystalDragonControl, CrystalDragonSensorMode,
};
use crate::crystal_dragon_engine::palette_groups::TemperatureGroup;

// ── Point range ──────────────────────────────────────────────────────────

/// Minimum point value (inclusive). Points below this are clamped up.
const POINT_MIN: u8 = 1;

/// Maximum point value (inclusive). Points above this are clamped down.
const POINT_MAX: u8 = 99;

// ── Group boundaries ─────────────────────────────────────────────────────

/// Upper boundary (inclusive) of the Cold group. Points 1–33 → Cold.
const COLD_MAX: u8 = 33;

/// Upper boundary (inclusive) of the Medium group. Points 34–66 → Medium.
const MEDIUM_MAX: u8 = 66;

// Hot group: points 67–99 (no explicit const needed).

// ── Sensor struct ────────────────────────────────────────────────────────

/// Crystal Dragon sensor state. Persists across poll ticks.
///
/// Carried as a field on `Cloud`. Owns the EMA-smoothed CPU%,
/// the last sample timestamp, and the active sensor mode.
#[derive(Clone, Copy)]
pub(crate) struct CrystalDragonSensor {
    /// EMA-smoothed CPU%. `None` until the first sample is taken.
    cpu_ema: Option<f32>,
    /// Last wall-clock instant when CPU was sampled.
    last_poll: Option<Instant>,
    /// Last raw CPU-ns reading (for delta computation).
    last_cpu_ns: Option<u64>,
    /// Current point (1–99). Updated each poll tick.
    current_point: u8,
    /// When the current theme was entered. Used for dwell hysteresis.
    theme_entered_at: Instant,
    /// Whether CPU sampling is supported on this platform.
    ///
    /// Probed once at construction. When `false`, sensor falls back
    /// to CLOCK mode regardless of config.
    cpu_supported: bool,
    /// Effective sensor mode (may differ from config if CPU unsupported).
    effective_mode: CrystalDragonSensorMode,
    /// EMA alpha for CPU% smoothing.
    ///
    /// S-master-1-v2: copied from `CrystalDragonControl::cpu_ema_alpha` at
    /// construction so the control field is the single source of truth
    /// (the `CRYSTAL_DRAGON_CPU_EMA_ALPHA` const only seeds the default).
    cpu_ema_alpha: f32,
}

impl CrystalDragonSensor {
    /// Construct a new sensor. Probes CPU sampling support once.
    ///
    /// `now` should be `Instant::now()` from the caller's context.
    /// `control` provides the configured sensor mode; if CPU is
    /// unsupported, the effective mode falls back to CLOCK.
    pub(crate) fn new(now: Instant, control: CrystalDragonControl) -> Self {
        let initial_cpu_ns = cpustat::current_cpu_ns();
        let cpu_supported = initial_cpu_ns.is_some();
        let effective_mode = if cpu_supported {
            control.sensor_mode
        } else {
            CrystalDragonSensorMode::Clock
        };
        // Start at point 17 (lower-middle of Cold group) for a calm
        // cold-start. This avoids an immediate theme change on the
        // first poll tick.
        Self {
            cpu_ema: None,
            last_poll: Some(now),
            last_cpu_ns: initial_cpu_ns,
            current_point: 17,
            theme_entered_at: now,
            cpu_supported,
            effective_mode,
            cpu_ema_alpha: control.cpu_ema_alpha,
        }
    }

    /// Poll the sensor and compute a new point.
    ///
    /// Called every `polling_secs` seconds by the Crystal Dragon tick.
    /// Samples CPU (if in CPU mode) or reads UTC time (if in CLOCK mode),
    /// then maps to a 1–99 point.
    ///
    /// `now` is the caller's `Instant`.
    pub(crate) fn poll(&mut self, now: Instant) {
        self.current_point = match self.effective_mode {
            CrystalDragonSensorMode::Cpu => self.poll_cpu(now),
            CrystalDragonSensorMode::Clock => self.poll_clock(),
        };
    }

    /// Current point (1–99). Read by the point system to select a theme.
    pub(crate) fn current_point(self) -> u8 {
        self.current_point
    }

    /// Current temperature group derived from the current point.
    pub(crate) fn current_group(self) -> TemperatureGroup {
        point_to_group(self.current_point)
    }

    /// When the current theme was entered. Used for dwell hysteresis.
    pub(crate) fn theme_entered_at(self) -> Instant {
        self.theme_entered_at
    }

    /// Record that a theme transition just occurred at `now`.
    pub(crate) fn record_theme_transition(&mut self, now: Instant) {
        self.theme_entered_at = now;
    }

    /// Whether CPU sampling is supported on this platform.
    pub(crate) fn cpu_supported(self) -> bool {
        self.cpu_supported
    }

    /// Last EMA-smoothed CPU% reading. `None` before first sample or
    /// when CPU is unsupported. Used by `--doctor` for diagnostics.
    pub(crate) fn cpu_ema(self) -> Option<f32> {
        self.cpu_ema
    }

    /// Shift all internal timestamps by `elapsed`. Called on resume
    /// from pause so the sensor doesn't think a long pause was a
    /// dwell period.
    pub(crate) fn shift_in_time(&mut self, elapsed: std::time::Duration) {
        if let Some(ref mut t) = self.last_poll {
            *t += elapsed;
        }
        self.theme_entered_at += elapsed;
    }

    // ── Private: CPU sampling ────────────────────────────────────────

    /// Sample CPU%, update EMA, map to 1–99 point.
    fn poll_cpu(&mut self, now: Instant) -> u8 {
        let cpu = self.sample_cpu_percent(now);
        match cpu {
            Some(pct) => {
                // Sqrt curve: spreads low CPU values across a wider point
                // range so all 3 temperature groups are reachable.
                // Linear (cpu*0.99) bottlenecked cosmostrix's typical
                // 0.5-8% CPU into points 1-8 (always Cold group).
                // Sqrt maps: 1%→10, 4%→20, 9%→30, 16%→40, 25%→50, 100%→99.
                let raw = (pct.sqrt() * 9.9).clamp(1.0, 99.0);
                (raw.round() as u8).clamp(POINT_MIN, POINT_MAX)
            }
            None => self.poll_clock(), // Fallback if sample failed mid-run
        }
    }

    /// Sample process CPU% and update the EMA. Returns the smoothed
    /// CPU% (or `None` if the sample failed).
    fn sample_cpu_percent(&mut self, now: Instant) -> Option<f32> {
        let prev_sample = self.last_poll?;
        let prev_ns = self.last_cpu_ns?;
        let wall_delta = now.saturating_duration_since(prev_sample).as_nanos();
        self.last_poll = Some(now);
        let cpu_ns_now = cpustat::current_cpu_ns()?;
        self.last_cpu_ns = Some(cpu_ns_now);
        if wall_delta == 0 {
            return self.cpu_ema;
        }
        let cpu_delta = cpu_ns_now.saturating_sub(prev_ns) as f64;
        let percent = ((cpu_delta / wall_delta as f64) * 100.0).clamp(0.0, 999.9) as f32;
        let smoothed = match self.cpu_ema {
            None => percent,
            Some(prev) => prev * (1.0 - self.cpu_ema_alpha) + percent * self.cpu_ema_alpha,
        };
        self.cpu_ema = Some(smoothed);
        self.cpu_ema
    }

    // ── Private: CLOCK fallback ──────────────────────────────────────

    /// Derive point from UTC time-of-day.
    ///
    /// Maps 00:00 → 1 (Cold), 12:00 → ~50 (Medium), 23:59 → 99 (Hot).
    /// Uses a monotonic ramp so time of day directly controls color
    /// temperature — a natural day/night cycle.
    fn poll_clock(&self) -> u8 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Hours since midnight (UTC)
        let hour_of_day = (secs % 86400) / 3600;
        let minute_of_hour = (secs % 3600) / 60;
        let hour_frac = hour_of_day as f32 + minute_of_hour as f32 / 60.0;
        // Map 0–24 hours → 1–99 points
        // 4.083 = (99 - 1) / 24 ≈ 4.083
        let raw = 1.0 + hour_frac * 4.083;
        (raw.round() as u8).clamp(POINT_MIN, POINT_MAX)
    }
}

// ── Pure functions ───────────────────────────────────────────────────────

/// Map a point (1–99) to a temperature group.
///
/// ```text
/// 1–33  → Cold
/// 34–66 → Medium
/// 67–99 → Hot
/// ```
#[must_use]
pub(crate) fn point_to_group(point: u8) -> TemperatureGroup {
    if point <= COLD_MAX {
        TemperatureGroup::Cold
    } else if point <= MEDIUM_MAX {
        TemperatureGroup::Medium
    } else {
        TemperatureGroup::Hot
    }
}

/// Map a temperature group to its point range (lo, hi inclusive).
#[must_use]
pub(crate) fn group_point_range(group: TemperatureGroup) -> (u8, u8) {
    match group {
        TemperatureGroup::Cold => (1, COLD_MAX),
        TemperatureGroup::Medium => (COLD_MAX + 1, MEDIUM_MAX),
        TemperatureGroup::Hot => (MEDIUM_MAX + 1, POINT_MAX),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
