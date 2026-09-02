// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-alpha.1: crystal-dragon-secs → Cloud wiring tests.
//!
//! The harmony knob's full path: CloudConfig.crystal_dragon_secs →
//! create_cloud → Cloud.crystal_dragon_control.polling_secs → the drift
//! tick + self-reset cadence. These tests pin the config→engine
//! contract (the live-reload application tests live in
//! tests_rejection_msg.rs; the strict-validation tests live in
//! testconf/tests_validation_order.rs).

use super::tests::minimal_cloud_config;

#[test]
fn create_cloud_applies_crystal_dragon_secs_to_engine_control() {
    // The CloudConfig field must reach the ENGINE control (polling_secs),
    // not just sit on the config struct. None → default 60.
    let base = minimal_cloud_config();
    let cloud = base.create_cloud(0.75);
    assert_eq!(
        cloud.crystal_dragon_control.polling_secs, 60.0,
        "unset crystal-dragon-secs must leave the engine at the 60s default"
    );

    let mut tuned = minimal_cloud_config();
    tuned.crystal_dragon_secs = Some(120.0);
    let cloud = tuned.create_cloud(0.75);
    assert_eq!(
        cloud.crystal_dragon_control.polling_secs, 120.0,
        "crystal-dragon-secs=120 must override the engine polling interval"
    );

    // Bounds: 0.0 and 86400.0 both flow through verbatim.
    let mut low = minimal_cloud_config();
    low.crystal_dragon_secs = Some(0.0);
    assert_eq!(
        low.create_cloud(0.75).crystal_dragon_control.polling_secs,
        0.0
    );
    let mut high = minimal_cloud_config();
    high.crystal_dragon_secs = Some(86400.0);
    assert_eq!(
        high.create_cloud(0.75).crystal_dragon_control.polling_secs,
        86400.0
    );
}

#[test]
fn create_cloud_keeps_min_dwell_floor_constant() {
    // The 60s anti-flicker floor is deliberately NOT tunable — polling
    // below 60 shifts cadence, palette flips still cap at one per minute.
    // This locks the over-engineering guard.
    let mut tuned = minimal_cloud_config();
    tuned.crystal_dragon_secs = Some(5.0);
    let cloud = tuned.create_cloud(0.75);
    assert_eq!(
        cloud.crystal_dragon_control.min_dwell_secs, 60.0,
        "min_dwell_secs must stay at the 60s anti-flicker constant"
    );
    assert_eq!(cloud.crystal_dragon_control.polling_secs, 5.0);
}

#[test]
fn effective_crystal_dragon_secs_helper_resolves_default() {
    // The CloudConfig resolve helper: None → default, Some(n) → n (f32).
    let base = minimal_cloud_config();
    assert_eq!(base.effective_crystal_dragon_secs(60.0), 60.0);
    let mut tuned = minimal_cloud_config();
    tuned.crystal_dragon_secs = Some(45.5);
    assert_eq!(tuned.effective_crystal_dragon_secs(60.0), 45.5);
}
