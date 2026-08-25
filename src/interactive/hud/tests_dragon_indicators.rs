// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// v50.0.0-beta.6: power-dragon and crystal-dragon on/off HUD indicators.
// Extracted from tests.rs to keep that file under the 1500-LOC cap.
// Tests verify: default values, live-reload toggle, layout (prdr/crdr
// above cid), and that values are NOT hardcoded (track runtime state).

use super::*;
use std::time::Instant;

#[test]
fn hud_prdr_defaults_to_on() {
    // Default power_dragon is true (protection enabled) — matches
    // CloudConfig default. The HUD must show "prdr: on" from frame 1
    // before any setter is called.
    let mut h = HudState::new();
    h.toggle();
    h.update_metrics(&[]);
    let (_, prdr_line) = &h.cached_lines[15];
    assert_eq!(
        prdr_line, " prdr: on",
        "prdr line must default to 'on' (power_dragon default = true)"
    );
}

#[test]
fn hud_crdr_defaults_to_off() {
    // Default crystal_dragon is false (drift off — palette is static).
    // The HUD must show "crdr: off" from frame 1.
    let mut h = HudState::new();
    h.toggle();
    h.update_metrics(&[]);
    let (_, crdr_line) = &h.cached_lines[16];
    assert_eq!(
        crdr_line, " crdr: off",
        "crdr line must default to 'off' (crystal_dragon default = false)"
    );
}

#[test]
fn hud_set_power_dragon_off_renders_off() {
    // Simulate live-reload: user sets power_dragon = false in config.toml.
    // The event loop calls set_power_dragon(false), and the next 1 Hz
    // metric tick must render "prdr: off".
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(false);
    h.update_metrics(&[]);
    let (_, prdr_line) = &h.cached_lines[15];
    assert_eq!(
        prdr_line, " prdr: off",
        "prdr line must show 'off' after set_power_dragon(false)"
    );
}

#[test]
fn hud_set_crystal_dragon_on_renders_on() {
    // Simulate live-reload: user sets crystal_dragon = true in config.toml.
    // The event loop calls set_crystal_dragon(true), and the next 1 Hz
    // metric tick must render "crdr: on".
    let mut h = HudState::new();
    h.toggle();
    h.set_crystal_dragon(true);
    h.update_metrics(&[]);
    let (_, crdr_line) = &h.cached_lines[16];
    assert_eq!(
        crdr_line, " crdr: on",
        "crdr line must show 'on' after set_crystal_dragon(true)"
    );
}

#[test]
fn hud_prdr_crdr_above_cid_in_layout() {
    // Owner mandate: "add this new 2 metrics on top of cid/commit id
    // indicator mean cid indicator keep last position metrics".
    // Verify the layout: prdr at 15, crdr at 16, cid at 17 (last).
    let h = HudState::new();
    // cid must be at row 17 (the last row).
    let (_, cid_line) = &h.cached_lines[17];
    assert!(
        cid_line.starts_with(" cid: "),
        "row 17 must be the cid line, got: {cid_line:?}"
    );
    // prdr and crdr initialize as empty strings (populated by update_metrics
    // at the 1 Hz tick). Verify they are at rows 15 and 16 respectively
    // by calling update_metrics and checking the rendered text.
    let mut h2 = HudState::new();
    h2.toggle();
    h2.update_metrics(&[]);
    let (_, prdr_line) = &h2.cached_lines[15];
    let (_, crdr_line) = &h2.cached_lines[16];
    assert!(
        prdr_line.starts_with(" prdr: "),
        "row 15 must be the prdr line, got: {prdr_line:?}"
    );
    assert!(
        crdr_line.starts_with(" crdr: "),
        "row 16 must be the crdr line, got: {crdr_line:?}"
    );
    // cid is still at row 17 (unchanged from h).
    let (_, cid_line_2) = &h2.cached_lines[17];
    assert!(
        cid_line_2.starts_with(" cid: "),
        "row 17 must still be the cid line after update_metrics, got: {cid_line_2:?}"
    );
}

#[test]
fn hud_prdr_crdr_live_reload_toggle() {
    // Simulate a full live-reload cycle: start with defaults (prdr=on,
    // crdr=off), then toggle both via setters (simulating config.toml
    // live-reload), then toggle back. The HUD must reflect the current
    // state at each step.
    //
    // Note: update_metrics is rate-limited at 1 Hz. Between steps we
    // force a metric refresh by rewinding last_metric_update (same
    // technique used by toggle() itself).
    let mut h = HudState::new();
    h.toggle();

    // Step 1: defaults.
    h.update_metrics(&[]);
    assert_eq!(h.cached_lines[15].1, " prdr: on");
    assert_eq!(h.cached_lines[16].1, " crdr: off");

    // Step 2: user live-reloads power_dragon=false, crystal_dragon=true.
    h.set_power_dragon(false);
    h.set_crystal_dragon(true);
    // Force the next update_metrics to pass the 1 Hz rate limiter.
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL * 2)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[15].1, " prdr: off",
        "prdr must reflect live-reloaded power_dragon=false"
    );
    assert_eq!(
        h.cached_lines[16].1, " crdr: on",
        "crdr must reflect live-reloaded crystal_dragon=true"
    );

    // Step 3: user live-reloads back to defaults.
    h.set_power_dragon(true);
    h.set_crystal_dragon(false);
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL * 2)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[15].1, " prdr: on",
        "prdr must reflect live-reloaded power_dragon=true"
    );
    assert_eq!(
        h.cached_lines[16].1, " crdr: off",
        "crdr must reflect live-reloaded crystal_dragon=false"
    );
}
