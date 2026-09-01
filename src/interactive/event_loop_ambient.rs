// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient event polling — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! v51.2 ambient contract (owner-approved extension of the v51.1
//! CLI-locked fallback audit to the snapback path): `ambient.*` keys are
//! a config-family overlay. When the schedule is emptied on disk, the
//! overlay lifts — an ambient-OWNED scene (no user shortkey/CLI override
//! since the last ambient apply) reverts to the locked startup scene
//! family instead of staying stuck on the stale ambient scene. The
//! ground-truth nuke below performs that revert directly; the
//! live-reload rebuild path performs it via
//! `event_loop_scene_sync::ambient_removed_between_maps` +
//! `SceneBaseAction::RestoreLocked`.

use std::collections::HashMap;
use std::time::Instant;

use crate::app::CloudConfig;
use crate::cloud::Cloud;
use crate::color_cache::ColorCache;
use crate::crystal_dragon_engine::ambient::{AmbientEntry, AmbientSchedule};
use crate::crystal_dragon_engine::ambient_scheduler::AmbientSchedulerHandle;
use crate::frame::Frame;
use crate::terminal::Terminal;

/// Poll ambient phase events + apply ground-truth guard + apply entry.
///
/// Handles AB-03/04 (poll + discard stale), AB-08 (ground-truth file
/// re-read), and ambient entry application (palette/scene/charset change).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(crate) fn poll_ambient_events(
    cloud: &mut Cloud,
    frame: &mut Frame,
    term: &mut Terminal,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    last_applied_cfg_map: &Option<HashMap<String, String>>,
    last_ambient_schedule: &mut AmbientSchedule,
    ambient_handle: &mut AmbientSchedulerHandle,
    last_applied_ambient_entry: &mut Option<AmbientEntry>,
    ambient_snapback_killed: &mut bool,
    last_ground_truth_check: &mut Instant,
    config_path_for_ground_truth: &Option<std::path::PathBuf>,
    next_frame: &mut Instant,
    w: u16,
    h: u16,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    current_cfg: &crate::app::CloudConfig,
    startup_cfg: &CloudConfig,
    last_user_input_at: std::time::Instant,
) {
    // AB-03+AB-04: poll ambient phase events. Empty schedule → drain; non-empty → discard stale.
    let mut last_ambient_entry: Option<crate::crystal_dragon_engine::ambient::AmbientEntry> = None;
    if !last_ambient_schedule.entries.is_empty() {
        while let Ok(entry) = ambient_handle.rx.try_recv() {
            if !last_ambient_schedule.entries.iter().any(|e| e == &entry) {
                continue;
            }
            last_ambient_entry = Some(entry);
        }
    } else {
        while ambient_handle.rx.try_recv().is_ok() {} // drain stale
    }
    // AB-08: ground-truth guard — if config file on disk says 0 entries but
    // we got an rx event, the event is stale. Discard + nuke ambient state.
    // v50 audit C-2: rate-limit to 1 check per 5s (was per-frame).
    if last_ambient_entry.is_some()
        && (*last_ground_truth_check).elapsed() >= std::time::Duration::from_secs(5)
    {
        *last_ground_truth_check = Instant::now();
        if let Some(ref path) = config_path_for_ground_truth {
            if let Ok(c) = crate::config_io::read_config_capped(path) {
                let pv = &crate::configfile::parse_config_text(&c).values;
                if crate::crystal_dragon_engine::ambient::collect_ambient_schedule(pv)
                    .entries
                    .is_empty()
                {
                    last_ambient_entry = None;
                    // v51.2: BEFORE clearing the tracker, capture whether the
                    // current scene is ambient-owned — the nuke must revert
                    // it (the overlay is lifting), not leave it stuck.
                    revert_ambient_owned_scene(
                        cloud,
                        frame,
                        term,
                        charset_preset,
                        scene_name,
                        scene_generation,
                        last_applied_ambient_entry,
                        startup_cfg,
                        user_ranges,
                        def_ascii,
                        w,
                        h,
                    );
                    last_ambient_schedule.entries.clear();
                    *last_applied_ambient_entry = None;
                    cloud.ambient_palette_locked = false;
                    // v51.2: do NOT fake `user_override_since_ambient = true`
                    // here. The visual state is still ambient-owned until the
                    // revert above (or a later rebuild) replaces it; faking
                    // user ownership poisoned the next rebuild's ambient
                    // overlay decision into SyncRuntime (scene stuck). The
                    // flag stays honest: ambient applied last. Re-application
                    // is already impossible via the other guards (empty
                    // schedule + rx drain + snapback killed + tracker None).
                    *ambient_snapback_killed = true;
                    ambient_handle
                        .reload(crate::crystal_dragon_engine::ambient::AmbientSchedule::default());
                    super::ambient_diag_schedule_empty();
                    super::ambient_diag_schedule_reload();
                    super::ambient_diag_snapback_killed();
                }
            }
        }
    }
    if let Some(entry) = last_ambient_entry {
        if (*last_applied_ambient_entry).as_ref() == Some(&entry)
            && !cloud.user_override_since_ambient
        {
            // Duplicate — already applied.
        } else if cloud.user_override_since_ambient {
            // v50.0.0-beta.7 masterclass: user has overridden (CLI scene,
            // 'x'/'X' key press, or CLI color/charset/etc). Don't re-apply
            // ambient from rx events — let try_auto_snapback handle
            // re-application after ambient-snapback-secs (default 30s).
            // Store the entry so snapback knows what to apply.
            *last_applied_ambient_entry = Some(entry.clone());
        } else {
            let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
            *charset_preset = cloud.apply_ambient_entry(
                &entry,
                &*charset_preset,
                user_ranges,
                def_ascii,
                &cfg_map,
            );
            *last_applied_ambient_entry = Some(entry.clone());
            *scene_name = entry.scene.clone();
            *scene_generation = (*scene_generation).wrapping_add(1);
            super::ambient_diag_rx();
            super::ambient_diag_scene_change(&format!("rx-event(scene={})", entry.scene));
            cloud.user_override_since_ambient = false;
            cloud.ambient_palette_locked = true;
            if *ambient_snapback_killed {
                *ambient_snapback_killed = false;
            }
            term.set_color_cache(ColorCache::new(&cloud.palette));
            *frame = Frame::new(w, h, cloud.palette.bg);
            super::fill_terminal_bg(cloud.palette.bg);
        }
    }
    // AB-08: snapback ground-truth guard — re-read config file (~50µs I/O, ≤ once per 30s).
    let _ab06_sked_len = last_ambient_schedule.entries.len() as u64;
    let _ab06_last_applied = last_applied_ambient_entry.is_some();
    super::ambient_diag_snapback_guard(_ab06_sked_len, _ab06_last_applied);
    let ground_truth_ambient_empty =
        if !*ambient_snapback_killed && _ab06_sked_len > 0 && _ab06_last_applied {
            let mut empty = false;
            if let Some(ref path) = config_path_for_ground_truth {
                if let Ok(c) = crate::config_io::read_config_capped(path) {
                    let pv = &crate::configfile::parse_config_text(&c).values;
                    if crate::crystal_dragon_engine::ambient::collect_ambient_schedule(pv)
                        .entries
                        .is_empty()
                    {
                        empty = true;
                    }
                }
            }
            empty
        } else {
            false
        };
    if ground_truth_ambient_empty {
        // v51.2: same overlay-lift revert as the rx-event nuke above —
        // capture ownership from the pre-clear state, then revert.
        revert_ambient_owned_scene(
            cloud,
            frame,
            term,
            charset_preset,
            scene_name,
            scene_generation,
            last_applied_ambient_entry,
            startup_cfg,
            user_ranges,
            def_ascii,
            w,
            h,
        );
        last_ambient_schedule.entries.clear();
        *last_applied_ambient_entry = None;
        cloud.ambient_palette_locked = false;
        // v51.2: same honesty rule as the rx-event nuke — no fake user
        // override; the state remains ambient-owned until the revert.
        *ambient_snapback_killed = true;
        ambient_handle.reload(crate::crystal_dragon_engine::ambient::AmbientSchedule::default());
        super::ambient_diag_schedule_empty();
        super::ambient_diag_schedule_reload();
        super::ambient_diag_snapback_killed();
    }
    if !*ambient_snapback_killed
        && _ab06_sked_len > 0
        && _ab06_last_applied
        && !ground_truth_ambient_empty
        && super::input::try_auto_snapback(
            cloud,
            charset_preset,
            scene_name,
            scene_generation,
            last_applied_ambient_entry,
            last_ambient_schedule,
            last_applied_cfg_map,
            user_ranges,
            def_ascii,
            last_user_input_at,
            current_cfg.effective_snapback_delay(crate::constants::AUTO_SNAPBACK_DELAY_SECS),
        )
    {
        term.set_color_cache(ColorCache::new(&cloud.palette));
        *frame = Frame::new(w, h, cloud.palette.bg);
        super::fill_terminal_bg(cloud.palette.bg);
        *next_frame = Instant::now();
    }
}

/// v51.2 ambient overlay lift: revert an ambient-owned scene to the locked
/// startup scene family.
///
/// "Ambient-owned" means the last visual change came from an ambient apply
/// (no user shortkey/CLI override since — `user_override_since_ambient ==
/// false`) AND the live scene matches the last applied ambient entry's
/// scene. Only then does removing the schedule revert the scene; a user's
/// shortkey scene outranks the ambient overlay and survives the removal
/// (the shortkey is the runtime top priority in the owner's contract).
///
/// The revert mirrors the rebuild path's `RestoreLocked` arm: the scene
/// name returns to the pristine startup resolution (CLI > config >
/// default), the scene's runtime profile is re-applied, and the render
/// triple (ColorCache / Frame / bg fill) is rebuilt. No-op when the scene
/// is not ambient-owned or already matches the startup scene.
#[allow(clippy::too_many_arguments)]
fn revert_ambient_owned_scene(
    cloud: &mut Cloud,
    frame: &mut Frame,
    term: &mut Terminal,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    last_applied_ambient_entry: &Option<AmbientEntry>,
    startup_cfg: &CloudConfig,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    w: u16,
    h: u16,
) {
    let ambient_owned = !cloud.user_override_since_ambient
        && *scene_name != startup_cfg.scene_name
        && last_applied_ambient_entry
            .as_ref()
            .is_some_and(|e| e.scene == *scene_name);
    if !ambient_owned {
        return;
    }
    crate::lr_trace!(
        "ambient: schedule emptied — reverting ambient-owned scene '{}' to the locked startup scene '{}'",
        scene_name,
        startup_cfg.scene_name
    );
    *scene_name = startup_cfg.scene_name.clone();
    *scene_generation = (*scene_generation).wrapping_add(1);
    *charset_preset = cloud.apply_scene_runtime(
        startup_cfg.scene_name.as_str(),
        &*charset_preset,
        user_ranges,
        def_ascii,
    );
    cloud.user_override_since_ambient = true;
    term.set_color_cache(ColorCache::new(&cloud.palette));
    *frame = Frame::new(w, h, cloud.palette.bg);
    super::fill_terminal_bg(cloud.palette.bg);
}
