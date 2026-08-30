// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient event polling — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.

use std::collections::HashMap;
use std::time::Instant;

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
            if let Ok(c) = std::fs::read_to_string(path) {
                let pv = &crate::configfile::parse_config_text(&c).values;
                if crate::crystal_dragon_engine::ambient::collect_ambient_schedule(pv)
                    .entries
                    .is_empty()
                {
                    last_ambient_entry = None;
                    last_ambient_schedule.entries.clear();
                    *last_applied_ambient_entry = None;
                    cloud.ambient_palette_locked = false;
                    cloud.user_override_since_ambient = true;
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
                if let Ok(c) = std::fs::read_to_string(path) {
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
        last_ambient_schedule.entries.clear();
        *last_applied_ambient_entry = None;
        cloud.ambient_palette_locked = false;
        cloud.user_override_since_ambient = true;
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
