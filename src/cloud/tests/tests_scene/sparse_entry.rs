// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sparse fresh-entry density tests — alive-count bounded, ramp start/clear,
//! repeated cycling stays sparse.

use super::make_monolith_cloud;
use crate::constants::WARM_START_SEED_MAX;
use crate::rain_style::RainStyle;

/// After monolith → matrix, the number of warm-started alive droplets must
/// be bounded by WARM_START_SEED_MAX — no per-column flooding.
#[test]
fn sparse_entry_matrix_alive_count_bounded() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive <= WARM_START_SEED_MAX,
        "sparse entry: alive droplets ({alive}) must be <= WARM_START_SEED_MAX ({WARM_START_SEED_MAX})"
    );
    assert!(
        alive >= 3,
        "sparse entry: must have at least 3 alive droplets for no-blank guarantee (got {alive})"
    );
}

/// After monolith → signal, the same sparse bound applies.
#[test]
fn sparse_entry_signal_alive_count_bounded() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("signal", "binary", &[], false);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive <= WARM_START_SEED_MAX,
        "sparse entry signal: alive ({alive}) must be <= WARM_START_SEED_MAX ({WARM_START_SEED_MAX})"
    );
    assert!(
        alive >= 3,
        "sparse entry signal: must have at least 3 alive droplets (got {alive})"
    );
}

/// The scene-entry ramp must be active immediately after switching to glyph.
#[test]
fn sparse_entry_ramp_starts_on_scene_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(
        cloud.glyph_entry_time.is_some(),
        "glyph_entry_time must be set after switching to glyph scene"
    );
}

/// The scene-entry ramp must be cleared when switching back to monolith.
#[test]
fn sparse_entry_ramp_cleared_on_monolith_switch() {
    let mut cloud = make_monolith_cloud();
    cloud.apply_scene_runtime("matrix", "binary", &[], false);
    assert!(cloud.glyph_entry_time.is_some());
    cloud.apply_scene_runtime("monolith", "binary", &[], false);
    assert!(
        cloud.glyph_entry_time.is_none(),
        "glyph_entry_time must be cleared when switching to monolith"
    );
}

/// Repeated x cycling must never overpopulate initial glyph scenes.
/// After each switch, alive count must stay within the sparse bound.
#[test]
fn sparse_entry_repeated_forward_stays_sparse() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
            assert!(
                alive <= WARM_START_SEED_MAX,
                "forward cycle '{scene}': alive ({alive}) must be <= {WARM_START_SEED_MAX}"
            );
        }
    }
}

/// Repeated X forward cycling must also stay sparse.
#[test]
fn sparse_entry_repeated_uppercase_forward_stays_sparse() {
    let mut cloud = make_monolith_cloud();
    let scenes = ["matrix", "signal", "monolith", "matrix", "signal"];
    for scene in &scenes {
        cloud.apply_scene_runtime(scene, "binary", &[], false);
        if matches!(cloud.rain_style(), RainStyle::Glyph) {
            let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
            assert!(
                alive <= WARM_START_SEED_MAX,
                "uppercase forward cycle '{scene}': alive ({alive}) must be <= {WARM_START_SEED_MAX}"
            );
        }
    }
}
