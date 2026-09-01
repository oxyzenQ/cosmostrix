// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: while-cloud.raining loop with deeply coupled mutable state across 20+ extracted sibling modules; further splitting requires a context struct refactor.

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
use super::adaptive::{EnduranceHealth, PerformanceSelfHealer, ReclaimState};
use super::event_loop_finalize::finalize_session;
use super::hud::HudState;
use super::input::{
    handle_keybinding, hud_toggle_accepted, is_unmodified, KeybindingCtx, PasteBurstGuard,
};
use super::watchdog::{GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE};

pub(crate) fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    // v51 killer-features hardening: mark the interactive session (alt
    // screen) as active BEFORE any config-block helper can fire a warning —
    // intro sequence + scene changes + live reload all resolve custom
    // palettes/charsets/scenes mid-rain. Warnings routed through
    // output::warn_runtime_or_now buffer until post-exit from this point on
    // (AB-10: never leak a stderr line into the rain matrix).
    crate::live_config::set_interactive_session_active();

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
    // Extracted to event_loop_intro.rs to keep this file under the 800-LOC
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
    // v51.1 masterclass: pristine startup snapshot — the LOCKED layer
    // (CLI > config@startup resolution baked in). Never mutated for the
    // whole session: the live-reload path restores the scene family from
    // it when the config `scene` key is removed, so a `--scene
    // crystal-dragon` run returns to crystal-dragon after the config
    // override is commented back out — no exit, no rerun.
    let startup_cfg = cfg.clone();
    // v50.0.0-alpha.7: track the LATEST live-reloaded CloudConfig so
    // finalize_session (at exit) reads the EFFECTIVE runtime values,
    // not the startup values. Without this, "final runtime state"
    // verbose section shows startup values (e.g. crystal_dragon=false)
    // instead of the live-reloaded values (e.g. crystal_dragon=true).
    let mut current_cfg = cfg.clone();
    // Pending rebuild: set when watcher sends new config, applied at top of next frame.
    let mut pending_config: Option<std::collections::HashMap<String, String>> = None;

    // Ambient scheduler: idle/wake thread sends AmbientEntry via mpsc.
    let mut ambient_handle =
        crate::crystal_dragon_engine::ambient_scheduler::spawn_ambient_scheduler(
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

    // v50.0.0-beta.7 masterclass: ambient startup delay.
    //
    // Owner's simple rule:
    //   - No CLI flags at all → ambient applies INSTANTLY (no delay).
    //     Example: `cosmostrix -v` (debug only, no scene/color/charset).
    //   - ANY CLI flag (--scene, --color, --charset, etc.) → ambient
    //     DEFERS for ambient-snapback-secs (default 30s). CLI wins first,
    //     then ambient takes over after the delay.
    //
    // This avoids confusion: user runs `cosmostrix --scene matrix` with
    // `ambient.12-00 = monolith` and sees matrix first, then after 30s
    // monolith kicks in. Without the delay, ambient immediately overrides
    // the CLI scene — user thinks `--scene matrix` is broken.
    //
    // Implementation: check if ANY CliExplicit flag is true. If none,
    // apply ambient instantly. If any, defer (skip startup apply + let
    // auto-snapback handle it after the delay).
    // v51.1: uses CliExplicit::any() — the old inline `||` chain listed
    // only 15 of the 21 flags (--bold, --shading-mode, --color-bg,
    // --colors-custom, --scene-custom, -mfs did NOT defer ambient).
    let cli_has_any_override = base_cfg.cli_explicit.any();
    let (new_charset, startup_entry) = if cli_has_any_override {
        // CLI flags present: defer ambient. Capture the entry for snapback.
        let now_min = crate::crystal_dragon_engine::ambient::current_minute_of_day();
        let deferred_entry = base_cfg.ambient_schedule.current_phase(now_min).cloned();
        crate::lr_trace!(
            "ambient: startup — CLI flags detected, deferring ambient apply until snapback. Entry: {:?}",
            deferred_entry.as_ref().map(|e| &e.scene)
        );
        last_applied_ambient_entry = deferred_entry;
        // Keep user_override_since_ambient = true so poll_ambient_events
        // defers re-application (the else-if branch in poll_ambient_events).
        (charset_preset.clone(), None)
    } else {
        // No CLI flags: apply ambient instantly (original behavior).
        crate::crystal_dragon_engine::ambient::apply_startup_ambient(
            &mut cloud,
            &base_cfg.ambient_schedule,
            &charset_preset,
            &user_ranges,
            def_ascii,
            &initial_cfg_map,
        )
    };
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

        // v50.0.0-beta.7 LOC refactor: config event draining extracted to
        // event_loop_config_drain.rs.
        if !super::event_loop_config_drain::drain_config_events(
            &config_rx,
            &mut pending_config,
            &mut cloud,
        ) {
            break;
        }

        // v50.0.0-beta.7 LOC refactor: config rebuild extracted to
        // event_loop_config_rebuild.rs.
        super::event_loop_config_rebuild::apply_config_rebuild(
            &mut pending_config,
            &mut base_cfg,
            &startup_cfg,
            &mut cloud,
            &mut frame,
            &mut term,
            &mut power_manager,
            &mut hud_state,
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &mut current_cfg,
            &mut last_applied_cfg_map,
            &mut last_ambient_schedule,
            &mut ambient_handle,
            &mut last_applied_ambient_entry,
            &mut ambient_snapback_killed,
            cfg,
            w,
            h,
            &user_ranges,
            &mut self_healer,
            def_ascii,
        );

        // v50.0.0-beta.7 LOC refactor: ambient polling extracted to
        // event_loop_ambient.rs.
        // v51.2: startup_cfg passed for the ambient overlay-lift revert
        // (ground-truth nuke path — see event_loop_ambient.rs).
        super::event_loop_ambient::poll_ambient_events(
            &mut cloud,
            &mut frame,
            &mut term,
            &mut charset_preset,
            &mut scene_name,
            &mut scene_generation,
            &last_applied_cfg_map,
            &mut last_ambient_schedule,
            &mut ambient_handle,
            &mut last_applied_ambient_entry,
            &mut ambient_snapback_killed,
            &mut last_ground_truth_check,
            &config_path_for_ground_truth,
            &mut next_frame,
            w,
            h,
            &user_ranges,
            def_ascii,
            &current_cfg,
            &startup_cfg,
            last_user_input_at,
        );

        // v50.0.0-beta.7 LOC refactor: adaptive throttling extracted to
        // event_loop_adaptive.rs.
        let throttle = super::event_loop_adaptive::run_adaptive_throttle(
            &mut cloud,
            &mut frame,
            &mut power_manager,
            &mut reclaim_state,
            &mut last_resync_time,
            &mut next_frame,
            scene_generation,
        );
        let loop_now = throttle.loop_now;
        let is_idle = throttle.is_idle;
        let scene_generation_at_frame_start = throttle.scene_generation_at_frame_start;

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
                        // v51 pause isolation (owner bug report 2026-08-30): 'i'
                        // must NOT respond while paused/decelerating — only 'p'
                        // and 'q' work during pause. The gate lives in input.rs
                        // (hud_toggle_accepted) so the predicate is testable and
                        // identical to the handle_keybinding pause guard.
                        if hud_toggle_accepted(&cloud)
                            && is_unmodified(k.modifiers)
                            && matches!(k.code, KeyCode::Char('i'))
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
                        // keys (q, c/C, s/S, x/X, p, i, [, ], r,
                        // Up/Down) work identically in --screensaver
                        // mode and normal mode — the ONLY differences
                        // are the two micro-scale scheduling details
                        // documented in docs/SCREENSAVER_MODE.md.
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
                            // Screensaver: break the event drain immediately
                            // once 'q' cleared cloud.raining (queued events are
                            // discarded instead of drained — see
                            // docs/SCREENSAVER_MODE.md §2 for the full audit).
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
        // v50.0.0-beta.7 LOC refactor: resize handler extracted to
        // event_loop_resize.rs.
        super::event_loop_resize::handle_resize(
            pending_resize,
            &mut w,
            &mut h,
            &mut cloud,
            &mut frame,
            &mut hud_state,
            &mut term,
            &current_cfg,
            &mut last_resync_time,
            cfg.screen_size.is_some(),
        );
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
        // v50.0.0-beta.7 LOC refactor: sim+draw extracted to
        // event_loop_sim_draw.rs.
        let sim_draw = super::event_loop_sim_draw::run_sim_and_draw(
            &mut cloud,
            &mut frame,
            &mut hud_state,
            &mut term,
            &power_manager,
            frame_period,
        )?;
        let work_start = sim_draw.work_start;
        let is_dirty_all = sim_draw.is_dirty_all;
        let dirty_len = sim_draw.dirty_len;
        let did_draw = sim_draw.did_draw;
        if sim_draw.terminal_gone {
            break;
        }

        // Z-master-1X round 5: push dirty-cell + total-cell counts to the
        // HUD for the dcel/tcel metrics. Must run AFTER sim_draw (which
        // produces dirty_len + is_dirty_all) and BEFORE the post-draw
        // accounting (which may early-out on terminal_gone, already handled
        // above). Total cells = frame.width × frame.height (the logical
        // screen size). Dirty count = dirty_len (or full screen if
        // is_dirty_all — sim_draw signals "everything changed").
        {
            let total_cells = (frame.width as u64) * (frame.height as u64);
            let dirty_count = if is_dirty_all {
                total_cells
            } else {
                dirty_len as u64
            };
            hud_state.set_dirty_cell_stats(dirty_count, total_cells);
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
    // v50.0.0-beta.7 LOC refactor: SessionStats construction extracted
    // to event_loop_stats.rs.
    let stats =
        super::event_loop_stats::build_session_stats(super::event_loop_stats::StatsInputs {
            start_time,
            perf_frames,
            perf_drawn_frames,
            perf_idle_frames,
            perf_overshoot_frames,
            perf_dirty_sum,
            perf_dirty_samples,
            perf_work_sum_s,
            perf_work_max_s,
            perf_pressure_sum,
            perf_pressure_max,
            perf_utilization_sum,
            perf_utilization_max,
            frame_time_tracker: &frame_time_tracker,
            power_manager: &power_manager,
            endurance_health: &endurance_health,
            cloud: &cloud,
        });
    finalize_session(
        &stats,
        term,
        &cloud,
        &scene_name,
        &charset_preset,
        &current_cfg,
    )
}
