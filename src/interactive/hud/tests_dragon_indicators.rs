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
    let (_, prdr_line) = &h.cached_lines[13];
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
    let (_, crdr_line) = &h.cached_lines[14];
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
    let (_, prdr_line) = &h.cached_lines[13];
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
    let (_, crdr_line) = &h.cached_lines[14];
    assert_eq!(
        crdr_line, " crdr: on",
        "crdr line must show 'on' after set_crystal_dragon(true)"
    );
}

#[test]
fn hud_prdr_crdr_above_cid_in_layout() {
    // Owner mandate: prdr/crdr above cid/commit id indicator.
    // Z-master-1X round 5: cid moved from row 19 to row 21 (dcel/tcel
    // inserted at rows 19-20 above cid).
    let h = HudState::new();
    // cid must be at row 21 (Z-master-1X round 5 — above the session footer).
    let (_, cid_line) = &h.cached_lines[21];
    assert!(
        cid_line.starts_with(" cid: "),
        "row 21 must be the cid line (Z-master-1X round 5), got: {cid_line:?}"
    );
    // prdr and crdr initialize as empty strings (populated by update_metrics
    // at the 1 Hz tick). Verify they are at rows 13 and 14 respectively
    // by calling update_metrics and checking the rendered text.
    let mut h2 = HudState::new();
    h2.toggle();
    h2.update_metrics(&[]);
    let (_, prdr_line) = &h2.cached_lines[13];
    let (_, crdr_line) = &h2.cached_lines[14];
    assert!(
        prdr_line.starts_with(" prdr: "),
        "row 13 must be the prdr line (v51 reorder), got: {prdr_line:?}"
    );
    assert!(
        crdr_line.starts_with(" crdr: "),
        "row 14 must be the crdr line (v51 reorder), got: {crdr_line:?}"
    );
    // cid is still at row 21 (unchanged from h).
    let (_, cid_line_2) = &h2.cached_lines[21];
    assert!(
        cid_line_2.starts_with(" cid: "),
        "row 21 must still be the cid line after update_metrics (Z-master-1X round 5), got: {cid_line_2:?}"
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
    assert_eq!(h.cached_lines[13].1, " prdr: on");
    assert_eq!(h.cached_lines[14].1, " crdr: off");

    // Step 2: user live-reloads power_dragon=false, crystal_dragon=true.
    h.set_power_dragon(false);
    h.set_crystal_dragon(true);
    // Force the next update_metrics to pass the 1 Hz rate limiter.
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL * 2)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[13].1, " prdr: off",
        "prdr must reflect live-reloaded power_dragon=false"
    );
    assert_eq!(
        h.cached_lines[14].1, " crdr: on",
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
        h.cached_lines[13].1, " prdr: on",
        "prdr must reflect live-reloaded power_dragon=true"
    );
    assert_eq!(
        h.cached_lines[14].1, " crdr: off",
        "crdr must reflect live-reloaded crystal_dragon=false"
    );
}

#[test]
fn hud_prdr_crdr_setter_must_be_called_with_live_value_not_startup_value() {
    // Regression test for v50.0.0-beta.6 bug: event_loop.rs was calling
    // set_power_dragon(cfg.power_dragon) and set_crystal_dragon(cfg.crystal_dragon)
    // using the STARTUP cfg (immutable reference) instead of current_cfg
    // (the live-reloaded copy). This meant live-reload edits to
    // power_dragon / crystal_dragon in config.toml never reached the HUD —
    // the prdr/crdr lines stayed stuck at the startup value for the entire
    // session.
    //
    // This test verifies the HudState-level contract: when the setter is
    // called with the NEW (live-reloaded) value, the HUD reflects it on
    // the next update_metrics tick. The event_loop.rs fix (use current_cfg
    // instead of cfg) is verified by code review — the integration test
    // for the full live-reload path requires a terminal + config watcher
    // and is covered by manual testing.
    //
    // Simulate the bug scenario: startup power_dragon=true, then user
    // live-reloads power_dragon=false. The event loop MUST call
    // set_power_dragon(false) — if it calls set_power_dragon(true) (the
    // stale startup value), the HUD stays "on" (bug).
    let mut h = HudState::new();
    h.toggle();
    // Startup: power_dragon=true (default), crystal_dragon=false (default).
    h.update_metrics(&[]);
    assert_eq!(h.cached_lines[13].1, " prdr: on");
    assert_eq!(h.cached_lines[14].1, " crdr: off");
    // Live-reload: user edits config.toml to power_dragon=false.
    // Event loop MUST call set_power_dragon(false) — the live value.
    h.set_power_dragon(false);
    // Force the next update_metrics to pass the 1 Hz rate limiter.
    h.last_metric_update = Instant::now()
        .checked_sub(HUD_METRIC_INTERVAL * 2)
        .unwrap_or_else(Instant::now);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[13].1, " prdr: off",
        "prdr must reflect the live-reloaded value (false), not the stale startup value (true)"
    );
}

// ── v50.0.0-beta.6 Option D: dynamic dsty tests ───────────────────
//
// dsty is DYNAMIC when power-dragon is ON (reflects throttle via
// compute_spawn_scale). dsty is STATIC when power-dragon is OFF (shows
// the user's configured density, no throttle).

#[test]
fn hud_dsty_static_when_power_dragon_off() {
    // power_dragon OFF → dsty = user density (no throttle).
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(false);
    h.set_droplet_density(0.75);
    h.set_effective_pressure(0.5); // high pressure, but power_dragon off
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.75",
        "dsty must be static when power_dragon is OFF (no throttle)"
    );
}

#[test]
fn hud_dsty_dynamic_when_power_dragon_on_no_pressure() {
    // power_dragon ON, pressure=0.0 → scale=1.0 → dsty = user density.
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(true);
    h.set_droplet_density(0.72);
    h.set_effective_pressure(0.0);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.72",
        "dsty must equal user density when pressure is 0 (no throttle)"
    );
}

#[test]
fn hud_dsty_dynamic_when_power_dragon_on_half_pressure() {
    // power_dragon ON, pressure=0.5 → scale = (1 - 0.75*0.5).clamp(0.25,1.0)
    // = 0.625. dsty = 0.72 * 0.625 = 0.45.
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(true);
    h.set_droplet_density(0.72);
    h.set_effective_pressure(0.5);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.45",
        "dsty must be throttled to 0.45 at 50% pressure (0.72 * 0.625)"
    );
}

#[test]
fn hud_dsty_dynamic_when_power_dragon_on_max_pressure() {
    // power_dragon ON, pressure=1.0 → scale = (1 - 0.75*1.0).clamp(0.25,1.0)
    // = 0.25 (floor). dsty = 0.72 * 0.25 = 0.18.
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(true);
    h.set_droplet_density(0.72);
    h.set_effective_pressure(1.0);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.18",
        "dsty must be floored to 0.18 at 100% pressure (0.72 * 0.25)"
    );
}

#[test]
fn hud_dsty_aggressive_throttle_drops_harder() {
    // power_dragon ON, pressure=0.5, aggressive_throttle=true
    // → scale = (1 - 0.9*0.5).clamp(0.10, 1.0) = 0.55
    // → dsty = 0.72 * 0.55 = 0.396 → 0.40
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(true);
    h.set_droplet_density(0.72);
    h.set_effective_pressure(0.5);
    h.set_aggressive_throttle(true);
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.40",
        "dsty must drop harder with aggressive_throttle (0.72 * 0.55 = 0.40)"
    );
}

#[test]
fn hud_dsty_cli_density_is_ceiling() {
    // CLI --density 1.0 sets droplet_density=1.0. Even at max pressure,
    // dsty = 1.0 * 0.25 = 0.25 (never exceeds 1.0).
    let mut h = HudState::new();
    h.toggle();
    h.set_power_dragon(true);
    h.set_droplet_density(1.0); // CLI value
    h.set_effective_pressure(1.0); // max pressure
    h.update_metrics(&[]);
    assert_eq!(
        h.cached_lines[12].1, " dsty: 0.25",
        "CLI density 1.0 caps dsty at 1.0; throttle reduces to 0.25 at max pressure"
    );
}
