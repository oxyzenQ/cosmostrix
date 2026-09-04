// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: --no-effects died after the first
//! config.toml live-reload): the effects-gate wiring tests.
//!
//! create_cloud owns the gate (CloudConfig.effects_enabled → Cloud at
//! CONSTRUCTION) so the live-reload rebuild (create_cloud +
//! inherit_ecosystem_state + swap) can never silently re-enable
//! cosmetics on a --no-effects session. Previously only
//! event_loop_setup applied the flag (startup path), so the fresh
//! rebuild cloud fell back to the Cloud::new default `true`.

use std::collections::HashMap;

use super::rebuild_cloud_config;
use super::tests::minimal_cloud_config;

#[test]
fn create_cloud_applies_effects_gate_at_construction() {
    let mut no_fx = minimal_cloud_config();
    no_fx.effects_enabled = false;
    let cloud = no_fx.create_cloud(0.75);
    assert!(
        !cloud.effects_enabled,
        "create_cloud must bake effects_enabled=false into the Cloud (--no-effects)"
    );

    let with_fx = minimal_cloud_config();
    let cloud = with_fx.create_cloud(0.75);
    assert!(
        cloud.effects_enabled,
        "default config keeps effects enabled"
    );
}

#[test]
fn rebuild_preserves_effects_gate_for_no_effects_runs() {
    // The full defect path: rebuild_cloud_config (what the render thread
    // runs on every config save) must carry effects_enabled from the
    // locked startup base — an unrelated key edit (crystal-dragon) must
    // NOT resurrect cosmetics on a --no-effects session.
    let mut base = minimal_cloud_config();
    base.effects_enabled = false; // --no-effects at startup
    let mut edit = HashMap::new();
    edit.insert("crystal-dragon".to_string(), "true".to_string());
    let new_cfg = rebuild_cloud_config(&base, &edit);
    assert!(
        !new_cfg.effects_enabled,
        "the rebuilt CloudConfig must keep the startup effects gate"
    );
    // And the Cloud built from it (what the rebuild swaps in):
    let cloud = new_cfg.create_cloud(0.75);
    assert!(
        !cloud.effects_enabled,
        "the fresh rebuild Cloud must keep --no-effects (no cosmetic resurrection)"
    );
}
