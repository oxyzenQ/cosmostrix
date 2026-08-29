// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: one giant while-cloud.raining loop with deeply coupled local state (40+ mutable locals shared across input dispatch, config rebuild, adaptive throttling, draw, and HUD update); splitting requires passing a context struct through every branch.

//! Main interactive event loop.
//!
//! Contains the `run_interactive()` function that drives the entire
//! interactive mode: signal handling, frame pacing, input dispatch,
//! simulation stepping, rendering, and performance reporting.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};

use crate::color_cache::ColorCache;
use crate::constants::*;
use crate::frame::Frame;
use crate::terminal::{is_terminal_gone, Terminal};

use super::super::{effective_density, CloudConfig};
use super::activity::{register_activity, spin_wait, FrameTimeTracker};
use super::adaptive::{
    adaptive_resync_interval, EnduranceHealth, PerformanceSelfHealer, ReclaimState,
};
use super::event_loop_finalize::{finalize_session, SessionStats};
use super::hud::HudState;
use super::input::{handle_keybinding, is_unmodified, KeybindingCtx, PasteBurstGuard};
use super::watchdog::{GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE};

pub(crate) fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    crate::spawn_kill9_terminal_guard();

    // Install signal handlers + watchdog (extracted to signal_handlers.rs).
    // v50.0.0-beta.7 LOC refactor: terminal + cloud + frame setup
    // extracted to event_loop_setup.rs.
    let setup = super::event_loop_setup::setup_terminal_cloud_frame(cfg)?;
    let mut term = setup.term;
    let mut cloud = setup.cloud;
    let mut frame = setup.frame;
    let mut w = setup.w;
    let mut h = setup.h;
    let signal_exit = setup.signal_exit;
    let term_reinit = setup.term_reinit;
    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    // v20/v31: modular cinematic intro (plays in screensaver too; 'q' skips).
    // Extracted to event_loop_intro.rs to keep this file under the 1500-LOC
    // cap. The intro selection chain (intro_color unset / builtin theme /
    // custom palette / invalid fallback) + bug #10 post-intro terminal
    // size re-read are owned by that module.
    super::event_loop_intro::run_intro_sequence(
        &mut term, &mut frame, &mut cloud, &mut w, &mut h, cfg, density,
    )?;

    let start_time = Instant::now();
    let end_time = cfg.duration_s.and_then(|s| {
        if !s.is_finite() || s <= 0.0 {
            return None;
        }
        let s = cfg.duration.unwrap_or(s);
        Some(start_time + Duration::from_secs_f64(s))
    });

    let mut next_frame = Instant::now();
    // (Phase 3): PowerManager owns perf_pressure, is_idle, effective FPS.
    let mut power_manager = PowerManager::new(cfg.target_fps, Instant::now());

    let mut perf_frames: u64 = 0;
    let mut perf_drawn_frames: u64 = 0;
    let mut perf_work_sum_s: f64 = 0.0;
    let mut perf_work_max_s: f64 = 0.0;
    let mut perf_pressure_sum: f64 = 0.0;
    let mut perf_pressure_max: f32 = 0.0;
    let mut perf_overshoot_frames: u64 = 0;
    let (mut perf_utilization_sum, mut perf_utilization_max) = (0.0_f64, 0.0_f32);
    let mut frame_time_tracker: FrameTimeTracker = FrameTimeTracker::new();

    // Live HUD overlay ('i' toggles). Zero cost when off.
    let mut hud_state: HudState = HudState::new();
    hud_state.set_screen_size(w, h, cfg.screen_size.is_some());
    hud_state.set_target_fps(cfg.target_fps); // seed so `tgt:` is right from frame 1

    // Perceived-motion diagnostics: visible-change vs idle frames.
    let mut perf_idle_frames: u64 = 0;
    let mut perf_dirty_sum: u64 = 0;
    let mut perf_dirty_samples: u64 = 0;

    let mut last_resize_event: Option<Instant> = None; // resize debounce

    let mut last_resync_time = Instant::now(); // Cloud concern, not power

    let mut reclaim_state = ReclaimState::new(); // P4 madvise rate-limiter
    let mut endurance_health = EnduranceHealth::new(); // P5 health score
    #[cfg(target_os = "linux")] // /proc sampling; macOS stays 0
    let mut last_ctxt_switches: u64 = 0;
    let mut last_ctxt_sample = Instant::now();
    let mut perf_rss_samples: u64 = 0;

    let mut last_user_input_at = Instant::now(); // auto-snapback driver
    let mut self_healer = PerformanceSelfHealer::new(); // P1+P2

    let mut charset_preset = cfg.charset_preset.clone();
    let mut scene_name = cfg.scene_name.clone();
    let mut scene_generation: u64 = 0; // Phase D: u64 compare vs String clone
    let user_ranges = cfg.user_ranges.clone();
    let def_ascii = cfg.def_ascii;
    let mut paste_guard = PasteBurstGuard::default();

    // Live config reload: spawn watcher for config.toml changes.
    // The watcher thread sends validated config HashMaps via mpsc channel.
    // We try_recv() each frame (non-blocking, ~1ns on empty channel).
    // On update, rebuild CloudConfig + Cloud (full rebuild, not delta).
    let config_rx = if let Some(path) = &cfg.config_path_for_watcher {
        crate::live_config::spawn_watcher(path.clone())
    } else {
        None
    };
    // Store base CloudConfig for rebuilds (clone before any moves).
    let mut base_cfg = cfg.clone();
    // v50.0.0-alpha.7: track the LATEST live-reloaded CloudConfig so
    // finalize_session (at exit) reads the EFFECTIVE runtime values,
    // not the startup values. Without this, "final runtime state"
    // verbose section shows startup values (e.g. crystal_dragon=false)
    // instead of the live-reloaded values (e.g. crystal_dragon=true).
    let mut current_cfg = cfg.clone();
    // Pending rebuild: set when watcher sends new config, applied at top of next frame.
    let mut pending_config: Option<std::collections::HashMap<String, String>> = None;

    // Ambient scheduler: idle/wake thread sends AmbientEntry via mpsc.
    let ambient_handle = crate::crystal_dragon_engine::ambient_scheduler::spawn_ambient_scheduler(
        base_cfg.ambient_schedule.clone(),
    );
    let mut last_ambient_schedule = base_cfg.ambient_schedule.clone();
    // last-applied ambient entry — re-applied after live-reload rebuilds.
    let mut last_applied_ambient_entry: Option<
        crate::crystal_dragon_engine::ambient::AmbientEntry,
    > = None;
    // AB-07: permanent snapback kill — once schedule is detected empty
    // (by any path), auto-snapback is disabled until a new rx event is
    // applied from a non-empty schedule.
    let mut ambient_snapback_killed: bool = false;
    // v50 audit C-2: rate-limit the ground-truth file re-read to 1 per 5s
    // instead of per-frame. The previous code read + TOML-parsed the config
    // file every frame when ambient was active — ~60 reads/sec + 60 parses/sec.
    // The 30s idle-snapback latency tolerates 5s staleness.
    let mut last_ground_truth_check: Instant = Instant::now();
    // AB-08: config file path for ground-truth re-read. The watcher can
    // lose events, leaving all cached state stale. File on disk is truth.
    let config_path_for_ground_truth = base_cfg.config_path_for_watcher.clone();
    //  last-applied cfg map for diff trace + startup ambient.
    let initial_cfg_map = base_cfg
        .config_path_for_watcher
        .as_deref()
        .map(|p| crate::configfile::load_config_file(Some(p)))
        .unwrap_or_default();
    let mut last_applied_cfg_map: Option<std::collections::HashMap<String, String>> =
        Some(initial_cfg_map.clone());

    // +hotfix: synchronous ambient apply at startup with REAL cfg map.
    let (new_charset, startup_entry) = crate::crystal_dragon_engine::ambient::apply_startup_ambient(
        &mut cloud,
        &base_cfg.ambient_schedule,
        &charset_preset,
        &user_ranges,
        def_ascii,
        &initial_cfg_map,
    );
    // startup ambient info for post-exit verbose (main.rs prints after drop).
    let ambient_info = match &startup_entry {
        Some(e) => format!(
            "ambient: startup phase {:02}:{:02} (scene={}) applied at cold start",
            e.hour, e.minute, e.scene
        ),
        None => "ambient: no active phase at startup, default scene retained".to_string(),
    };
    super::set_startup_ambient_info(&ambient_info);
    if let Some(entry) = startup_entry {
        charset_preset = new_charset;
        scene_name = entry.scene.clone();
        scene_generation = scene_generation.wrapping_add(1);
        cloud.user_override_since_ambient = false;
        cloud.ambient_palette_locked = true;
        term.set_color_cache(ColorCache::new(&cloud.palette));
        frame = Frame::new(w, h, cloud.palette.bg);
        super::fill_terminal_bg(cloud.palette.bg);
        last_applied_ambient_entry = Some(entry);
        super::ambient_diag_startup();
        super::ambient_diag_scene_change("startup");
    }
    // Track runtime state for post-exit verbose summary.
    while cloud.raining {
        // Graceful shutdown from signal handler (clean exit via Terminal::drop).
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            cloud.raining = false;
            break;
        }

        // Live config reload: non-blocking check for config events.
        if let Some(ref rx) = config_rx {
            while let Ok(event) = rx.try_recv() {
                crate::lr_trace!("render thread received config event from watcher channel");
                match event {
                    Ok(cfg) => {
                        crate::lr_trace!(
                            "render thread: pending config set ({} keys) — will rebuild next frame",
                            cfg.len()
                        );
                        pending_config = Some(cfg);
                    }
                    Err(msg) => {
                        // config validation errors cause immediate exit.
                        crate::lr_trace!(
                            "render thread: config validation error — setting exit code + breaking rain loop"
                        );
                        if let Ok(mut guard) = crate::live_config::LIVE_RELOAD_ERROR.lock() {
                            *guard = Some(msg);
                        }
                        crate::live_config::LIVE_RELOAD_EXIT_CODE
                            .store(2, std::sync::atomic::Ordering::Release);
                        cloud.raining = false;
                        break;
                    }
                }
            }
        }

        // Apply pending Cloud rebuild (swaps Cloud + Frame between frames).
        if let Some(new_cfg_map) = pending_config.take() {
            // v50.0.0-beta.6 masterclass: temporal precedence.
            // Startup: CLI > config > scene defaults.
            // Runtime: config > scene defaults (CLI retired).
            // CLI flags are zeroed before rebuild so config has full
            // authority on live-reload. Scene defaults synced first.
            if !new_cfg_map.contains_key("scene") {
                super::event_loop_scene_sync::sync_base_cfg_with_runtime_scene(
                    &mut base_cfg,
                    &scene_name,
                );
            }
            base_cfg.cli_explicit = crate::app::CliExplicit::default();
            let new_cfg = crate::live_config::rebuild_cloud_config(&base_cfg, &new_cfg_map);
            // v50.0.0-alpha.7: track latest config for finalize_session.
            current_cfg = new_cfg.clone();
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
            crate::live_config_trace::trace_config_diff(
                last_applied_cfg_map.as_ref(),
                &new_cfg_map,
            );
            last_applied_cfg_map = Some(new_cfg_map.clone());
            // Phase D #9: preserve ecosystem + post-FX across reload.
            // AB-02: capture override state for schedule-empty restore.
            let preserve_user_override = cloud.user_override_since_ambient;
            let preserved_color_scheme = cloud.color_scheme;
            let preserved_palette = cloud.palette.clone();
            let preserved_scene_name = scene_name.clone();
            let mut new_cloud = new_cfg.create_cloud(density);
            new_cloud.inherit_ecosystem_state(&cloud);
            cloud = new_cloud;
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
            frame = Frame::new(w, h, cloud.palette.bg);
            super::fill_terminal_bg(cloud.palette.bg);
            charset_preset = new_cfg.charset_preset.clone();
            //  recompute target FPS from new config.
            let safe_fps = new_cfg.resolve_capped_fps(cfg.target_fps);
            power_manager.set_target_fps(safe_fps);
            // v30: keep HUD tgt: in sync with live-reloaded fps.
            hud_state.set_target_fps(safe_fps);
            // AB-07: count every config rebuild for diagnostics.
            super::ambient_diag_config_rebuild();
            // Ambient: push new schedule to scheduler if it changed.
            if new_cfg.ambient_schedule != last_ambient_schedule {
                super::ambient_diag_schedule_reload();
                ambient_handle.reload(new_cfg.ambient_schedule.clone());
                last_ambient_schedule = new_cfg.ambient_schedule.clone();
                if new_cfg.ambient_schedule.entries.is_empty() {
                    super::ambient_diag_schedule_empty();
                    if let Some(ref le) = last_applied_ambient_entry {
                        if scene_name == le.scene {
                            scene_name = new_cfg.scene_name.clone();
                            scene_generation = scene_generation.wrapping_add(1);
                        }
                    }
                    last_applied_ambient_entry = None;
                    cloud.ambient_palette_locked = false;
                    cloud.user_override_since_ambient = true;
                    ambient_snapback_killed = true;
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
                        last_ambient_schedule = new_cfg.ambient_schedule.clone();
                        super::ambient_diag_schedule_reload();
                        super::ambient_diag_schedule_empty();
                    }
                    last_applied_ambient_entry = None;
                    cloud.ambient_palette_locked = false;
                    cloud.user_override_since_ambient = true;
                    ambient_snapback_killed = true;
                    super::ambient_diag_snapback_killed();
                }
            } else if ambient_snapback_killed {
                ambient_snapback_killed = false;
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
                    charset_preset = cloud.apply_ambient_entry(
                        last_entry,
                        &charset_preset,
                        &user_ranges,
                        def_ascii,
                        &cm,
                    );
                    scene_name = last_entry.scene.clone();
                    scene_generation = scene_generation.wrapping_add(1);
                    cloud.user_override_since_ambient = false;
                    cloud.ambient_palette_locked = true;
                    super::ambient_diag_reapply();
                    super::ambient_diag_scene_change("rebuild-reapply");
                    term.set_color_cache(ColorCache::new(&cloud.palette));
                    frame = Frame::new(w, h, cloud.palette.bg);
                    super::fill_terminal_bg(cloud.palette.bg);
                } else if !still_in {
                    crate::lr_trace!(
                        "ambient: last entry no longer in schedule — clearing tracker"
                    );
                    last_applied_ambient_entry = None;
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
                        scene_name = preserved_scene_name;
                    } else {
                        scene_name = new_cfg.scene_name.clone();
                        scene_generation = scene_generation.wrapping_add(1);
                        charset_preset = cloud.apply_scene_runtime(
                            &scene_name,
                            &charset_preset,
                            &user_ranges,
                            def_ascii,
                        );
                        term.set_color_cache(ColorCache::new(&cloud.palette));
                        frame = Frame::new(w, h, cloud.palette.bg);
                        super::fill_terminal_bg(cloud.palette.bg);
                    }
                } else {
                    scene_name = new_cfg.scene_name.clone();
                    scene_generation = scene_generation.wrapping_add(1);
                    charset_preset = cloud.apply_scene_runtime(
                        &scene_name,
                        &charset_preset,
                        &user_ranges,
                        def_ascii,
                    );
                    term.set_color_cache(ColorCache::new(&cloud.palette));
                    frame = Frame::new(w, h, cloud.palette.bg);
                    super::fill_terminal_bg(cloud.palette.bg);
                }
                cloud.user_override_since_ambient = true;
                cloud.ambient_palette_locked = false;
            }
        }
        // AB-03+AB-04: poll ambient phase events. Empty schedule → drain; non-empty → discard stale.
        let mut last_ambient_entry: Option<crate::crystal_dragon_engine::ambient::AmbientEntry> =
            None;
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
            && last_ground_truth_check.elapsed() >= std::time::Duration::from_secs(5)
        {
            last_ground_truth_check = Instant::now();
            if let Some(ref path) = config_path_for_ground_truth {
                if let Ok(c) = std::fs::read_to_string(path) {
                    let pv = &crate::configfile::parse_config_text(&c).values;
                    if crate::crystal_dragon_engine::ambient::collect_ambient_schedule(pv)
                        .entries
                        .is_empty()
                    {
                        last_ambient_entry = None;
                        last_ambient_schedule.entries.clear();
                        last_applied_ambient_entry = None;
                        cloud.ambient_palette_locked = false;
                        cloud.user_override_since_ambient = true;
                        ambient_snapback_killed = true;
                        ambient_handle.reload(
                            crate::crystal_dragon_engine::ambient::AmbientSchedule::default(),
                        );
                        super::ambient_diag_schedule_empty();
                        super::ambient_diag_schedule_reload();
                        super::ambient_diag_snapback_killed();
                    }
                }
            }
        }
        if let Some(entry) = last_ambient_entry {
            if last_applied_ambient_entry.as_ref() == Some(&entry)
                && !cloud.user_override_since_ambient
            {
                // Duplicate — already applied.
            } else {
                let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
                charset_preset = cloud.apply_ambient_entry(
                    &entry,
                    &charset_preset,
                    &user_ranges,
                    def_ascii,
                    &cfg_map,
                );
                last_applied_ambient_entry = Some(entry.clone());
                scene_name = entry.scene.clone();
                scene_generation = scene_generation.wrapping_add(1);
                super::ambient_diag_rx();
                super::ambient_diag_scene_change(&format!("rx-event(scene={})", entry.scene));
                cloud.user_override_since_ambient = false;
                cloud.ambient_palette_locked = true;
                if ambient_snapback_killed {
                    ambient_snapback_killed = false;
                }
                term.set_color_cache(ColorCache::new(&cloud.palette));
                frame = Frame::new(w, h, cloud.palette.bg);
                super::fill_terminal_bg(cloud.palette.bg);
            }
        }
        // AB-08: snapback ground-truth guard — re-read config file (~50µs I/O, ≤ once per 30s).
        let _ab06_sked_len = last_ambient_schedule.entries.len() as u64;
        let _ab06_last_applied = last_applied_ambient_entry.is_some();
        super::ambient_diag_snapback_guard(_ab06_sked_len, _ab06_last_applied);
        let ground_truth_ambient_empty =
            if !ambient_snapback_killed && _ab06_sked_len > 0 && _ab06_last_applied {
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
            last_applied_ambient_entry = None;
            cloud.ambient_palette_locked = false;
            cloud.user_override_since_ambient = true;
            ambient_snapback_killed = true;
            ambient_handle
                .reload(crate::crystal_dragon_engine::ambient::AmbientSchedule::default());
            super::ambient_diag_schedule_empty();
            super::ambient_diag_schedule_reload();
            super::ambient_diag_snapback_killed();
        }
        if !ambient_snapback_killed
            && _ab06_sked_len > 0
            && _ab06_last_applied
            && !ground_truth_ambient_empty
            && super::input::try_auto_snapback(
                &mut cloud,
                &mut charset_preset,
                &mut scene_name,
                &mut scene_generation,
                &mut last_applied_ambient_entry,
                &last_ambient_schedule,
                &last_applied_cfg_map,
                &user_ranges,
                def_ascii,
                last_user_input_at,
                current_cfg.effective_snapback_delay(crate::constants::AUTO_SNAPBACK_DELAY_SECS),
            )
        {
            term.set_color_cache(ColorCache::new(&cloud.palette));
            frame = Frame::new(w, h, cloud.palette.bg);
            super::fill_terminal_bg(cloud.palette.bg);
            next_frame = Instant::now();
        }
        // Adaptive throttling: reduce FPS when idle to save CPU.
        let loop_now = Instant::now();
        // Capture scene generation at frame start — u64 copy for self-healer.
        let scene_generation_at_frame_start = scene_generation;
        // (Phase 3): PowerManager.begin_frame — is_idle, predictor, idle_started.
        let is_idle = power_manager.begin_frame(loop_now);
        // P2: adaptive resync interval based on sustained idle duration.
        let idle_secs = power_manager
            .idle_started()
            .map(|t| loop_now.saturating_duration_since(t).as_secs_f64())
            .unwrap_or(0.0);
        let effective_resync_interval = adaptive_resync_interval(idle_secs);
        if is_idle
            && loop_now
                .saturating_duration_since(last_resync_time)
                .as_secs_f64()
                >= effective_resync_interval
        {
            cloud.force_draw_everything();
            last_resync_time = loop_now;
            next_frame = loop_now;
            // P4: Hint kernel to reclaim stale pages during sustained idle.
            if reclaim_state.should_reclaim(loop_now) {
                let cells_ptr = frame.cells.as_ptr();
                let cells_len = frame.cells.len() * std::mem::size_of_val(&frame.cells[0]);
                // SAFETY: frame.cells is a valid Vec allocation.
                // hint_reclaim_pages advises only pages fully interior to
                // the allocation (never shared arena edge pages) — see
                // reclaim_state.rs for the corrected MADV_DONTNEED
                // semantics (zero-fill-on-demand). The zeroed interior
                // cells read as blank: force_draw_everything() was set
                // above, and the next rain_at() bumps the content
                // generation before any cell is read.
                unsafe {
                    super::adaptive::hint_reclaim_pages(cells_ptr as *const u8, cells_len);
                }
                reclaim_state.mark_reclaimed(loop_now);
            }
        }
        // P2: reuse loop_now (captured at top of loop) instead of another Instant::now().
        if end_time.is_some_and(|end| loop_now >= end) {
            cloud.raining = false;
            break;
        }
        let mut pending_resize: Option<(u16, u16)> = None;
        if crate::platform::swap_term_reinit(&term_reinit) {
            drop(term);
            term = Terminal::with_signal_exit(signal_exit.clone())?;
            // v17: always re-enable mouse reporting after SIGCONT (see
            // startup comment for rationale — block copy in all modes).
            if term.enable_mouse_capture().is_ok() {
                MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
            }
            let (nw, nh) = term.size()?;
            pending_resize = Some((nw, nh));
            cloud.force_draw_everything();
            let reinit_time = Instant::now();
            last_resync_time = reinit_time;
            next_frame = reinit_time;
        }
        loop {
            // Drain pending events. On Windows (ConPTY) and Termux (Android
            // PTY), crossterm's event::poll/read can fail with transient I/O
            // errors — treat as non-fatal (break drain, render frame). Watchdog
            // catches persistent failures. Terminal-gone (EIO/EBADF/BrokenPipe):
            // poll returns Ok(true) forever, read returns Err(EIO); we set
            // cloud.raining = false to exit the wait-phase immediately (else
            // spin-wait burns 100% CPU for seconds).
            loop {
                match Terminal::poll_event(Duration::from_millis(0)) {
                    Ok(false) => break,
                    Err(e) if is_terminal_gone(&e) => {
                        cloud.raining = false;
                        break;
                    }
                    Err(_) => break,
                    Ok(true) => {}
                }
                let ev = match Terminal::read_event() {
                    Ok(e) => e,
                    Err(e) if is_terminal_gone(&e) => {
                        cloud.raining = false;
                        break;
                    }
                    Err(_) => break,
                };
                match ev {
                    Event::Resize(nw, nh) => {
                        // --screen-size: ignore terminal resize when in fixed mode
                        if cfg.screen_size.is_some() {
                            // Fixed mode — ignore resize, keep virtual size
                        } else {
                            // Dynamic mode — clamp to safe bounds before storing
                            let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
                            let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
                            pending_resize = Some((cw, ch));
                            last_resize_event = Some(Instant::now());
                        }
                    }
                    Event::Key(k) => {
                        // Android/Termux: accept Press+Repeat, skip Release (Press-only
                        // guard silently dropped 'i' on Android). Desktop: Press-only.
                        // v50 audit C-3: cache Termux detection via OnceLock
                        // (was per-keypress std::env::var x2 — ~30 mutex
                        // locks/sec on held-key auto-repeat).
                        static IS_TERMUX: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
                        let is_android = *IS_TERMUX.get_or_init(|| {
                            std::env::var("TERMUX_VERSION").is_ok()
                                || std::env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"))
                        });
                        if is_android {
                            if k.kind == KeyEventKind::Release {
                                continue;
                            }
                        } else if k.kind != KeyEventKind::Press {
                            continue;
                        }
                        let activity_time = Instant::now();
                        if paste_guard.ignore_plain_key(&k, activity_time) {
                            let _ = register_activity(
                                &mut power_manager,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            cloud.force_draw_everything();
                            next_frame = activity_time;
                            continue;
                        }
                        // HUD toggle ('i'): check BEFORE screensaver exit to prevent
                        // self-exit on Android/Termux. v30: lowercase-only. Toggling
                        // OFF calls force_draw_everything() to clear stale HUD residue.
                        // Modifier guard: only bare 'i' (NONE). Rejects Shift+'i'
                        // (which produces 'I', no binding) and all other modifiers
                        // (Ctrl/Super/Alt/Hyper/Meta+'i'). See is_unmodified().
                        if is_unmodified(k.modifiers) && matches!(k.code, KeyCode::Char('i')) {
                            let now_visible = hud_state.toggle();
                            if !now_visible {
                                cloud.force_draw_everything();
                            }
                            // Set next_frame=activity_time so HUD appears immediately;
                            // otherwise idle-mode delay could defer render by seconds.
                            let _ = register_activity(
                                &mut power_manager,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            next_frame = activity_time;
                            continue;
                        }
                        // v50.0.0-beta.6: the 'h' shortkey is REMOVED
                        // completely (was a HUD position toggle, now purged).
                        // HUD always renders flush-left at column 0. Any
                        // user input resets idle timer for adaptive throttling.
                        if register_activity(
                            &mut power_manager,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        ) {
                            cloud.force_draw_everything();
                            next_frame = activity_time;
                        }
                        // refresh auto-snapback idle timer on every key press.
                        last_user_input_at = activity_time;
                        // Process the keybinding. This lets interactive
                        // keys (q, c/C, s/S, p, x, [, ], Space, Up/Down,
                        // i) work even in --screensaver mode.
                        let redraw_needed = handle_keybinding(
                            &mut KeybindingCtx {
                                cloud: &mut cloud,
                                frame: &mut frame,
                                charset_preset: &mut charset_preset,
                                scene_name: &mut scene_name,
                                scene_generation: &mut scene_generation,
                                user_ranges: &user_ranges,
                                def_ascii,
                                cfg,
                                term_reinit: &term_reinit,
                            },
                            &k,
                        );
                        if cfg.screensaver {
                            // Screensaver: recognized keys process+continue; others ignored.
                            // Mouse click doesn't exit (v17). Only 'q' quits.
                            if !cloud.raining {
                                break;
                            }
                            // No is_recognized_key check — all unrecognized
                            // keys fall through to handle_keybinding's
                            // `_ => {}` catch-all and are silently ignored.
                        } else if redraw_needed {
                            next_frame = Instant::now();
                        }
                    }
                    Event::Paste(_) => {
                        let activity_time = Instant::now();
                        paste_guard.note_bracketed_paste(activity_time);
                        let _ = register_activity(
                            &mut power_manager,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        );
                        cloud.force_draw_everything();
                        next_frame = activity_time;
                    }
                    Event::Mouse(m) => {
                        // Mouse events always captured (blocks drag-select). No force_draw
                        // on MOVE (old: bright-color flash). CLICK wakes renderer
                        // on idle→active (old: click effect vanished at 30 FPS idle cadence).
                        let activity_time = Instant::now();
                        let is_click = matches!(m.kind, MouseEventKind::Down(_));
                        let was_idle = is_idle;
                        let _ = register_activity(
                            &mut power_manager,
                            &mut last_resync_time,
                            activity_time,
                            was_idle,
                            false,
                        );
                        // Hover/click visual effects are ALWAYS ON (--mouse deleted).
                        // BUT: when paused OR decelerating, skip click wave
                        // effects to prevent queued flash waves from
                        // accumulating and causing "stuck particles" on
                        // resume (owner-reported bug: rapid pause/unpause
                        // cycles left effects hanging).
                        //
                        // Must check `is_paused_or_decelerating()` (not just
                        // `pause`) because the deceleration phase is also a
                        // pause-related state where click effects should be
                        // suppressed.
                        // Mouse position is still tracked (hover glow) and
                        // the event is still consumed (blocks drag-select).
                        cloud.set_mouse_position(m.column, m.row);
                        if is_click && !cloud.is_paused_or_decelerating() {
                            cloud.set_mouse_click(m.column, m.row);
                            // Wake renderer immediately on idle→active click.
                            if was_idle {
                                cloud.force_draw_everything();
                                next_frame = activity_time;
                            }
                        }
                    }
                    Event::FocusGained => {
                        let activity_time = Instant::now();
                        if register_activity(
                            &mut power_manager,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            true,
                        ) {
                            cloud.force_draw_everything();
                            next_frame = activity_time;
                        }
                    }
                    _ => {}
                }
            }
            // Break when resize debounce elapses (coalesces drag storms), or
            // immediately on SIGHUP/SIGTERM / dead PTY. Without the shutdown
            // check, the wait loop burns CPU until next_frame after the signal.
            if !cloud.raining || GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
                break;
            }
            if pending_resize.is_some() {
                let debounce_elapsed = last_resize_event
                    .map(|t| t.elapsed() >= Duration::from_millis(RESIZE_DEBOUNCE_MS))
                    .unwrap_or(true);
                if debounce_elapsed {
                    break;
                }
            }
            let now = Instant::now();
            // Monotonic clock jump guard
            let frame_elapsed = now.saturating_duration_since(next_frame);
            if frame_elapsed.as_secs_f64() > CLOCK_JUMP_GUARD_SECS {
                next_frame = now;
                break;
            }
            if now >= next_frame {
                break;
            }
            let mut timeout = next_frame - now;
            if let Some(end) = end_time {
                if now >= end {
                    break;
                }
                timeout = timeout.min(end - now);
            }
            // Spin-sleep hybrid: poll_event for bulk of wait, spin-wait final
            // ~500μs for sub-ms deadline accuracy (spin_budget from constants.rs).
            // Dead-PTY guard: on force-close, POLLHUP makes poll_event return
            // Ok(true) forever; we continue to drain which catches EIO via
            // is_terminal_gone, dropping post-SIGHUP CPU burn from 20s→<1ms.
            let spin_budget = FRAME_SPIN_BUDGET;
            if timeout > spin_budget {
                // poll_event Err on dead PTY (EIO/BadFd). Propagating via `?`
                // would double-panic on broken stderr → abort. Treat as
                // terminal-gone: stop rain, break; post-loop drop exits cleanly.
                match Terminal::poll_event(timeout - spin_budget) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) if is_terminal_gone(&e) => {
                        cloud.raining = false;
                        break;
                    }
                    Err(e) => return Err(e),
                }
                // Spin-wait the remaining time for precise deadline alignment.
                // The spin is capped at 1ms internally to handle edge cases.
                // Only reached when poll returned Ok(false) — no events, so
                // spinning to the deadline is the correct behavior.
                spin_wait(next_frame);
            } else {
                // Already close to deadline (< 500μs away): spin-wait to hit
                // it precisely, then drain any events that arrived.
                spin_wait(next_frame);
                match Terminal::poll_event(Duration::from_millis(0)) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) if is_terminal_gone(&e) => {
                        cloud.raining = false;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        if !cloud.raining {
            break;
        }
        if let Some((nw, nh)) = pending_resize {
            // v50.0.0-beta.6 CRITICAL FIX: update the local w/h variables
            // alongside cloud + frame. Previously only cloud.reset() and
            // Frame::new() were called with the new dimensions, but the
            // local `w` and `h` variables stayed at the pre-resize values.
            // When a live-reload triggered the rebuild path (line 342-399),
            // it used the STALE w/h — reverting the screen to the pre-resize
            // size (e.g. 150x32 after the user had gone fullscreen to 212x64).
            // This was a FATAL visual bug for LTS release. Now w/h are kept
            // in sync with the actual terminal dimensions at all times.
            w = nw;
            h = nh;
            cloud.reset(nw, nh);
            frame = Frame::new(nw, nh, cloud.palette.bg);
            // v50.0.0-beta.6: use current_cfg (live-reloaded) instead of
            // cfg (startup) for density settings. If the user live-reloads
            // density_auto or base_density, the resize handler must respect
            // the new values — otherwise a resize after live-reload would
            // use stale startup density.
            if current_cfg.density_auto {
                cloud.set_droplet_density(effective_density(current_cfg.base_density, nw, true));
            }
            cloud.force_draw_everything();
            // H1 (internal independent QA): refresh the SGR color cache after
            // resize — every other palette-affecting path calls set_color_cache,
            // but the resize handler was missing it. Without this, a live-reload
            // palette change coinciding with a resize could produce a 1-frame
            // color flicker from a stale cache.
            term.set_color_cache(ColorCache::new(&cloud.palette));
            last_resync_time = Instant::now();
            // Update HUD screen size on dynamic resize (fixed mode ignores resize)
            if cfg.screen_size.is_none() {
                hud_state.set_screen_size(nw, nh, false);
            }
        }
        // Key handling can toggle pause/resume after the frame period was
        // chosen for the wait phase. Recompute before simulation and
        // scheduling so the first resumed frame does not inherit the paused
        // 250ms cadence.
        // (Phase 3): PowerManager.effective_fps() replaces the
        // target_period / idle_period / pause_period Duration cascade.
        // v50.0.0-beta.6: use current_cfg.power_dragon (live-reloaded) so
        // live-reloading power_dragon=false immediately affects frame pacing.
        let frame_period = Duration::from_secs_f64(
            1.0 / power_manager.effective_fps(cloud.pause, current_cfg.power_dragon),
        );
        let frame_period_s = frame_period.as_secs_f32().max(0.000_001);
        // v30 (2026-08-05): announce frame pacing mode to the HUD so the
        // `tgt:` line can show an `idle` / `paused` suffix. Cheap (one enum
        // set + one method call). Placed AFTER the pause/idle/active branch
        // so the mode reflects the actual cadence used for this frame, not
        // the previous frame's.
        // v50.0.0-beta.7 LOC refactor: HUD state update extracted to
        // event_loop_hud.rs.
        super::event_loop_hud::update_hud_state(
            &mut hud_state,
            &mut cloud,
            &power_manager,
            &scene_name,
            &charset_preset,
            &current_cfg,
        );
        let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
        // (perf audit): clamp lower bound is now `SIM_FACTOR_MIN`
        // from constants.rs — was a hardcoded `0.3` inline.
        let sim_factor = (1.0
            - (power_manager.effective_pressure() as f64) * SIM_PRESSURE_SCALE_FACTOR)
            .clamp(SIM_FACTOR_MIN, 1.0);
        let sim_min_s = (frame_period.as_secs_f64() * SIM_MIN_FRACTION).max(0.001);
        let sim_max_s = sim_base_s.min(SIM_MAX_CAP_SECS);
        // When frame_period is large (pause mode: 250ms, or very low FPS),
        // sim_min_s can exceed sim_max_s, which would panic in f64::clamp.
        // Sanitize: use sim_max_s as the effective lower bound when inverted.
        let sim_cap_s = if sim_min_s <= sim_max_s {
            (sim_base_s * sim_factor).clamp(sim_min_s, sim_max_s)
        } else {
            sim_max_s
        };
        cloud.set_max_sim_delta(Duration::from_secs_f64(sim_cap_s));
        let work_start = Instant::now();
        // v30 dragon-egg hunt: removed `cloud.is_idle = is_idle` write —
        // the field was a zombie (set here every frame, never read by any
        // cloud code path). The "Weather Director tick" mentioned in the
        // old comment never existed. The interactive event loop already
        // uses `is_idle` directly for frame_period selection above and
        // for the resync logic; the simulation itself does not need it.
        // P1: call rain_at directly with work_start instead of cloud.rain()
        // (which calls Instant::now() internally). Saves 1 Instant::now()
        // per frame (~20ns).
        cloud.rain_at(&mut frame, work_start);
        // Refresh HUD line colors every frame (cheap — 4 brighten_color
        // calls ≈ 2 µs). This is split out of the 1 Hz `update_metrics`
        // tick so a runtime palette change (`c`/`C` key cycle, auto-color-
        // drift, live-config reload, scene transition) is reflected on
        // the very next frame, with no perceptible delay. Previously,
        // colors were computed inside `update_metrics` (rate-limited to
        // 1 Hz), so a palette change took up to 1 second to appear in
        // the HUD — the rain had already adopted the new palette while
        // the HUD still showed the old colors. The owner explicitly
        // flagged this as 'slight delay every owner changes colors at
        // runtime'. The split eliminates the delay without raising the
        // metric-tick rate (which would cause number flicker).
        //
        // Must run BEFORE write_to_frame so the colors used for THIS
        // frame's HUD cells are fresh — write_to_frame reads the Color
        // half of each cached_lines tuple.
        hud_state.refresh_colors(cloud.hud_colors());

        // Write HUD into the frame buffer BEFORE term.draw() so it's
        // part of the same flush — eliminates fullscreen flicker.
        // v16: Pass palette bg so HUD background follows --color-bg setting.
        hud_state.write_to_frame(&mut frame, cloud.cols, cloud.palette.bg);

        // Cache dirty checks once per frame to avoid redundant method calls.
        let is_dirty_all = frame.is_dirty_all();
        let dirty_len = frame.dirty_indices().len();
        let did_draw = is_dirty_all || dirty_len > 0;
        if did_draw {
            if let Err(e) = term.draw(&mut frame) {
                // EIO on Linux = terminal (PTY) was closed/destroyed.
                // BrokenPipe = write to closed pipe (macOS, some Linux).
                // In both cases, the terminal is gone — exit gracefully
                // instead of continuing to write to a dead fd.
                if is_terminal_gone(&e) {
                    cloud.raining = false;
                    break;
                }
                // Other I/O errors: propagate normally.
                return Err(e);
            }
        }
        // v50.0.0-beta.7 LOC refactor: post-draw accounting extracted to
        // event_loop_post_draw.rs.
        let post_draw = super::event_loop_post_draw::post_draw_accounting(
            &mut hud_state,
            &mut power_manager,
            &term,
            &cloud,
            work_start,
            frame_period_s,
        );
        let work_s = post_draw.work_s;
        let overshoot = post_draw.overshoot;
        let utilization = post_draw.utilization;

        // v50.0.0-beta.7 LOC refactor: P5 health sampling extracted to
        // event_loop_p5.rs.
        if !super::event_loop_p5::sample_p5_health(
            &mut endurance_health,
            &mut hud_state,
            &mut power_manager,
            &mut term,
            &mut cloud,
            work_s as f64,
            work_start,
            &mut perf_rss_samples,
            #[cfg(target_os = "linux")]
            &mut last_ctxt_switches,
            &mut last_ctxt_sample,
        ) {
            break;
        }

        // v50.0.0-beta.7 LOC refactor: perf stats display extracted to
        // event_loop_perf_stats.rs.
        super::event_loop_perf_stats::update_perf_stats(
            &mut perf_frames,
            &mut perf_drawn_frames,
            &mut perf_idle_frames,
            &mut perf_dirty_sum,
            &mut perf_dirty_samples,
            &mut perf_work_sum_s,
            &mut perf_work_max_s,
            &mut perf_pressure_sum,
            &mut perf_pressure_max,
            &mut perf_utilization_sum,
            &mut perf_utilization_max,
            &mut perf_overshoot_frames,
            &mut frame_time_tracker,
            &frame,
            &power_manager,
            work_s,
            did_draw,
            is_dirty_all,
            dirty_len,
            overshoot,
            utilization,
            cfg.perf_stats,
        );

        // v50.0.0-beta.7 LOC refactor: performance self-healer extracted
        // to event_loop_self_heal.rs.
        super::event_loop_self_heal::run_self_healer(
            &mut self_healer,
            &mut reclaim_state,
            &mut cloud,
            &mut frame,
            &current_cfg,
            &scene_name,
            scene_generation,
            scene_generation_at_frame_start,
            power_manager.effective_pressure(),
            loop_now,
            endurance_health.score(),
        );

        // Schedule next frame relative to the ideal deadline, using the
        // pre-work timestamp to prevent drift between render work and
        // scheduling. Single-reschedule: if we overslept past the next tick,
        // snap forward by exactly one period from now instead of
        // double-advancing (which caused visible stutter on frames that took
        // just 1μs too long).
        let frame_ts = work_start;
        let next = next_frame.checked_add(frame_period).unwrap_or(frame_ts);
        next_frame = if frame_ts > next {
            frame_ts.checked_add(frame_period).unwrap_or(frame_ts)
        } else {
            next
        };
    }

    // Post-loop finalization extracted to event_loop_finalize.rs (file-cap
    // compliance). Bundles shutdown signal, final FPS line, perf report,
    // terminal drop (AB-10), final-state handoff.
    let stats = SessionStats {
        start_time,
        perf_frames,
        perf_drawn_frames,
        perf_idle_frames,
        perf_overshoot_frames,
        perf_dirty_sum,
        perf_dirty_samples,
        // Exit-time grid snapshot — denominator for the runtime
        // avg_dirty_cell_ratio_percent (owner request 2026-08-23).
        grid_cols: cloud.cols,
        grid_lines: cloud.lines,
        perf_work_sum_s,
        perf_work_max_s,
        perf_pressure_sum,
        perf_pressure_max,
        perf_utilization_sum,
        perf_utilization_max,
        frame_time_tracker: &frame_time_tracker,
        power_manager_phase_transitions: power_manager.phase_transitions_observed(),
        power_manager_base_target_fps: power_manager.base_target_fps(),
        endurance_health_score: endurance_health.score(),
        endurance_health_classification: endurance_health.classification(),
    };
    finalize_session(
        &stats,
        term,
        &cloud,
        &scene_name,
        &charset_preset,
        &current_cfg,
    )
}
