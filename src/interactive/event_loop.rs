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
use super::activity::{is_runtime_idle, register_activity, spin_wait, FrameTimeTracker};
use super::adaptive::{
    adaptive_resync_interval, local_secs_since_midnight, EnduranceHealth, PerformanceSelfHealer,
    PhasePredictor, ReclaimState, SelfHealAction,
};
use super::hud::{FrameMode, HudState};
use super::input::{handle_keybinding, PasteBurstGuard};
use super::watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE, SHUTDOWN};

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

    let mut term = Terminal::with_signal_exit(signal_exit.clone())?;
    // v17: Mouse reporting ALWAYS on (blocks text selection in alt screen).
    // The --mouse flag was REMOVED — mouse + hover/click effects are always on.
    // Terminal safety on abrupt death: Terminal::drop, panic hook, signal
    // handlers, watchdog, fork-based SIGKILL guard (Linux).
    if term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    // --screen-size: use fixed virtual size if specified, else dynamic terminal size.
    let mut w: u16;
    let mut h: u16;
    let (w_init, h_init) = if let Some(fixed) = cfg.screen_size {
        let (tw, th) = term.size().unwrap_or((fixed.0, fixed.1));
        if fixed.0 > tw || fixed.1 > th {
            eprintln!(
                "warning: --screen-size {}x{} exceeds terminal {}x{}; will clip to top-left",
                fixed.0, fixed.1, tw, th
            );
        }
        fixed
    } else {
        term.size()?
    };
    w = w_init;
    h = h_init;

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

    // v16: Fill the entire alternate screen with the palette's background
    // color before the first frame. Without this, edges/margins keep the
    // terminal's native bg, creating visible gaps.
    super::fill_terminal_bg(cloud.palette.bg);

    // v20: Modular cinematic intro (--intro <type> flag).
    // v31: Removed the `!cfg.screensaver` guard — the intro now plays in
    // screensaver mode too. The owner reversed the v17 "auto-skip in
    // screensaver" decision: the intro is cosmostrix's signature and should
    // not be suppressed by input mode. Skip policy (only `q` skips) is unchanged.
    if cfg.intro != crate::config::IntroType::None {
        super::intro::run_intro(&mut term, &mut frame, &cloud, w, h, cfg.intro)?;
        cloud.force_draw_everything();
        frame.clear_with_bg(cloud.palette.bg);

        // v25.11 (bug #10): re-read terminal size after intro returns.
        // The intro can take several seconds (cosmic particle animation,
        // logo reveal). If the user resized the terminal during the intro,
        // the (w, h) captured at line 60 is now stale. Without this check,
        // the rain loop starts with the old dimensions — rain renders at
        // the wrong size until the first SIGWINCH event is polled and
        // processed (which may take 1+ frames, causing a visible glitch
        // where rain fills only a portion of the resized terminal).
        //
        // Fixed mode (--screen-size) ignores terminal resize — the virtual
        // size is what the user explicitly requested.
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

    let mut target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
    let pause_period = Duration::from_millis(PAUSE_PERIOD_MS);
    let mut next_frame = Instant::now();
    let mut perf_pressure: f32 = 0.0;

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
    // v30 (2026-08-05): seed the HUD with the user-configured target_fps
    // so the `tgt:` line shows the right value from the very first frame.
    // Without this, the HUD would show `tgt: 60` (the default) until the
    // first live-config reload, even when the user passed `--fps 30`.
    hud_state.set_target_fps(cfg.target_fps);

    // Perceived-motion diagnostics: track visible-change frames vs idle frames.
    let mut perf_idle_frames: u64 = 0; // frames where dirty_count == 0
    let mut perf_dirty_sum: u64 = 0; // total dirty cells across all frames
    let mut perf_dirty_samples: u64 = 0; // number of frames sampled for dirty avg

    // Resize debounce: coalesce rapid resize storms into a single apply.
    let mut last_resize_event: Option<Instant> = None;

    // Adaptive throttling: reduce effective FPS after IDLE_THRESHOLD_SECS idle.
    let mut last_input_time = Instant::now();
    let mut last_resync_time = last_input_time;
    let mut idle_period = Duration::from_secs_f64(1.0 / (cfg.target_fps * IDLE_FPS_FACTOR));

    // P1: Phase predictor — learns daily activity cycle for proactive idle.
    let mut phase_predictor = PhasePredictor::new();
    let mut was_active = true; // Start assuming active; first idle transition records.

    // P2: Track sustained idle duration for adaptive resync interval.
    let mut idle_started: Option<Instant> = None;

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

    // Performance self-healer (P1 + P2): drives auto scene downgrade when
    // perf_pressure is sustained high, and EnduranceHealth-triggered
    // mitigations when the composite score drops into the investigate band.
    // See docs/research/SELF_HEALING_AUDIT.md for the design rationale.
    let mut self_healer = PerformanceSelfHealer::new();

    let mut charset_preset = cfg.charset_preset.clone();
    let mut scene_name = cfg.scene_name.clone();
    // Phase D (hot-path): bumped on every reassignment of `scene_name` so the
    // event loop can detect "scene changed during this frame" with a u64
    // compare instead of cloning the String per frame (~60 allocs/sec saved).
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

    // Ambient phase scheduler: spawn the dynamic idle/wake thread. The thread
    // sleeps until the next phase boundary (zero CPU between boundaries),
    // then sends an `AmbientEntry` via mpsc polled each frame. On live-reload,
    // we push the new schedule via `reload`.
    let ambient_handle =
        crate::ambient_scheduler::spawn_ambient_scheduler(base_cfg.ambient_schedule.clone());
    let mut last_ambient_schedule = base_cfg.ambient_schedule.clone();
    // v30.3: track the last-applied ambient entry to RE-APPLY it after
    // live-reload rebuilds. Without this, a duplicate notify event triggers
    // a rebuild that loses ambient state. Ambient wins over CLI override.
    let mut last_applied_ambient_entry: Option<crate::ambient::AmbientEntry> = None;
    // v25.5: last-applied config map for diff trace. Phase 4 P4-7 (positive
    // finding): intentional clone — verbose diff needs full map, ~1KB/reload.
    let mut last_applied_cfg_map: Option<std::collections::HashMap<String, String>> = None;

    // Track runtime state changes for post-exit verbose summary.
    // No eprintln during rain — would flicker in alternate-screen mode.
    let _verbose = cfg.verbose;

    while cloud.raining {
        // Check for graceful shutdown request from signal handler.
        // This allows clean exit via Terminal::drop() instead of racing
        // on stdout with the signal handler thread.
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
                        // v25.13 (bug #15): config validation errors during
                        // live reload now cause IMMEDIATE exit. The previous
                        // v25.6 design ("don't kill the process, keep rain
                        // running on last valid config") was reversed because
                        // it caused verbose rejection text to leak into the
                        // rain matrix — the watcher thread's stderr writes
                        // appeared as "weird text" in the alternate-screen
                        // buffer, polluting the cinematic render. Now: set
                        // the exit code, store the error, break the rain
                        // loop. main.rs prints the error AFTER terminal
                        // restoration (post-exit), so it never touches the
                        // alternate screen.
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

            // Phase D Bug #9 fix: preserve color_ecosystem + atmospheric
            // post-FX state across live-reload so the user doesn't see a
            // brightness / saturation / hue discontinuity when editing
            // config. (v30 2026-08-05: renamed from "atmosphere state" to
            // "atmospheric post-FX state" to disambiguate from the
            // eliminated atmosphere engine subsystem — this refers to the
            // live `EntropyDrift` cloud drift + Chroma Dragon
            // post-FX shader, NOT the deleted atmosphere engine.)
            // Previously this created a fresh Cloud with defaults (0.85,
            // 0.85, 0.0) — if the previous cloud had drifted to 0.78
            // (dim), the new cloud jumped back to 0.85 (brighter).
            let mut new_cloud = new_cfg.create_cloud(density);
            new_cloud.inherit_ecosystem_state(&cloud);
            cloud = new_cloud;
            cloud.reset(w, h);
            cloud.enable_events();
            cloud.set_component_timing(new_cfg.perf_stats);
            // Live config rebuild creates a fresh Cloud — any in-flight
            // self-healer downgrade is now moot (the new Cloud's scene is
            // from the config, not from a prior auto-downgrade). Reset so
            // the healer starts fresh with the new baseline.
            self_healer.reset();
            // Rebuild color cache + frame for new palette.
            term.set_color_cache(ColorCache::new(&cloud.palette));
            frame = Frame::new(w, h, cloud.palette.bg);
            // v16: Fill terminal with new bg on live reload too.
            super::fill_terminal_bg(cloud.palette.bg);
            // Update charset_preset for runtime cycling.
            charset_preset = new_cfg.charset_preset.clone();
            // v25.5 depth-test fix: recompute target/idle_period from new
            // target_fps. Guard against fps <= 0.
            let safe_fps = new_cfg.target_fps.max(0.0);
            let safe_fps = if safe_fps > 0.0 {
                safe_fps
            } else {
                cfg.target_fps.max(1.0)
            };
            target_period = Duration::from_secs_f64(1.0 / safe_fps);
            idle_period = Duration::from_secs_f64(1.0 / (safe_fps * IDLE_FPS_FACTOR));
            // v30 (2026-08-05): keep HUD tgt: line in sync with live-reloaded fps.
            hud_state.set_target_fps(safe_fps);

            // Ambient: push new schedule to scheduler if it changed.
            if new_cfg.ambient_schedule != last_ambient_schedule {
                crate::lr_trace!(
                    "ambient: schedule changed (was {} entries, now {}) — pushing to scheduler thread",
                    last_ambient_schedule.entries.len(),
                    new_cfg.ambient_schedule.entries.len()
                );
                ambient_handle.reload(new_cfg.ambient_schedule.clone());
                last_ambient_schedule = new_cfg.ambient_schedule.clone();
            }

            // v30.3: RE-APPLY the last-known ambient entry to the new Cloud.
            // Rebuild created a fresh Cloud with CLI override but NOT the
            // ambient scene. If the entry was removed from the schedule,
            // clear the tracker.
            if let Some(ref last_entry) = last_applied_ambient_entry {
                let still_in_schedule = new_cfg
                    .ambient_schedule
                    .entries
                    .iter()
                    .any(|e| e == last_entry);
                if still_in_schedule {
                    crate::lr_trace!(
                        "ambient: re-applying last entry after rebuild (scene={})",
                        last_entry.scene
                    );
                    charset_preset = cloud.apply_ambient_entry(
                        last_entry,
                        &charset_preset,
                        &user_ranges,
                        def_ascii,
                        &last_applied_cfg_map.clone().unwrap_or_default(),
                    );
                    scene_name = last_entry.scene.clone();
                    scene_generation = scene_generation.wrapping_add(1);
                    term.set_color_cache(ColorCache::new(&cloud.palette));
                    frame = Frame::new(w, h, cloud.palette.bg);
                    super::fill_terminal_bg(cloud.palette.bg);
                } else {
                    crate::lr_trace!(
                        "ambient: last entry no longer in schedule — clearing tracker"
                    );
                    last_applied_ambient_entry = None;
                }
            }
        }

        // Ambient phase scheduler: poll for phase-fire events (non-blocking).
        // Drain all pending events, apply the LAST one (latest phase wins).
        let mut last_ambient_entry: Option<crate::ambient::AmbientEntry> = None;
        while let Ok(entry) = ambient_handle.rx.try_recv() {
            crate::lr_trace!(
                "ambient: received phase event {:02}:{:02} (scene={})",
                entry.hour,
                entry.minute,
                entry.scene
            );
            last_ambient_entry = Some(entry);
        }
        if let Some(entry) = last_ambient_entry {
            // v30.2: apply the entry's scene via apply_ambient_entry, which
            // delegates to apply_scene_runtime_with_cfg. This handles both
            // built-in scenes (fast path) and custom scenes (looks up
            // [scene-custom.<name>] block, applies base-scene defaults first,
            // then the block's own overrides). No override layer — the scene
            // IS the spec, so there's no field that can be "lost".
            let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
            charset_preset = cloud.apply_ambient_entry(
                &entry,
                &charset_preset,
                &user_ranges,
                def_ascii,
                &cfg_map,
            );
            // v30.3: track the applied entry so we can re-apply it after the
            // next live-reload rebuild (preserves ambient priority over CLI
            // override across rebuilds).
            last_applied_ambient_entry = Some(entry.clone());
            // Re-sync scene_name to the applied entry's scene.
            scene_name = entry.scene.clone();
            scene_generation = scene_generation.wrapping_add(1);
            // Rebuild color cache + frame for new palette (scene may have
            // changed color/charset/density/speed).
            term.set_color_cache(ColorCache::new(&cloud.palette));
            frame = Frame::new(w, h, cloud.palette.bg);
            super::fill_terminal_bg(cloud.palette.bg);
        }

        // Adaptive throttling: detect idle state (no input for
        // IDLE_THRESHOLD_SECS) and reduce effective FPS to save CPU.
        let loop_now = Instant::now();
        // Capture scene generation at frame start to detect user-initiated
        // scene changes and reset the self-healer. Phase D: u64 copy
        // replaces a per-frame String clone.
        let scene_generation_at_frame_start = scene_generation;
        let reactive_idle = is_runtime_idle(last_input_time, loop_now);
        let predicted_idle = phase_predictor
            .predicts_active(local_secs_since_midnight())
            .map(|active| !active)
            .unwrap_or(false);
        let is_idle = reactive_idle || predicted_idle;

        // Track phase transitions for the predictor.
        let now_active = !is_idle;
        if now_active != was_active {
            phase_predictor.record_transition(now_active, local_secs_since_midnight());
            was_active = now_active;
        }

        // Track idle duration for P2 adaptive resync.
        if is_idle && idle_started.is_none() {
            idle_started = Some(loop_now);
        } else if !is_idle {
            idle_started = None;
        }

        // P2: Use adaptive resync interval based on sustained idle duration.
        let idle_secs = idle_started
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
            // errors. Using `?` would propagate to run_interactive() and exit
            // silently — Terminal::drop restores alt-screen before main.rs can
            // print. Fix: treat event I/O errors as non-fatal; break the drain
            // loop and proceed to frame rendering. Persistent failures are
            // caught by the watchdog (2s stuck detection) + GRACEFUL_SHUTDOWN.
            //
            // Terminal-gone detection (EIO/EBADF/BrokenPipe): when the PTY
            // master disappears (user force-closes the terminal), poll_event
            // returns Ok(true) instantly forever (POLLHUP makes the fd
            // perpetually "readable") and read_event returns Err(EIO). We
            // detect this in the drain loop and set cloud.raining = false so
            // the wait-phase break condition exits the inner loop immediately
            // — without this, the wait phase spin-waits for the full frame
            // period on every frame, burning 100% CPU for seconds.
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
                        // On Android/Termux, key events may arrive with
                        // KeyEventKind::Release or Repeat instead of Press.
                        // The Press-only guard caused 'i' (HUD toggle) to
                        // be silently dropped, falling through to the
                        // screensaver exit path. On Android, accept Press
                        // and Repeat but skip Release (prevents double-toggle).
                        // On desktop, keep Press-only for precision.
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
                                &mut last_input_time,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            cloud.force_draw_everything();
                            next_frame = activity_time;
                            continue;
                        }

                        // HUD toggle: check BEFORE screensaver exit so the
                        // toggle key doesn't cause self-exit on Android/Termux
                        // where the screensaver path would otherwise fire on
                        // any unrecognized key event.
                        //
                        // v30 simplify: lowercase-only. 'i' toggles; uppercase
                        // 'I' removed (was for sticky-shift keyboards).
                        //
                        // When toggling OFF, force_draw_everything() clears
                        // stale HUD cells from the frame buffer. The rain uses
                        // diff-based rendering (frame.set, not set_force), so
                        // cells the rain doesn't actively write keep their
                        // previous content — including HUD text + black bg.
                        // Without force_draw, this leaves "HUD residue" in
                        // regions with no active rain this frame.
                        if matches!((k.code, k.modifiers), (KeyCode::Char('i'), _)) {
                            let now_visible = hud_state.toggle();
                            if !now_visible {
                                cloud.force_draw_everything();
                            }
                            // v16 audit: Update next_frame so the HUD appears
                            // immediately. Without this, if the user was in
                            // idle mode (reduced FPS), pressing 'i' would
                            // schedule the HUD render at the next idle-frame
                            // time (potentially seconds away). On Windows,
                            // the long poll_event wait during idle could
                            // trigger a console error that silently exits
                            // the program. Setting next_frame = activity_time
                            // forces an immediate frame render, bypassing
                            // the long wait.
                            let _ = register_activity(
                                &mut last_input_time,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            next_frame = activity_time;
                            continue;
                        }

                        // h: toggle HUD position.
                        // v30 simplify: lowercase-only shortcuts for consistency.
                        // Uppercase 'H' removed (was accepted for Android soft
                        // keyboards where Shift may not work — but lowercase 'h'
                        // works on all keyboards). See audit task flags-audit-4 /
                        // docs/research/SHORTCUT_KEYS_AUDIT.md.
                        if matches!((k.code, k.modifiers), (KeyCode::Char('h'), _)) {
                            if hud_state.toggle_position() {
                                cloud.force_draw_everything();
                            }
                            let _ = register_activity(
                                &mut last_input_time,
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
                            &mut last_input_time,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        ) {
                            cloud.force_draw_everything();
                            next_frame = activity_time;
                        }

                        // Process the keybinding FIRST. This lets interactive
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
                            // Screensaver mode (v15 "only q quits" policy):
                            //
                            // - q: quit (handle_keybinding set raining=false)
                            // - Recognized interactive keys (c/C, s/S, p,
                            //   x/X, [, ], i/I, h/H, Space, Up/Down):
                            //   process and continue. The user can still
                            //   cycle colors, toggle HUD, etc. while the
                            //   screensaver is active.
                            // - Unrecognized keys (a, m, g, B/b, z, F1-F12,
                            //   Home/End, PageUp/Down, Esc, Ctrl+Z, etc.):
                            //   SILENTLY IGNORED. They do NOT exit the
                            //   screensaver and do NOT cause any visual
                            //   glitch. The user must press 'q' to quit.
                            //   This matches the "only q quits" policy enforced
                            //   in normal (non-screensaver) mode — consistency
                            //   is the world-class invariant.
                            //
                            // v17: Mouse click does NOT exit either. The old
                            // "classic screensaver click to dismiss" behavior
                            // was removed for policy consistency. See Event::Mouse
                            // handler below for the rationale.
                            //
                            // The "unrecognized key exits" behavior was REMOVED
                            // in v15 because it was surprising: pressing B/b
                            // or any letter not in the recognized set would
                            // kick the user out. Now only q exits.
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
                            &mut last_input_time,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        );
                        cloud.force_draw_everything();
                        next_frame = activity_time;
                    }
                    Event::Mouse(m) => {
                        // v17: Mouse events are ALWAYS captured (mouse reporting
                        // is always on). This blocks plain drag-select text copy
                        // in ALL modes, preserving the ephemeral screensaver
                        // aesthetic.
                        //
                        // v17 mastery: REMOVED force_draw_everything on mouse
                        // move. The old code called force_draw on idle→active
                        // transition, which rendered ALL cells (including trail
                        // middles that normally skip) + seeded phosphor everywhere.
                        // This produced a visible brightness flash that persisted
                        // ~400ms until phosphor decayed — the owner reported
                        // 'bright colors when moving mouse'. Now we only update
                        // the idle timer (for FPS throttling) and mouse position.
                        // The next regular diff frame handles rendering naturally.
                        let activity_time = Instant::now();
                        let _ = register_activity(
                            &mut last_input_time,
                            &mut last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        );
                        // v17 mastery: hover/click visual effects are ALWAYS ON
                        // (--mouse flag deleted). No cfg.mouse gate.
                        cloud.set_mouse_position(m.column, m.row);
                        if matches!(m.kind, MouseEventKind::Down(_)) {
                            cloud.set_mouse_click(m.column, m.row);
                        }
                    }
                    Event::FocusGained => {
                        let activity_time = Instant::now();
                        if register_activity(
                            &mut last_input_time,
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

            // Break out of the poll loop when we have a resize to apply,
            // but only after the debounce window has elapsed. This coalesces
            // rapid resize events (e.g. window drag) into a single reset.
            //
            // Also break immediately if SIGHUP/SIGTERM fired (GRACEFUL_SHUTDOWN)
            // or if the drain loop detected a dead PTY (cloud.raining = false).
            // Without the GRACEFUL_SHUTDOWN check, the inner wait loop would
            // keep spin-waiting until next_frame even after the signal handler
            // set the flag — burning CPU for the remainder of the frame period.
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

            // Spin-sleep hybrid: poll_event for the bulk of the wait (also
            // processes input events), then spin-wait the final ~500μs for
            // sub-millisecond deadline accuracy. Eliminates OS scheduling
            // jitter from the frame cadence.
            //
            // v25.15 (perf audit): spin_budget is now `FRAME_SPIN_BUDGET`
            // from constants.rs — was a hardcoded `Duration::from_micros(500)`
            // inline.
            //
            // Dead-PTY guard: when the terminal is force-closed, POLLHUP makes
            // the tty fd perpetually "readable", so poll_event returns Ok(true)
            // instantly and forever. If we fall through to spin_wait on Ok(true),
            // we burn 500us-1ms of busy-spin per iteration. Instead, continue
            // back to the drain phase which will call read_event — that returns
            // Err(UnexpectedEof/EIO) which is_terminal_gone catches, setting
            // cloud.raining = false. This drops post-SIGHUP CPU burn from 20s
            // of 100% to < 1ms in rain mode.
            let spin_budget = FRAME_SPIN_BUDGET;
            if timeout > spin_budget {
                // v25: poll_event can return Err when the terminal is closed
                // (SIGHUP / PTY gone — crossterm's mio returns EIO/BadFd).
                // Propagating via `?` sends the error to main.rs, which calls
                // eprintln! on broken stderr → double-panic → abort → coredump.
                // Treat EIO/BrokenPipe as "terminal gone": stop rain and break,
                // mirroring the draw() EIO guard below. Post-loop shutdown
                // drops Terminal (uses `let _ =` in cleanup) and exits cleanly.
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
        let active_is_idle = is_idle;
        let frame_period = if cloud.pause {
            pause_period
        } else if active_is_idle {
            idle_period
        } else {
            target_period
        };
        let frame_period_s = frame_period.as_secs_f32().max(0.000_001);

        // v30 (2026-08-05): announce frame pacing mode to the HUD so the
        // `tgt:` line can show an `idle` / `paused` suffix. Cheap (one enum
        // set + one method call). Placed AFTER the pause/idle/active branch
        // so the mode reflects the actual cadence used for this frame, not
        // the previous frame's.
        let frame_mode = if cloud.pause {
            FrameMode::Paused
        } else if active_is_idle {
            FrameMode::Idle
        } else {
            FrameMode::Active
        };
        hud_state.set_frame_mode(frame_mode);

        cloud.set_perf_pressure(perf_pressure);
        let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
        // v25.15 (perf audit): clamp lower bound is now `SIM_FACTOR_MIN`
        // from constants.rs — was a hardcoded `0.3` inline.
        let sim_factor =
            (1.0 - (perf_pressure as f64) * SIM_PRESSURE_SCALE_FACTOR).clamp(SIM_FACTOR_MIN, 1.0);
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
        // Feed it into perf_pressure so the self-healer downgrades the
        // scene before the consumer OOMs (SIGTRAP scenario).
        let write_overshoot = if frame_period_s > 0.0 {
            ((term.last_write_ns() as f32 / 1e9) / frame_period_s - 0.5).clamp(0.0, 2.0)
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
        if overshoot > 0.0 {
            perf_pressure = (perf_pressure + (overshoot * PERF_PRESSURE_INCREMENT)).min(1.0);
        } else {
            perf_pressure = (perf_pressure - PERF_PRESSURE_DECAY).max(0.0);
        }
        if write_overshoot > 0.0 {
            perf_pressure = (perf_pressure + (write_overshoot * PERF_PRESSURE_INCREMENT)).min(1.0);
        }

        if cfg.perf_stats {
            perf_frames = perf_frames.saturating_add(1);
            if did_draw {
                perf_drawn_frames = perf_drawn_frames.saturating_add(1);
            } else {
                perf_idle_frames = perf_idle_frames.saturating_add(1);
            }
            perf_dirty_sum = perf_dirty_sum.saturating_add(dirty_len as u64);
            perf_dirty_samples = perf_dirty_samples.saturating_add(1);
            perf_work_sum_s += work_s as f64;
            perf_work_max_s = perf_work_max_s.max(work_s as f64);
            perf_pressure_sum += perf_pressure as f64;
            perf_pressure_max = perf_pressure_max.max(perf_pressure);
            perf_utilization_sum += utilization as f64;
            perf_utilization_max = perf_utilization_max.max(utilization);
            if overshoot > 0.0 {
                perf_overshoot_frames = perf_overshoot_frames.saturating_add(1);
            }
            frame_time_tracker.push(work_s as f64 * 1000.0);

            // P5: Feed endurance health tracker.
            endurance_health.push_frame_time(work_s as f64 * 1000.0);
            // Sample RSS every 60 frames (~1s at 60fps) to avoid /proc overhead.
            if perf_rss_samples % 60 == 0 {
                #[cfg(target_os = "linux")]
                {
                    let rss = super::intro::read_self_rss_kb();
                    endurance_health.push_rss(rss as f64);
                }
                // Context switch rate sampling.
                // P2: reuse work_start (captured just before cloud.rain_at) instead
                // of another Instant::now(). The timing difference is <1ms, negligible
                // for context switch rate measurement (sampled every 60 frames ≈ 1s).
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

            // P5: periodic stdout fd health probe.
            //
            // Runs on the same slow tick as the P4 stuck-cell sweep
            // (FD_HEALTH_PROBE_INTERVAL_FRAMES ≈ 60 s at 60 FPS). Detects
            // fd corruption BEFORE a write fails — closing the idle-period
            // window where stdout could break (SSH disconnect, terminal
            // crash, parent death) without anything noticing until the
            // next render attempt.
            //
            // On Unix: calls isatty(stdout_fd). If false, reuses the P3
            // recovery path (recover_to_tty with an empty buffer + a
            // synthetic BrokenPipe error) which sets GRACEFUL_SHUTDOWN.
            // On non-Unix: no-op (always returns true).
            //
            // Cost: one isatty syscall per minute. Negligible.
            if perf_rss_samples % FD_HEALTH_PROBE_INTERVAL_FRAMES == 0
                && !term.probe_stdout_health()
            {
                // Recovery attempted — GRACEFUL_SHUTDOWN is set.
                // Break the loop; the normal shutdown path runs
                // (Terminal::drop restores the TTY from /dev/tty).
                cloud.raining = false;
                break;
            }
        }

        // Performance self-healer (P1 + P2).
        //
        // Called every frame after perf_pressure is finalized and (when
        // perf_stats is on) endurance_health has been recomputed. The
        // self-healer is a pure policy — it returns an action enum and
        // we apply the side effects here.
        //
        // When perf_stats is off, endurance_health.score() stays at its
        // initial 100.0 (no samples pushed), so P2's `score < 60` check
        // never fires. P1 still works — it only needs perf_pressure,
        // which is always tracked.
        //
        // `now` uses `loop_now` (captured at top of frame) for consistency
        // with the rest of the timing-sensitive logic in this loop.

        // If the scene changed since frame start (user 'x' key, live config
        // reload, or adaptive-custom), reset the self-healer. Must happen
        // BEFORE observe() so the self-healer doesn't fire a downgrade/
        // restore on the same frame the user switched scenes. Phase D:
        // u64 counter compare replaces a String-clone + String-ne.
        if scene_generation != scene_generation_at_frame_start {
            self_healer.reset();
        }

        let heal_action = self_healer.observe(perf_pressure, loop_now, {
            // Only pass a real score when perf_stats is on — otherwise pass
            // None so the self-healer skips the P2 check entirely (cheaper
            // than passing 100.0 and having it compare every frame).
            if cfg.perf_stats {
                Some(endurance_health.score())
            } else {
                None
            }
        });
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
                // P1: save the current scene, then switch to the fallback.
                // Skip if the user is already on the fallback scene (no-op
                // downgrade would leave us in a weird state where
                // pre_degraded_scene == FALLBACK_SCENE).
                if scene_name != PerformanceSelfHealer::FALLBACK_SCENE {
                    self_healer.record_downgrade(&scene_name);
                    let prior_scene = scene_name.clone();
                    let new_charset = cloud.apply_scene_runtime(
                        PerformanceSelfHealer::FALLBACK_SCENE,
                        &charset_preset,
                        &user_ranges,
                        def_ascii,
                    );
                    scene_name = PerformanceSelfHealer::FALLBACK_SCENE.to_string();
                    scene_generation = scene_generation.wrapping_add(1);
                    charset_preset = new_charset;
                    // Log via write_fmt (broken-pipe-safe, same pattern as
                    // the watchdog). Helps users understand why their scene
                    // changed unexpectedly.
                    use std::io::Write;
                    let _ = std::io::stderr().write_fmt(format_args!(
                        "[self-heal] sustained high CPU pressure — downgrading '{}' → '{}'\n",
                        prior_scene,
                        PerformanceSelfHealer::FALLBACK_SCENE
                    ));
                }
            }
            SelfHealAction::RestoreScene => {
                // P1: restore the scene that was active before the downgrade.
                if let Some(prior) = self_healer.take_pre_degraded_scene() {
                    let new_charset =
                        cloud.apply_scene_runtime(&prior, &charset_preset, &user_ranges, def_ascii);
                    scene_name = prior;
                    scene_generation = scene_generation.wrapping_add(1);
                    charset_preset = new_charset;
                    use std::io::Write;
                    let _ = std::io::stderr().write_fmt(format_args!(
                        "[self-heal] CPU pressure recovered — restoring scene\n"
                    ));
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

    // v30 fix: always print a one-line final FPS summary on exit so the
    // user has an honest number without needing --perf-stats. Previously
    // "FRAME OVERSHOOT avg: 0.000" was misread as "0 FPS" — it's actually
    // perf_pressure (0 by design when renderer keeps up with target_fps).
    let final_elapsed = start_time.elapsed();
    let final_elapsed_s = final_elapsed.as_secs_f64().max(0.000_001);
    let final_avg_fps = (perf_frames as f64) / final_elapsed_s;
    let last_work_ms = frame_time_tracker.rolling_avg_ms();
    let final_instant_fps = if last_work_ms > 0.0 {
        (1000.0 / last_work_ms).min(cfg.target_fps)
    } else {
        cfg.target_fps
    };
    eprintln!(
        "[cosmostrix] final FPS: {:.1} (instant: {:.1}, target: {:.1}), frames: {}, elapsed: {:.2}s",
        final_avg_fps, final_instant_fps, cfg.target_fps, perf_frames, final_elapsed_s
    );

    if cfg.perf_stats {
        // Capture encoding stats BEFORE dropping the terminal -- the stats
        // live inside the Terminal/ColorCache and would be lost on drop.
        // Tier 2: also capture tier2_stats (backpressure_skips, ris_resets,
        // bytes_since_ris) before drop.
        let (enc_bytes, enc_flushes, sgr_hits, sgr_misses) = term.encoding_stats();
        let (tier2_skips, tier2_resets, tier2_bytes_since) = term.tier2_stats();
        drop(term);
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
                target_period,
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
                &phase_predictor.transitions_observed().to_string(),
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

    // Store final runtime state for post-exit verbose summary.
    let final_color_name = format!("{:?}", cloud.color_scheme());
    super::set_final_state(
        &final_color_name,
        &scene_name,
        &charset_preset,
        cloud.chars_per_sec,
        cloud.droplet_density,
    );

    Ok(())
}
