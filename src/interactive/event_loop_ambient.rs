// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient event polling — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! v80.0.0-beta.1 ambient contract (owner-approved extension of the v80.0.0-beta.1
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

/// v80.0.0-alpha.2 (S-master-HUNT-4): minimum interval between ANY
/// ground-truth config re-reads (the rx-event nuke above AND the snapback
/// guard below share one budget via `last_ground_truth_check`).
///
/// Pre-alpha.2 the snapback guard had NO rate limit — it re-read +
/// re-parsed the config EVERY FRAME while ambient was applied (~60
/// reads/sec), contradicting its own "≤ once per 30s" comment, flooding
/// the watcher with inotify Access events (2 trace lines per read — the
/// 1000-entry debug drain exhausted in seconds), and burning I/O for a
/// pure backup path. 5s staleness is acceptable: the watcher delivers
/// real ambient-removal edits within ~100ms (the zero-key fix in
/// watcher.rs makes even the all-commented case reliable); this guard
/// only exists to catch watcher-missed removals.
const GROUND_TRUTH_MIN_INTERVAL_SECS: u64 = 5;

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
) -> Option<f64> {
    // v80.0.0-beta.2 (S-master-LOGIC-3) fps intent: the ambient scene
    // owns fps exactly like it owns color/charset/speed/density/glitch
    // (owner contract: "this wins runtime over config: fps, speed,
    // density, glitch-level, color, charset, scene"). The Cloud does not
    // own frame pacing, so this function RETURNS the fps the event loop
    // should apply to the power manager + HUD when an ambient event
    // changed the effective scene:
    //   - an rx-event / snapback applied an entry  -> the scene's fps;
    //   - the overlay lifted (revert to startup)   -> the locked startup
    //     fps (the CLI/config startup resolution);
    //   - no state change                          -> None (untouched).
    let mut fps_intent: Option<f64> = None;
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
    // v50 audit C-2: rate-limit to 1 check per GROUND_TRUTH_MIN_INTERVAL_SECS
    // (was per-frame).
    if last_ambient_entry.is_some()
        && (*last_ground_truth_check).elapsed()
            >= std::time::Duration::from_secs(GROUND_TRUTH_MIN_INTERVAL_SECS)
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
                    // v80.0.0-beta.1: BEFORE clearing the tracker, capture whether the
                    // current scene is ambient-owned — the nuke must revert
                    // it (the overlay is lifting), not leave it stuck.
                    if revert_ambient_owned_scene(
                        cloud,
                        frame,
                        term,
                        charset_preset,
                        scene_name,
                        scene_generation,
                        last_applied_ambient_entry,
                        startup_cfg,
                        w,
                        h,
                    ) {
                        // v80.0.0-beta.2: the overlay lifted — the fps falls
                        // back to the LOCKED startup resolution (CLI
                        // --fps/config fps at startup), same as the rest of
                        // the scene family.
                        fps_intent = Some(startup_cfg.target_fps);
                    }
                    last_ambient_schedule.entries.clear();
                    *last_applied_ambient_entry = None;
                    cloud.ambient_palette_locked = false;
                    // v80.0.0-beta.1: do NOT fake `user_override_since_ambient = true`
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
            // v80.0.0-beta.2 (S-master-LOGIC-3): the freshly-applied
            // ambient scene owns fps too — hand the scene's declared fps
            // (built-in default or scene-custom field) to the caller.
            fps_intent = crate::scene_custom::ambient_scene_fps(&entry.scene, &cfg_map);
        }
    }
    // AB-08: snapback ground-truth guard — re-read config file (~50µs I/O),
    // rate-limited to the shared GROUND_TRUTH_MIN_INTERVAL_SECS budget
    // (see the const doc for the pre-alpha.2 per-frame-read defect this
    // fixes: I/O burn + inotify Access-event flood exhausting the
    // 1000-entry debug trace). Backup-only path — the watcher delivers
    // real ambient-removal edits within ~100ms.
    let _ab06_sked_len = last_ambient_schedule.entries.len() as u64;
    let _ab06_last_applied = last_applied_ambient_entry.is_some();
    super::ambient_diag_snapback_guard(_ab06_sked_len, _ab06_last_applied);
    let guard_budget_ok = (*last_ground_truth_check).elapsed()
        >= std::time::Duration::from_secs(GROUND_TRUTH_MIN_INTERVAL_SECS);
    let ground_truth_ambient_empty =
        if !*ambient_snapback_killed && guard_budget_ok && _ab06_sked_len > 0 && _ab06_last_applied
        {
            // v80.0.0-alpha.2: consume the shared budget — reset the
            // timestamp when THIS guard performs a read, so the interval
            // is actually enforced (the rx-event guard above only resets
            // when an rx event arrives).
            *last_ground_truth_check = Instant::now();
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
        // v80.0.0-beta.1: same overlay-lift revert as the rx-event nuke above —
        // capture ownership from the pre-clear state, then revert.
        if revert_ambient_owned_scene(
            cloud,
            frame,
            term,
            charset_preset,
            scene_name,
            scene_generation,
            last_applied_ambient_entry,
            startup_cfg,
            w,
            h,
        ) {
            fps_intent = Some(startup_cfg.target_fps);
        }
        last_ambient_schedule.entries.clear();
        *last_applied_ambient_entry = None;
        cloud.ambient_palette_locked = false;
        // v80.0.0-beta.1: same honesty rule as the rx-event nuke — no fake user
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
        // v80.0.0-beta.2 (S-master-LOGIC-3): snapback re-asserted the
        // ambient entry — it owns fps too. The entry the snapback applied
        // is in `last_applied_ambient_entry` (updated by the callee).
        if let Some(entry) = last_applied_ambient_entry.as_ref() {
            let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
            fps_intent = crate::scene_custom::ambient_scene_fps(&entry.scene, &cfg_map);
        }
    }

    fps_intent
}

/// v80.0.0-beta.1 ambient overlay lift: revert an ambient-owned scene to the locked
/// startup scene family.
///
/// "Ambient-owned" means the last visual change came from an ambient apply
/// (no user shortkey/CLI override since — `user_override_since_ambient ==
/// false`) AND the live scene matches the last applied ambient entry's
/// scene. Only then does removing the schedule revert the scene; a user's
/// shortkey scene outranks the ambient overlay and survives the removal
/// (the shortkey is the runtime top priority in the owner's contract).
///
/// v80.0.0-beta.2 (S-master-HUNT) verbatim-snapshot restore: the revert now
/// restores the startup snapshot's scene-family VALUES directly instead of
/// re-deriving them via `apply_scene_runtime_with_cfg(startup scene)`.
/// Re-derivation re-applied the scene definition + its scene-custom block
/// layer over the lock — for a CLI-shadowed startup (`--scene hacker-mode
/// -c test -C test`) that stomped the CLI-locked color/charset back to the
/// block values, the exact defect family the rebuild path's
/// `restore_locked_scene_family` fixes (it restores values verbatim and the
/// tail block is ownership-gated). The cloud-level restore here mirrors
/// that contract: the snapshot is the resolved truth (CLI flags shadow
/// scene/block fields at startup), so every dimension comes straight from
/// `startup_cfg`. `last_applied_cfg_map` is no longer needed — nothing is
/// re-derived from the config.
///
/// No-op when the scene is not ambient-owned or already matches the
/// startup scene.
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
    w: u16,
    h: u16,
) -> bool {
    let ambient_owned = !cloud.user_override_since_ambient
        && *scene_name != startup_cfg.scene_name
        && last_applied_ambient_entry
            .as_ref()
            .is_some_and(|e| e.scene == *scene_name);
    if !ambient_owned {
        return false;
    }
    crate::lr_trace!(
        "ambient: schedule emptied — reverting ambient-owned scene '{}' to the locked startup scene '{}' (verbatim snapshot)",
        scene_name,
        startup_cfg.scene_name
    );
    *scene_name = startup_cfg.scene_name.clone();
    *scene_generation = (*scene_generation).wrapping_add(1);
    cloud.set_scene_label(startup_cfg.scene_name.as_str());

    // Rain style: transition only when it actually differs (mirrors the
    // scene-runtime guards — a same-style transition would needlessly
    // reset spawn/phosphor state).
    if cloud.rain_style != startup_cfg.rain_style {
        cloud.transition_rain_style(startup_cfg.rain_style);
    }

    // Color: the startup snapshot is the resolved truth. A locked custom
    // palette is restored as a palette (name + values); otherwise the
    // locked scheme is restored (set_color_scheme clears any lingering
    // ambient custom palette, matching startup parity).
    if let Some(ref locked_palette) = startup_cfg.custom_palette {
        cloud.set_palette(
            startup_cfg.custom_palette_name.as_deref(),
            locked_palette.clone(),
        );
    } else {
        cloud.set_color_scheme(startup_cfg.color_scheme);
    }

    // Charset: verbatim snapshot values (preset name + resolved glyph
    // pool). A preset change transitions the pool (same visual continuity
    // the scene-runtime path gives); an identical preset is a no-op.
    if *charset_preset != startup_cfg.charset_preset {
        cloud.transition_chars(startup_cfg.chars.clone());
    }
    *charset_preset = startup_cfg.charset_preset.clone();

    // Speed / density / glitch: snapshot values (CLI flags shadowed the
    // scene/block fields at startup — re-derivation would lose that).
    cloud.set_chars_per_sec(startup_cfg.speed);
    cloud.set_droplet_density(startup_cfg.density);
    cloud.apply_glitch_level_runtime(startup_cfg.glitch_level);

    // Clean redraw bookkeeping (same flags the scene-runtime applies set).
    cloud.semantic_invalidate = true;
    cloud.force_draw_everything = true;

    cloud.user_override_since_ambient = true;
    term.set_color_cache(ColorCache::new(&cloud.palette));
    *frame = Frame::new(w, h, cloud.palette.bg);
    super::fill_terminal_bg(cloud.palette.bg);
    true
}
