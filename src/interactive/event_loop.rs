// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Main interactive event loop.
//!
//! Contains the `run_interactive()` function that drives the entire
//! interactive mode: signal handling, frame pacing, input dispatch,
//! simulation stepping, rendering, and performance reporting.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};

#[cfg(unix)]
use signal_hook::consts::{SIGCONT, SIGHUP, SIGINT, SIGQUIT, SIGSTOP, SIGTERM, SIGTSTP};
#[cfg(unix)]
use signal_hook::iterator::Signals;
#[cfg(unix)]
use signal_hook::low_level;

use crate::color_cache::ColorCache;
use crate::constants::*;
use crate::frame::Frame;
use crate::report::Report;
use crate::terminal::{restore_terminal_best_effort, Terminal};

use super::super::{effective_density, CloudConfig};
use super::activity::{is_runtime_idle, register_activity, spin_wait, FrameTimeTracker};
use super::adaptive::{
    adaptive_resync_interval, local_secs_since_midnight, EnduranceHealth, PhasePredictor,
    ReclaimState,
};
use super::hud::HudState;
use super::input::{handle_keybinding, PasteBurstGuard};
use super::watchdog::{
    spawn_watchdog, FRAME_COUNTER, GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE, SHUTDOWN,
};

pub(crate) fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    crate::spawn_kill9_terminal_guard();

    #[cfg(unix)]
    let term_reinit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // Signal-exit flag: shared between signal handler threads and Terminal.
    // Created before signal handlers so the handler closure can capture a
    // clone. When set, Terminal::drop() clears the alternate screen viewport
    // before switching back, preventing rain frame residue. Normal q/esc
    // exit never sets this flag.
    let signal_exit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        let se = signal_exit.clone();
        if let Ok(mut signals) = Signals::new([SIGINT, SIGTERM, SIGHUP, SIGQUIT]) {
            std::thread::spawn(move || {
                if let Some(_sig) = signals.forever().next() {
                    // Request graceful shutdown via AtomicBool instead of
                    // directly writing ANSI restore sequences to stdout.
                    // This avoids racing with the main thread on the same fd.
                    GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
                    // Mark this as a signal-triggered exit so Terminal::drop()
                    // clears the visible viewport before leaving the alternate
                    // screen. This prevents rain frame residue on the main
                    // screen after pkill -TERM or Ctrl-C.
                    se.store(true, Ordering::Release);
                    // Wait for the main loop to notice GRACEFUL_SHUTDOWN, set
                    // SHUTDOWN, and run Terminal::drop().  Do NOT call
                    // restore_terminal_best_effort() + process::exit() here —
                    // that races on stdout with the main loop's buffered writer
                    // and skips Terminal::drop(), which is the only path that
                    // flushes the final frame before leaving the alternate
                    // screen.  If the main loop is truly stuck (e.g. deadlock
                    // inside a syscall), the watchdog thread (20s timeout)
                    // will handle the hard restore instead.
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        if SHUTDOWN.load(Ordering::Acquire) {
                            break;
                        }
                    }
                }
            });
        }

        let term_reinit = term_reinit.clone();
        if let Ok(mut signals) = Signals::new([SIGTSTP, SIGCONT]) {
            std::thread::spawn(move || {
                for sig in signals.forever() {
                    match sig {
                        SIGTSTP => {
                            // Disable mouse capture before suspending so the
                            // terminal is usable while cosmostrix is stopped.
                            if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                                use crossterm::ExecutableCommand;
                                let _ = std::io::stdout()
                                    .execute(crossterm::event::DisableMouseCapture);
                                MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
                            }
                            restore_terminal_best_effort();
                            term_reinit.store(true, Ordering::Release);
                            let _ = low_level::raise(SIGSTOP);
                        }
                        SIGCONT => {
                            term_reinit.store(true, Ordering::Release);
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    #[cfg(windows)]
    {
        let se = signal_exit.clone();
        if let Err(e) = ctrlc::set_handler(move || {
            GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
            se.store(true, Ordering::Release);
            std::thread::sleep(std::time::Duration::from_secs(1));
            if !SHUTDOWN.load(Ordering::Acquire) {
                if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                    use crossterm::ExecutableCommand;
                    let _ = std::io::stdout().execute(crossterm::event::DisableMouseCapture);
                }
                restore_terminal_best_effort();
                std::process::exit(130);
            }
        }) {
            eprintln!("failed to install Ctrl-C handler: {}", e);
        }
    }

    // Spawn watchdog thread
    spawn_watchdog();

    let mut term = Terminal::with_signal_exit(signal_exit.clone())?;
    // Mouse reporting is opt-in because abrupt process death can leave some
    // terminals echoing raw mouse escape sequences until they are reset.
    if cfg.mouse && term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    // --screen-size: use fixed virtual size if specified, else dynamic terminal size.
    let (w, h) = if let Some(fixed) = cfg.screen_size {
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

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

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

    let start_time = Instant::now();
    let end_time = cfg.duration_s.and_then(|s| {
        if !s.is_finite() || s <= 0.0 {
            return None;
        }
        let s = cfg.duration.unwrap_or(s);
        Some(start_time + Duration::from_secs_f64(s))
    });

    let target_period = Duration::from_secs_f64(1.0 / cfg.target_fps);
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
    let mut frame_time_tracker: FrameTimeTracker = FrameTimeTracker::new();

    // Live HUD overlay state — toggled with 'i'. When visible, renders a
    // compact FPS/p99/RSS overlay in the top-right corner at 4 Hz.
    // Zero cost when off (all methods short-circuit on visible==false).
    // 'i' is used instead of '?' because Android/Termux soft keyboards
    // may send '?' as a multi-byte sequence or with unexpected modifier
    // bits, which falls through to the screensaver exit path. A simple
    // lowercase printable letter is sent reliably by every keyboard.
    let mut hud_state: HudState = HudState::new();
    hud_state.set_screen_size(w, h, cfg.screen_size.is_some());

    // Perceived-motion diagnostics: track how many frames produce visible
    // changes vs. frames where nothing visually changed. This helps diagnose
    // the "feels like 10 FPS" problem where the renderer runs at 60 FPS but
    // row advances only happen every ~8 frames.
    let mut perf_idle_frames: u64 = 0; // frames where dirty_count == 0
    let mut perf_dirty_sum: u64 = 0; // total dirty cells across all frames
    let mut perf_dirty_samples: u64 = 0; // number of frames sampled for dirty avg

    // Resize debounce: track when the last resize event arrived so rapid
    // resize storms (e.g. window drag) are coalesced into a single apply.
    let mut last_resize_event: Option<Instant> = None;

    // Adaptive throttling: track last user input time for idle detection.
    // After IDLE_THRESHOLD_SECS with no input, effective FPS is reduced to
    // IDLE_FPS_FACTOR × target_fps. Any input event instantly restores.
    let mut last_input_time = Instant::now();
    let mut last_resync_time = last_input_time;
    let idle_period = Duration::from_secs_f64(1.0 / (cfg.target_fps * IDLE_FPS_FACTOR));

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

    let mut charset_preset = cfg.charset_preset.clone();
    let mut scene_name = crate::scene::DEFAULT_SCENE.to_string();
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

    // Adaptive color shift: check current hour's target color every 30s.
    // When the phase changes (e.g. Deep Void → Compression), the target
    // color changes. We apply it via cloud.set_color_scheme() which does
    // a smooth palette transition wave.
    //
    // Owner design: start with the user's config color (e.g. Green3),
    // then after 30s shift to the adaptive target. This gives the user
    // a brief moment to see their chosen color before the Dragon breathes.
    let mut last_color_check = Instant::now();
    let mut last_adaptive_color: Option<&str> = None;
    const COLOR_CHECK_INTERVAL: Duration = Duration::from_secs(30);

    // Helper: track runtime state changes for post-exit verbose summary.
    // We do NOT eprintln during rain — that causes screen flicker because
    // stderr output appears in the terminal during alternate-screen mode.
    // Instead, we track changes silently and print a full summary after exit.
    let verbose = cfg.verbose;
    let mut last_color_scheme = cloud.color_scheme();
    let mut last_scene_name = scene_name.clone();
    let mut last_charset = charset_preset.clone();

    // Parse custom time map from config (if [adaptive-custom] is defined).
    // This overrides the default 5-phase adaptive engine.
    let mut custom_time_map: Option<crate::atmosphere_custom::CustomTimeMap> = {
        let cfg_map = crate::configfile::load_config_file(cfg.config_path_for_watcher.as_deref());
        match crate::atmosphere_custom::parse_custom_time_map(&cfg_map) {
            Ok(map) if !map.is_empty() => Some(map),
            Ok(_) => None,
            Err(e) => {
                eprintln!("[adaptive-custom] parse error: {e}. Using default adaptive.");
                None
            }
        }
    };

    while cloud.raining {
        // Check for graceful shutdown request from signal handler.
        // This allows clean exit via Terminal::drop() instead of racing
        // on stdout with the signal handler thread.
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            cloud.raining = false;
            break;
        }

        // Live config reload: non-blocking check for config events.
        // Ok = valid config → rebuild Cloud. Err = invalid → EXIT cosmostrix.
        if let Some(ref rx) = config_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    Ok(cfg) => pending_config = Some(cfg),
                    Err(msg) => {
                        // Store error for main.rs to print AFTER terminal
                        // restore. Printing here (alternate-screen) = invisible.
                        if let Ok(mut guard) = crate::live_config::LIVE_RELOAD_ERROR.lock() {
                            *guard = Some(msg);
                        }
                        crate::live_config::LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
                        cloud.raining = false;
                        break;
                    }
                }
            }
        }

        // Time-driven color/scene/speed/density shift: every 30s, check the
        // current time against the configured schedule and apply changes via
        // a smooth palette transition wave.
        //
        // Two sources of schedule, in priority order:
        //   1. Custom time map ([adaptive-custom.HH-MM] entries in config).
        //      Runs whenever any entry is defined — this is an explicit user
        //      opt-in that overrides atmosphere-mode. The user defined a
        //      schedule, so we follow it regardless of whether the built-in
        //      atmosphere engine is enabled.
        //   2. Built-in 5-phase adaptive engine (Deep Void / Compression /
        //      Pulse / Calm / Signal). Only runs when the user explicitly
        //      opts in via atmosphere-mode = controlled-live AND
        //      atmosphere-regime = adaptive in config. This prevents the
        //      "color auto-changes without my consent" bug from commit
        //      5172f39's default-flip.
        //
        // v15 Dragon: custom time map is decoupled from atmosphere_mode so
        // defining adaptive-custom.* is sufficient to enable scheduling.
        let now = Instant::now();
        if now.duration_since(last_color_check) >= COLOR_CHECK_INTERVAL {
            last_color_check = now;

            if let Some(ref custom_map) = custom_time_map {
                // Custom time map — runs regardless of atmosphere_mode.
                // The user explicitly defined a schedule, so we honor it.
                if let Some(cp) = custom_map.params_at(crate::atmosphere_adaptive::current_hour()) {
                    // Apply color if changed.
                    // v16: Try built-in theme first, then fall back to custom color.
                    if let Some(ref color_name) = cp.color {
                        if let Ok(scheme) = crate::cli::parse_color_scheme(color_name) {
                            // Built-in theme found.
                            if scheme != cloud.color_scheme() {
                                cloud.set_color_scheme(scheme);
                                last_color_scheme = scheme;
                            }
                        } else {
                            // Not a built-in theme — try custom color from config.
                            let cfg_map = crate::configfile::load_config_file(
                                cfg.config_path_for_watcher.as_deref(),
                            );
                            if let Ok(palette) =
                                crate::colors_custom::load_custom_palette(&cfg_map, color_name)
                            {
                                // Custom palette found — apply via set_palette.
                                // Note: last_color_scheme (ColorScheme enum) is NOT
                                // updated for custom palettes since they don't have
                                // an enum variant. The verbose exit summary will
                                // show the last built-in scheme, which is acceptable.
                                cloud.set_palette(palette);
                            }
                        }
                    }
                    // Apply speed if changed.
                    if let Some(speed) = cp.speed {
                        cloud.set_chars_per_sec(speed);
                    }
                    // Apply density if changed.
                    if let Some(density) = cp.density {
                        cloud.set_droplet_density(density);
                    }
                    // Apply charset if changed.
                    if let Some(ref charset_name) = cp.charset {
                        if *charset_name != charset_preset {
                            if let Ok(cs) = crate::charset::charset_from_str(charset_name, false) {
                                let chars =
                                    crate::charset::build_chars(cs, &user_ranges, def_ascii);
                                cloud.transition_chars(chars);
                                charset_preset = charset_name.clone();
                            }
                        }
                    }
                }
            } else if cfg.atmosphere_mode.allows_modulation() {
                // Built-in adaptive engine — only when atmosphere is
                // explicitly enabled (controlled-live + adaptive regime).
                // When atmosphere_mode = Disabled (the default), this branch
                // is skipped and the rain keeps the user's startup color.
                if let Some(target) = crate::atmosphere_adaptive::current_color_target() {
                    if last_adaptive_color != Some(target) {
                        if let Ok(scheme) = crate::cli::parse_color_scheme(target) {
                            cloud.set_color_scheme(scheme);
                            last_adaptive_color = Some(target);
                            last_color_scheme = scheme;
                        }
                    }
                }
            }
        }

        // Apply pending Cloud rebuild (full rebuild, not delta).
        // This swaps Cloud + Frame + color cache between frames — no mid-frame
        // visual glitch. Rain streams reset (expected for color/charset changes).
        if let Some(new_cfg_map) = pending_config.take() {
            let new_cfg = crate::live_config::rebuild_cloud_config(&base_cfg, &new_cfg_map);
            let density = effective_density(
                new_cfg.base_density,
                w,
                h,
                new_cfg.fullwidth,
                new_cfg.density_auto,
            );
            cloud = new_cfg.create_cloud(density);
            cloud.reset(w, h);
            cloud.enable_events();
            cloud.set_component_timing(new_cfg.perf_stats);
            // Rebuild color cache + frame for new palette.
            term.set_color_cache(ColorCache::new(&cloud.palette));
            frame = Frame::new(w, h, cloud.palette.bg);
            // Update charset_preset for runtime cycling.
            charset_preset = new_cfg.charset_preset.clone();
            // Re-parse custom time map from the new config (live reload
            // may have added/changed/removed adaptive-custom entries).
            custom_time_map = match crate::atmosphere_custom::parse_custom_time_map(&new_cfg_map) {
                Ok(map) if !map.is_empty() => Some(map),
                Ok(_) => None,
                Err(e) => {
                    eprintln!("[adaptive-custom] parse error after live reload: {e}. Using default adaptive.");
                    None
                }
            };
        }

        // Adaptive throttling: detect idle state (no input for IDLE_THRESHOLD_SECS)
        // and reduce effective FPS to conserve CPU/battery. Any input event
        // instantly restores full performance.
        //
        // P1: Phase predictor can proactively suggest idle before the 30s
        // threshold if it has learned the daily pattern.
        // P2: Resync interval adapts based on sustained idle duration.
        let loop_now = Instant::now();
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
            if cfg.mouse && term.enable_mouse_capture().is_ok() {
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
            while Terminal::poll_event(Duration::from_millis(0))? {
                let ev = Terminal::read_event()?;
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
                        // 'i'/'I' is used instead of '?' because Android soft
                        // keyboards send simple printable letters reliably,
                        // while '?' may arrive with unexpected modifier bits
                        // or as a different keycode entirely, causing the
                        // keypress to fall through to the screensaver exit
                        // path. 'I' is also accepted for keyboards where the
                        // Shift state is sticky or set unexpectedly.
                        //
                        // When toggling OFF, we MUST call force_draw_everything()
                        // to clear stale HUD cells from the frame buffer. The
                        // rain uses diff-based rendering (frame.set, not
                        // set_force), so cells that the rain doesn't actively
                        // write this frame keep their previous content —
                        // including the HUD text + black bg cells. Without
                        // force_draw, this leaves visible "HUD residue" in
                        // regions with no active rain this frame. force_draw
                        // triggers frame.clear_with_bg() on the next rain
                        // update, wiping the stale HUD cells cleanly.
                        if matches!(
                            (k.code, k.modifiers),
                            (KeyCode::Char('i'), _) | (KeyCode::Char('I'), _)
                        ) {
                            let now_visible = hud_state.toggle();
                            if !now_visible {
                                cloud.force_draw_everything();
                            }
                            let _ = register_activity(
                                &mut last_input_time,
                                &mut last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            continue;
                        }

                        // H or h: toggle HUD position. Lowercase 'h' accepted
                        // for Android soft keyboards where Shift may not work.
                        if matches!(
                            (k.code, k.modifiers),
                            (KeyCode::Char('H'), _) | (KeyCode::Char('h'), _)
                        ) {
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
                        // keys (x, s, c, g, a, p, m, Space, Up/Down, etc.)
                        // work even in --screensaver mode.
                        let redraw_needed = handle_keybinding(
                            &mut cloud,
                            &mut frame,
                            &k,
                            &mut charset_preset,
                            &mut scene_name,
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
                            // - Recognized interactive keys (c/s/x/g/a/p/m/i/h,
                            //   Space, Up/Down, 0-9, etc.): process and continue.
                            //   The user can still cycle colors, toggle HUD, etc.
                            //   while the screensaver is active.
                            // - Unrecognized keys (B/b, z, F1-F12, Home/End,
                            //   PageUp/Down, Esc, Ctrl+C, etc.): SILENTLY IGNORED.
                            //   They do NOT exit the screensaver and do NOT cause
                            //   any visual glitch. The user must press 'q' to quit.
                            //   This matches the "only q quits" policy enforced
                            //   in normal (non-screensaver) mode — consistency
                            //   is the world-class invariant.
                            //
                            // Mouse click (if --mouse enabled) still exits —
                            // classic screensaver convention. See Event::Mouse
                            // handler below.
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

                        // Track runtime changes silently (no eprintln
                        // during rain — causes screen flicker). The
                        // final summary is printed after exit by main.rs.
                        if verbose {
                            let cur_color = cloud.color_scheme();
                            if cur_color != last_color_scheme {
                                last_color_scheme = cur_color;
                            }
                            if scene_name != last_scene_name {
                                last_scene_name = scene_name.clone();
                            }
                            if charset_preset != last_charset {
                                last_charset = charset_preset.clone();
                            }
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
                    Event::Mouse(m) if cfg.mouse => {
                        // Screensaver mouse-click exit: classic screensaver
                        // behavior. Any mouse button click exits the
                        // screensaver. Mouse movement alone does NOT exit
                        // (too sensitive — accidental trackpad jitter would
                        // kick the user out). This matches macOS/iOS/Linux
                        // screensaver convention: click or key to dismiss.
                        if cfg.screensaver && matches!(m.kind, MouseEventKind::Down(_)) {
                            cloud.raining = false;
                            break;
                        }
                        // Mouse interaction resets idle timer.
                        let activity_time = Instant::now();
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
            if !cloud.raining {
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

            // Spin-sleep hybrid: use poll_event for the bulk of the wait
            // (which also processes input events), then spin-wait the final
            // ~500μs for sub-millisecond deadline accuracy. This eliminates
            // OS scheduling jitter from the frame cadence.
            let spin_budget = Duration::from_micros(500);
            if timeout > spin_budget {
                let _ = Terminal::poll_event(timeout - spin_budget)?;
                // Spin-wait the remaining time for precise deadline alignment.
                // The spin is capped at 1ms internally to handle edge cases.
                spin_wait(next_frame);
            } else {
                // Already close to deadline (< 500μs away): spin-wait to hit
                // it precisely, then drain any events that arrived.
                spin_wait(next_frame);
                let _ = Terminal::poll_event(Duration::from_millis(0))?;
            }
        }

        if !cloud.raining {
            break;
        }

        if let Some((nw, nh)) = pending_resize {
            cloud.reset(nw, nh);
            frame = Frame::new(nw, nh, cloud.palette.bg);
            if cfg.density_auto {
                cloud.set_droplet_density(effective_density(
                    cfg.base_density,
                    nw,
                    nh,
                    cfg.fullwidth,
                    true,
                ));
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

        cloud.set_perf_pressure(perf_pressure);
        let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
        let sim_factor = (1.0 - (perf_pressure as f64) * SIM_PRESSURE_SCALE_FACTOR).clamp(0.3, 1.0);
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
        // Pass idle state to Cloud for Weather Director tick
        cloud.is_idle = is_idle;
        // P1: call rain_at directly with work_start instead of cloud.rain()
        // (which calls Instant::now() internally). Saves 1 Instant::now()
        // per frame (~20ns).
        cloud.rain_at(&mut frame, work_start);

        // Write HUD into the frame buffer BEFORE term.draw() so it's
        // part of the same flush — eliminates fullscreen flicker.
        hud_state.write_to_frame(&mut frame, cloud.cols);

        // Cache dirty checks once per frame to avoid redundant method calls.
        let is_dirty_all = frame.is_dirty_all();
        let dirty_len = frame.dirty_indices().len();
        let did_draw = is_dirty_all || dirty_len > 0;
        if did_draw {
            term.draw(&mut frame)?;
        }
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);

        let work_s = work_start.elapsed().as_secs_f32();

        // Live HUD: push frame time, sample RSS, recompute metrics.
        // All methods short-circuit when HUD is off (zero cost).
        // write_to_frame() above handles the actual display.
        hud_state.push_frame_time(work_s as f64 * 1000.0);
        hud_state.maybe_sample_rss();
        hud_state.update_metrics(cloud.hud_colors());

        let overshoot = ((work_s / frame_period_s) - 1.0).clamp(0.0, 2.0);
        if overshoot > 0.0 {
            perf_pressure = (perf_pressure + (overshoot * PERF_PRESSURE_INCREMENT)).min(1.0);
        } else {
            perf_pressure = (perf_pressure - PERF_PRESSURE_DECAY).max(0.0);
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
                    let rss = read_self_rss_kb();
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
                        let cur = read_self_voluntary_ctxt();
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

    if cfg.perf_stats {
        // Capture encoding stats BEFORE dropping the terminal — the stats
        // live inside the Terminal/ColorCache and would be lost on drop.
        let (enc_bytes, enc_flushes, sgr_hits, sgr_misses) = term.encoding_stats();
        drop(term);
        let elapsed = start_time.elapsed();
        let elapsed_s = elapsed.as_secs_f64().max(0.000_001);

        let frames = perf_frames.max(1);
        let avg_work_ms = (perf_work_sum_s / frames as f64) * 1000.0;
        let avg_pressure = perf_pressure_sum / frames as f64;
        let avg_fps = (perf_frames as f64) / elapsed_s;
        let drawn_ratio = (perf_drawn_frames as f64) / (perf_frames as f64).max(1.0);
        let overshoot_ratio =
            (perf_overshoot_frames as f64) / (perf_frames as f64).max(1.0) * 100.0;
        let pressure_class = if avg_pressure < 0.05 {
            "low"
        } else if avg_pressure < 0.3 {
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
            let s = r.section("PRESSURE");
            s.field("avg", &format!("{:.3}", avg_pressure));
            s.field("peak", &format!("{:.3}", perf_pressure_max));
            s.field("classification", pressure_class);
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

        r.print();
    }

    // Store final runtime state for post-exit verbose summary.
    let final_color_name = format!("{:?}", cloud.color_scheme());
    super::set_final_state(&final_color_name, &scene_name, &charset_preset);

    Ok(())
}

/// Read this process's current RSS from `/proc/self/status` (Linux only).
#[cfg(target_os = "linux")]
fn read_self_rss_kb() -> u64 {
    // Read VmRSS from /proc/self/status. Lightweight: single line match.
    // Falls back to 0 if unavailable (shouldn't happen on Linux).
    use std::io::BufRead;
    let f = match std::fs::File::open("/proc/self/status") {
        Ok(f) => f,
        Err(_) => return 0,
    };
    for l in std::io::BufReader::new(f).lines().map_while(Result::ok) {
        if l.starts_with("VmRSS:") {
            // Format: "VmRSS:    2800 kB"
            return l
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

/// Read voluntary context switches from `/proc/self/stat` (Linux only).
#[cfg(target_os = "linux")]
fn read_self_voluntary_ctxt() -> u64 {
    // /proc/self/stat field 20 = voluntary_ctxt_switches
    let stat = match std::fs::read_to_string("/proc/self/stat") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    // Fields are space-separated; field 20 (1-indexed) is voluntary_ctxt_switches.
    // But comm (field 2) may contain spaces inside parens, so split after the closing paren.
    let after_paren = match stat.rfind(')') {
        Some(idx) => &stat[idx + 1..],
        None => return 0,
    };
    let fields: Vec<&str> = after_paren.split_whitespace().collect();
    // After ')', field indices shift: field 3 in the original = fields[0] here.
    // voluntary_ctxt_switches is field 20 (1-indexed), so fields[17] (0-indexed).
    fields.get(17).and_then(|s| s.parse().ok()).unwrap_or(0)
}
