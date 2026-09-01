// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! HUD state update — extracted from `event_loop.rs` to keep that file
//! under the 800-LOC cap. Pure code motion — no behavior change.

use super::hud::{FrameMode, HudState};
use crate::app::CloudConfig;
use crate::central_control_dragon_power::PowerManager;
use crate::cloud::Cloud;
use crate::config::GlitchLevel;

/// Update HUD state every frame with live values.
///
/// Pushes frame mode (paused/idle/active), scene name, color scheme,
/// custom palette name, charset preset, droplet density, chars per sec,
/// effective pressure, aggressive throttle, power_dragon, and
/// crystal_dragon to the HUD state so the 1 Hz metric tick renders
/// current values.
///
/// v80.0.0-beta.1 pause freeze (owner bug fix 2026-08-30): `set_metrics_paused`
/// must run BEFORE the metric setters below — on the pause frame it
/// arms the freeze before any sampler can tick, and on the resume
/// frame it lifts the freeze before the setters deliver fresh values.
pub(crate) fn update_hud_state(
    hud_state: &mut HudState,
    cloud: &mut Cloud,
    power_manager: &PowerManager,
    scene_name: &str,
    charset_preset: &str,
    current_cfg: &CloudConfig,
) {
    hud_state.set_metrics_paused(cloud.is_paused_or_decelerating());
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
    // v80.0.0-beta.1 power-dragon gate (owner masterclass mandate): the pressure
    // FEED to the render path is gated on `current_cfg.power_dragon`.
    //
    // v50 Option D gated only the DISPLAY side (dsty static when off) while
    // `cloud.set_perf_pressure()` kept feeding the raw value — so with
    // power-dragon off, rain_at() still throttled the spawn scale and the
    // config promise "rain stays at user-configured density/speed regardless
    // of CPU pressure" was broken. Now the gated feed drives EVERY cloud
    // consumer (spawn scale, phosphor ramp, glitch gate, atmospheric event
    // gate, CRT vignette) to their zero-pressure behavior when the dragon is
    // off, and the HUD `prs:` line shows the same applied value so prs/dsty
    // never disagree. `power_manager` still accumulates the real pressure
    // internally (the self-healer + post-exit report keep their signal).
    let applied_pressure = if current_cfg.power_dragon {
        power_manager.effective_pressure()
    } else {
        0.0
    };
    hud_state.set_effective_pressure(applied_pressure);
    cloud.set_perf_pressure(applied_pressure);
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

    // v50.0.0-beta.7 Option C expansion — 4 new owner-mandated metrics.
    // ambt: auto-detect ambient on/off from the ambient schedule entries.
    hud_state.set_ambient_on(!current_cfg.ambient_schedule.entries.is_empty());
    // glth: glitch level — read from cloud state so it follows runtime
    // scene switches (apply_scene_runtime updates cloud.glitchy/glitch_pct,
    // and apply_glitch_level_runtime sets the level). We derive the
    // GlitchLevel from cloud.glitch_pct because Cloud doesn't store the
    // enum (only the resolved numeric values). Thresholds match the
    // preset definitions in scene_runtime.rs apply_glitch_level_runtime.
    let cloud_glitch_level = if !cloud.glitchy {
        GlitchLevel::None
    } else if cloud.glitch_pct < 0.05 {
        GlitchLevel::Subtle
    } else if cloud.glitch_pct < 0.15 {
        GlitchLevel::Default
    } else {
        GlitchLevel::Intense
    };
    hud_state.set_glitch_level(cloud_glitch_level);
    // ctun: custom if any ColorTune field ≠ 1.0 (IDENTITY).
    let ct = &current_cfg.color_tune;
    let is_custom = ct.saturation != 1.0
        || ct.brightness != 1.0
        || ct.head != 1.0
        || ct.body != 1.0
        || ct.tail != 1.0;
    hud_state.set_color_tune_custom(is_custom);
    // mnst: monolith size — only meaningful when scene is monolith-based
    // (rain_style == Monolith). For non-monolith scenes, show "unknown".
    if cloud.rain_style() == crate::rain_style::RainStyle::Monolith {
        hud_state.set_monolith_size(cloud.monolith_size());
    } else {
        hud_state.set_monolith_size_unknown();
    }
}
