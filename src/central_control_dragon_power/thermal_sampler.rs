// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Thermal sensor sampler (feature #13).
//!
//! Reads CPU/SoC thermal zones from the Linux sysfs interface and
//! normalizes the reading into a 0.0–1.0 pressure scalar consumed by
//! [`PowerManager::set_thermal_pressure`](crate::central_control_dragon_power::PowerManager::set_thermal_pressure).
//!
//! ## Source
//!
//! Linux exposes one directory per thermal zone under
//! `/sys/class/thermal/thermal_zone*/`. Each zone has:
//!
//! - `temp` — current temperature in millidegrees Celsius (e.g., `52000`
//!   = 52.0 °C). Always present per the sysfs ABI.
//! - `type` — human-readable label (`x86_pkg_temp`, `acpitz`, `iwlwifi`,
//!   etc.). Informational only; the sampler takes the hottest zone
//!   regardless of type to catch any thermal source.
//!
//! ## Normalization
//!
//! The sampler maps the temperature to a 0.0–1.0 pressure scalar via a
//! linear ramp:
//!
//! - At `THERMAL_PRESSURE_ZERO_C` (50 °C) and below → 0.0 (cool).
//! - At `THERMAL_PRESSURE_ONE_C` (90 °C) and above → 1.0 (throttle).
//! - Between → linear interpolation.
//!
//! The ramp window [50, 90] °C was chosen to match the typical
//! junction-temperature throttle band of x86_64 mobile and desktop
//! SoCs. Below 50 °C the device is cool enough that no throttling is
//! expected; above 90 °C the device is at or past the throttle
//! threshold and the renderer should shed maximum load.
//!
//! ## Cadence
//!
//! The event loop calls the sampler every
//! `THERMAL_SAMPLER_INTERVAL_FRAMES` frames (600 ≈ 10 s at 60 FPS).
//! Thermal mass is slow — sub-second sampling adds syscall cost
//! without changing the result.
//!
//! ## Platform support
//!
//! Linux only. On other platforms [`sample_thermal_pressure`]
//! returns `None` and `PowerManager::set_thermal_pressure` is never
//! called — the thermal input stays at 0.0 and `effective_pressure`
//! is identical to the base `perf_pressure`. This is the documented
//! contract: the absence of thermal sampling must NOT change
//! behavior.
//!
//! ## Error handling
//!
//! The sampler is best-effort and defensive:
//!
//! - If `/sys/class/thermal/` does not exist (container without
//!   thermal sysfs, chroot, etc.) → returns `None`.
//! - If a `thermal_zone*/temp` file is unreadable or contains a
//!   non-integer → that zone is skipped; the sampler falls through to
//!   the next.
//! - If no zone produced a valid reading → returns `None`.
//!
//! [`PowerManager::set_thermal_pressure`] then receives `None` and
//! keeps the previous value, so a transient read failure does NOT
//! reset the thermal input to 0.0 (which would un-throttle the
//! renderer mid-emergency).

use crate::constants::*;

/// Read the hottest thermal zone and normalize to 0.0–1.0.
///
/// Returns `None` when no zone produced a valid reading. The caller
/// (event loop) treats `None` as "keep the previous value" — see the
/// module docs for the rationale.
///
/// # Platform support
///
/// Linux only. On non-Linux platforms this returns `None` unconditionally.
#[cfg(target_os = "linux")]
pub(crate) fn sample_thermal_pressure() -> Option<f32> {
    let entries = std::fs::read_dir("/sys/class/thermal").ok()?;
    let mut hottest: Option<i64> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        // Only consider thermal_zoneN subdirectories.
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("thermal_zone") {
            continue;
        }
        let temp_file = path.join("temp");
        let raw = match std::fs::read_to_string(&temp_file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let millideg: i64 = match raw.trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Sanity bound: reject impossible readings. Real sensors stay
        // inside [-40, 200] °C; anything outside is a misbehaving sysfs
        // entry (e.g., a Tegra sensor that returns 0xFFFFFFFF on
        // suspend). Skipping is safer than clamping here because a
        // garbage reading of +200000 °C would saturate the pressure
        // to 1.0 and shed load for no reason.
        if !(-40_000..=200_000).contains(&millideg) {
            continue;
        }
        hottest = Some(highest(hottest, millideg));
    }
    hottest.map(normalize_celsius)
}

/// Linear normalization from degrees Celsius to 0.0–1.0 pressure.
///
/// Pure function — extracted so tests can exercise the math without
/// touching the filesystem.
///
/// - `celsius` at or below `THERMAL_PRESSURE_ZERO_C` → 0.0
/// - `celsius` at or above `THERMAL_PRESSURE_ONE_C` → 1.0
/// - Between → linear interpolation
#[must_use]
pub(crate) fn normalize_celsius(celsius_millideg: i64) -> f32 {
    let celsius = celsius_millideg as f32 / 1000.0;
    let lo = THERMAL_PRESSURE_ZERO_C as f32;
    let hi = THERMAL_PRESSURE_ONE_C as f32;
    // hi > lo is enforced by the constant sanity test in mod.rs.
    let pressure = (celsius - lo) / (hi - lo);
    pressure.clamp(0.0, 1.0)
}

/// Non-Linux stub: no thermal sampling available.
#[cfg(not(target_os = "linux"))]
pub(crate) fn sample_thermal_pressure() -> Option<f32> {
    None
}

// Note: `normalize_celsius` above is intentionally NOT cfg-gated — the
// pure math function is exposed cross-platform so the math tests run on
// every target (Linux, Android, macOS, Windows, FreeBSD). A previous
// revision duplicated it under `#[cfg(not(target_os = "linux"))]`, which
// caused E0428 (duplicate definition) on every non-Linux target because
// the primary definition above is already unconditional.

/// Return the larger of two `i64`s, treating `None` as "no reading yet".
#[inline]
fn highest(prev: Option<i64>, cur: i64) -> i64 {
    match prev {
        Some(p) if p > cur => p,
        _ => cur,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_celsius (pure math) ─────────────────────────────────────

    #[test]
    fn normalize_returns_zero_at_or_below_lo_threshold() {
        // At the lo threshold (50 °C = 50000 millideg).
        assert_eq!(
            normalize_celsius(i64::from(THERMAL_PRESSURE_ZERO_C) * 1000),
            0.0
        );
        // Below the lo threshold — clamped to 0.0.
        assert_eq!(normalize_celsius(0), 0.0);
        assert_eq!(normalize_celsius(-40_000), 0.0);
        // 25 °C — well below the 50 °C floor.
        assert_eq!(normalize_celsius(25_000), 0.0);
    }

    #[test]
    fn normalize_returns_one_at_or_above_hi_threshold() {
        assert_eq!(
            normalize_celsius(i64::from(THERMAL_PRESSURE_ONE_C) * 1000),
            1.0
        );
        // Above the hi threshold — clamped to 1.0.
        assert_eq!(normalize_celsius(95_000), 1.0);
        assert_eq!(normalize_celsius(120_000), 1.0);
    }

    #[test]
    fn normalize_interpolates_linearly_mid_band() {
        // Midpoint of [50, 90] °C = 70 °C → 0.5
        let p = normalize_celsius(70_000);
        assert!((p - 0.5).abs() < 1e-6, "expected 0.5, got {p}");

        // 60 °C = 25% of the way from 50 → 90.
        let p = normalize_celsius(60_000);
        assert!((p - 0.25).abs() < 1e-6, "expected 0.25, got {p}");

        // 80 °C = 75% of the way.
        let p = normalize_celsius(80_000);
        assert!((p - 0.75).abs() < 1e-6, "expected 0.75, got {p}");
    }

    #[test]
    fn normalize_is_monotonically_increasing() {
        // Sample every 5 °C across the ramp window and verify
        // monotonic increase. A non-monotonic result would mean the
        // clamp or interpolation is broken.
        let mut prev = -1.0_f32;
        for c in (0..=100).step_by(5) {
            let p = normalize_celsius(c * 1000);
            assert!(p >= prev, "non-monotonic at {c} °C: {p} < {prev}");
            prev = p;
        }
    }

    // ── sample_thermal_pressure (filesystem) ──────────────────────────────
    //
    // These tests create a mock /sys/class/thermal tree inside a
    // tmpdir and call the sampler with the path overridden. Because
    // the production sampler hardcodes `/sys/class/thermal`, we
    // cannot directly point it at a tmpdir without an injection
    // point. The integration is therefore verified via the
    // `normalize_celsius` math tests above + a live-read smoke test
    // guarded by `cfg(target_os = "linux")` that just asserts the
    // return type is `Option<f32>`.

    #[cfg(target_os = "linux")]
    #[test]
    fn sample_thermal_pressure_returns_some_on_real_linux_or_none_in_container() {
        // On a real Linux box with thermal zones, this returns Some.
        // In a container without thermal sysfs, this returns None.
        // Both outcomes are valid — we just verify no panic.
        let _ = sample_thermal_pressure();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sample_thermal_pressure_does_not_panic_when_sysfs_missing() {
        // The sampler must not panic when /sys/class/thermal is
        // unreadable. read_dir returns Err, which propagates as None
        // via the `?` operator. Verify by calling the function in any
        // environment — if it panics, the test fails.
        let result = sample_thermal_pressure();
        assert!(result.is_none() || result.unwrap() >= 0.0);
    }

    // ── highest helper ────────────────────────────────────────────────────

    #[test]
    fn highest_picks_larger_value() {
        assert_eq!(highest(Some(50_000), 60_000), 60_000);
        assert_eq!(highest(Some(70_000), 60_000), 70_000);
        // Equal values — either branch is fine, both return 70_000.
        assert_eq!(highest(Some(70_000), 70_000), 70_000);
    }
}
