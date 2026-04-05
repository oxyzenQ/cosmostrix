// Copyright (c) 2026 rezky_nightky

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyEventKind};

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
use crate::runtime::{ColorScheme, ShadingMode};
use crate::terminal::{restore_terminal_best_effort, Terminal};

use super::{cycle_charset_preset, cycle_color_scheme, effective_density, CloudConfig};

/// Global frame counter for the watchdog thread (AtomicU64 for lock-free watchdog).
pub static FRAME_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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
                    restore_terminal_best_effort();
                    std::process::exit(128 + sig);
                }
            });
        }

        let term_reinit = term_reinit.clone();
        if let Ok(mut signals) = Signals::new([SIGTSTP, SIGCONT]) {
            std::thread::spawn(move || {
                for sig in signals.forever() {
                    match sig {
                        SIGTSTP => {
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
            restore_terminal_best_effort();
            std::process::exit(130);
        }) {
            eprintln!("failed to install Ctrl-C handler: {}", e);
        }
    }

    // Spawn watchdog thread
    spawn_watchdog();

    let mut term = Terminal::new()?;
    // Enable mouse capture (non-fatal if terminal doesn't support it)
    let _ = term.enable_mouse_capture();
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
    let mut perf_work_max_s: f32 = 0.0;
    let mut perf_pressure_sum: f64 = 0.0;
    let mut perf_pressure_max: f32 = 0.0;
    let mut perf_overshoot_frames: u64 = 0;

    let mut charset_preset = cfg.charset_preset.clone();
    let user_ranges = cfg.user_ranges.clone();
    let def_ascii = cfg.def_ascii;

    while cloud.raining {
        let frame_period = if cloud.pause {
            pause_period
        } else {
            target_period
        };
        let frame_period_s = frame_period.as_secs_f32().max(0.000_001);

        if end_time.is_some_and(|end| Instant::now() >= end) {
            cloud.raining = false;
            break;
        }
        let mut pending_resize: Option<(u16, u16)> = None;

        #[cfg(unix)]
        if term_reinit.swap(false, Ordering::Acquire) {
            drop(term);
            term = Terminal::new()?;
            let (nw, nh) = term.size()?;
            pending_resize = Some((nw, nh));
            cloud.force_draw_everything();
            next_frame = Instant::now();
        }

        loop {
            while Terminal::poll_event(Duration::from_millis(0))? {
                let ev = Terminal::read_event()?;
                match ev {
                    Event::Resize(nw, nh) => {
                        pending_resize = Some((nw, nh));
                    }
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if cfg.screensaver {
                            cloud.raining = false;
                            break;
                        }

                        handle_keybinding(
                            &mut cloud,
                            &mut frame,
                            &k,
                            &mut charset_preset,
                            &user_ranges,
                            def_ascii,
                            cfg,
                            #[cfg(unix)]
                            &term_reinit,
                        );
                    }
                    Event::Mouse(m) => {
                        cloud.set_mouse_position(m.column, m.row);
                    }
                    _ => {}
                }
            }

            if !cloud.raining || pending_resize.is_some() {
                break;
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
            let _ = Terminal::poll_event(timeout)?;
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
        }

        cloud.set_perf_pressure(perf_pressure);
        let sim_base_s = frame_period.as_secs_f64() * SIM_BASE_MULTIPLIER;
        let sim_factor = (1.0 - (perf_pressure as f64) * SIM_PRESSURE_SCALE_FACTOR).clamp(0.3, 1.0);
        let sim_min_s = (frame_period.as_secs_f64() * SIM_MIN_FRACTION).max(0.001);
        let sim_max_s = sim_base_s.min(SIM_MAX_CAP_SECS);
        let sim_cap_s = (sim_base_s * sim_factor).clamp(sim_min_s, sim_max_s);
        cloud.set_max_sim_delta(Duration::from_secs_f64(sim_cap_s));

        let work_start = Instant::now();
        cloud.rain(&mut frame);
        let did_draw = frame.is_dirty_all() || !frame.dirty_indices().is_empty();
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
            }
            perf_work_sum_s += work_s as f64;
            perf_work_max_s = perf_work_max_s.max(work_s);
            perf_pressure_sum += perf_pressure as f64;
            perf_pressure_max = perf_pressure_max.max(perf_pressure);
            if overshoot > 0.0 {
                perf_overshoot_frames = perf_overshoot_frames.saturating_add(1);
            }
        }

        let now = Instant::now();
        next_frame = next_frame.checked_add(frame_period).unwrap_or(now);
        if now > next_frame {
            next_frame = now.checked_add(frame_period).unwrap_or(now);
        }
    }

    if cfg.perf_stats {
        drop(term);
        let elapsed = start_time.elapsed();
        let elapsed_s = elapsed.as_secs_f64().max(0.000_001);

        let frames = perf_frames.max(1);
        let avg_work_ms = (perf_work_sum_s / frames as f64) * 1000.0;
        let avg_pressure = perf_pressure_sum / frames as f64;
        let avg_fps = (perf_frames as f64) / elapsed_s;
        let drawn_ratio = (perf_drawn_frames as f64) / (perf_frames as f64).max(1.0);

        println!("PERF STATS:");
        println!("  elapsed_s: {:.3}", elapsed_s);
        println!("  target_fps: {:.3}", cfg.target_fps);
        println!("  avg_fps: {:.3}", avg_fps);
        println!("  frames: {}", perf_frames);
        println!(
            "  drawn_frames: {} ({:.1}%)",
            perf_drawn_frames,
            drawn_ratio * 100.0
        );
        println!("  avg_work_ms: {:.3}", avg_work_ms);
        println!("  max_work_ms: {:.3}", perf_work_max_s as f64 * 1000.0);
        println!(
            "  overshoot_frames: {} ({:.1}%)",
            perf_overshoot_frames,
            (perf_overshoot_frames as f64) / (perf_frames as f64).max(1.0) * 100.0
        );
        println!("  avg_perf_pressure: {:.3}", avg_pressure);
        println!("  max_perf_pressure: {:.3}", perf_pressure_max);
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
) {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) => cloud.raining = false,
        (KeyCode::Char('q'), _) => cloud.raining = false,
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            #[cfg(unix)]
            {
                restore_terminal_best_effort();
                term_reinit.store(true, Ordering::Release);
                let _ = low_level::raise(SIGSTOP);
            }
        }
        (KeyCode::Char(' '), _) => {
            cloud.reset(frame.width, frame.height);
            cloud.force_draw_everything();
        }
        (KeyCode::Char('c'), _) => {
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
                cloud.init_chars(chars);
                cloud.force_draw_everything();
            }
        }
        (KeyCode::Char('S'), _) => {
            let prev = cycle_charset_preset(charset_preset, -1);
            *charset_preset = prev.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.init_chars(chars);
                cloud.force_draw_everything();
            }
        }
        (KeyCode::Char('a'), _) => {
            cloud.set_async(!cloud.async_mode);
        }
        (KeyCode::Char('g'), _) => {
            cloud.set_glitchy(!cloud.glitchy);
        }
        (KeyCode::Char('p'), _) => {
            cloud.toggle_pause();
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
        (KeyCode::Left, _) => {
            if cloud.glitchy {
                let gp = (cloud.glitch_pct - GLITCH_PCT_STEP).max(0.0);
                cloud.set_glitch_pct(gp);
            }
        }
        (KeyCode::Right, _) => {
            if cloud.glitchy {
                let gp = (cloud.glitch_pct + GLITCH_PCT_STEP).min(1.0);
                cloud.set_glitch_pct(gp);
            }
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
}

fn spawn_watchdog() {
    let counter = &FRAME_COUNTER as &std::sync::atomic::AtomicU64;
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        let current = counter.load(Ordering::Relaxed);
        std::thread::sleep(Duration::from_secs(WATCHDOG_INTERVAL_SECS));
        let next = counter.load(Ordering::Relaxed);
        if current == next {
            eprintln!(
                "[watchdog] main loop appears stuck (frame counter unchanged for {}s)",
                WATCHDOG_INTERVAL_SECS * 2
            );
        }
    });
}
