// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic math helpers for monolith breathing, hero pulse, and benchmark
//! classification. These are the only zactrix-origin functions that survived
//! the v11 cleanup — everything else was diagnostic overhead.

#[must_use]
pub(crate) fn classify_frame_jitter(jitter_std_ms: f64) -> &'static str {
    if jitter_std_ms < 0.5 {
        "low"
    } else if jitter_std_ms < 2.0 {
        "medium"
    } else {
        "high"
    }
}

#[must_use]
pub(crate) fn classify_frame_time_stability(jitter_std_ms: f64) -> &'static str {
    if jitter_std_ms < 0.3 {
        "excellent"
    } else if jitter_std_ms < 0.5 {
        "good"
    } else if jitter_std_ms < 2.0 {
        "moderate"
    } else {
        "high"
    }
}

#[must_use]
pub(crate) fn dirty_threshold_cells(total_cells: usize, threshold_divisor: usize) -> usize {
    if total_cells > 0 && threshold_divisor > 0 {
        total_cells / threshold_divisor
    } else {
        0
    }
}

#[must_use]
pub(crate) fn estimates_full_redraw(
    total_cells: usize,
    dirty_cells: usize,
    dirty_all: bool,
    threshold_divisor: usize,
) -> bool {
    dirty_all
        || (total_cells > 0 && dirty_cells >= dirty_threshold_cells(total_cells, threshold_divisor))
}

#[must_use]
#[inline]
pub(crate) fn monolith_motion_factor(phase: f32, head: f32) -> f32 {
    let wave = triangle_wave01(phase + head * 0.041);
    (0.965 + wave * 0.070).clamp(0.965, 1.035)
}

#[must_use]
#[inline]
pub(crate) fn monolith_breathing_factor(phase: f32, head: f32, layer: u8) -> f32 {
    let amplitude = match layer {
        0 => 0.018,
        1 => 0.026,
        _ => 0.034,
    };
    let centered = triangle_wave01(phase + head * 0.027) * 2.0 - 1.0;
    (1.0 + centered * amplitude).clamp(0.965, 1.035)
}

#[must_use]
#[inline]
pub(crate) fn monolith_hero_pulse(phase: f32, segment_offset: u16, head_fraction: f32) -> f32 {
    let wave = triangle_wave01(phase * 0.5 + segment_offset as f32 * 0.073 + head_fraction * 0.5);
    (0.992 + wave * 0.053).clamp(0.992, 1.045)
}

#[must_use]
#[inline]
pub(crate) fn monolith_spine_cadence(phase: f32, layer: u8) -> u16 {
    3 + (((phase.clamp(0.0, 1.0) * 11.0) as u16 + layer as u16) & 1)
}

#[inline]
fn triangle_wave01(value: f32) -> f32 {
    let t = value.rem_euclid(1.0);
    1.0 - (t * 2.0 - 1.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_frame_jitter_boundaries() {
        assert_eq!(classify_frame_jitter(0.0), "low");
        assert_eq!(classify_frame_jitter(0.49), "low");
        assert_eq!(classify_frame_jitter(0.5), "medium");
        assert_eq!(classify_frame_jitter(1.99), "medium");
        assert_eq!(classify_frame_jitter(2.0), "high");
    }

    #[test]
    fn classify_frame_time_stability_boundaries() {
        assert_eq!(classify_frame_time_stability(0.0), "excellent");
        assert_eq!(classify_frame_time_stability(0.29), "excellent");
        assert_eq!(classify_frame_time_stability(0.3), "good");
        assert_eq!(classify_frame_time_stability(0.49), "good");
        assert_eq!(classify_frame_time_stability(0.5), "moderate");
        assert_eq!(classify_frame_time_stability(1.99), "moderate");
        assert_eq!(classify_frame_time_stability(2.0), "high");
    }

    #[test]
    fn dirty_threshold_cells_divides_correctly() {
        assert_eq!(dirty_threshold_cells(4800, 3), 1600);
        assert_eq!(dirty_threshold_cells(0, 3), 0);
        assert_eq!(dirty_threshold_cells(4800, 0), 0);
    }

    #[test]
    fn estimates_full_redraw_when_dirty_all() {
        assert!(estimates_full_redraw(4800, 0, true, 3));
        assert!(!estimates_full_redraw(4800, 0, false, 3));
        assert!(estimates_full_redraw(4800, 1600, false, 3));
        assert!(!estimates_full_redraw(4800, 1599, false, 3));
    }

    #[test]
    fn monolith_motion_factor_is_bounded() {
        for phase in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for head in [0.0, 10.0, 50.0, 100.0] {
                let v = monolith_motion_factor(phase, head);
                assert!((0.965..=1.035).contains(&v), "motion out of range: {v}");
            }
        }
    }

    #[test]
    fn monolith_breathing_factor_is_bounded() {
        for layer in 0..=2 {
            for phase in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for head in [0.0, 10.0, 50.0, 100.0] {
                    let v = monolith_breathing_factor(phase, head, layer);
                    assert!((0.965..=1.035).contains(&v), "breath out of range: {v}");
                }
            }
        }
    }

    #[test]
    fn monolith_hero_pulse_is_bounded() {
        for phase in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for offset in [0u16, 5, 10, 20] {
                for frac in [0.0, 0.25, 0.5, 1.0] {
                    let v = monolith_hero_pulse(phase, offset, frac);
                    assert!((0.992..=1.045).contains(&v), "hero out of range: {v}");
                }
            }
        }
    }

    #[test]
    fn monolith_spine_cadence_returns_3_or_4() {
        for layer in 0..=2 {
            for phase in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let v = monolith_spine_cadence(phase, layer);
                assert!(v == 3 || v == 4, "cadence should be 3 or 4, got {v}");
            }
        }
    }
}
