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

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};

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
    let (w, h) = term.size()?;

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);
    // Enable atmospheric events for interactive mode (ghosts, etc.).
    cloud.enable_events();

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

    // Live HUD overlay state — toggled with '?'. When visible, renders a
    // compact FPS/p99/RSS overlay in the top-right corner at 4 Hz.
    // Zero cost when off (all methods short-circuit on visible==false).
    let mut hud_state: HudState = HudState::new();

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

    while cloud.raining {
        // Check for graceful shutdown request from signal handler.
        // This allows clean exit via Terminal::drop() instead of racing
        // on stdout with the signal handler thread.
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            cloud.raining = false;
            break;
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

        if end_time.is_some_and(|end| Instant::now() >= end) {
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
                        // Clamp to safe bounds before storing — raw crossterm
                        // values can be degenerate (0×0, 65535×65535) during
                        // window transitions and would panic in Uniform::new
                        // or cause massive allocations inside cloud.reset().
                        let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
                        let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
                        pending_resize = Some((cw, ch));
                        last_resize_event = Some(Instant::now());
                    }
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
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

                        // HUD toggle: check BEFORE screensaver exit so '?'
                        // doesn't cause self-exit on Android/Termux where
                        // the soft keyboard may send unexpected key events.
                        // On Android, '?' may arrive as '/', '/', or '?'
                        // with various modifier states — match all variants.
                        if matches!(
                            (k.code, k.modifiers),
                            (KeyCode::Char('?'), _)
                                | (KeyCode::Char('/'), KeyModifiers::SHIFT)
                                | (KeyCode::Char('/'), _)
                        ) {
                            hud_state.toggle();
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
                        if cfg.screensaver {
                            cloud.raining = false;
                            break;
                        }

                        if handle_keybinding(
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
                        ) {
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
                    Event::Mouse(m) if cfg.mouse => {
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
        cloud.rain(&mut frame);

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
                let now = Instant::now();
                let elapsed = now
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
                    last_ctxt_sample = now;
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

        r.print();
    }

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
