// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Interactive runtime loop for cosmostrix.
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
//! Unix signals (SIGTERM, SIGHUP, SIGQUIT, SIGTSTP, SIGCONT) are handled via
//! a dedicated signal thread that sets an atomic `GRACEFUL_SHUTDOWN` flag.
//! SIGINT (Ctrl+C) is deprecated — only 'q' exits cosmostrix.
//! The main loop checks this flag each iteration and exits cleanly, allowing
//! `Terminal::drop()` to restore the terminal without racing on stdout.
//! A fallback force-restore fires after 1 second if the main loop is stuck.
//!
//! ## Watchdog
//!
//! A background watchdog thread monitors a global frame counter. If no frames
//! are produced for 1+ second, it restores the terminal and exits —
//! protecting against infinite loops that would leave the TTY in a broken state.

mod activity;
mod adaptive;
mod bg_fill;
mod event_loop;
mod event_loop_adaptive;
mod event_loop_ambient;
mod event_loop_config_drain;
mod event_loop_config_rebuild;
mod event_loop_finalize;
mod event_loop_hud;
mod event_loop_intro;
mod event_loop_p5;
mod event_loop_perf_stats;
mod event_loop_post_draw;
mod event_loop_resize;
mod event_loop_scene_sync;
mod event_loop_self_heal;
mod event_loop_setup;
mod event_loop_sim_draw;
mod event_loop_stats;
mod hud;
mod input;
mod signal_handlers;
mod watchdog;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_fmt_opt;

// v50 LTS regression tests (first-reload scene reset crash).
#[cfg(test)]
#[path = "tests_v50_first_reload.rs"]
mod v50_first_reload;

// v50.0.0-beta.7 Z-master-1B: kitty CSI-u Shift+letter normalization
// (base lowercase codepoint + SHIFT -> uppercase reverse-cycle arm).
#[cfg(test)]
#[path = "tests_v50_kitty_shift.rs"]
mod v50_kitty_shift;

// v51 Z-master-1B: intro brand color (EnergyZen, immune to -c) +
// pause shortkey isolation ('i' rejected while paused).
#[cfg(test)]
#[path = "tests_v51_intro_brand_pause.rs"]
mod v51_intro_brand_pause;

// v51 Z-master-1B: exhaustive shortkey no-op lock — every key outside
// the documented active set must be a complete no-op ('a', 'h', Tab,
// removed density aliases, digits, punctuation, F-keys, ...).
#[cfg(test)]
#[path = "tests_v51_shortkey_noop.rs"]
mod v51_shortkey_noop;

#[cfg(test)]
mod tests_v35;

#[cfg(test)]
mod tests_v35_modifier_rejection;

// Re-export ambient_diag from crystal_dragon_engine
pub(crate) use crate::crystal_dragon_engine::ambient_diag::{
    ambient_diag_config_rebuild, ambient_diag_consistency_fix, ambient_diag_reapply,
    ambient_diag_rx, ambient_diag_scene_change, ambient_diag_schedule_empty,
    ambient_diag_schedule_reload, ambient_diag_snapback, ambient_diag_snapback_guard,
    ambient_diag_snapback_killed, ambient_diag_startup, ambient_diag_summary,
};
pub(crate) use bg_fill::fill_terminal_bg;
pub(crate) use event_loop::run_interactive;
// v52 intro_style refactor: the intro subsystem moved to crate-root
// `intro_style/`; these two items it needs from `interactive` are
// re-exported at the facade so the submodules stay private.
pub(crate) use input::is_unmodified_or_shift;
pub(crate) use watchdog::{FRAME_COUNTER, GRACEFUL_SHUTDOWN};
// `clear_mouse_capture_flag` is called cross-platform (terminal.rs:508).
// `request_graceful_shutdown` is only called from the Unix `recover_to_tty`
// path (terminal.rs:425) — gate the re-export so Windows doesn't warn.
pub(crate) use watchdog::clear_mouse_capture_flag;
#[cfg(unix)]
pub(crate) use watchdog::request_graceful_shutdown;

use std::sync::OnceLock;

// Final runtime state — stored as Strings to avoid enum discriminant issues
// with 52 ColorScheme variants. Set once by event loop before returning.
// OnceLock eliminates mutex overhead for write-once-read-many semantics.
static FINAL_COLOR: OnceLock<String> = OnceLock::new();
static FINAL_SCENE: OnceLock<String> = OnceLock::new();
static FINAL_CHARSET: OnceLock<String> = OnceLock::new();
static FINAL_SPEED: OnceLock<f32> = OnceLock::new();
static FINAL_DENSITY: OnceLock<f32> = OnceLock::new();
// v50.0.0-alpha.7: extended final-state tracking for live-reload honesty.
// Owner found that --verbose showed startup values (e.g. msg_mode=true)
// instead of the effective runtime values (e.g. msg_mode=false after
// live-reload edit). These fields are now tracked + printed post-exit.
static FINAL_MSG_MODE: OnceLock<bool> = OnceLock::new();
static FINAL_MESSAGE: OnceLock<Option<String>> = OnceLock::new();
static FINAL_MESSAGE_BORDER: OnceLock<bool> = OnceLock::new();
// v51 msg-fill-style: track the effective reveal style so the post-exit
// "final runtime state" section can disclose live-reload edits to
// `msg-fill-style` (same honest-reporting contract as msg_mode/message).
static FINAL_MSG_FILL_STYLE: OnceLock<String> = OnceLock::new();
static FINAL_POWER_DRAGON: OnceLock<bool> = OnceLock::new();
static FINAL_CRYSTAL_DRAGON: OnceLock<bool> = OnceLock::new();
static FINAL_ASYNC_MODE: OnceLock<bool> = OnceLock::new();
static FINAL_INTRO_COLOR: OnceLock<Option<String>> = OnceLock::new();
// v50.0.0-beta.7 LTS audit: track ambient runtime state so the post-exit
// "final runtime state" section can show the EFFECTIVE ambient config —
// owner found that ambient + ambient-snapback-secs were missing entirely
// from final_runtime_verbose, making it impossible to verify what value
// was actually in effect when the session ended (live-reload edits to
// ambient-snapback-secs were silently lost on exit).
static FINAL_AMBIENT_SNAPBACK_SECS: OnceLock<Option<f64>> = OnceLock::new();
static FINAL_AMBIENT_ENTRIES: OnceLock<usize> = OnceLock::new();

/// Store final runtime state for post-exit verbose summary.
///
/// v50.0.0-alpha.7: extended with msg_mode, message, message_border,
/// power_dragon, crystal_dragon, async_mode, intro_color — so the
/// post-exit "final runtime state" section reflects ALL live-reload
/// changes, not just color/scene/charset/speed/density.
///
/// v50.0.0-beta.7 LTS audit: extended with ambient_snapback_secs +
/// ambient_entries so the final-runtime section reports the EFFECTIVE
/// ambient config (owner audit found these missing — live-reload edits
/// to ambient-snapback-secs were silently lost on exit, making it
/// impossible to verify the actual snapback delay in effect when the
/// session ended).
#[allow(clippy::too_many_arguments)]
pub(crate) fn set_final_state(
    color: &str,
    scene: &str,
    charset: &str,
    speed: f32,
    density: f32,
    msg_mode: bool,
    message: Option<&str>,
    message_border: bool,
    msg_fill_style: &str,
    power_dragon: bool,
    crystal_dragon: bool,
    async_mode: bool,
    intro_color: Option<&str>,
    ambient_snapback_secs: Option<f64>,
    ambient_entries: usize,
) {
    let _ = FINAL_COLOR.set(color.to_string());
    let _ = FINAL_SCENE.set(scene.to_string());
    let _ = FINAL_CHARSET.set(charset.to_string());
    let _ = FINAL_SPEED.set(speed);
    let _ = FINAL_DENSITY.set(density);
    let _ = FINAL_MSG_MODE.set(msg_mode);
    let _ = FINAL_MESSAGE.set(message.map(|s| s.to_string()));
    let _ = FINAL_MESSAGE_BORDER.set(message_border);
    // v51 msg-fill-style: stored as the canonical lowercase label.
    let _ = FINAL_MSG_FILL_STYLE.set(msg_fill_style.to_string());
    let _ = FINAL_POWER_DRAGON.set(power_dragon);
    let _ = FINAL_CRYSTAL_DRAGON.set(crystal_dragon);
    let _ = FINAL_ASYNC_MODE.set(async_mode);
    let _ = FINAL_INTRO_COLOR.set(intro_color.map(|s| s.to_string()));
    let _ = FINAL_AMBIENT_SNAPBACK_SECS.set(ambient_snapback_secs);
    let _ = FINAL_AMBIENT_ENTRIES.set(ambient_entries);
}

/// v50.0.0-alpha.7: accessor for final msg_mode (post-live-reload).
pub(crate) fn last_msg_mode() -> bool {
    *FINAL_MSG_MODE.get().unwrap_or(&true)
}

/// v50.0.0-alpha.7: accessor for final message text (post-live-reload).
pub(crate) fn last_message() -> Option<&'static str> {
    FINAL_MESSAGE.get().and_then(|m| m.as_deref())
}

/// v50.0.0-alpha.7: accessor for final message_border (post-live-reload).
pub(crate) fn last_message_border() -> bool {
    *FINAL_MESSAGE_BORDER.get().unwrap_or(&false)
}

/// v51 msg-fill-style: accessor for the final reveal style label
/// (post-live-reload). Defaults to "typewriter" when set_final_state
/// never ran (early-exit paths).
pub(crate) fn last_msg_fill_style() -> String {
    FINAL_MSG_FILL_STYLE
        .get()
        .cloned()
        .unwrap_or_else(|| "typewriter".to_string())
}

/// v50.0.0-alpha.7: accessor for final power_dragon (post-live-reload).
pub(crate) fn last_power_dragon() -> bool {
    *FINAL_POWER_DRAGON.get().unwrap_or(&true)
}

/// v50.0.0-alpha.7: accessor for final crystal_dragon (post-live-reload).
pub(crate) fn last_crystal_dragon() -> bool {
    *FINAL_CRYSTAL_DRAGON.get().unwrap_or(&false)
}

/// v50.0.0-alpha.7: accessor for final async_mode (post-live-reload).
pub(crate) fn last_async_mode() -> bool {
    *FINAL_ASYNC_MODE.get().unwrap_or(&true)
}

/// v50.0.0-alpha.7: accessor for final intro_color (post-live-reload).
pub(crate) fn last_intro_color() -> Option<&'static str> {
    FINAL_INTRO_COLOR.get().and_then(|m| m.as_deref())
}

/// v50.0.0-beta.7 LTS: accessor for final ambient_snapback_secs
/// (post-live-reload). None = unset in config → runtime used
/// `AUTO_SNAPBACK_DELAY_SECS` (30.0). Some(secs) = user-set value via
/// `ambient-snapback-secs` config key.
pub(crate) fn last_ambient_snapback_secs() -> Option<f64> {
    FINAL_AMBIENT_SNAPBACK_SECS.get().copied().flatten()
}

/// v50.0.0-beta.7 LTS: accessor for final ambient schedule entries count
/// (post-live-reload). 0 = scheduler idles (no ambient phases configured).
pub(crate) fn last_ambient_entries() -> usize {
    *FINAL_AMBIENT_ENTRIES.get().unwrap_or(&0)
}

/// Format an `Option<&str>` for the live-reload change tracker.
///
/// `Some("text")` -> `"text"` (quoted, matches verbose.rs style)
/// `None`         -> `(none)` (matches live_config/mod.rs style)
///
/// Replaces the ambiguous `{:?}` Debug format which produced
/// `Some("...")` / `None` in the verbose output — the Rust wrapper
/// made the live-reload tracker read like a REPL instead of a UX.
pub(super) fn fmt_opt_str(opt: Option<&str>) -> String {
    match opt {
        Some(s) => format!("\"{s}\""),
        None => "(none)".to_string(),
    }
}

/// v50.0.0-alpha.7: print "final runtime state" section showing live-reload
/// changes between startup and exit. Extracted from main.rs to keep that
/// file under the 800-LOC cap.
///
/// Compares startup CloudConfig values against the final OnceLock values
/// set by `set_final_state`. Only prints fields that actually changed
/// during the session (live-reload edits). Honest reporting: shows the
/// EFFECTIVE runtime value (post-live-reload), not the startup value.
///
/// v50.0.0-rc.1: the section now ALWAYS prints (previously it early-
/// returned when nothing changed) so the user can see how long cosmostrix
/// ran via `cosmostrix -v`. The first content line after the header is:
///
/// ```text
/// [verbose] [HH:MM]   exit_time:     YYYY-MM-DD HH:MM:SS ±HH:MM | duration: Xm Ys
/// ```
///
/// `exit_time` is the local wall-clock at the moment of exit; `duration`
/// is the elapsed monotonic time since the `Instant` captured at the top
/// of `main()`. Changed live-reload fields (if any) follow, then the
/// ambient diagnostics summary closes the section.
#[allow(clippy::too_many_arguments)]
pub(crate) fn print_final_runtime_state(
    startup_color: &str,
    startup_scene: &str,
    startup_charset: &str,
    startup_speed: f32,
    startup_density: f32,
    startup_msg_mode: bool,
    startup_message: Option<&str>,
    startup_message_border: bool,
    startup_msg_fill_style: &str,
    startup_power_dragon: bool,
    startup_crystal_dragon: bool,
    startup_async_mode: bool,
    startup_intro_color: Option<&str>,
    // v50.0.0-rc.1: program-start Instant captured at the top of main().
    // Used to compute `duration:` in the exit summary. Monotonic so NTP
    // jumps cannot produce a negative duration.
    start_time: std::time::Instant,
    // v50.0.0-beta.7 LTS: ambient startup state — paired with the final
    // values read from FINAL_AMBIENT_* OnceLocks below so the post-exit
    // section ALWAYS reports the effective ambient config (owner audit:
    // these were missing entirely from final_runtime_verbose).
    startup_ambient_snapback_secs: Option<f64>,
    startup_ambient_entries: usize,
) {
    let final_color = last_color_scheme();
    let final_scene = last_scene_name();
    let final_charset = last_charset_preset();
    let final_speed = last_speed();
    let final_density = last_density();
    let final_msg_mode = last_msg_mode();
    let final_message = last_message();
    let final_message_border = last_message_border();
    let final_msg_fill_style = last_msg_fill_style();
    let final_power_dragon = last_power_dragon();
    let final_crystal_dragon = last_crystal_dragon();
    let final_async_mode = last_async_mode();
    let final_intro_color = last_intro_color();
    // v50.0.0-beta.7 LTS: read final ambient state (post-live-reload).
    let final_ambient_snapback_secs = last_ambient_snapback_secs();
    let final_ambient_entries = last_ambient_entries();

    // v50.0.0-rc.1: previously this function early-returned when no field
    // changed, suppressing the entire section. Now the section ALWAYS
    // prints so the user can see the exit_time + duration line regardless
    // of whether any live-reload edits happened during the session. The
    // per-field `if final_X != startup_X` guards below still suppress
    // unchanged fields, keeping the section scannable.

    let ts = crate::output::now_hhmm();
    let purple = crate::output::brand_open();
    let reset = crate::output::reset();
    crate::output::eprintln_verbose_purple("final runtime state");

    // v50.0.0-beta.6: exit_time now uses UTC (was local + offset in rc.1).
    // UTC is LTS-stable: no DST transitions, no tzdata drift, consistent
    // across environments. Format: YYYY-MM-DD HH:MM:SSZ (ISO 8601 UTC).
    // duration unchanged — monotonic Instant elapsed since main() start.
    let exit_time = crate::clock::now_utc_datetime();
    let duration = crate::clock::format_duration_compact(start_time.elapsed());
    crate::output::eprintln_safe!(
        "{purple}[verbose]{reset} {ts} {purple}  exit_time:{reset}     {exit_time} | duration: {duration}{reset}"
    );

    if final_color != startup_color {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  color_scheme:{reset}  {} (was {})",
            final_color,
            startup_color
        );
    }
    if final_scene != startup_scene {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  scene:{reset}         {} (was {})",
            final_scene,
            startup_scene
        );
    }
    if final_charset != startup_charset {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  charset:{reset}       {} (was {})",
            final_charset,
            startup_charset
        );
    }
    if (final_speed - startup_speed).abs() >= 0.01 {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  speed:{reset}         {:.1} (was {:.1})",
            final_speed,
            startup_speed
        );
    }
    if (final_density - startup_density).abs() >= 0.01 {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  density:{reset}       {:.2} (was {:.2})",
            final_density,
            startup_density
        );
    }
    if final_msg_mode != startup_msg_mode {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  msg_mode:{reset}       {} (was {})",
            final_msg_mode,
            startup_msg_mode
        );
    }
    if final_message != startup_message {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  message:{reset}        {} (was {})",
            fmt_opt_str(final_message),
            fmt_opt_str(startup_message)
        );
    }
    if final_message_border != startup_message_border {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  message_border:{reset} {} (was {})",
            final_message_border,
            startup_message_border
        );
    }
    // v51 msg-fill-style: ALWAYS printed (not change-gated) so users can
    // verify the effective reveal style at session end — same policy as
    // the ambient lines below. The `(was X)` suffix appears only when a
    // live-reload edit changed it.
    let style_was_label = if final_msg_fill_style != startup_msg_fill_style {
        format!(" (was {startup_msg_fill_style})")
    } else {
        String::new()
    };
    crate::output::eprintln_safe!(
        "{purple}[verbose]{reset} {ts} {purple}  msg_fill_style:{reset}  {final_msg_fill_style}{style_was_label}"
    );
    if final_power_dragon != startup_power_dragon {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  power_dragon:{reset}   {} (was {})",
            final_power_dragon,
            startup_power_dragon
        );
    }
    if final_crystal_dragon != startup_crystal_dragon {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  crystal_dragon:{reset} {} (was {})",
            final_crystal_dragon,
            startup_crystal_dragon
        );
    }
    if final_async_mode != startup_async_mode {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  async_mode:{reset}     {} (was {})",
            final_async_mode,
            startup_async_mode
        );
    }
    if final_intro_color != startup_intro_color {
        crate::output::eprintln_safe!(
            "{purple}[verbose]{reset} {ts} {purple}  intro_color:{reset}     {} (was {})",
            fmt_opt_str(final_intro_color),
            fmt_opt_str(startup_intro_color)
        );
    }

    // v50.0.0-beta.7 LTS audit: ALWAYS print the ambient runtime state
    // (not gated by change) so the user can verify what was actually in
    // effect at session end. Owner found these missing entirely from
    // final_runtime_verbose — without them, it's impossible to confirm
    // whether a live-reload edit to `ambient-snapback-secs` survived
    // or whether the ambient schedule was loaded at all. The `(was X)`
    // suffix appears only when startup != final (live-reload happened).
    let snapback_now =
        final_ambient_snapback_secs.unwrap_or(crate::constants::AUTO_SNAPBACK_DELAY_SECS);
    let snapback_was =
        startup_ambient_snapback_secs.unwrap_or(crate::constants::AUTO_SNAPBACK_DELAY_SECS);
    let snapback_src = if final_ambient_snapback_secs.is_some() {
        "config"
    } else {
        "default (unset — 30.0s)"
    };
    let snapback_was_label = if final_ambient_snapback_secs != startup_ambient_snapback_secs {
        format!(" (was {snapback_was:.1}s)")
    } else {
        String::new()
    };
    crate::output::eprintln_safe!(
        "{purple}[verbose]{reset} {ts} {purple}  ambient_snapback_secs:{reset} {snapback_now:.1}s ({snapback_src}){snapback_was_label}"
    );
    let entries_was_label = if final_ambient_entries != startup_ambient_entries {
        format!(" (was {})", startup_ambient_entries)
    } else {
        String::new()
    };
    crate::output::eprintln_safe!(
        "{purple}[verbose]{reset} {ts} {purple}  ambient_entries:{reset}    {}{entries_was_label}",
        final_ambient_entries
    );

    let diag = ambient_diag_summary();
    crate::output::eprintln_safe!("{purple}[verbose]{reset} {ts} {purple}  {diag}{reset}");
}

/// AB-10 (rain-screen cleanliness): emit pre-alt-screen warnings to stderr
/// BEFORE `Terminal::with_signal_exit()` enters the alternate screen.
/// Otherwise the warning lines leak into the rain matrix on startup.
///
/// Reads the terminal size via `crossterm::terminal::size()`, which does NOT
/// require raw mode or alt-screen entry. Applies the same clamp(MIN, MAX)
/// used by `Terminal::size()` so the comparison reflects the renderer's
/// actual working area.
///
/// Two warnings:
///   1. `--screen-size WxH` exceeds the live terminal size (clipped).
///   2. Intro requested but terminal smaller than MIN_INTRO_COLS x
///      MIN_INTRO_LINES (intro will be silently skipped by `run_intro`).
pub(crate) fn emit_pre_alt_screen_warnings(fixed_size: Option<(u16, u16)>, intro_enabled: bool) {
    use crate::constants::{
        MAX_TERMINAL_COLS, MAX_TERMINAL_LINES, MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
    };
    if let Some(fixed) = fixed_size {
        let (tw, th) = crossterm::terminal::size().unwrap_or((fixed.0, fixed.1));
        let tw = tw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let th = th.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
        if fixed.0 > tw || fixed.1 > th {
            crate::output::eprintln_safe!(
                "warning: --screen-size {}x{} exceeds terminal {}x{}; will clip to top-left",
                fixed.0,
                fixed.1,
                tw,
                th
            );
        }
    }
    if intro_enabled {
        let (tw, th) = crossterm::terminal::size().unwrap_or((0, 0));
        let tw = tw.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let th = th.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
        if tw < crate::intro_style::MIN_INTRO_COLS || th < crate::intro_style::MIN_INTRO_LINES {
            crate::output::eprintln_safe!(
                "Terminal too small for intro ({}x{} < {}x{}). Starting rain...",
                tw,
                th,
                crate::intro_style::MIN_INTRO_COLS,
                crate::intro_style::MIN_INTRO_LINES
            );
        }
    }
}

/// Get the final color scheme name after the rain loop exited.
pub(crate) fn last_color_scheme() -> String {
    FINAL_COLOR
        .get()
        .cloned()
        .unwrap_or_else(|| "cosmos".to_string())
}

/// Get the final scene name after the rain loop exited.
pub(crate) fn last_scene_name() -> String {
    FINAL_SCENE
        .get()
        .cloned()
        .unwrap_or_else(|| "monolith".to_string())
}

/// Get the final charset preset after the rain loop exited.
pub(crate) fn last_charset_preset() -> String {
    FINAL_CHARSET
        .get()
        .cloned()
        .unwrap_or_else(|| "binary".to_string())
}

/// Get the final rain speed after the rain loop exited.
pub(crate) fn last_speed() -> f32 {
    *FINAL_SPEED.get().unwrap_or(&9.0)
}

/// Get the final density after the rain loop exited.
pub(crate) fn last_density() -> f32 {
    *FINAL_DENSITY.get().unwrap_or(&0.75)
}

// Startup ambient info — stored in a static so main.rs can print
// it AFTER Terminal::drop exits the alternate screen. Printing inside
// event_loop is invisible because the terminal is in alternate screen
// mode and the output is discarded on exit.
static STARTUP_AMBIENT_INFO: OnceLock<String> = OnceLock::new();

/// Store the startup ambient phase info for post-exit verbose summary.
/// Called from event_loop right after `apply_startup_ambient`. The string
/// is the fully-formatted verbose line (without the `[verbose]` prefix,
/// which `eprintln_verbose_raw` adds).
pub(crate) fn set_startup_ambient_info(info: &str) {
    let _ = STARTUP_AMBIENT_INFO.set(info.to_string());
}

/// Get the stored startup ambient info (None if no ambient schedule active
/// or if event_loop never ran). Used by main.rs post-exit verbose dump.
pub(crate) fn startup_ambient_info() -> Option<String> {
    STARTUP_AMBIENT_INFO.get().cloned()
}
