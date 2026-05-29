// Copyright (c) 2026 rezky_nightky

//! Interactive runtime loop for Cosmostrix.
//!
//! Manages the main event loop, frame pacing, signal handling, keyboard
//! input dispatch, performance tracking, and the watchdog thread.
//!
//! ## Frame Pacing
//!
//! The pacing system uses a spin-sleep hybrid approach: the bulk of each
//! frame's idle time is spent in `poll_event()` (which also processes input),
//! while the final ~500μs uses a busy-wait spin loop for sub-millisecond
//! deadline accuracy. This eliminates OS scheduling jitter from the frame
//! cadence.
//!
//! When a frame overshoots its deadline, the next frame is scheduled from
//! `now + period` rather than `next + period`, preventing cascading stutter
//! from a single late frame.
//!
//! Under sustained performance pressure, the simulation time budget is
//! adaptively reduced (down to 30% of nominal) to prevent frame queue
//! buildup. This trades visual complexity for temporal consistency.
//!
//! ## Signal Handling
//!
//! Unix signals (SIGINT, SIGTERM, SIGHUP, SIGTSTP, SIGCONT) are handled via
//! a dedicated signal thread that sets an atomic `GRACEFUL_SHUTDOWN` flag.
//! The main loop checks this flag each iteration and exits cleanly, allowing
//! `Terminal::drop()` to restore the terminal without racing on stdout.
//! A fallback force-restore fires after 1 second if the main loop is stuck.
//!
//! ## Watchdog
//!
//! A background watchdog thread monitors a global frame counter. If no frames
//! are produced for 10+ seconds, it restores the terminal and exits —
//! protecting against infinite loops that would leave the TTY in a broken state.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::Arc;

use crossterm::event::{Event, KeyEventKind, MouseEventKind};

#[cfg(unix)]
use signal_hook::consts::{SIGCONT, SIGHUP, SIGINT, SIGSTOP, SIGTERM, SIGTSTP};
#[cfg(unix)]
use signal_hook::iterator::Signals;
#[cfg(unix)]
use signal_hook::low_level;

use crate::charset::{build_chars, charset_from_str};
use crate::cloud::Cloud;
use crate::constants::*;
use crate::frame::Frame;
use crate::report::Report;
use crate::runtime::{ColorScheme, ShadingMode};
use crate::terminal::{restore_terminal_best_effort, Terminal};

use super::{cycle_charset_preset, cycle_color_scheme, effective_density, CloudConfig};

/// Spin-wait until `deadline` is reached, capped at 1ms to avoid wasting CPU
/// on pathological cases (clock jumps, VM pauses).
///
/// Used for the final sub-millisecond portion of frame pacing where OS sleep
/// granularity (~0.5–2ms) is insufficient. The busy-wait ensures we hit the
/// frame deadline with microsecond precision rather than millisecond.
#[inline]
fn spin_wait(deadline: Instant) {
    let spin_limit = Duration::from_micros(1000);
    let spin_start = Instant::now();
    while Instant::now() < deadline && spin_start.elapsed() < spin_limit {
        std::hint::spin_loop();
    }
}

#[inline]
fn is_runtime_idle(last_input_time: Instant, now: Instant) -> bool {
    now.saturating_duration_since(last_input_time).as_secs_f64() >= IDLE_THRESHOLD_SECS
}

#[inline]
fn idle_resync_due(is_idle: bool, last_resync_time: Instant, now: Instant) -> bool {
    is_idle
        && now
            .saturating_duration_since(last_resync_time)
            .as_secs_f64()
            >= IDLE_REDRAW_RESYNC_INTERVAL_SECS
}

#[inline]
fn register_activity(
    last_input_time: &mut Instant,
    last_resync_time: &mut Instant,
    now: Instant,
    was_idle: bool,
    force_resync: bool,
) -> bool {
    *last_input_time = now;
    if was_idle || force_resync {
        *last_resync_time = now;
        true
    } else {
        false
    }
}

const PASTE_BURST_SUPPRESS_MS: u64 = 50;

#[derive(Default)]
struct PasteBurstGuard {
    suppress_until: Option<Instant>,
}

impl PasteBurstGuard {
    fn ignore_plain_key(
        &mut self,
        key: &crossterm::event::KeyEvent,
        now: Instant,
        queued_event_ready: bool,
    ) -> bool {
        if !is_plain_printable_key(key) {
            return false;
        }

        if self.suppress_until.is_some_and(|until| now <= until) || queued_event_ready {
            self.suppress_until = Some(now + Duration::from_millis(PASTE_BURST_SUPPRESS_MS));
            true
        } else {
            false
        }
    }

    fn note_bracketed_paste(&mut self, now: Instant) {
        self.suppress_until = Some(now + Duration::from_millis(PASTE_BURST_SUPPRESS_MS));
    }
}

fn is_plain_printable_key(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    matches!(key.code, KeyCode::Char(_))
        && (key.modifiers.is_empty()
            || key.modifiers == KeyModifiers::SHIFT
            || key.modifiers == KeyModifiers::NONE)
}

/// Rolling frame time tracker: allocation-free fixed-size ring buffer.
///
/// Tracks the last 60 frame times in milliseconds. Only used when
/// `--perf-stats` is enabled; otherwise has zero cost.
struct FrameTimeTracker {
    times: [f64; 60],
    index: usize,
    count: usize,
}

impl FrameTimeTracker {
    const fn new() -> Self {
        Self {
            times: [0.0; 60],
            index: 0,
            count: 0,
        }
    }

    fn push(&mut self, ms: f64) {
        self.times[self.index] = ms;
        self.index = (self.index + 1) % 60;
        if self.count < 60 {
            self.count += 1;
        }
    }

    fn rolling_avg(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: f64 = self.times[..self.count].iter().sum();
        sum / self.count as f64
    }

    fn std_dev(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let mean = self.rolling_avg();
        let variance: f64 = self.times[..self.count]
            .iter()
            .map(|&t| (t - mean) * (t - mean))
            .sum::<f64>()
            / (self.count - 1) as f64;
        variance.sqrt()
    }

    fn jitter_classification(&self) -> &'static str {
        let sd = self.std_dev();
        if sd < 0.5 {
            "low"
        } else if sd < 2.0 {
            "medium"
        } else {
            "high"
        }
    }
}

/// Global flag set when mouse capture was successfully enabled.
/// Signal handlers check this to decide whether DisableMouseCapture is needed.
static MOUSE_CAPTURE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Clear the global `MOUSE_CAPTURE_ACTIVE` flag. Called by `Terminal` when
/// mouse capture is disabled (e.g. on drop) so that signal handlers don't
/// attempt a redundant `DisableMouseCapture` on an already-restored terminal.
pub fn clear_mouse_capture_flag() {
    MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
}

/// Global frame counter for the watchdog thread (AtomicU64 for lock-free watchdog).
pub static FRAME_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Global shutdown flag. Set to `true` when the main loop exits so the
/// watchdog thread can terminate instead of running forever.
static SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Graceful shutdown request flag. Set by signal handler threads instead of
/// calling `restore_terminal_best_effort()` + `process::exit()` directly.
/// The main loop checks this flag each iteration and exits cleanly, allowing
/// `Terminal::drop()` to restore the terminal without racing on stdout.
/// Falls back to direct restore after a timeout for stuck-loop protection.
static GRACEFUL_SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    super::spawn_kill9_terminal_guard();

    #[cfg(unix)]
    let term_reinit: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        if let Ok(mut signals) = Signals::new([SIGINT, SIGTERM, SIGHUP]) {
            std::thread::spawn(move || {
                if let Some(sig) = signals.forever().next() {
                    // Request graceful shutdown via AtomicBool instead of
                    // directly writing ANSI restore sequences to stdout.
                    // This avoids racing with the main thread on the same fd.
                    GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
                    // Wait briefly for the main loop to notice and exit.
                    // If the main loop is stuck (e.g., infinite loop), the
                    // watchdog thread will handle the hard restore.
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    // Fallback: if still alive after timeout, force restore.
                    if !SHUTDOWN.load(Ordering::Acquire) {
                        if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                            use crossterm::ExecutableCommand;
                            let _ =
                                std::io::stdout().execute(crossterm::event::DisableMouseCapture);
                        }
                        restore_terminal_best_effort();
                        std::process::exit(128 + sig);
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
        if let Err(e) = ctrlc::set_handler(|| {
            GRACEFUL_SHUTDOWN.store(true, Ordering::Release);
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

    let mut term = Terminal::new()?;
    // Mouse reporting is opt-in because abrupt process death can leave some
    // terminals echoing raw mouse escape sequences until they are reset.
    if cfg.mouse && term.enable_mouse_capture().is_ok() {
        MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
    }
    let (w, h) = term.size()?;

    let density = effective_density(cfg.base_density, w, h, cfg.fullwidth, cfg.density_auto);

    let mut cloud = cfg.create_cloud(density);
    cloud.reset(w, h);

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

    let mut charset_preset = cfg.charset_preset.clone();
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
        let loop_now = Instant::now();
        let is_idle = is_runtime_idle(last_input_time, loop_now);
        if idle_resync_due(is_idle, last_resync_time, loop_now) {
            cloud.force_draw_everything();
            last_resync_time = loop_now;
            next_frame = loop_now;
        }

        if end_time.is_some_and(|end| Instant::now() >= end) {
            cloud.raining = false;
            break;
        }
        let mut pending_resize: Option<(u16, u16)> = None;

        #[cfg(unix)]
        if term_reinit.swap(false, Ordering::Acquire) {
            drop(term);
            term = Terminal::new()?;
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
                        let queued_event_ready = Terminal::poll_event(Duration::from_millis(0))?;
                        if paste_guard.ignore_plain_key(&k, activity_time, queued_event_ready) {
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
        let active_is_idle = is_runtime_idle(last_input_time, Instant::now());
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
        cloud.rain(&mut frame);
        // Cache dirty checks once per frame to avoid redundant method calls.
        let is_dirty_all = frame.is_dirty_all();
        let dirty_len = frame.dirty_indices().len();
        let did_draw = is_dirty_all || dirty_len > 0;
        if did_draw {
            term.draw(&mut frame)?;
        }
        FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);

        let work_s = work_start.elapsed().as_secs_f32();
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
                &format!("{:.3}ms", frame_time_tracker.rolling_avg()),
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

        r.print();
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn handle_keybinding(
    cloud: &mut Cloud,
    frame: &mut Frame,
    k: &crossterm::event::KeyEvent,
    charset_preset: &mut String,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    _cfg: &CloudConfig,
    #[cfg(unix)] term_reinit: &Arc<AtomicBool>,
) -> bool {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) => cloud.raining = false,
        (KeyCode::Char('q'), _) => cloud.raining = false,
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            #[cfg(unix)]
            {
                // Disable mouse capture before suspending so the terminal
                // is not left with mouse reporting active while cosmostrix
                // is in the background.
                if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                    use crossterm::ExecutableCommand;
                    let _ = std::io::stdout().execute(crossterm::event::DisableMouseCapture);
                    MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
                }
                restore_terminal_best_effort();
                term_reinit.store(true, Ordering::Release);
                let _ = low_level::raise(SIGSTOP);
            }
        }
        (KeyCode::Char(' '), _) => {
            cloud.reset(frame.width, frame.height);
            cloud.force_draw_everything();
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            cloud.raining = false;
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            let next = cycle_color_scheme(cloud.color_scheme(), 1);
            cloud.set_color_scheme(next);
        }
        (KeyCode::Char('C'), _) => {
            let prev = cycle_color_scheme(cloud.color_scheme(), -1);
            cloud.set_color_scheme(prev);
        }
        (KeyCode::Char('s'), _) => {
            let next = cycle_charset_preset(charset_preset, 1);
            *charset_preset = next.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
        }
        (KeyCode::Char('S'), _) => {
            let prev = cycle_charset_preset(charset_preset, -1);
            *charset_preset = prev.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
        }
        (KeyCode::Char('a'), _) => {
            cloud.set_async(!cloud.async_mode);
        }
        (KeyCode::Char('g'), _) => {
            cloud.set_glitchy(!cloud.glitchy);
        }
        (KeyCode::Char('p'), _) => {
            return cloud.toggle_pause();
        }
        (KeyCode::Char('m'), _) => {
            cloud.cycle_profile();
        }
        (KeyCode::Up, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 0.5 {
                cps *= 2.0;
            } else {
                cps += 1.0;
            }
            cloud.set_chars_per_sec(cps.min(1000.0));
        }
        (KeyCode::Down, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 1.0 {
                cps /= 2.0;
            } else {
                cps -= 1.0;
            }
            cloud.set_chars_per_sec(cps.max(0.001));
        }
        (KeyCode::Left, _) if cloud.glitchy => {
            let gp = (cloud.glitch_pct - GLITCH_PCT_STEP).max(0.0);
            cloud.set_glitch_pct(gp);
        }
        (KeyCode::Right, _) if cloud.glitchy => {
            let gp = (cloud.glitch_pct + GLITCH_PCT_STEP).min(1.0);
            cloud.set_glitch_pct(gp);
        }
        (KeyCode::Tab, _) => {
            let sm = if cloud.shading_distance {
                ShadingMode::Random
            } else {
                ShadingMode::DistanceFromHead
            };
            cloud.set_shading_mode(sm);
        }
        (KeyCode::Char('-'), _) | (KeyCode::Char('['), _) | (KeyCode::Char('_'), _) => {
            let d = (cloud.droplet_density - DENSITY_STEP).max(0.01);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char('+'), _)
        | (KeyCode::Char('='), KeyModifiers::SHIFT)
        | (KeyCode::Char(']'), _) => {
            let d = (cloud.droplet_density + DENSITY_STEP).min(5.0);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char('1'), _) => cloud.set_color_scheme(ColorScheme::Green),
        (KeyCode::Char('2'), _) => cloud.set_color_scheme(ColorScheme::Green2),
        (KeyCode::Char('3'), _) => cloud.set_color_scheme(ColorScheme::Green3),
        (KeyCode::Char('4'), _) => cloud.set_color_scheme(ColorScheme::Gold),
        (KeyCode::Char('5'), _) => cloud.set_color_scheme(ColorScheme::Neon),
        (KeyCode::Char('6'), _) => cloud.set_color_scheme(ColorScheme::Red),
        (KeyCode::Char('7'), _) => cloud.set_color_scheme(ColorScheme::Blue),
        (KeyCode::Char('8'), _) => cloud.set_color_scheme(ColorScheme::Cyan),
        (KeyCode::Char('9'), _) => cloud.set_color_scheme(ColorScheme::Purple),
        (KeyCode::Char('0'), _) => cloud.set_color_scheme(ColorScheme::Gray),
        (KeyCode::Char('!'), _) => cloud.set_color_scheme(ColorScheme::Rainbow),
        (KeyCode::Char('@'), _) => cloud.set_color_scheme(ColorScheme::Yellow),
        (KeyCode::Char('#'), _) => cloud.set_color_scheme(ColorScheme::Orange),
        (KeyCode::Char('$'), _) => cloud.set_color_scheme(ColorScheme::Fire),
        (KeyCode::Char('%'), _) => cloud.set_color_scheme(ColorScheme::Vaporwave),
        _ => {}
    }

    false
}

fn spawn_watchdog() {
    let counter = &FRAME_COUNTER as &std::sync::atomic::AtomicU64;
    let shutdown = &SHUTDOWN as &std::sync::atomic::AtomicBool;
    let mut stuck_count: u32 = 0;
    std::thread::spawn(move || loop {
        // Check shutdown flag before each sleep cycle
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let current = counter.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        let next = counter.load(Ordering::Relaxed);
        if current == next {
            stuck_count += 1;
            if stuck_count >= 2 {
                // Main loop has been stuck for multiple check intervals.
                // Attempt to restore the terminal so the user isn't left
                // with a broken shell, then exit.
                restore_terminal_best_effort();
                eprintln!(
                    "[watchdog] main loop stuck for {}s — restoring terminal and exiting",
                    WATCHDOG_INTERVAL_SECS * 2 * stuck_count as u64
                );
                std::process::exit(1);
            }
            eprintln!(
                "[watchdog] main loop appears stuck (frame counter unchanged for {}s)",
                WATCHDOG_INTERVAL_SECS * 2
            );
        } else {
            stuck_count = 0;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    #[test]
    fn idle_resync_uses_wall_clock_time() {
        let start = Instant::now();
        let due = start + Duration::from_secs_f64(IDLE_REDRAW_RESYNC_INTERVAL_SECS + 0.1);
        let early = start + Duration::from_secs_f64(IDLE_REDRAW_RESYNC_INTERVAL_SECS - 0.1);

        assert!(!idle_resync_due(true, start, early));
        assert!(idle_resync_due(true, start, due));
        assert!(!idle_resync_due(false, start, due));
    }

    #[test]
    fn idle_to_active_activity_schedules_resync() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(60);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            true,
            false,
        ));
        assert_eq!(last_input_time, activity_time);
        assert_eq!(last_resync_time, activity_time);
    }

    #[test]
    fn active_mouse_activity_does_not_force_resync_every_frame() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(!register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            false,
            false,
        ));
        assert_eq!(last_input_time, activity_time);
        assert_eq!(last_resync_time, start);
    }

    #[test]
    fn focus_activity_can_force_resync_while_active() {
        let start = Instant::now();
        let activity_time = start + Duration::from_secs(1);
        let mut last_input_time = start;
        let mut last_resync_time = start;

        assert!(register_activity(
            &mut last_input_time,
            &mut last_resync_time,
            activity_time,
            false,
            true,
        ));
        assert_eq!(last_resync_time, activity_time);
    }

    #[test]
    fn idle_state_stays_idle_until_activity_resets_timer() {
        let start = Instant::now();
        let idle_now = start + Duration::from_secs_f64(IDLE_THRESHOLD_SECS + 0.1);
        let later_idle_now = idle_now + Duration::from_secs(5);
        let active_now = start + Duration::from_secs(1);

        assert!(!is_runtime_idle(start, active_now));
        assert!(is_runtime_idle(start, idle_now));
        assert!(is_runtime_idle(start, later_idle_now));
    }

    #[test]
    fn plain_shortcut_key_is_not_ignored_without_burst() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        assert!(!guard.ignore_plain_key(&key('p'), now, false));
    }

    #[test]
    fn paste_burst_ignores_shortcut_letters() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        assert!(guard.ignore_plain_key(&key('p'), now, true));
        assert!(guard.ignore_plain_key(&key('c'), now + Duration::from_millis(1), false));
        assert!(guard.ignore_plain_key(&key('s'), now + Duration::from_millis(2), false));
    }

    #[test]
    fn paste_burst_suppression_expires() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        assert!(guard.ignore_plain_key(&key('p'), now, true));
        assert!(!guard.ignore_plain_key(
            &key('p'),
            now + Duration::from_millis(PASTE_BURST_SUPPRESS_MS + 1),
            false,
        ));
    }

    #[test]
    fn bracketed_paste_starts_printable_suppression_window() {
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        guard.note_bracketed_paste(now);

        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1), false));
    }

    #[test]
    fn paste_suppression_does_not_trigger_shortcut_actions() {
        // Verify that paste events go through the Paste branch, not Key,
        // so they never trigger 'c', 's', 'p', or other shortcuts.
        let now = Instant::now();
        let mut guard = PasteBurstGuard::default();

        // Simulate a bracketed paste event
        guard.note_bracketed_paste(now);

        // Printable keys during the suppression window should be silently
        // ignored — they must not reach the keybinding handler.
        assert!(guard.ignore_plain_key(&key('c'), now + Duration::from_millis(1), false));
        assert!(guard.ignore_plain_key(&key('s'), now + Duration::from_millis(1), false));
        assert!(guard.ignore_plain_key(&key('p'), now + Duration::from_millis(1), false));
    }
}
