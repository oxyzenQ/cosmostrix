// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config rebuild — extracted from `event_loop.rs` to keep that file
//! under the 800-LOC cap. Pure code motion — no behavior change.

use std::collections::HashMap;

use super::adaptive::{PerformanceSelfHealer, PowerManager};
use super::hud::HudState;
use crate::app::CloudConfig;
use crate::cloud::Cloud;
use crate::color_cache::ColorCache;
use crate::crystal_dragon_engine::ambient::{AmbientEntry, AmbientSchedule};
use crate::crystal_dragon_engine::ambient_scheduler::AmbientSchedulerHandle;
use crate::effective_density;
use crate::frame::Frame;
use crate::terminal::Terminal;

/// Apply pending Cloud rebuild (swaps Cloud + Frame between frames).
///
/// Handles the full live-reload rebuild path: scene base resolution
/// (v51.1 CLI-locked fallback), config rebuild, ecosystem inheritance,
/// palette transition, HUD sync, ambient schedule reload + consistency
/// fix, and ambient entry application.
///
/// Returns true if a rebuild was applied, false if pending_config was None.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn apply_config_rebuild(
    pending_config: &mut Option<HashMap<String, String>>,
    base_cfg: &mut CloudConfig,
    startup_cfg: &CloudConfig,
    cloud: &mut Cloud,
    frame: &mut Frame,
    term: &mut Terminal,
    power_manager: &mut PowerManager,
    hud_state: &mut HudState,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    current_cfg: &mut CloudConfig,
    last_applied_cfg_map: &mut Option<HashMap<String, String>>,
    last_ambient_schedule: &mut AmbientSchedule,
    ambient_handle: &mut AmbientSchedulerHandle,
    last_applied_ambient_entry: &mut Option<AmbientEntry>,
    ambient_snapback_killed: &mut bool,
    cfg: &CloudConfig,
    w: u16,
    h: u16,
    user_ranges: &[(char, char)],
    self_healer: &mut PerformanceSelfHealer,
    def_ascii: bool,
) -> bool {
    if let Some(new_cfg_map) = pending_config.take() {
        // v51.1 masterclass: CLI-locked fallback (owner contract, 2026-09-01).
        //
        // Startup:  CLI > config.toml > scene defaults.
        // Runtime:  config key present > CLI lock (locked startup value).
        //
        // `startup_cfg` is the pristine startup snapshot (never mutated);
        // `base_cfg` may only diverge from it via the runtime scene sync
        // below (shortkey/ambient preservation). The v50.0.0-beta.6 model
        // (zero cli_explicit + unconditional runtime-scene sync) retired the
        // CLI at the first reload AND permanently contaminated base_cfg's
        // scene family — so commenting the config `scene` key back out left
        // the engine stuck on the config-driven scene (the owner's bug:
        // `--scene crystal-dragon` + `scene = cinematic` + re-comment →
        // stayed cinematic). The delta-based resolution below fixes both:
        //
        //   key present        → rebuild_cloud_config applies it (config
        //                        wins — most recent user intent);
        //   key just removed   → restore the LOCKED startup scene family
        //                        (CLI crystal-dragon returns, no rerun);
        //   key never present  → keep the runtime scene (shortkey x/X
        //                        cycles, ambient fires, startup scene) by
        //                        syncing its managed defaults into base.
        //
        // cli_explicit is NOT zeroed anymore — the flags stay alive as the
        // locked layer for rebuild_cloud_config's fallback arms and the
        // scene-default gates.
        match super::event_loop_scene_sync::resolve_scene_base_action(
            &new_cfg_map,
            last_applied_cfg_map.as_ref(),
        ) {
            super::event_loop_scene_sync::SceneBaseAction::ApplyConfig => {
                // The scene block inside rebuild_cloud_config applies the
                // config scene (including the managed defaults, gated on
                // the CLI locks per field).
            }
            super::event_loop_scene_sync::SceneBaseAction::RestoreLocked => {
                crate::lr_trace!(
                    "scene key removed — reverting to the locked startup scene '{}' (runtime was '{}')",
                    startup_cfg.scene_name, scene_name
                );
                super::event_loop_scene_sync::restore_locked_scene_family(base_cfg, startup_cfg);
            }
            super::event_loop_scene_sync::SceneBaseAction::SyncRuntime => {
                super::event_loop_scene_sync::sync_base_cfg_with_runtime_scene(
                    base_cfg, scene_name,
                );
            }
        }
        let new_cfg = crate::live_config::rebuild_cloud_config(base_cfg, &new_cfg_map);
        // v50.0.0-alpha.7: track latest config for finalize_session.
        *current_cfg = new_cfg.clone();
        let density = effective_density(new_cfg.base_density, w, new_cfg.density_auto);
        // v25: bulletproof trace that rebuild reached render thread.
        crate::live_config_trace::trace_rebuild_applied(
            &new_cfg.color_scheme,
            new_cfg.charset_preset.as_str(),
            new_cfg.speed,
            new_cfg.density,
            new_cfg.target_fps,
        );

        // field-level config diff trace (extracted to live_config_trace.rs).
        crate::live_config_trace::trace_config_diff(last_applied_cfg_map.as_ref(), &new_cfg_map);
        *last_applied_cfg_map = Some(new_cfg_map.clone());
        // Phase D #9: preserve ecosystem + post-FX across reload.
        // AB-02: capture override state for schedule-empty restore.
        let preserve_user_override = cloud.user_override_since_ambient;
        let preserved_color_scheme = cloud.color_scheme;
        let preserved_palette = cloud.palette.clone();
        let preserved_scene_name = scene_name.clone();
        let mut new_cloud = new_cfg.create_cloud(density);
        new_cloud.inherit_ecosystem_state(cloud);
        *cloud = new_cloud;
        cloud.reset(w, h);
        cloud.enable_events();
        cloud.set_component_timing(new_cfg.perf_stats);
        // v50.0.0-beta.6: re-apply phosphor tuning + speed after rebuild.
        let c = term.phosphor_tuning();
        cloud.set_phosphor_tuning(c.0, c.1, c.2);
        // Smooth palette transition on live config reload.
        //
        // Previously, the Cloud rebuild produced an instant color jump
        // (transition_start = None on the fresh Cloud). Now, if the
        // color scheme changed, we store the old palette in the circular
        // buffer's previous slot and activate the 300ms wave — matching
        // the smooth transition used by 'c' keypress, crystal-dragon,
        // and scene runtime. The shader's apply_l_smoothing will
        // interpolate between old and new via OKLab L + polar chroma.
        if cloud.color_scheme != preserved_color_scheme {
            cloud.start_transition_from_previous_palette(preserved_palette);
        }
        // Fresh Cloud from rebuild — reset self-healer.
        self_healer.reset();
        // Rebuild color cache + frame + fill bg + charset.
        term.set_color_cache(ColorCache::new(&cloud.palette));
        *frame = Frame::new(w, h, cloud.palette.bg);
        super::fill_terminal_bg(cloud.palette.bg);
        *charset_preset = new_cfg.charset_preset.clone();
        //  recompute target FPS from new config.
        let safe_fps = new_cfg.resolve_capped_fps(cfg.target_fps);
        power_manager.set_target_fps(safe_fps);
        // v30: keep HUD tgt: in sync with live-reloaded fps.
        hud_state.set_target_fps(safe_fps);
        // AB-07: count every config rebuild for diagnostics.
        super::ambient_diag_config_rebuild();
        // Ambient: push new schedule to scheduler if it changed.
        if new_cfg.ambient_schedule != *last_ambient_schedule {
            super::ambient_diag_schedule_reload();
            ambient_handle.reload(new_cfg.ambient_schedule.clone());
            *last_ambient_schedule = new_cfg.ambient_schedule.clone();
            if new_cfg.ambient_schedule.entries.is_empty() {
                super::ambient_diag_schedule_empty();
                if let Some(ref le) = last_applied_ambient_entry {
                    if *scene_name == le.scene {
                        *scene_name = new_cfg.scene_name.clone();
                        *scene_generation = (*scene_generation).wrapping_add(1);
                    }
                }
                *last_applied_ambient_entry = None;
                cloud.ambient_palette_locked = false;
                cloud.user_override_since_ambient = true;
                *ambient_snapback_killed = true;
                super::ambient_diag_snapback_killed();
            }
        }
        // AB-07: consistency fix — if rebuilt config has empty schedule
        // but stale state remains, clean up immediately.
        if new_cfg.ambient_schedule.entries.is_empty() {
            if last_applied_ambient_entry.is_some()
                || cloud.ambient_palette_locked
                || !last_ambient_schedule.entries.is_empty()
            {
                super::ambient_diag_consistency_fix();
                if !last_ambient_schedule.entries.is_empty() {
                    ambient_handle.reload(new_cfg.ambient_schedule.clone());
                    *last_ambient_schedule = new_cfg.ambient_schedule.clone();
                    super::ambient_diag_schedule_reload();
                    super::ambient_diag_schedule_empty();
                }
                *last_applied_ambient_entry = None;
                cloud.ambient_palette_locked = false;
                cloud.user_override_since_ambient = true;
                *ambient_snapback_killed = true;
                super::ambient_diag_snapback_killed();
            }
        } else if *ambient_snapback_killed {
            *ambient_snapback_killed = false;
        }
        // re-apply last ambient entry to fresh Cloud.
        if let Some(ref last_entry) = last_applied_ambient_entry {
            let still_in = new_cfg
                .ambient_schedule
                .entries
                .iter()
                .any(|e| e == last_entry);
            if still_in && !cloud.custom_palette_active {
                let cm = last_applied_cfg_map.clone().unwrap_or_default();
                *charset_preset = cloud.apply_ambient_entry(
                    last_entry,
                    &*charset_preset,
                    user_ranges,
                    def_ascii,
                    &cm,
                );
                *scene_name = last_entry.scene.clone();
                *scene_generation = (*scene_generation).wrapping_add(1);
                cloud.user_override_since_ambient = false;
                cloud.ambient_palette_locked = true;
                super::ambient_diag_reapply();
                super::ambient_diag_scene_change("rebuild-reapply");
                term.set_color_cache(ColorCache::new(&cloud.palette));
                *frame = Frame::new(w, h, cloud.palette.bg);
                super::fill_terminal_bg(cloud.palette.bg);
            } else if !still_in {
                crate::lr_trace!("ambient: last entry no longer in schedule — clearing tracker");
                *last_applied_ambient_entry = None;
            }
        }
        // AB-05: full visual-state restore when schedule emptied.
        if new_cfg.ambient_schedule.entries.is_empty() {
            if preserve_user_override {
                // v50 fix: only preserve the user's color override if
                // the new config did NOT explicitly change the color
                // scheme. If the config's color_scheme differs from
                // the preserved value, the user edited config.toml to
                // change the color — respect that change instead of
                // reverting to the old scheme. This fixes the bug
                // where editing config.toml (e.g. color to "greens")
                // left the HUD showing the old scheme name.
                if new_cfg.color_scheme == preserved_color_scheme {
                    cloud.color_scheme = preserved_color_scheme;
                }
                // v50 fix: same pattern for scene_name — only preserve
                // if the config didn't explicitly change the scene. When
                // the config DID change the scene (new != preserved),
                // respect it by applying the new scene's runtime defaults
                // (mirrors the non-preserve branch below). Without this
                // else branch, the local `scene_name` variable — the
                // HUD's source of truth (line 925: set_scene_name) — was
                // left stale at the old value, so the `scn:` HUD line
                // showed the previous scene even after the user edited
                // config.toml. Unlike `cloud.color_scheme` (a Cloud field
                // auto-refreshed by `cloud = new_cloud` at line 297),
                // `scene_name` is a local variable and must be explicitly
                // updated here.
                if new_cfg.scene_name == preserved_scene_name {
                    *scene_name = preserved_scene_name;
                } else {
                    *scene_name = new_cfg.scene_name.clone();
                    *scene_generation = (*scene_generation).wrapping_add(1);
                    *charset_preset = cloud.apply_scene_runtime(
                        &*scene_name,
                        &*charset_preset,
                        user_ranges,
                        def_ascii,
                    );
                    term.set_color_cache(ColorCache::new(&cloud.palette));
                    *frame = Frame::new(w, h, cloud.palette.bg);
                    super::fill_terminal_bg(cloud.palette.bg);
                }
            } else {
                *scene_name = new_cfg.scene_name.clone();
                *scene_generation = (*scene_generation).wrapping_add(1);
                *charset_preset = cloud.apply_scene_runtime(
                    &*scene_name,
                    &*charset_preset,
                    user_ranges,
                    def_ascii,
                );
                term.set_color_cache(ColorCache::new(&cloud.palette));
                *frame = Frame::new(w, h, cloud.palette.bg);
                super::fill_terminal_bg(cloud.palette.bg);
            }
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
    }

    false
}
