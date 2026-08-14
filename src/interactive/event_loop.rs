// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
use crate::report::Report;
use crate::terminal::{is_terminal_gone, Terminal};

use super::super::{effective_density, CloudConfig};
use super::activity::{register_activity, spin_wait, FrameTimeTracker};
use super::adaptive::{
    adaptive_resync_interval, EnduranceHealth, PerformanceSelfHealer, ReclaimState, SelfHealAction,
};
use super::hud::{FrameMode, HudState};
use super::input::{handle_keybinding, PasteBurstGuard};
use super::watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE, SHUTDOWN};
use crate::central_control_dragon_power::sample_thermal_pressure;

pub(crate) fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    crate::spawn_kill9_terminal_guard();

    // Install signal handlers + watchdog (extracted to signal_handlers.rs).
    let (signal_exit, term_reinit) = super::signal_handlers::install_signal_handlers();
    // On non-Unix (Windows), term_reinit is unused — no SIGCONT/SIGTSTP.
    // Consume it here to suppress the unused_variable warning. On Unix,
    // this line is cfg'd out and term_reinit is used later for SIGCONT.
    #[cfg(not(unix))]
    let _ = term_reinit;

    // AB-10: emit pre-alt-screen warnings BEFORE Terminal::with_signal_exit()
    // enters the alt screen. Otherwise they leak into the rain matrix.
    let fixed_size = cfg.screen_size;
    super::emit_pre_alt_screen_warnings(fixed_size, cfg.intro != crate::config::IntroType::None);

    let mut term = Terminal::with_signal_exit(signal_exit.clone())?;
    if term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    let (mut w, mut h) = if let Some(fixed) = fixed_size {
        fixed
    } else {
        term.size()?
    };

    let density = effective_density(cfg.base_density, w, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    // Enable atmospheric events for interactive mode (ghosts, etc.).
    cloud.enable_events();
    // P1: enable per-component timing only when --perf-stats is requested.
    // When off, rain_at() skips 2 Instant::now() calls per frame (~40ns).
    cloud.set_component_timing(cfg.perf_stats);

    // Build color byte cache from the palette so the draw hot path can
    // emit pre-formatted ANSI SGR sequences instead of formatting on the fly.
    term.set_color_cache(ColorCache::new(&cloud.palette));

    let mut frame = Frame::new(w, h, cloud.palette.bg);

    // v16: Fill entire alternate screen with palette bg before first frame (avoids visible edge gaps).
    super::fill_terminal_bg(cloud.palette.bg);

    // v20: Modular cinematic intro (--intro <type> flag).
    // v31: intro now plays in screensaver mode too (cosmostrix's signature; only 'q' skips).
    if cfg.intro != crate::config::IntroType::None {
        super::intro::run_intro(&mut term, &mut frame, &cloud, w, h, cfg.intro)?;
        cloud.force_draw_everything();
        frame.clear_with_bg(cloud.palette.bg);

        // v25.11 (bug #10): re-read terminal size after intro returns.
        // Intro can take seconds; if user resized during it, (w,h) is stale
        // — rain renders wrong until SIGWINCH. Fixed mode ignores resize.
        if cfg.screen_size.is_none() {
            if let Ok((nw, nh)) = term.size() {
                if nw != w || nh != h {
                    let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
                    let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
                    w = cw;
                    h = ch;
                    cloud.reset(cw, ch);
                    frame = Frame::new(cw, ch, cloud.palette.bg);
                    if cfg.density_auto {
                        cloud.set_droplet_density(effective_density(cfg.base_density, cw, true));
                    }
                    cloud.force_draw_everything();
                    super::fill_terminal_bg(cloud.palette.bg);
                }
            }
        }
    }

    let start_time = Instant::now();
    let end_time = cfg.duration_s.and_then(|s| {
        if !s.is_finite() || s <= 0.0 {
            return None;
        }
        let s = cfg.duration.unwrap_or(s);
        Some(start_time + Duration::from_secs_f64(s))
    });

    let mut next_frame = Instant::now();
    // v30.8 (Phase 3): PowerManager owns perf_pressure accumulation, is_idle
    // detection, and effective FPS resolution. Replaces scattered Duration cascade.
    let mut power_manager = PowerManager::new(cfg.target_fps, Instant::now());

    let mut perf_frames: u64 = 0;
    let mut perf_drawn_frames: u64 = 0;
    let mut perf_work_sum_s: f64 = 0.0;
    let mut perf_work_max_s: f64 = 0.0;
    let mut perf_pressure_sum: f64 = 0.0;
    let mut perf_pressure_max: f32 = 0.0;
    let mut perf_overshoot_frames: u64 = 0;
    // Utilization = work_s / frame_period_s (always non-zero).
    let (mut perf_utilization_sum, mut perf_utilization_max) = (0.0_f64, 0.0_f32);
    let mut frame_time_tracker: FrameTimeTracker = FrameTimeTracker::new();

    // Live HUD overlay state — toggled with 'i'. When visible, renders a
    // compact FPS/p99/RSS/CPU overlay in the top-right corner at 1 Hz.
    // Zero cost when off (all methods short-circuit on visible==false).
    // 'i' is the canonical toggle; 'h' moves the overlay between left
    // and right corners. (v30 simplify: lowercase-only shortcuts.)
    let mut hud_state: HudState = HudState::new();
    hud_state.set_screen_size(w, h, cfg.screen_size.is_some());
    // v30: seed HUD with user-configured target_fps so `tgt:` shows the
    // right value from frame 1 (otherwise shows default 60 until live-reload).
    hud_state.set_target_fps(cfg.target_fps);

    // Perceived-motion diagnostics: track visible-change frames vs idle frames.
    let mut perf_idle_frames: u64 = 0; // frames where dirty_count == 0
    let mut perf_dirty_sum: u64 = 0; // total dirty cells across all frames
    let mut perf_dirty_samples: u64 = 0; // number of frames sampled for dirty avg

    // Resize debounce: coalesce rapid resize storms into a single apply.
    let mut last_resize_event: Option<Instant> = None;

    // Adaptive throttling: PowerManager owns the idle timer + phase
    // predictor + idle_started tracker. last_resync_time stays here
    // because resync scheduling is a Cloud concern, not a power concern.
    let mut last_resync_time = Instant::now();

    // P4: Memory reclaim state — rate-limits madvise hints during idle.
    let mut reclaim_state = ReclaimState::new();

    // P5: Endurance health score tracker.
    let mut endurance_health = EnduranceHealth::new();
    // Only Linux samples context-switch rate via /proc; on macOS this stays 0
    // and the assignment inside the cfg block is skipped.
    #[cfg(target_os = "linux")]
    let mut last_ctxt_switches: u64 = 0;
    let mut last_ctxt_sample = Instant::now();
    let mut perf_rss_samples: u64 = 0;

    // v35.1: last key press — drives idle-based auto-snapback (see AMBIENT_SCHEDULER_AUDIT.md §2.2).
    let mut last_user_input_at = Instant::now();

    // Self-healer (P1+P2): auto downgrade on high perf_pressure; EnduranceHealth mitigations.
    let mut self_healer = PerformanceSelfHealer::new();

    let mut charset_preset = cfg.charset_preset.clone();
    let mut scene_name = cfg.scene_name.clone();
    // Phase D: bumped on scene_name reassignment — u64 compare replaces
    // per-frame String clone (~60 allocs/sec saved).
    let mut scene_generation: u64 = 0;
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
    let base_cfg = cfg.clone();
    // Pending rebuild: set when watcher sends new config, applied at top of next frame.
    let mut pending_config: Option<std::collections::HashMap<String, String>> = None;

    // Ambient scheduler: idle/wake thread sends AmbientEntry via mpsc.
    let ambient_handle =
        crate::ambient_scheduler::spawn_ambient_scheduler(base_cfg.ambient_schedule.clone());
    let mut last_ambient_schedule = base_cfg.ambient_schedule.clone();
    // v30.3: last-applied ambient entry — re-applied after live-reload rebuilds.
    let mut last_applied_ambient_entry: Option<crate::ambient::AmbientEntry> = None;
    // AB-07: permanent snapback kill — once schedule is detected empty
    // (by any path), auto-snapback is disabled until a new rx event is
    // applied from a non-empty schedule.
    let mut ambient_snapback_killed: bool = false;
    // AB-08: config file path for ground-truth re-read. The watcher can
    // lose events, leaving all cached state stale. File on disk is truth.
    let config_path_for_ground_truth = base_cfg.config_path_for_watcher.clone();
    // v25.5+v30.4: last-applied cfg map for diff trace + startup ambient.
    let initial_cfg_map = base_cfg
        .config_path_for_watcher
        .as_deref()
        .map(|p| crate::configfile::load_config_file(Some(p)))
        .unwrap_or_default();
    let mut last_applied_cfg_map: Option<std::collections::HashMap<String, String>> =
        Some(initial_cfg_map.clone());

    // v30.3+hotfix: synchronous ambient apply at startup with REAL cfg map.
    let (new_charset, startup_entry) = crate::ambient::apply_startup_ambient(
        &mut cloud,
        &base_cfg.ambient_schedule,
        &charset_preset,
        &user_ranges,
        def_ascii,
        &initial_cfg_map,
    );
    // v30.5: startup ambient info for post-exit verbose (main.rs prints after drop).
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
        // v35: ambient asserted at startup — lock palette, clear override.
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
                        // v25.13: config validation errors cause immediate exit.
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
            let new_cfg = crate::live_config::rebuild_cloud_config(&base_cfg, &new_cfg_map);
            let density = effective_density(new_cfg.base_density, w, new_cfg.density_auto);
            // v25: bulletproof trace that rebuild reached render thread.
            crate::live_config_trace::trace_rebuild_applied(
                &new_cfg.color_scheme,
                new_cfg.charset_preset.as_str(),
                new_cfg.speed,
                new_cfg.density,
                new_cfg.target_fps,
            );

            // v25.5: field-level config diff trace.
            let mut changed: Vec<String> = Vec::new();
            let mut added: Vec<String> = Vec::new();
            let mut removed: Vec<String> = Vec::new();
            match &last_applied_cfg_map {
                None => {
                    let mut keys: Vec<&String> = new_cfg_map.keys().collect();
                    keys.sort();
                    for k in keys {
                        crate::lr_trace!("config diff [initial]: {} = {}", k, new_cfg_map[k]);
                    }
                }
                Some(old_map) => {
                    let all_keys: std::collections::BTreeSet<&String> =
                        old_map.keys().chain(new_cfg_map.keys()).collect();
                    for k in &all_keys {
                        match (old_map.get(*k), new_cfg_map.get(*k)) {
                            (Some(o), Some(n)) => {
                                if o != n {
                                    changed.push(format!("{}: {} → {}", k, o, n));
                                }
                            }
                            (None, Some(n)) => added.push(format!("{}: {}", k, n)),
                            (Some(o), None) => removed.push(format!("{}: {}", k, o)),
                            (None, None) => unreachable!(),
                        }
                    }
                    if !changed.is_empty() {
                        crate::lr_trace!(
                            "config diff [changed {}]: {}",
                            changed.len(),
                            changed.join(", ")
                        );
                    }
                    if !added.is_empty() {
                        crate::lr_trace!(
                            "config diff [added {}]: {}",
                            added.len(),
                            added.join(", ")
                        );
                    }
                    if !removed.is_empty() {
                        crate::lr_trace!(
                            "config diff [removed {}]: {}",
                            removed.len(),
                            removed.join(", ")
                        );
                    }
                    if changed.is_empty() && added.is_empty() && removed.is_empty() {
                        crate::lr_trace!(
                            "config diff: no field-level changes (whitespace/comment edit)"
                        );
                    }
                }
            }
            last_applied_cfg_map = Some(new_cfg_map.clone());
            // Phase D #9: preserve ecosystem + post-FX across reload.
            // AB-02: capture override state for schedule-empty restore.
            let preserve_user_override = cloud.user_override_since_ambient;
            let preserved_color_scheme = cloud.color_scheme;
            let preserved_scene_name = scene_name.clone();
            let mut new_cloud = new_cfg.create_cloud(density);
            new_cloud.inherit_ecosystem_state(&cloud);
            cloud = new_cloud;
            cloud.reset(w, h);
            cloud.enable_events();
            cloud.set_component_timing(new_cfg.perf_stats);
            // Fresh Cloud from rebuild — reset self-healer.
            self_healer.reset();
            // Rebuild color cache + frame + fill bg + charset.
            term.set_color_cache(ColorCache::new(&cloud.palette));
            frame = Frame::new(w, h, cloud.palette.bg);
            super::fill_terminal_bg(cloud.palette.bg);
            charset_preset = new_cfg.charset_preset.clone();
            // v25.5+v30.8+v35.2: recompute target FPS from new config.
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
            // v30.3: re-apply last ambient entry to fresh Cloud.
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
                    cloud.color_scheme = preserved_color_scheme;
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
                cloud.user_override_since_ambient = true;
                cloud.ambient_palette_locked = false;
            }
        }
        // AB-03+AB-04: poll ambient phase events. Empty schedule → drain all.
        // Non-empty → discard events no longer in schedule (membership check).
        let mut last_ambient_entry: Option<crate::ambient::AmbientEntry> = None;
        if !last_ambient_schedule.entries.is_empty() {
            while let Ok(entry) = ambient_handle.rx.try_recv() {
                if !last_ambient_schedule.entries.iter().any(|e| e == &entry) {
                    continue;
                }
                last_ambient_entry = Some(entry);
            }
        } else {
            // Drain stale events when schedule is empty.
            while ambient_handle.rx.try_recv().is_ok() {}
        }
        // AB-08: ground-truth guard on rx event — if config file on disk says
        // 0 ambient entries but we got an rx event, the event is stale (watcher
        // missed the config change). Discard + nuke all ambient state.
        if last_ambient_entry.is_some() {
            if let Some(ref path) = config_path_for_ground_truth {
                if let Ok(c) = std::fs::read_to_string(path) {
                    let pv = &crate::configfile::parse_config_text(&c).values;
                    if crate::ambient::collect_ambient_schedule(pv)
                        .entries
                        .is_empty()
                    {
                        last_ambient_entry = None;
                        last_ambient_schedule.entries.clear();
                        last_applied_ambient_entry = None;
                        cloud.ambient_palette_locked = false;
                        cloud.user_override_since_ambient = true;
                        ambient_snapback_killed = true;
                        ambient_handle.reload(crate::ambient::AmbientSchedule::default());
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
        // AB-08: snapback ground-truth guard — re-read config file from disk.
        // Cached state can go stale if watcher loses an event. File on disk
        // is the only authoritative source. I/O cost ~50µs, only when snapback
        // might fire (≤ once per 30s), so negligible.
        let _ab06_sked_len = last_ambient_schedule.entries.len() as u64;
        let _ab06_last_applied = last_applied_ambient_entry.is_some();
        super::ambient_diag_snapback_guard(_ab06_sked_len, _ab06_last_applied);
        let ground_truth_ambient_empty =
            if !ambient_snapback_killed && _ab06_sked_len > 0 && _ab06_last_applied {
                let mut empty = false;
                if let Some(ref path) = config_path_for_ground_truth {
                    if let Ok(c) = std::fs::read_to_string(path) {
                        let pv = &crate::configfile::parse_config_text(&c).values;
                        if crate::ambient::collect_ambient_schedule(pv)
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
            ambient_handle.reload(crate::ambient::AmbientSchedule::default());
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
                crate::constants::AUTO_SNAPBACK_DELAY_SECS,
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
        // v30.8 (Phase 3): PowerManager.begin_frame — is_idle, predictor, idle_started.
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
                // SAFETY: frame.cells is a valid Vec allocation; we only hint.
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
        #[cfg(unix)]
        if term_reinit.swap(false, Ordering::AcqRel) {
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
                        let is_android = std::env::var("TERMUX_VERSION").is_ok()
                            || std::env::var("PREFIX").is_ok_and(|p| p.contains("com.termux"));
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
                        // Bug fix: reject Ctrl+I — only bare 'i' toggles HUD.
                        if k.modifiers.is_empty()
                            && matches!((k.code, k.modifiers), (KeyCode::Char('i'), _))
                        {
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
                        // 'h': toggle HUD position. v30: lowercase-only (uppercase
                        // 'H' removed; lowercase works on all keyboards including Android).
                        // Bug fix: reject Ctrl+H — only bare 'h' moves HUD.
                        if k.modifiers.is_empty()
                            && matches!((k.code, k.modifiers), (KeyCode::Char('h'), _))
                        {
                            if hud_state.toggle_position() {
                                cloud.force_draw_everything();
                            }
                            let _ = register_activity(
                                &mut power_manager,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            // v16 audit: Update next_frame for immediate redraw
                            // (same fix as 'i' handler — see comment above).
                            next_frame = activity_time;
                            continue;
                        }
                        // Any user input resets idle timer for adaptive throttling.
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
                        // v35.1: refresh auto-snapback idle timer on every key press.
                        last_user_input_at = activity_time;
                        // Process the keybinding. This lets interactive
                        // keys (q, c/C, s/S, p, x/X, [, ], Space, Up/Down,
                        // i/I, h/H) work even in --screensaver mode.
                        let redraw_needed = handle_keybinding(
                            &mut cloud,
                            &mut frame,
                            &k,
                            &mut charset_preset,
                            &mut scene_name,
                            &mut scene_generation,
                            &user_ranges,
                            def_ascii,
                            cfg,
                            #[cfg(unix)]
                            &term_reinit,
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
                        // on MOVE (old: bright-color flash). v30.10: CLICK wakes renderer
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
                        cloud.set_mouse_position(m.column, m.row);
                        if is_click {
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
            cloud.reset(nw, nh);
            frame = Frame::new(nw, nh, cloud.palette.bg);
            if cfg.density_auto {
                cloud.set_droplet_density(effective_density(cfg.base_density, nw, true));
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
        // v30.8 (Phase 3): PowerManager.effective_fps() replaces the
        // target_period / idle_period / pause_period Duration cascade.
        let frame_period = Duration::from_secs_f64(1.0 / power_manager.effective_fps(cloud.pause));
        let frame_period_s = frame_period.as_secs_f32().max(0.000_001);
        // v30 (2026-08-05): announce frame pacing mode to the HUD so the
        // `tgt:` line can show an `idle` / `paused` suffix. Cheap (one enum
        // set + one method call). Placed AFTER the pause/idle/active branch
        // so the mode reflects the actual cadence used for this frame, not
        // the previous frame's.
        let frame_mode = if cloud.pause {
            FrameMode::Paused
        } else if power_manager.is_idle() {
            FrameMode::Idle
        } else {
            FrameMode::Active
        };
        hud_state.set_frame_mode(frame_mode);
        cloud.set_perf_pressure(power_manager.effective_pressure());
        let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
        // v25.15 (perf audit): clamp lower bound is now `SIM_FACTOR_MIN`
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
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);

        let work_s = work_start.elapsed().as_secs_f32();

        // v30 (VSCode crash fix): feed write latency into perf_pressure.
        // VSCode's xterm.js falls behind over long runs; a write taking
        // >50% of frame period signals the consumer cannot keep up.
        //
        // v30.6 (bug fix): also feed a synthetic overshoot when the last
        // flush was suppressed by Tier 2.1 byte-budget backpressure.
        // Otherwise the suppression masks itself: no write_with_recovery
        // call → last_write_ns stale → perf_pressure doesn't accumulate
        // → self-healer never fires even though xterm.js is backing up.
        let write_overshoot = if frame_period_s > 0.0 {
            let raw = ((term.last_write_ns() as f32 / 1e9) / frame_period_s - 0.5).clamp(0.0, 2.0);
            // Suppressed flush: synthetic 1.0 signal (layered via .max).
            if term.last_flush_suppressed() {
                raw.max(1.0)
            } else {
                raw
            }
        } else {
            0.0
        };

        // Live HUD: push frame time, sample RSS + CPU%, recompute metrics.
        // All methods short-circuit when HUD is off (zero cost).
        hud_state.push_frame_time(work_s as f64 * 1000.0);
        hud_state.maybe_sample_rss();
        hud_state.maybe_sample_cpu();
        hud_state.update_metrics(cloud.hud_colors());

        let overshoot = ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0);
        let utilization = work_s / frame_period_s;
        // v30.8 (Phase 3): PowerManager.observe_frame_end() replaces the
        // inline perf_pressure increment/decay. Same math, same constants.
        // overshoot is kept as a local for the perf_stats overshoot-frame
        // counter below.
        power_manager.observe_frame_end(work_s, frame_period_s, write_overshoot);

        // ── P5: Endurance health sampling (ALWAYS ON) ──
        //
        // v30.6 (bug fix): previously this entire block was gated by
        // `cfg.perf_stats`. When the user ran without --perf-stats, no
        // samples were ever pushed to EnduranceHealth, so its score stayed
        // at the initial 100.0 forever. That made the P2 self-healer
        // (TriggerHealthMitigation) silently disable — a major footgun:
        // a safety mitigation layer that vanishes the moment you turn off
        // the display. The `--perf-stats` flag must control ONLY display,
        // never mitigation. So we now always sample RSS + ctxt + frame
        // time and always recompute. Cost: 2 syscalls/sec (read
        // /proc/self/status + /proc/self/stat) + 1 isatty/min. Negligible.
        endurance_health.push_frame_time(work_s as f64 * 1000.0);
        if perf_rss_samples % 60 == 0 {
            #[cfg(target_os = "linux")]
            {
                let rss = super::intro::read_self_rss_kb();
                endurance_health.push_rss(rss as f64);
            }
            // P2: reuse work_start (captured just before cloud.rain_at)
            // instead of another Instant::now(). Timing diff <1ms.
            let elapsed = work_start
                .saturating_duration_since(last_ctxt_sample)
                .as_secs_f64();
            if elapsed > 0.0 {
                #[cfg(target_os = "linux")]
                {
                    let cur = super::intro::read_self_voluntary_ctxt();
                    if last_ctxt_switches > 0 {
                        let rate = (cur.saturating_sub(last_ctxt_switches)) as f64 / elapsed;
                        endurance_health.push_ctxt_rate(rate);
                    }
                    last_ctxt_switches = cur;
                }
                last_ctxt_sample = work_start;
            }
            endurance_health.recompute();
        }
        perf_rss_samples = perf_rss_samples.saturating_add(1);

        // P5: periodic stdout fd health probe (ALWAYS ON — not display state).
        // Runs on the same slow tick (FD_HEALTH_PROBE_INTERVAL_FRAMES ≈
        // 60s at 60 FPS). Detects fd corruption BEFORE a write fails.
        // Cost: one isatty syscall per minute.
        if perf_rss_samples % FD_HEALTH_PROBE_INTERVAL_FRAMES == 0 && !term.probe_stdout_health() {
            // Recovery attempted — GRACEFUL_SHUTDOWN is set.
            cloud.raining = false;
            break;
        }

        // Feature #13: thermal sensor sampling (Linux only).
        // Reads /sys/class/thermal/thermal_zone*/temp, normalizes the
        // hottest zone to 0.0–1.0, and feeds it into PowerManager.
        // Every downstream consumer of effective_pressure() (spawn
        // cascade, self-healer, sim factor) automatically responds.
        // On non-Linux or in containers without thermal sysfs, the
        // sampler returns None and the previous thermal_pressure value
        // is preserved (NOT reset to 0.0).
        if perf_rss_samples % THERMAL_SAMPLER_INTERVAL_FRAMES == 0 {
            if let Some(p) = sample_thermal_pressure() {
                power_manager.set_thermal_pressure(p);
            }
        }

        // Display-only stats. IN-01: `perf_frames` + `frame_time_tracker.push`
        // moved OUTSIDE the `cfg.perf_stats` gate so the always-on post-exit
        // FPS summary reports honest numbers without --perf-stats.
        perf_frames = perf_frames.saturating_add(1);
        frame_time_tracker.push(work_s as f64 * 1000.0);
        if cfg.perf_stats {
            if did_draw {
                perf_drawn_frames = perf_drawn_frames.saturating_add(1);
            } else {
                perf_idle_frames = perf_idle_frames.saturating_add(1);
            }
            perf_dirty_sum = perf_dirty_sum.saturating_add(dirty_len as u64);
            perf_dirty_samples = perf_dirty_samples.saturating_add(1);
            perf_work_sum_s += work_s as f64;
            perf_work_max_s = perf_work_max_s.max(work_s as f64);
            perf_pressure_sum += power_manager.effective_pressure() as f64;
            perf_pressure_max = perf_pressure_max.max(power_manager.effective_pressure());
            perf_utilization_sum += utilization as f64;
            perf_utilization_max = perf_utilization_max.max(utilization);
            if overshoot > 0.0 {
                perf_overshoot_frames = perf_overshoot_frames.saturating_add(1);
            }
        }

        // Performance self-healer (P1 + P2).
        //
        // Called every frame after perf_pressure is finalized and
        // endurance_health has been recomputed (always-on since v30.6). The
        // self-healer is a pure policy — it returns an action enum and
        // we apply the side effects here.
        //
        // v30.6: always pass Some(score). Before v30.6, when perf_stats
        // was off, None was passed and P2 silently disabled. Now sampling
        // is always-on (see P5 block above), so the score is always real.
        //
        // `now` uses `loop_now` (captured at top of frame) for consistency
        // with the rest of the timing-sensitive logic in this loop.

        // If the scene changed since frame start (user 'x' key, live config
        // reload, or ambient), reset the self-healer. Must happen
        // BEFORE observe() so the self-healer doesn't fire a downgrade/
        // restore on the same frame the user switched scenes. Phase D:
        // u64 counter compare replaces a String-clone + String-ne.
        if scene_generation != scene_generation_at_frame_start {
            self_healer.reset();
        }

        let heal_action = self_healer.observe(
            power_manager.effective_pressure(),
            loop_now,
            Some(endurance_health.score()),
        );
        match heal_action {
            SelfHealAction::None => {}
            SelfHealAction::TriggerHealthMitigation => {
                // P2: force a full redraw to clear any potential stuck state,
                // and bypass ReclaimState's cooldown to issue an immediate
                // madvise hint. The ReclaimState is also marked so its
                // 1-hour interval resets from this point.
                cloud.force_draw_everything();
                #[cfg(target_os = "linux")]
                {
                    // Reuse the frame buffer pointer/len computation from
                    // the P4 reclaim path. We call hint_reclaim_pages
                    // directly here because the self-healer's bypass is
                    // intentional — the cooldown is enforced inside the
                    // self-healer itself (last_health_mitigation field).
                    let cells_ptr = frame.cells.as_ptr();
                    let cells_len = frame.cells.len() * std::mem::size_of_val(&frame.cells[0]);
                    // SAFETY: frame.cells is a valid Vec allocation; we only
                    // pass the pointer and length to madvise which reads
                    // metadata only, does not dereference the data.
                    unsafe {
                        super::adaptive::hint_reclaim_pages(cells_ptr as *const u8, cells_len);
                    }
                    reclaim_state.mark_reclaimed(loop_now);
                }
                #[cfg(not(target_os = "linux"))]
                {
                    // Non-Linux: madvise is a no-op, but we still mark the
                    // reclaim state so the regular P4 path doesn't immediately
                    // fire on the next idle check (consistency).
                    reclaim_state.mark_reclaimed(loop_now);
                }
            }
            SelfHealAction::DowngradeScene => {
                // AB-11 (dragon power audit, option 2): do NOT switch scenes.
                // The old code called cloud.apply_scene_runtime("low-power")
                // which silently overrode the user's color, charset, density,
                // speed, and glitch_level — violating the owner's principle
                // that dragon power must not change visual identity.
                //
                // Instead: set the aggressive_throttle flag. This makes
                // rain_at() use a steeper spawn-scale curve (0.9 vs 0.75)
                // + lower floor (0.10 vs 0.25) + disables glitches entirely.
                // The user's color/charset/density/speed/glitch_level are
                // NEVER touched. When pressure recovers, the flag is cleared
                // and spawn-scale returns to normal on the next frame.
                if !self_healer.is_downgraded() {
                    self_healer.record_downgrade(&scene_name);
                    cloud.set_aggressive_throttle(true);
                    crate::live_config::push_runtime_warning(&format!(
                        "[self-heal] sustained high CPU pressure — throttling spawn rate (visual identity preserved: scene='{}')",
                        scene_name
                    ));
                }
            }
            SelfHealAction::RestoreScene => {
                // AB-11: clear the aggressive_throttle flag. No scene restore
                // needed — the user's scene was never changed.
                if self_healer.is_downgraded() {
                    self_healer.take_pre_degraded_scene(); // drain the saved name
                    cloud.set_aggressive_throttle(false);
                    crate::live_config::push_runtime_warning(
                        "[self-heal] CPU pressure recovered — spawn throttle released",
                    );
                }
            }
        }

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

    // Signal the watchdog thread to stop so it doesn't outlive the main
    // loop and falsely detect a "stuck" state after normal exit.
    SHUTDOWN.store(true, Ordering::Release);

    // v30 fix: compute the final FPS summary line now, but defer the
    // eprintln to AFTER `drop(term)` below — otherwise the summary leaks
    // into the alternate-screen rain matrix (AB-10 rain-screen cleanliness).
    let final_elapsed = start_time.elapsed();
    let final_elapsed_s = final_elapsed.as_secs_f64().max(0.000_001);
    let final_avg_fps = (perf_frames as f64) / final_elapsed_s;
    let last_work_ms = frame_time_tracker.rolling_avg_ms();
    let final_instant_fps = if last_work_ms > 0.0 {
        (1000.0 / last_work_ms).min(cfg.target_fps)
    } else {
        cfg.target_fps
    };
    let final_fps_line = format!(
        "[cosmostrix] final FPS: {:.1} (instant: {:.1}, target: {:.1}), frames: {}, elapsed: {:.2}s",
        final_avg_fps, final_instant_fps, cfg.target_fps, perf_frames, final_elapsed_s
    );

    // Capture terminal stats BEFORE drop. Cheap (two field reads); unifies
    // the drop path so we always leave the alt screen before stderr writes.
    let (enc_bytes, enc_flushes, sgr_hits, sgr_misses) = term.encoding_stats();
    let (tier2_skips, tier2_resets, tier2_bytes_since) = term.tier2_stats();

    if cfg.perf_stats {
        let elapsed = final_elapsed;
        let elapsed_s = elapsed.as_secs_f64().max(0.000_001);

        let frames = perf_frames.max(1);
        let avg_work_ms = (perf_work_sum_s / frames as f64) * 1000.0;
        let avg_pressure = perf_pressure_sum / frames as f64;
        let avg_fps = (perf_frames as f64) / elapsed_s;
        let drawn_ratio = (perf_drawn_frames as f64) / (perf_frames as f64).max(1.0);
        let overshoot_ratio =
            (perf_overshoot_frames as f64) / (perf_frames as f64).max(1.0) * 100.0;
        let pressure_class = if avg_pressure < PERF_PRESSURE_CLASS_LOW {
            "low"
        } else if avg_pressure < PERF_PRESSURE_CLASS_MEDIUM {
            "medium"
        } else {
            "high"
        };

        let mut r = Report::new("COSMOSTRIX PERFORMANCE REPORT");

        {
            let s = r.section("TIMING");
            s.field("elapsed", &format!("{:.3}s", elapsed_s));
            s.field("target_fps", &format!("{:.3}", cfg.target_fps));
            s.field("avg_fps", &format!("{:.3}", avg_fps));
            // v30: real instantaneous FPS from last ~1s of frame work times.
            // Capped at target_fps (loop sleeps to maintain target). Read
            // this for "what FPS am I seeing now" — distinct from avg_fps
            // (whole-run average) and BACKPRESSURE.avg (load-shed signal).
            s.field("instant_fps", &format!("{:.3}", final_instant_fps));
            s.field(
                "rolling_avg_frame_time",
                &format!("{:.3}ms", frame_time_tracker.rolling_avg_ms()),
            );
        }

        {
            let s = r.section("FRAMES");
            s.field("total", &perf_frames.to_string());
            s.field(
                "drawn",
                &format!("{} ({:.1}%)", perf_drawn_frames, drawn_ratio * 100.0),
            );
            s.field(
                "idle_visual",
                &format!(
                    "{} ({:.1}%)",
                    perf_idle_frames,
                    (perf_idle_frames as f64) / (perf_frames as f64).max(1.0) * 100.0
                ),
            );
            s.field(
                "overshoot",
                &format!("{} ({:.1}%)", perf_overshoot_frames, overshoot_ratio),
            );
        }

        {
            let s = r.section("MOTION");
            let avg_dirty = if perf_dirty_samples > 0 {
                perf_dirty_sum as f64 / perf_dirty_samples as f64
            } else {
                0.0
            };
            s.field("avg_dirty_cells", &format!("{:.1}", avg_dirty));
            s.field(
                "visual_fps_hint",
                &format!(
                    "{:.1} ({} of {} frames had visual changes)",
                    drawn_ratio * cfg.target_fps,
                    perf_drawn_frames,
                    perf_frames
                ),
            );
        }

        {
            let s = r.section("LATENCY");
            s.field("avg_frame_time", &format!("{:.3}ms", avg_work_ms));
            s.field(
                "max_frame_time",
                &format!("{:.3}ms", perf_work_max_s * 1000.0),
            );
            s.field("jitter", frame_time_tracker.jitter_classification());
        }

        {
            // Backpressure = clamp(work/budget - 1, 0, 2): non-zero ONLY when
            // renderer can't keep up. budget_utilization = work/budget (always
            // non-zero) — companion so the section is informative on healthy hw.
            crate::bench_helpers::format_backpressure_section(
                &mut r,
                avg_pressure,
                perf_pressure_max,
                perf_utilization_sum,
                perf_utilization_max,
                perf_frames,
                Duration::from_secs_f64(1.0 / power_manager.base_target_fps()),
                avg_work_ms,
                pressure_class,
                perf_overshoot_frames,
                overshoot_ratio,
            );
        }

        // P5: Endurance health score
        {
            let s = r.section("ENDURANCE");
            s.field(
                "health_score",
                &format!("{:.1}/100", endurance_health.score()),
            );
            s.field("classification", endurance_health.classification());
            s.field(
                "phase_transitions",
                &power_manager.phase_transitions_observed().to_string(),
            );
        }

        // ENCODING: actual measured ANSI bytes/frame + SGR cache hit rate.
        // These prove the diff-based + RLE + color cache optimizations work.
        {
            let s = r.section("ENCODING");
            let total_sgr = sgr_hits + sgr_misses;
            let hit_rate = if total_sgr > 0 {
                (sgr_hits as f64 / total_sgr as f64) * 100.0
            } else {
                0.0
            };
            let avg_bytes_per_frame = if enc_flushes > 0 {
                enc_bytes as f64 / enc_flushes as f64
            } else {
                0.0
            };
            let bandwidth_kib_s = (enc_bytes as f64 / 1024.0) / elapsed_s;

            s.field("total_ansi_bytes", &enc_bytes.to_string());
            s.field("frames_flushed", &enc_flushes.to_string());
            s.field(
                "avg_bytes_per_frame",
                &format!("{:.1}", avg_bytes_per_frame),
            );
            s.field("bandwidth", &format!("{:.1} KiB/s", bandwidth_kib_s));
            s.field("sgr_cache_hits", &sgr_hits.to_string());
            s.field("sgr_cache_misses", &sgr_misses.to_string());
            s.field("sgr_cache_hit_rate", &format!("{:.1}%", hit_rate));
        }

        // Tier 2: xterm.js host defenses (byte-budget backpressure + RIS reset).
        // All three fields are 0 on native terminals; nonzero only inside
        // VSCode/Hyper/WaveTerminal/Tabby/WarpTerminal. Useful for diagnosing
        // whether the multi-hour OOM crash mode is actually being mitigated.
        {
            let s = r.section("TIER2_XTERMJS");
            s.field("backpressure_skips", &tier2_skips.to_string());
            s.field("ris_resets", &tier2_resets.to_string());
            s.field("bytes_since_last_ris", &tier2_bytes_since.to_string());
        }

        r.print();
    }

    // AB-10: drop the terminal BEFORE any stderr write so the alt screen
    // is restored and the final FPS line lands on the main screen, not
    // polluting the rain matrix on exit.
    drop(term);

    // Store final runtime state for post-exit verbose summary.
    let final_color_name = format!("{:?}", cloud.color_scheme());
    super::set_final_state(
        &final_color_name,
        &scene_name,
        &charset_preset,
        cloud.chars_per_sec,
        cloud.droplet_density,
    );

    // AB-10: only print final FPS when --perf-stats is requested.
    // Previously this always printed (v30 design), but owner considers
    // it a verbose leak — without -v or --perf-stats, the user sees
    // unexpected output after exit. Now gated by cfg.perf_stats.
    if cfg.perf_stats {
        eprintln!("{}", final_fps_line);
    }

    Ok(())
}
