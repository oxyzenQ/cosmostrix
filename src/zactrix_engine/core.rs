// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Small deterministic helpers for the Zactrix Core architecture seam.
//!
//! Zactrix Core is an internal discipline for turning renderer observations
//! into bounded, verifiable decisions. It is inspired by eBPF architecture
//! shapes (probe, map, filter, verifier, bounded history), but it is plain
//! stable Rust and has no Linux eBPF, root, kernel, or BPF dependency.

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
pub(crate) fn monolith_motion_factor(phase: f32, head: f32) -> f32 {
    let wave = triangle_wave01(phase + head * 0.041);
    (0.965 + wave * 0.070).clamp(0.965, 1.035)
}

#[must_use]
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
pub(crate) fn monolith_hero_pulse(phase: f32, segment_offset: u16, head_fraction: f32) -> f32 {
    let wave = triangle_wave01(phase * 0.5 + segment_offset as f32 * 0.073 + head_fraction * 0.5);
    (0.992 + wave * 0.053).clamp(0.992, 1.045)
}

#[must_use]
pub(crate) fn monolith_spine_cadence(phase: f32, layer: u8) -> u16 {
    3 + (((phase.clamp(0.0, 1.0) * 11.0) as u16 + layer as u16) & 1)
}

fn triangle_wave01(value: f32) -> f32 {
    let t = value.rem_euclid(1.0);
    1.0 - (t * 2.0 - 1.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_stability_classification_boundaries_match_existing_output() {
        assert_eq!(classify_frame_jitter(0.49), "low");
        assert_eq!(classify_frame_jitter(0.5), "medium");
        assert_eq!(classify_frame_jitter(2.0), "high");
        assert_eq!(classify_frame_time_stability(0.29), "excellent");
        assert_eq!(classify_frame_time_stability(0.3), "good");
        assert_eq!(classify_frame_time_stability(0.5), "moderate");
        assert_eq!(classify_frame_time_stability(2.0), "high");
    }

    #[test]
    fn dirty_redraw_estimator_is_bounded() {
        assert_eq!(dirty_threshold_cells(0, 3), 0);
        assert_eq!(dirty_threshold_cells(1200, 3), 400);
        assert!(!estimates_full_redraw(0, 1, false, 3));
        assert!(estimates_full_redraw(1200, 400, false, 3));
        assert!(estimates_full_redraw(1200, 0, true, 3));
    }

    #[test]
    fn monolith_subtle_depth_helpers_are_bounded_and_deterministic() {
        let motion = monolith_motion_factor(0.37, 42.0);
        assert!((0.965..=1.035).contains(&motion));
        assert_eq!(motion, monolith_motion_factor(0.37, 42.0));
        for layer in 0..=3 {
            let breath = monolith_breathing_factor(0.11, 88.0, layer);
            assert!((0.965..=1.035).contains(&breath));
        }
        let hero = monolith_hero_pulse(0.61, 12, 0.42);
        assert!((0.992..=1.045).contains(&hero));
        assert_eq!(hero, monolith_hero_pulse(0.61, 12, 0.42));
        for layer in 0..=3 {
            let cadence = monolith_spine_cadence(0.29, layer);
            assert!((3..=4).contains(&cadence));
        }
    }
}
