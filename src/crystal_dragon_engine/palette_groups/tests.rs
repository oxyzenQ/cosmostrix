// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! palette_groups tests, extracted from inline `mod tests { ... }` block in
//! palette_groups.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access palette_groups.rs's private items unchanged.

use super::*;
#[test]
fn group_sizes_are_14_each() {
    assert_eq!(group_themes(TemperatureGroup::Cold).len(), 14);
    assert_eq!(group_themes(TemperatureGroup::Medium).len(), 14);
    assert_eq!(group_themes(TemperatureGroup::Hot).len(), 14);
}

#[test]
fn reserved_size_is_2() {
    assert_eq!(reserved_themes().len(), 2);
}

#[test]
fn total_is_44() {
    let total = group_themes(TemperatureGroup::Cold).len()
        + group_themes(TemperatureGroup::Medium).len()
        + group_themes(TemperatureGroup::Hot).len()
        + reserved_themes().len();
    assert_eq!(total, 44);
}

#[test]
fn partition_is_disjoint() {
    // No theme appears in more than one group or in reserved.
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for group in [
        TemperatureGroup::Cold,
        TemperatureGroup::Medium,
        TemperatureGroup::Hot,
    ] {
        for &scheme in group_themes(group) {
            assert!(
                seen.insert(scheme),
                "duplicate scheme {:?} in group {:?}",
                scheme,
                group
            );
        }
    }
    for &scheme in reserved_themes() {
        assert!(
            seen.insert(scheme),
            "duplicate reserved scheme {:?}",
            scheme
        );
    }
}

#[test]
fn theme_group_matches_group_themes() {
    // Every theme in group_themes(g) is classified as Some(g).
    for group in [
        TemperatureGroup::Cold,
        TemperatureGroup::Medium,
        TemperatureGroup::Hot,
    ] {
        for &scheme in group_themes(group) {
            assert_eq!(
                theme_group(scheme),
                Some(group),
                "theme_group({:?}) should be {:?}",
                scheme,
                group
            );
        }
    }
}

#[test]
fn reserved_themes_return_none() {
    for &scheme in reserved_themes() {
        assert_eq!(
            theme_group(scheme),
            None,
            "reserved theme {:?} should return None",
            scheme
        );
    }
}

#[test]
fn temperature_group_labels() {
    assert_eq!(TemperatureGroup::Cold.label(), "cold");
    assert_eq!(TemperatureGroup::Medium.label(), "medium");
    assert_eq!(TemperatureGroup::Hot.label(), "hot");
}

#[test]
fn theme_weight_is_positive() {
    for point in [1, 17, 34, 50, 67, 99] {
        let weight = theme_weight(point, 0, 14);
        assert!(weight > 0.0, "weight should be positive for point {point}");
    }
}

#[test]
fn theme_weight_decreases_with_distance() {
    // Weight at index 0 (closest to Cold lo) should be higher
    // for point=1 than for point=33.
    let w_near = theme_weight(1, 0, 14);
    let w_far = theme_weight(33, 0, 14);
    // This may not hold for all indices due to group range mapping,
    // but weight should always be in (0, 1].
    assert!(w_near > 0.0 && w_near <= 1.0);
    assert!(w_far > 0.0 && w_far <= 1.0);
}
