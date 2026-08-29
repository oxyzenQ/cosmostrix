// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD state update — extracted from `event_loop.rs` to keep that file
//! under the 800-LOC cap. Pure code motion — no behavior change.

use crate::central_control_dragon_power::PowerManager;
use crate::cloud::Cloud;
use crate::app::CloudConfig;

use super::hud::{FrameMode, HudState};

/// Update HUD state every frame with live values.
///
/// Pushes frame mode (paused/idle/active), scene name, color scheme,
/// custom palette name, charset preset, droplet density, chars per sec,
/// effective pressure, aggressive throttle, power_dragon, and
/// crystal_dragon to the HUD state so the 1 Hz metric tick renders
/// current values.
pub(crate) fn update_hud_state(
    hud_state: &mut HudState,
    cloud: &mut Cloud,
    power_manager: &PowerManager,
    scene_name: &str,
    charset_preset: &str,
    current_cfg: &CloudConfig,
) {
    let frame_mode = if cloud.pause {
        FrameMode::Paused
    } else if power_manager.is_idle() {
        FrameMode::Idle
    } else {
        FrameMode::Active
    };
    hud_state.set_frame_mode(frame_mode);
    // v50 (2026-08-17) HUD expansion — push 6 dynamic values to the HUD
    // every frame so the 7 new owner-mandated metric lines (rows 6-12)
    // always reflect the live state. Setters are cheap (single field
    // write; String setters use clear+push_str on an existing
    // allocation so they don't reallocate once the cap is warmed up).
    // The text is rendered at the 1 Hz metric tick in `update_metrics`
    // (matches the fps/p99/max/rss cadence — avoids number flicker).
    hud_state.set_scene_name(scene_name);
    hud_state.set_color_scheme(cloud.color_scheme);
    // Show custom palette name on the clr: HUD line when active.
    // cloud.custom_palette_active tracks whether the user loaded a
    // --colors-custom palette. current_cfg.custom_palette_name holds
    // the name. v50.0.0-beta.6 bugfix: uses current_cfg (live-reloaded)
    // instead of cfg (startup) so live-reload of custom_palette_name
    // propagates to the clr: HUD line.
    hud_state.set_custom_palette_name(if cloud.custom_palette_active {
        current_cfg.custom_palette_name.as_deref()
    } else {
        None
    });
    hud_state.set_charset_preset(charset_preset);
    hud_state.set_droplet_density(cloud.droplet_density());
    hud_state.set_chars_per_sec(cloud.chars_per_sec());
    hud_state.set_effective_pressure(power_manager.effective_pressure());
    cloud.set_perf_pressure(power_manager.effective_pressure());
    // v50.0.0-beta.6 Option D: push the aggressive-throttle flag to the
    // HUD so dsty: can reflect the steeper curve when the self-healer
    // has detected sustained high CPU pressure. Mirrors cloud's flag.
    hud_state.set_aggressive_throttle(cloud.aggressive_throttle);
    // v50.0.0-beta.6: push the live power_dragon / crystal_dragon state
    // to the HUD every frame. These track the current_cfg (live-reloaded)
    // values, NOT the startup config — so when the user edits
    // power_dragon=false or crystal_dragon=true in config.toml and
    // live-reload applies it, the HUD prdr/crdr lines reflect the new
    // state on the next 1 Hz metric tick. Owner explicitly mandated
    // these are NOT hardcoded — they must reflect runtime behavior.
    //
    // BUGFIX: previously used `cfg` (the startup immutable reference)
    // instead of `current_cfg` (the live-reloaded copy updated at line
    // 345 when the watcher delivers a new config). This meant live-reload
    // edits to power_dragon / crystal_dragon never reached the HUD — the
    // prdr/crdr lines stayed stuck at the startup value for the entire
    // session. Now uses `current_cfg` so live-reload propagates.
    hud_state.set_power_dragon(current_cfg.power_dragon);
    hud_state.set_crystal_dragon(current_cfg.crystal_dragon);
}
