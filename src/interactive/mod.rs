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
#[path = "../../test/interactive/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../test/interactive/tests_final_state.rs"]
mod tests_final_state;
#[cfg(test)]
#[path = "../../test/interactive/tests_fmt_opt.rs"]
mod tests_fmt_opt;

// v50 LTS regression tests (first-reload scene reset crash).
#[cfg(test)]
#[path = "../../test/interactive/tests_v50_first_reload.rs"]
mod v50_first_reload;

// v50.0.0-beta.7 Z-master-1B: kitty CSI-u Shift+letter normalization (lowercase codepoint + SHIFT -> uppercase reverse-cycle arm).
#[cfg(test)]
#[path = "../../test/interactive/tests_v50_kitty_shift.rs"]
mod v50_kitty_shift;

// v80.0.0-beta.1 Z-master-1B: intro brand color (EnergyZen, immune to -c) + pause shortkey isolation ('i' rejected while paused).
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_intro_brand_pause.rs"]
mod v51_intro_brand_pause;

// v80.0.0-beta.1 Z-master-1B: exhaustive shortkey no-op lock — every key outside the active set is a complete no-op.
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_shortkey_noop.rs"]
mod v51_shortkey_noop;

// v80.0.0-beta.1 power-dragon gate: render-path pressure feed gated on power-dragon + stale aggressive release.
#[cfg(test)]
#[path = "../../test/interactive/tests_v51_2_power_dragon_gate.rs"]
mod v51_2_power_dragon_gate;

#[cfg(test)]
#[path = "../../test/interactive/tests_v35.rs"]
mod tests_v35;

#[cfg(test)]
#[path = "../../test/interactive/tests_v35_modifier_rejection.rs"]
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

// Final runtime state — stored as Strings to avoid enum discriminant
// issues with 52 ColorScheme variants. Set once by the event loop before
// returning; OnceLock gives write-once-read-many semantics.
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
// v80.0.0-beta.1 msg-fill-style: track the effective reveal style so the post-exit
// "final runtime state" section can disclose live-reload edits to
// `msg-fill-style` (same honest-reporting contract as msg_mode/message).
static FINAL_MSG_FILL_STYLE: OnceLock<String> = OnceLock::new();
static FINAL_POWER_DRAGON: OnceLock<bool> = OnceLock::new();
static FINAL_CRYSTAL_DRAGON: OnceLock<bool> = OnceLock::new();
static FINAL_ASYNC_MODE: OnceLock<bool> = OnceLock::new();
static FINAL_INTRO_COLOR: OnceLock<Option<String>> = OnceLock::new();
// v50.0.0-beta.7 LTS audit: ambient runtime state tracked so the post-exit
// section shows the EFFECTIVE ambient config (edits to
// ambient-snapback-secs were previously silently lost on exit).
static FINAL_AMBIENT_SNAPBACK_SECS: OnceLock<Option<f64>> = OnceLock::new();
static FINAL_AMBIENT_ENTRIES: OnceLock<usize> = OnceLock::new();
// v80.0.0-alpha.1: final-state tracking for the crystal-dragon-secs
// harmony knob — a mid-run edit must be verifiable at exit.
static FINAL_CRYSTAL_DRAGON_SECS: OnceLock<Option<f64>> = OnceLock::new();
// v80.0.0-beta.2 (S-master-LOGIC-1): final-state completeness — the
// post-exit section discloses EVERY live-reload-able field (owner found
// bold/shading-mode edits unverifiable). fps (config key, scene field,
// ambient ownership), glitch_level (derived from the live Cloud — the
// CloudConfig enum is stale after an ambient apply), bold/shading/
// monolith/color_bg/color_tune (top-level keys; scene-custom block
// ownership was REMOVED in beta.2).
static FINAL_FPS: OnceLock<f64> = OnceLock::new();
static FINAL_GLIITCH_LEVEL: OnceLock<String> = OnceLock::new();
static FINAL_BOLD_MODE: OnceLock<String> = OnceLock::new();
static FINAL_SHADING_MODE: OnceLock<String> = OnceLock::new();
static FINAL_MONOLITH_SIZE: OnceLock<String> = OnceLock::new();
static FINAL_COLOR_BG: OnceLock<bool> = OnceLock::new();
static FINAL_COLOR_TUNE: OnceLock<String> = OnceLock::new();

/// Store final runtime state for post-exit verbose summary. Extended over
/// v50/v80 to cover EVERY live-reload-able field: msg family, dragons,
/// async, intro_color, ambient (snapback + entries), fps, glitch, bold,
/// shading, monolith, color_bg, color_tune, and (v80.0.0-alpha.1)
/// crystal_dragon_secs — enums stored as Debug labels for (was X) diffs.
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
    // v80.0.0-alpha.1: effective (post-live-reload) crystal-dragon-secs.
    crystal_dragon_secs: Option<f64>,
    // v80.0.0-beta.2 (S-master-LOGIC-1) final-state completeness:
    fps: f64,
    glitch_level: &str,
    bold_mode: &str,
    shading_mode: &str,
    monolith_size: &str,
    color_bg: bool,
    color_tune: &str,
) {
    let _ = FINAL_COLOR.set(color.to_string());
    let _ = FINAL_SCENE.set(scene.to_string());
    let _ = FINAL_CHARSET.set(charset.to_string());
    let _ = FINAL_SPEED.set(speed);
    let _ = FINAL_DENSITY.set(density);
    let _ = FINAL_MSG_MODE.set(msg_mode);
    let _ = FINAL_MESSAGE.set(message.map(|s| s.to_string()));
    let _ = FINAL_MESSAGE_BORDER.set(message_border);
    // v80.0.0-beta.1 msg-fill-style: stored as the canonical lowercase label.
    let _ = FINAL_MSG_FILL_STYLE.set(msg_fill_style.to_string());
    let _ = FINAL_POWER_DRAGON.set(power_dragon);
    let _ = FINAL_CRYSTAL_DRAGON.set(crystal_dragon);
    let _ = FINAL_ASYNC_MODE.set(async_mode);
    let _ = FINAL_INTRO_COLOR.set(intro_color.map(|s| s.to_string()));
    let _ = FINAL_AMBIENT_SNAPBACK_SECS.set(ambient_snapback_secs);
    let _ = FINAL_AMBIENT_ENTRIES.set(ambient_entries);
    let _ = FINAL_CRYSTAL_DRAGON_SECS.set(crystal_dragon_secs);
    // v80.0.0-beta.2 (S-master-LOGIC-1):
    let _ = FINAL_FPS.set(fps);
    let _ = FINAL_GLIITCH_LEVEL.set(glitch_level.to_string());
    let _ = FINAL_BOLD_MODE.set(bold_mode.to_string());
    let _ = FINAL_SHADING_MODE.set(shading_mode.to_string());
    let _ = FINAL_MONOLITH_SIZE.set(monolith_size.to_string());
    let _ = FINAL_COLOR_BG.set(color_bg);
    let _ = FINAL_COLOR_TUNE.set(color_tune.to_string());
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

/// v80.0.0-beta.1 msg-fill-style: accessor for the final reveal style label
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

/// v80.0.0-alpha.1: accessor for final crystal_dragon_secs
/// (post-live-reload). None = unset → runtime used
/// CRYSTAL_DRAGON_POLLING_SECS (60.0).
pub(crate) fn last_crystal_dragon_secs() -> Option<f64> {
    FINAL_CRYSTAL_DRAGON_SECS.get().copied().flatten()
}

/// v50.0.0-beta.7 LTS: accessor for final ambient schedule entries count
/// (post-live-reload). 0 = scheduler idles (no ambient phases configured).
pub(crate) fn last_ambient_entries() -> usize {
    *FINAL_AMBIENT_ENTRIES.get().unwrap_or(&0)
}

// v80.0.0-beta.2 (S-master-LOGIC-1) accessors — defaults mirror the
// pre-v80 startup defaults so early-exit paths (set_final_state never
// ran) degrade to the same values the old section showed.

/// Final fps target (post-live-reload / ambient ownership). Default 60.0.
pub(crate) fn last_fps() -> f64 {
    *FINAL_FPS.get().unwrap_or(&60.0)
}

/// Final glitch level label (post scene/ambient applies), e.g. "Subtle".
pub(crate) fn last_glitch_level() -> String {
    FINAL_GLIITCH_LEVEL
        .get()
        .cloned()
        .unwrap_or_else(|| "Subtle".to_string())
}

/// Final bold mode label: "Off" | "Random" | "All".
pub(crate) fn last_bold_mode() -> String {
    FINAL_BOLD_MODE
        .get()
        .cloned()
        .unwrap_or_else(|| "Random".to_string())
}

/// Final shading mode label: "Random" | "DistanceFromHead".
pub(crate) fn last_shading_mode() -> String {
    FINAL_SHADING_MODE
        .get()
        .cloned()
        .unwrap_or_else(|| "DistanceFromHead".to_string())
}

/// Final monolith size label: "Small" | "Normal" | "Large".
pub(crate) fn last_monolith_size() -> String {
    FINAL_MONOLITH_SIZE
        .get()
        .cloned()
        .unwrap_or_else(|| "Normal".to_string())
}

/// Final color-bg flag: true = default background, false = solid black.
pub(crate) fn last_color_bg() -> bool {
    *FINAL_COLOR_BG.get().unwrap_or(&false)
}

/// Final color-tune label, e.g. "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00".
pub(crate) fn last_color_tune() -> String {
    FINAL_COLOR_TUNE
        .get()
        .cloned()
        .unwrap_or_else(|| "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00".to_string())
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
    // v80.0.0-alpha.1: startup baseline for the crystal-dragon-secs knob
    // (paired with last_crystal_dragon_secs() for the (was X) suffix).
    startup_crystal_dragon_secs: Option<f64>,
    // v80.0.0-beta.2 (S-master-LOGIC-1) final-state completeness: startup
    // baselines for the seven newly tracked fields, paired with the
    // FINAL_* OnceLocks so the section discloses EVERY live-reload-able
    // dimension (the owner found bold/shading-mode edits unverifiable
    // because the section never showed them).
    startup_fps: f64,
    startup_glitch_level: &str,
    startup_bold_mode: &str,
    startup_shading_mode: &str,
    startup_monolith_size: &str,
    startup_color_bg: bool,
    startup_color_tune: &str,
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
    // v80.0.0-alpha.1: final value of the crystal-dragon-secs harmony knob.
    let final_crystal_dragon_secs = last_crystal_dragon_secs();
    // v80.0.0-beta.2 (S-master-LOGIC-1): final values for the newly
    // tracked dimensions.
    let final_fps = last_fps();
    let final_glitch_level = last_glitch_level();
    let final_bold_mode = last_bold_mode();
    let final_shading_mode = last_shading_mode();
    let final_monolith_size = last_monolith_size();
    let final_color_bg = last_color_bg();
    let final_color_tune = last_color_tune();

    // v50.0.0-rc.1: the section ALWAYS prints (exit_time + duration even
    // with no changes); per-field `if final_X != startup_X` guards below
    // still suppress unchanged fields, keeping it scannable.

    crate::output::eprintln_verbose_purple("final runtime state");

    // v50.0.0-beta.6: exit_time now uses UTC (was local + offset in rc.1).
    // UTC is LTS-stable: no DST transitions, no tzdata drift, consistent
    // across environments. Format: YYYY-MM-DD HH:MM:SSZ (ISO 8601 UTC).
    // duration unchanged — monotonic Instant elapsed since main() start.
    let exit_time = crate::clock::now_utc_datetime();
    let duration = crate::clock::format_duration_compact(start_time.elapsed());
    crate::output::eprintln_verbose(
        "  exit_time:",
        &format!(" {exit_time} | duration: {duration}"),
    );

    if final_color != startup_color {
        crate::output::eprintln_verbose(
            "  color_scheme:",
            &format!(" {} (was {})", final_color, startup_color),
        );
    }
    if final_scene != startup_scene {
        crate::output::eprintln_verbose(
            "  scene:",
            &format!(" {} (was {})", final_scene, startup_scene),
        );
    }
    if final_charset != startup_charset {
        crate::output::eprintln_verbose(
            "  charset:",
            &format!(" {} (was {})", final_charset, startup_charset),
        );
    }
    if (final_speed - startup_speed).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  speed:",
            &format!(" {:.1} (was {:.1})", final_speed, startup_speed),
        );
    }
    if (final_density - startup_density).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  density:",
            &format!(" {:.2} (was {:.2})", final_density, startup_density),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): fps + glitch_level join the
    // change-tracked motion fields — both are ambient-owned and
    // config-editable now, so a mid-run change must be verifiable here.
    if (final_fps - startup_fps).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  fps:",
            &format!(" {:.1} (was {:.1})", final_fps, startup_fps),
        );
    }
    if final_glitch_level != startup_glitch_level {
        crate::output::eprintln_verbose(
            "  glitch_level:",
            &format!(" {} (was {})", final_glitch_level, startup_glitch_level),
        );
    }
    if final_msg_mode != startup_msg_mode {
        crate::output::eprintln_verbose(
            "  msg_mode:",
            &format!(" {} (was {})", final_msg_mode, startup_msg_mode),
        );
    }
    if final_message != startup_message {
        crate::output::eprintln_verbose(
            "  message:",
            &format!(
                " {} (was {})",
                fmt_opt_str(final_message),
                fmt_opt_str(startup_message)
            ),
        );
    }
    if final_message_border != startup_message_border {
        crate::output::eprintln_verbose(
            "  message_border:",
            &format!(" {} (was {})", final_message_border, startup_message_border),
        );
    }
    // v80.0.0-beta.1 msg-fill-style: ALWAYS printed (not change-gated) so users can
    // verify the effective reveal style at session end — same policy as
    // the ambient lines below. The `(was X)` suffix appears only when a
    // live-reload edit changed it.
    let style_was_label = if final_msg_fill_style != startup_msg_fill_style {
        format!(" (was {startup_msg_fill_style})")
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  msg_fill_style:",
        &format!(" {final_msg_fill_style}{style_was_label}"),
    );
    if final_power_dragon != startup_power_dragon {
        crate::output::eprintln_verbose(
            "  power_dragon:",
            &format!(" {} (was {})", final_power_dragon, startup_power_dragon),
        );
    }
    if final_crystal_dragon != startup_crystal_dragon {
        crate::output::eprintln_verbose(
            "  crystal_dragon:",
            &format!(" {} (was {})", final_crystal_dragon, startup_crystal_dragon),
        );
    }
    if final_async_mode != startup_async_mode {
        crate::output::eprintln_verbose(
            "  async_mode:",
            &format!(" {} (was {})", final_async_mode, startup_async_mode),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): bold / shading — top-level
    // config keys since the scene-custom v2 schema removed the block
    // fields; their live-reload edits are finally verifiable here.
    if final_bold_mode != startup_bold_mode {
        crate::output::eprintln_verbose(
            "  bold:",
            &format!(" {} (was {})", final_bold_mode, startup_bold_mode),
        );
    }
    if final_shading_mode != startup_shading_mode {
        crate::output::eprintln_verbose(
            "  shading:",
            &format!(" {} (was {})", final_shading_mode, startup_shading_mode),
        );
    }
    if final_intro_color != startup_intro_color {
        crate::output::eprintln_verbose(
            "  intro_color:",
            &format!(
                " {} (was {})",
                fmt_opt_str(final_intro_color),
                fmt_opt_str(startup_intro_color)
            ),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): monolith / color_bg / color_tune
    // close the per-key coverage — every live-reload-able config key now
    // has a final-state line.
    if final_monolith_size != startup_monolith_size {
        crate::output::eprintln_verbose(
            "  monolith:",
            &format!(" {} (was {})", final_monolith_size, startup_monolith_size),
        );
    }
    if final_color_bg != startup_color_bg {
        let bg_label = |default: bool| {
            if default {
                "default"
            } else {
                "black"
            }
        };
        crate::output::eprintln_verbose(
            "  color_bg:",
            &format!(
                " {} (was {})",
                bg_label(final_color_bg),
                bg_label(startup_color_bg)
            ),
        );
    }
    if final_color_tune != startup_color_tune {
        crate::output::eprintln_verbose(
            "  color_tune:",
            &format!(" {} (was {})", final_color_tune, startup_color_tune),
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
    crate::output::eprintln_verbose(
        "  snapback_secs:",
        &format!(" {snapback_now:.1}s ({snapback_src}){snapback_was_label}"),
    );
    let entries_was_label = if final_ambient_entries != startup_ambient_entries {
        format!(" (was {})", startup_ambient_entries)
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  ambient_entries:",
        &format!(" {}{entries_was_label}", final_ambient_entries),
    );

    // v80.0.0-alpha.1: ALWAYS print the crystal-dragon-secs runtime state
    // (same always-print policy as the ambient lines) so a live-reload edit
    // to the poll cadence is verifiable at session end. `(was X)` appears
    // only when startup != final.
    let cd_secs_now = final_crystal_dragon_secs.unwrap_or(
        crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS as f64,
    );
    let cd_secs_was = startup_crystal_dragon_secs.unwrap_or(
        crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS as f64,
    );
    let cd_secs_src = if final_crystal_dragon_secs.is_some() {
        "CLI/config"
    } else {
        "default (unset — 60.0s)"
    };
    let cd_secs_was_label = if final_crystal_dragon_secs != startup_crystal_dragon_secs {
        format!(" (was {cd_secs_was:.1}s)")
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  cadence_secs:",
        &format!(" {cd_secs_now:.1}s ({cd_secs_src}){cd_secs_was_label}"),
    );

    let diag = ambient_diag_summary();
    crate::output::eprintln_verbose_purple(&format!("  {diag}"));
}

/// AB-10 (rain-screen cleanliness): emit pre-alt-screen warnings to stderr
/// BEFORE `Terminal::with_signal_exit()` enters the alternate screen
/// (otherwise the lines leak into the rain matrix). Reads the terminal
/// size via crossterm (no raw mode needed), applying the same clamp
/// `Terminal::size()` uses. Warns when: (1) `--screen-size WxH` exceeds
/// the live terminal (clipped); (2) intro requested but the terminal is
/// smaller than MIN_INTRO_COLS x MIN_INTRO_LINES (intro silently skipped).
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
