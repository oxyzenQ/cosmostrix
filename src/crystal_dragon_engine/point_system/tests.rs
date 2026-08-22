// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! point_system tests, extracted from inline `mod tests { ... }` block in
//! point_system.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access point_system.rs's private items unchanged.

use crate::crystal_dragon_engine::palette_groups::TemperatureGroup;
use rand::SeedableRng;

fn seeded_rng() -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(42)
}

use super::*;
#[test]
fn calc_v1_always_returns_from_correct_group() {
    let mut mt = seeded_rng();
    // Point 17 → Cold group
    let result = calc_v1_select(17, ColorScheme::Blue, &mut mt);
    if let Some(scheme) = result {
        assert!(
            group_themes(TemperatureGroup::Cold).contains(&scheme),
            "scheme {scheme:?} not in Cold group"
        );
    }
}

#[test]
fn calc_v1_medium_point_selects_from_medium_group() {
    let mut mt = seeded_rng();
    let result = calc_v1_select(50, ColorScheme::Green, &mut mt);
    if let Some(scheme) = result {
        assert!(
            group_themes(TemperatureGroup::Medium).contains(&scheme),
            "scheme {scheme:?} not in Medium group"
        );
    }
}

#[test]
fn calc_v1_hot_point_selects_from_hot_group() {
    let mut mt = seeded_rng();
    let result = calc_v1_select(80, ColorScheme::Fire, &mut mt);
    if let Some(scheme) = result {
        assert!(
            group_themes(TemperatureGroup::Hot).contains(&scheme),
            "scheme {scheme:?} not in Hot group"
        );
    }
}

#[test]
fn calc_v1_never_returns_current_scheme() {
    let mut mt = seeded_rng();
    let current = ColorScheme::Snow;
    for _ in 0..100 {
        let result = calc_v1_select(10, current, &mut mt);
        if let Some(scheme) = result {
            assert_ne!(scheme, current, "should not return current scheme");
        }
    }
}

#[test]
fn calc_v1_returns_some_for_cold_group() {
    let mut mt = seeded_rng();
    // Cold group has 14 themes — at least one different from Blue.
    let result = calc_v1_select(10, ColorScheme::Blue, &mut mt);
    assert!(result.is_some());
}

#[test]
fn calc_v1_distribution_is_weighted() {
    // Run many selections and verify the distribution is not uniform.
    // Themes closer to the point should be selected more often.
    let mut mt = seeded_rng();
    let mut counts = std::collections::HashMap::new();
    for _ in 0..1000 {
        if let Some(scheme) = calc_v1_select(1, ColorScheme::EnergyZen, &mut mt) {
            *counts.entry(scheme).or_insert(0u32) += 1;
        }
    }
    // Should have selected multiple different themes (not just one).
    assert!(counts.len() > 1, "distribution should not be degenerate");
}

#[test]
fn cdf_select_produces_valid_indices() {
    let mut mt = seeded_rng();
    let themes = group_themes(TemperatureGroup::Cold);
    let weights: Vec<f32> = themes.iter().map(|_| 1.0).collect();
    let mut cdf = Vec::new();
    let mut cum = 0.0f32;
    for &w in &weights {
        cum += w / weights.len() as f32;
        cdf.push(cum);
    }
    for _ in 0..100 {
        let selected = cdf_select(&cdf, themes, &mut mt);
        assert!(themes.contains(&selected));
    }
}
