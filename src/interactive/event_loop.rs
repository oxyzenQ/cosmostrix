// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Main interactive event loop.
//!
//! Contains the `run_interactive()` function that drives the entire
//! interactive mode: signal handling, frame pacing, input dispatch,
//! simulation stepping, rendering, and performance reporting.
//!
//! NIGHT-hunter-21 (wart #3, owner mandate 2026-09-08): the ~45 loop
//! state locals that were threaded into the sibling modules as
//! positional `&mut` parameters now live in one
//! [`LoopCtx`][super::event_loop_ctx::LoopCtx] (domain sub-structs:
//! scene identity, config layers, ambient state, perf counters). See
//! `event_loop_ctx.rs` for the hazard analysis this removes. The
//! sibling signatures shrank accordingly — the two monsters
//! (`apply_config_rebuild`, `poll_ambient_events`) now take one
//! `&mut LoopCtx` instead of 23/21 parameters.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEventKind};

use crate::color_cache::ColorCache;
use crate::constants::*;
use crate::frame::Frame;
use crate::terminal::{is_terminal_gone, Terminal};

use super::super::{effective_density, CloudConfig};
use super::activity::{register_activity, spin_wait};
use super::event_loop_ctx::{FrameObs, LoopCtx, LoopCtxCore, SceneIdentity};
use super::event_loop_finalize::finalize_session;
use super::input::{handle_keybinding, hud_toggle_accepted, is_unmodified, KeybindingCtx};
use super::watchdog::{GRACEFUL_SHUTDOWN, MOUSE_CAPTURE_ACTIVE};

pub(crate) fn run_interactive(cfg: &CloudConfig) -> std::io::Result<()> {
    // NIGHT-hunter-6 (owner hunt 2026-09-05): warn at frame zero when
    // stdout is piped or redirected — BEFORE the alternate screen is
    // entered and before set_interactive_session_active() (below)
    // starts buffering runtime warnings to post-exit (AB-10). Until the
    // P5 stdout-health probe synthesizes the graceful exit (3600 frames
    // in), a piped stdout receives raw ANSI frames; this warning
    // teaches --benchmark / the text modes instead. See
    // docs/USAGE_PIPE_REDIRECT.md for the full fatal-usage catalog.
    super::watchdog::warn_if_stdout_not_terminal();

    // v80.0.0-beta.1 killer-features hardening: mark the interactive session (alt
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
    // NIGHT-hunter-22 F2: `duration_s` is the single duration source of
    // truth (validated at startup: finite, 0.1..=86400 in range, or the
    // 0 = run-forever sentinel). The old cross-read of the raw twin
    // field is gone — it always resolved to the same value by
    // construction (both fields came from args.duration).
    let end_time = cfg
        .duration_s
        .filter(|s| s.is_finite() && *s > 0.0)
        .map(|s| start_time + Duration::from_secs_f64(s));

    // NIGHT-hunter-21: build the loop context — every piece of state the
    // rain loop mutates, in one struct (see event_loop_ctx.rs). The intro
    // ran BEFORE this point (it owns term/cloud/frame/w/h pre-loop); the
    // startup-ambient block below runs AFTER it (it mutates ctx fields
    // like every other pre-frame step). startup_cfg is the pristine
    // LOCKED layer (full contract: ConfigLayers::startup docs in
    // event_loop_ctx.rs — replaces both the old `startup_cfg` local AND
    // the `cfg` fn arg the siblings threaded as duplicate same-typed
    // refs).
    let startup_cfg = cfg.clone();
    // Live config reload: spawn watcher for config.toml changes.
    // The watcher thread sends validated config HashMaps via mpsc channel.
    // We try_recv() each frame (non-blocking, ~1ns on empty channel).
    // On update, rebuild CloudConfig + Cloud (full rebuild, not delta).
    let config_rx = if let Some(path) = &cfg.config_path_for_watcher {
        crate::live_config::spawn_watcher(path.clone())
    } else {
        None
    };
    // Ambient scheduler: idle/wake thread sends AmbientEntry via mpsc.
    let ambient_handle = crate::crystal_dragon_engine::ambient_scheduler::spawn_ambient_scheduler(
        startup_cfg.ambient_schedule.clone(),
    );
    //  last-applied cfg map for diff trace + startup ambient.
    let initial_cfg_map = startup_cfg
        .config_path_for_watcher
        .as_deref()
        .map(|p| crate::configfile::load_config_file(Some(p)))
        .unwrap_or_default();
    let mut ctx = LoopCtx::new(
        LoopCtxCore {
            term,
            cloud,
            frame,
            w,
            h,
        },
        startup_cfg,
        ambient_handle,
        SceneIdentity {
            charset_preset: cfg.charset_preset.clone(),
            scene_name: cfg.scene_name.clone(),
            scene_generation: 0, // Phase D: u64 compare vs String clone
        },
        cfg.user_ranges.clone(),
        cfg.def_ascii,
    );
    // Last-applied cfg map (diff trace source) — set post-construction
    // (needs the file-loaded map, not a ctx-derived clone).
    ctx.config.last_applied_map = Some(initial_cfg_map.clone());

    // v50.0.0-beta.7 masterclass: ambient startup delay (owner rule: no
    // CLI flags → ambient applies INSTANTLY; ANY CLI flag → ambient
    // defers ambient-snapback-secs so CLI wins first, then ambient takes
    // over — without the delay, ambient stomps `--scene matrix` at once
    // and the flag looks broken). v80.0.0-beta.1: uses CliExplicit::any()
    // — the old inline `||` chain listed only 15 of the 21 flags.
    let cli_has_any_override = ctx.config.base.cli_explicit.any();
    let (new_charset, startup_entry) = if cli_has_any_override {
        // CLI flags present: defer ambient. Capture the entry for snapback.
        let now_min = crate::crystal_dragon_engine::ambient::current_minute_of_day();
        let deferred_entry = ctx
            .config
            .base
            .ambient_schedule
            .current_phase(now_min)
            .cloned();
        crate::lr_trace!(
            "ambient: startup — CLI flags detected, deferring ambient apply until snapback. Entry: {:?}",
            deferred_entry.as_ref().map(|e| &e.scene)
        );
        ctx.ambient.last_applied_entry = deferred_entry;
        // Keep user_override_since_ambient = true so poll_ambient_events
        // defers re-application (the else-if branch in poll_ambient_events).
        (ctx.scene.charset_preset.clone(), None)
    } else {
        // No CLI flags: apply ambient instantly (original behavior).
        crate::crystal_dragon_engine::ambient::apply_startup_ambient(
            &mut ctx.cloud,
            &ctx.config.base.ambient_schedule,
            &ctx.scene.charset_preset,
            &ctx.user_ranges,
            ctx.def_ascii,
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
        ctx.scene.charset_preset = new_charset;
        ctx.scene.scene_name = entry.scene.clone();
        ctx.scene.scene_generation = ctx.scene.scene_generation.wrapping_add(1);
        ctx.cloud.user_override_since_ambient = false;
        ctx.cloud.ambient_palette_locked = true;
        ctx.term
            .set_color_cache(ColorCache::new(&ctx.cloud.palette));
        ctx.frame = Frame::new(ctx.w, ctx.h, ctx.cloud.palette.bg);
        super::fill_terminal_bg(ctx.cloud.palette.bg);
        ctx.ambient.last_applied_entry = Some(entry);
        super::ambient_diag_startup();
        super::ambient_diag_scene_change("startup");
        // v80.0.0-beta.2 (S-master-LOGIC-3): the startup-applied ambient
        // scene owns fps like every other scene-family dimension — apply
        // its declared fps (built-in default or scene-custom field) to
        // the power manager, HUD, and the effective-config tracker.
        if let Some(fps) =
            crate::scene_custom::ambient_scene_fps(&ctx.scene.scene_name, &initial_cfg_map)
        {
            super::event_loop_config_rebuild::apply_ambient_fps(fps, &mut ctx);
        }
    }
    // Seed the HUD so `tgt:` + the screen size are right from frame 1.
    ctx.hud_state
        .set_screen_size(ctx.w, ctx.h, ctx.config.startup.screen_size.is_some());
    ctx.hud_state.set_target_fps(ctx.config.startup.target_fps);
    // Track runtime state for post-exit verbose summary.
    while ctx.cloud.raining {
        // Graceful shutdown from signal handler (clean exit via Terminal::drop).
        if GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
            ctx.cloud.raining = false;
            break;
        }

        // v50.0.0-beta.7 LOC refactor: config event draining extracted to
        // event_loop_config_drain.rs.
        if !super::event_loop_config_drain::drain_config_events(
            &config_rx,
            &mut ctx.config.pending,
            &mut ctx.cloud,
        ) {
            break;
        }

        // v50.0.0-beta.7 LOC refactor: config rebuild extracted to
        // event_loop_config_rebuild.rs.
        // NIGHT-hunter-21: the 23-parameter signature collapsed to the
        // context struct (the old list carried four same-typed
        // CloudConfig refs — base/startup/current/cfg — plus the
        // charset/scene &mut String pair and w/h; all are ctx fields now).
        super::event_loop_config_rebuild::apply_config_rebuild(&mut ctx);

        // v50.0.0-beta.7 LOC refactor: ambient polling extracted to
        // event_loop_ambient.rs.
        // v80.0.0-beta.1: startup_cfg passed for the ambient overlay-lift revert
        // (ground-truth nuke path — see event_loop_ambient.rs).
        // v80.0.0-beta.2 (S-master-LOGIC-3): poll_ambient_events returns
        // the ambient-owned fps intent (Some when an rx-event / snapback /
        // overlay-lift changed the effective scene, None otherwise). The
        // Cloud does not own frame pacing — the event loop applies the
        // intent to the power manager + HUD + effective-config tracker.
        // NIGHT-hunter-21: the 21-parameter signature collapsed to the
        // context struct.
        if let Some(fps) = super::event_loop_ambient::poll_ambient_events(&mut ctx) {
            super::event_loop_config_rebuild::apply_ambient_fps(fps, &mut ctx);
        }

        // v50.0.0-beta.7 LOC refactor: adaptive throttling extracted to
        // event_loop_adaptive.rs. NIGHT-hunter-21: takes the context.
        let throttle = super::event_loop_adaptive::run_adaptive_throttle(&mut ctx);
        let loop_now = throttle.loop_now;
        let is_idle = throttle.is_idle;
        let scene_generation_at_frame_start = throttle.scene_generation_at_frame_start;

        // P2: reuse loop_now (captured at top of loop) instead of another Instant::now().
        if end_time.is_some_and(|end| loop_now >= end) {
            ctx.cloud.raining = false;
            break;
        }
        let mut pending_resize: Option<(u16, u16)> = None;
        if crate::platform::swap_term_reinit(&term_reinit) {
            ctx.term = Terminal::with_signal_exit(signal_exit.clone())?;
            // v17: always re-enable mouse reporting after SIGCONT (see
            // startup comment for rationale — block copy in all modes).
            if ctx.term.enable_mouse_capture().is_ok() {
                MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Release);
            }
            let (nw, nh) = ctx.term.size()?;
            pending_resize = Some((nw, nh));
            ctx.cloud.force_draw_everything();
            let reinit_time = Instant::now();
            ctx.last_resync_time = reinit_time;
            ctx.next_frame = reinit_time;
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
                        ctx.cloud.raining = false;
                        break;
                    }
                    Err(_) => break,
                    Ok(true) => {}
                }
                let ev = match Terminal::read_event() {
                    Ok(e) => e,
                    Err(e) if is_terminal_gone(&e) => {
                        ctx.cloud.raining = false;
                        break;
                    }
                    Err(_) => break,
                };
                match ev {
                    Event::Resize(nw, nh) => {
                        // --screen-size: ignore terminal resize when in fixed mode
                        if ctx.config.startup.screen_size.is_some() {
                            // Fixed mode — ignore resize, keep virtual size
                        } else {
                            // Dynamic mode — clamp to safe bounds before storing
                            let cw = nw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
                            let ch = nh.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
                            pending_resize = Some((cw, ch));
                            ctx.last_resize_event = Some(Instant::now());
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
                        if ctx.paste_guard.ignore_plain_key(&k, activity_time) {
                            let _ = register_activity(
                                &mut ctx.power_manager,
                                &mut ctx.last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            ctx.cloud.force_draw_everything();
                            ctx.next_frame = activity_time;
                            continue;
                        }
                        // HUD toggle ('i'): check BEFORE screensaver exit to prevent
                        // self-exit on Android/Termux. v30: lowercase-only.
                        // Toggling OFF calls force_draw_everything() to clear
                        // stale HUD residue.
                        // NIGHT-hunter-16: for Glyph (droplet family),
                        // force_draw_everything() routes through
                        // Frame::force_repaint() (HUNT-25) which only sets
                        // dirty_all=true WITHOUT bumping the content gen or
                        // clearing cells. HUNT-27's cell-level skip in
                        // term.draw() then sees frame.cells[idx] (old HUD
                        // text) == last.cells[idx] (old HUD text) → SKIP →
                        // stale HUD metrics stay on screen until a rain
                        // droplet happens to overwrite those exact cells
                        // ("after some seconds hide/clean and need rain
                        // passed it for cleaning" — owner report).
                        // Structured styles don't have this bug because their
                        // force_draw_everything block calls clear_with_bg()
                        // (bumps gen → cells read as blank → emit → cleared).
                        // Fix: arm semantic_invalidate = true so the
                        // invalidate_semantic() path runs BEFORE the
                        // force_draw_everything block. invalidate_semantic()
                        // calls clear_with_bg() which bumps gen → all cells
                        // read as blank via the gen-mismatch path → HUNT-27
                        // cell-skip sees blank != old_HUD_text → emit → HUD
                        // cleared immediately on ALL 7 rain styles.
                        // Modifier guard: only bare 'i' (NONE). Rejects Shift+'i'
                        // (which produces 'I', no binding) and all other modifiers
                        // (Ctrl/Super/Alt/Hyper/Meta+'i'). See is_unmodified().
                        // v80.0.0-beta.1 pause isolation (owner bug report 2026-08-30): 'i'
                        // must NOT respond while paused/decelerating — only 'p'
                        // and 'q' work during pause. The gate lives in input.rs
                        // (hud_toggle_accepted) so the predicate is testable and
                        // identical to the handle_keybinding pause guard.
                        if hud_toggle_accepted(&ctx.cloud)
                            && is_unmodified(k.modifiers)
                            && matches!(k.code, KeyCode::Char('i'))
                        {
                            let now_visible = ctx.hud_state.toggle();
                            if !now_visible {
                                ctx.cloud.semantic_invalidate = true;
                                ctx.cloud.force_draw_everything();
                            }
                            // Set next_frame=activity_time so HUD appears immediately;
                            // otherwise idle-mode delay could defer render by seconds.
                            let _ = register_activity(
                                &mut ctx.power_manager,
                                &mut ctx.last_resync_time,
                                activity_time,
                                is_idle,
                                false,
                            );
                            ctx.next_frame = activity_time;
                            continue;
                        }
                        // v50.0.0-beta.6: the 'h' shortkey is REMOVED
                        // completely (was a HUD position toggle, now purged).
                        // HUD always renders flush-left at column 0. Any
                        // user input resets idle timer for adaptive throttling.
                        if register_activity(
                            &mut ctx.power_manager,
                            &mut ctx.last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        ) {
                            ctx.cloud.force_draw_everything();
                            ctx.next_frame = activity_time;
                        }
                        // refresh auto-snapback idle timer on every key press.
                        ctx.last_user_input_at = activity_time;
                        // Process the keybinding. Interactive keys
                        // (q, c/C, s/S, x/X, p, i, r, [, ], Up/Down)
                        // work identically in both modes — see
                        // docs/SCREENSAVER_MODE.md.
                        let key_outcome = handle_keybinding(
                            &mut KeybindingCtx {
                                cloud: &mut ctx.cloud,
                                frame: &mut ctx.frame,
                                charset_preset: &mut ctx.scene.charset_preset,
                                scene_name: &mut ctx.scene.scene_name,
                                scene_generation: &mut ctx.scene.scene_generation,
                                user_ranges: &ctx.user_ranges,
                                def_ascii: ctx.def_ascii,
                                cfg: &ctx.config.startup,
                                term_reinit: &term_reinit,
                                cfg_map: ctx.config.last_applied_map.as_ref(),
                            },
                            &k,
                        );
                        // NIGHT-hunter-27: 'r' — re-assert scene fps.
                        if matches!(key_outcome, super::input::KeyOutcome::FreshScene) {
                            super::event_loop_config_rebuild::apply_fresh_scene_fps(&mut ctx);
                        }
                        if ctx.config.startup.screensaver {
                            // Screensaver: break the event drain immediately
                            // once 'q' cleared cloud.raining (queued events are
                            // discarded instead of drained — see
                            // docs/SCREENSAVER_MODE.md §2 for the full audit).
                            // Mouse click doesn't exit (v17). Only 'q' quits.
                            if !ctx.cloud.raining {
                                break;
                            }
                            // No is_recognized_key check — all unrecognized
                            // keys fall through to handle_keybinding's
                            // `_ => {}` catch-all and are silently ignored.
                        } else if key_outcome.wakes_renderer() {
                            ctx.next_frame = Instant::now();
                        }
                    }
                    Event::Paste(_) => {
                        let activity_time = Instant::now();
                        ctx.paste_guard.note_bracketed_paste(activity_time);
                        let _ = register_activity(
                            &mut ctx.power_manager,
                            &mut ctx.last_resync_time,
                            activity_time,
                            is_idle,
                            false,
                        );
                        ctx.cloud.force_draw_everything();
                        ctx.next_frame = activity_time;
                    }
                    Event::Mouse(m) => {
                        // Mouse events always captured (blocks drag-select). No force_draw
                        // on MOVE (old: bright-color flash). CLICK wakes renderer
                        // on idle→active (old: click effect vanished at 30 FPS idle cadence).
                        let activity_time = Instant::now();
                        let is_click = matches!(m.kind, MouseEventKind::Down(_));
                        let was_idle = is_idle;
                        let _ = register_activity(
                            &mut ctx.power_manager,
                            &mut ctx.last_resync_time,
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
                        ctx.cloud.set_mouse_position(m.column, m.row);
                        if is_click && !ctx.cloud.is_paused_or_decelerating() {
                            ctx.cloud.set_mouse_click(m.column, m.row);
                            // Wake renderer immediately on idle→active click.
                            if was_idle {
                                ctx.cloud.force_draw_everything();
                                ctx.next_frame = activity_time;
                            }
                        }
                    }
                    Event::FocusGained => {
                        let activity_time = Instant::now();
                        if register_activity(
                            &mut ctx.power_manager,
                            &mut ctx.last_resync_time,
                            activity_time,
                            is_idle,
                            true,
                        ) {
                            ctx.cloud.force_draw_everything();
                            ctx.next_frame = activity_time;
                        }
                    }
                    _ => {}
                }
            }
            // Break when resize debounce elapses (coalesces drag storms), or
            // immediately on SIGHUP/SIGTERM / dead PTY. Without the shutdown
            // check, the wait loop burns CPU until next_frame after the signal.
            if !ctx.cloud.raining || GRACEFUL_SHUTDOWN.load(Ordering::Acquire) {
                break;
            }
            if pending_resize.is_some() {
                let debounce_elapsed = ctx
                    .last_resize_event
                    .map(|t| t.elapsed() >= Duration::from_millis(RESIZE_DEBOUNCE_MS))
                    .unwrap_or(true);
                if debounce_elapsed {
                    break;
                }
            }
            let now = Instant::now();
            // Monotonic clock jump guard
            let frame_elapsed = now.saturating_duration_since(ctx.next_frame);
            if frame_elapsed.as_secs_f64() > CLOCK_JUMP_GUARD_SECS {
                ctx.next_frame = now;
                break;
            }
            if now >= ctx.next_frame {
                break;
            }
            let mut timeout = ctx.next_frame - now;
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
                        ctx.cloud.raining = false;
                        break;
                    }
                    Err(e) => return Err(e),
                }
                // Spin-wait the remaining time for precise deadline alignment.
                // The spin is capped at 1ms internally to handle edge cases.
                // Only reached when poll returned Ok(false) — no events, so
                // spinning to the deadline is the correct behavior.
                spin_wait(ctx.next_frame);
            } else {
                // Already close to deadline (< 500μs away): spin-wait to hit
                // it precisely, then drain any events that arrived.
                spin_wait(ctx.next_frame);
                match Terminal::poll_event(Duration::from_millis(0)) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(e) if is_terminal_gone(&e) => {
                        ctx.cloud.raining = false;
                        break;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        if !ctx.cloud.raining {
            break;
        }
        // v50.0.0-beta.7 LOC refactor: resize handler extracted to
        // event_loop_resize.rs. NIGHT-hunter-21: takes the context + the
        // pending resize (the fixed-screen-size flag is derived from
        // ctx.config.startup inside).
        super::event_loop_resize::handle_resize(&mut ctx, pending_resize);
        // Key handling can toggle pause/resume after the frame period was
        // chosen for the wait phase. Recompute before simulation and
        // scheduling so the first resumed frame does not inherit the paused
        // 250ms cadence.
        // (Phase 3): PowerManager.effective_fps() replaces the
        // target_period / idle_period / pause_period Duration cascade.
        // v50.0.0-beta.6: use current_cfg.power_dragon (live-reloaded) so
        // live-reloading power_dragon=false immediately affects frame pacing.
        let frame_period = Duration::from_secs_f64(
            1.0 / ctx
                .power_manager
                .effective_fps(ctx.cloud.pause, ctx.config.current.power_dragon),
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
            &mut ctx.hud_state,
            &mut ctx.cloud,
            &ctx.power_manager,
            &ctx.scene.scene_name,
            &ctx.scene.charset_preset,
            &ctx.config.current,
        );
        // v50.0.0-beta.7 LOC refactor: sim+draw extracted to
        // event_loop_sim_draw.rs.
        // NIGHT-hunter-2: the sim-delta cap consumes the same applied
        // visual pressure that update_hud_state fed the cloud one call
        // earlier (power_dragon-gated + EMA-smoothed) — one value, one
        // contract, no drift between the cloud feed and the sim cap.
        let applied_pressure = ctx
            .power_manager
            .applied_visual_pressure(ctx.config.current.power_dragon);
        let sim_draw = super::event_loop_sim_draw::run_sim_and_draw(
            &mut ctx.cloud,
            &mut ctx.frame,
            &mut ctx.hud_state,
            &mut ctx.term,
            frame_period,
            applied_pressure,
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
            let total_cells = (ctx.frame.width as u64) * (ctx.frame.height as u64);
            let dirty_count = if is_dirty_all {
                total_cells
            } else {
                dirty_len as u64
            };
            ctx.hud_state.set_dirty_cell_stats(dirty_count, total_cells);
        }

        // v50.0.0-beta.7 LOC refactor: post-draw accounting extracted to
        // event_loop_post_draw.rs.
        // HUNT-23: did_draw gates the write-latency overshoot (stale
        // last_write_ns on non-drawing frames must not feed the drain
        // backoff / perf_pressure).
        let post_draw = super::event_loop_post_draw::post_draw_accounting(
            &mut ctx.hud_state,
            &mut ctx.power_manager,
            &ctx.term,
            &ctx.cloud,
            work_start,
            frame_period_s,
            did_draw,
        );
        let work_s = post_draw.work_s;
        let overshoot = post_draw.overshoot;
        let utilization = post_draw.utilization;

        // S-master-HUNT-24: dynamic effects congestion gate. Runs after
        // observe_frame_end (so drain_backoff reflects THIS frame's write
        // latency) and before the self-healer (which reads effective
        // pressure, not the backoff). Sticky — once it fires, effects stay
        // off for the session; brief spikes reset the sustain timer.
        ctx.effects_auto_gate
            .observe(ctx.power_manager.drain_backoff(), loop_now, &mut ctx.cloud);

        // v50.0.0-beta.7 LOC refactor: P5 health sampling extracted to
        // event_loop_p5.rs.
        // HUNT-23: frame_period_s feeds the utilization-based frame signal.
        // NIGHT-hunter-21: the per-frame observation values (work_s,
        // frame_period_s, work_start, did_draw, dirty_len, overshoot,
        // utilization) travel in one named-field FrameObs from here on —
        // the frame-tail siblings share it, so the three f32 values are no
        // longer positionally transposable.
        let obs = FrameObs {
            work_start,
            work_s,
            frame_period_s,
            did_draw,
            is_dirty_all,
            dirty_len,
            overshoot,
            utilization,
        };
        if !super::event_loop_p5::sample_p5_health(&mut ctx, &obs) {
            break;
        }

        // v50.0.0-beta.7 LOC refactor: perf stats display extracted to
        // event_loop_perf_stats.rs.
        super::event_loop_perf_stats::update_perf_stats(&mut ctx, &obs);

        // v50.0.0-beta.7 LOC refactor: performance self-healer extracted
        // to event_loop_self_heal.rs. NIGHT-hunter-21: value inputs travel
        // in the named-field HealInputs struct (the old positional list
        // carried the scene-generation u64 pair + three float/timestamp
        // values); the four mutable targets stay granular — distinct
        // types, compiler-checked against transposition — so the unit
        // tests keep constructing four small objects instead of a full
        // context.
        super::event_loop_self_heal::run_self_healer(
            &mut ctx.self_healer,
            &mut ctx.reclaim_state,
            &mut ctx.cloud,
            &mut ctx.frame,
            super::event_loop_self_heal::HealInputs {
                cfg: &ctx.config.current,
                scene_name: &ctx.scene.scene_name,
                scene_generation: ctx.scene.scene_generation,
                scene_generation_at_frame_start,
                effective_pressure: ctx.power_manager.effective_pressure(),
                loop_now,
                endurance_health_score: ctx.endurance_health.score(),
            },
        );

        // Schedule next frame relative to the ideal deadline, using the
        // pre-work timestamp to prevent drift between render work and
        // scheduling. Single-reschedule: if we overslept past the next tick,
        // snap forward by exactly one period from now instead of
        // double-advancing (which caused visible stutter on frames that took
        // just 1μs too long).
        let frame_ts = work_start;
        let next = ctx.next_frame.checked_add(frame_period).unwrap_or(frame_ts);
        ctx.next_frame = if frame_ts > next {
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
            perf_frames: ctx.perf.frames,
            perf_drawn_frames: ctx.perf.drawn_frames,
            perf_idle_frames: ctx.perf.idle_frames,
            perf_overshoot_frames: ctx.perf.overshoot_frames,
            perf_dirty_sum: ctx.perf.dirty_sum,
            perf_dirty_samples: ctx.perf.dirty_samples,
            perf_work_sum_s: ctx.perf.work_sum_s,
            perf_work_max_s: ctx.perf.work_max_s,
            perf_pressure_sum: ctx.perf.pressure_sum,
            perf_pressure_max: ctx.perf.pressure_max,
            perf_utilization_sum: ctx.perf.utilization_sum,
            perf_utilization_max: ctx.perf.utilization_max,
            frame_time_tracker: &ctx.frame_time_tracker,
            power_manager: &ctx.power_manager,
            endurance_health: &ctx.endurance_health,
            cloud: &ctx.cloud,
        });
    finalize_session(
        &stats,
        ctx.term,
        &ctx.cloud,
        &ctx.scene.scene_name,
        &ctx.scene.charset_preset,
        &ctx.config.current,
    )
}
