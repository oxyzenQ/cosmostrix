// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-exit final-state tracking — extracted from `interactive/mod.rs`.
//!
//! Owns the session-state snapshot ([`SessionState`]), the `FINAL_*`
//! OnceLock statics it feeds, the `last_*` accessors, and the post-exit
//! "final runtime state" verbose section. The snapshot replaces the
//! 25-parameter positional signatures that `set_final_state` and
//! `print_final_runtime_state` carried since v50 — the last three
//! `too_many_arguments` suppressions in the interactive family were
//! exactly these post-exit printers (NIGHT-hunter-21 follow-up).
//!
//! [`SessionState`] captures the same 23 live-reload-able dimensions at
//! the two moments that matter:
//!
//! - [`SessionState::from_startup`] — the resolution the session
//!   launched with (post CLI > config > scene layering).
//! - [`SessionState::from_live`] — the effective state at loop exit
//!   (post live-reload, post ambient ownership).
//!
//! `print_final_runtime_state` diffs the two: every field that moved
//! mid-session gets a `(was X)` line, so a live-reload edit is always
//! verifiable at exit. Labels are Debug strings; enums never round-trip
//! through a parser.

use std::sync::OnceLock;
use std::time::Instant;

use crate::cloud::Cloud;
use crate::color_tune::ColorTune;
use crate::runtime::ColorScheme;
use crate::CloudConfig;

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
static FINAL_GLITCH_LEVEL: OnceLock<String> = OnceLock::new();
static FINAL_BOLD_MODE: OnceLock<String> = OnceLock::new();
static FINAL_SHADING_MODE: OnceLock<String> = OnceLock::new();
static FINAL_MONOLITH_SIZE: OnceLock<String> = OnceLock::new();
static FINAL_COLOR_BG: OnceLock<bool> = OnceLock::new();
static FINAL_COLOR_TUNE: OnceLock<String> = OnceLock::new();

/// Point-in-time snapshot of every live-reload-able runtime field.
///
/// One struct serves both post-exit roles: `set_final_state` moves a
/// [`SessionState::from_live`] snapshot into the `FINAL_*` OnceLocks at
/// loop exit, and `print_final_runtime_state` diffs a
/// [`SessionState::from_startup`] snapshot against those locks. The two
/// constructors are the only production builders; tests construct the
/// literal directly.
///
/// Field docs mirror the accessors below (defaults for the early-exit
/// path where `set_final_state` never ran).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SessionState {
    /// Color label: custom palettes read `"<name> (custom)"`, builtins
    /// the Debug scheme name.
    pub color: String,
    /// Active scene name (default `"monolith"`).
    pub scene: String,
    /// Charset preset label (default `"binary"`).
    pub charset: String,
    /// Effective rain speed, chars/sec (default 9.0).
    pub speed: f32,
    /// Effective droplet density (default 0.75).
    pub density: f32,
    /// Message overlay master switch (default true).
    pub msg_mode: bool,
    /// Message overlay text, if any.
    pub message: Option<String>,
    /// Message border flag (default false).
    pub message_border: bool,
    /// Message reveal style label (default `"typewriter"`).
    pub msg_fill_style: String,
    /// Power Dragon protection flag (default true).
    pub power_dragon: bool,
    /// Crystal Dragon ambient drift flag (default false).
    pub crystal_dragon: bool,
    /// Async variable pacing flag (default true).
    pub async_mode: bool,
    /// Intro color override, if any.
    pub intro_color: Option<String>,
    /// Ambient snapback delay, seconds (None = 30.0 default).
    pub ambient_snapback_secs: Option<f64>,
    /// Ambient schedule entry count (0 = scheduler idles).
    pub ambient_entries: usize,
    /// Crystal Dragon poll cadence, seconds (None = 60.0 default).
    pub crystal_dragon_secs: Option<f64>,
    /// Effective target FPS (default 60.0).
    pub fps: f64,
    /// Glitch level Debug label (default `"Subtle"`).
    pub glitch_level: String,
    /// Bold mode Debug label: "Off" | "Random" | "All".
    pub bold_mode: String,
    /// Shading mode Debug label: "Random" | "DistanceFromHead".
    pub shading_mode: String,
    /// Monolith size Debug label: "Small" | "Normal" | "Large".
    pub monolith_size: String,
    /// Background flag: true = default, false = solid black.
    pub color_bg: bool,
    /// Color-tune label, e.g. "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00".
    pub color_tune: String,
}

impl SessionState {
    /// Startup resolution snapshot — the values the session launched
    /// with, after the full CLI > config > scene-custom > scene
    /// layering has settled (post `apply_config_and_runtime_defaults`
    /// + `build_cloud_cfg`).
    ///
    /// The `color` label follows the same shape the exit snapshot uses:
    /// a startup custom palette reads `"<name> (custom)"` (so a mid-run
    /// palette switch diffs cleanly), otherwise the Debug scheme name.
    pub(crate) fn from_startup(cloud_cfg: &CloudConfig, color_scheme: ColorScheme) -> Self {
        let color = match cloud_cfg.custom_palette_name.as_deref() {
            Some(name) => format!("{name} (custom)"),
            None => format!("{color_scheme:?}"),
        };
        Self {
            color,
            scene: cloud_cfg.scene_name.clone(),
            charset: cloud_cfg.charset_preset.clone(),
            speed: cloud_cfg.speed,
            density: cloud_cfg.density,
            msg_mode: cloud_cfg.msg_mode,
            message: cloud_cfg.message.clone(),
            message_border: cloud_cfg.message_border,
            msg_fill_style: cloud_cfg.msg_fill_style.as_str().to_string(),
            power_dragon: cloud_cfg.power_dragon,
            crystal_dragon: cloud_cfg.crystal_dragon,
            async_mode: cloud_cfg.async_mode,
            intro_color: cloud_cfg.intro_color.clone(),
            ambient_snapback_secs: cloud_cfg.ambient_snapback_secs,
            ambient_entries: cloud_cfg.ambient_schedule.entries.len(),
            crystal_dragon_secs: cloud_cfg.crystal_dragon_secs,
            fps: cloud_cfg.target_fps,
            glitch_level: format!("{:?}", cloud_cfg.glitch_level),
            bold_mode: format!("{:?}", cloud_cfg.bold_mode),
            shading_mode: format!("{:?}", cloud_cfg.shading_mode),
            monolith_size: format!("{:?}", cloud_cfg.monolith_size),
            color_bg: cloud_cfg.default_bg,
            color_tune: color_tune_label(&cloud_cfg.color_tune),
        }
    }

    /// Live-exit snapshot — the effective state at loop exit, read from
    /// the LIVE Cloud + the current (post-reload) config.
    ///
    /// v80.0.0-beta.2 (S-master-HUNT) source alignment: the palette name
    /// reads the CLOUD first (cloud.custom_palette_name — set by every
    /// set_palette activation), falling back to the config. The HUD `clr:`
    /// line reads the cloud tracker; the previous config-only read
    /// diverged whenever a runtime activation path (ambient fire,
    /// snapback, scene-runtime custom-scene switch) set the cloud name
    /// without a matching config update — the HUD said "clr: tron_legacy"
    /// while the final state printed the stale scheme enum. Both surfaces
    /// report the same source now.
    ///
    /// fps / bold / shading / monolith / color_bg / color_tune come from
    /// the EFFECTIVE config (current_cfg — post-live-reload,
    /// post-ambient-fps-ownership). glitch_level is derived from the
    /// live Cloud (the enum on the config is stale after an ambient
    /// apply writes the cloud fields directly).
    pub(crate) fn from_live(
        cloud: &Cloud,
        cfg: &CloudConfig,
        scene_name: &str,
        charset_preset: &str,
    ) -> Self {
        let color = if cloud.custom_palette_active {
            cloud
                .custom_palette_name
                .as_deref()
                .or(cfg.custom_palette_name.as_deref())
                .map(|n| format!("{n} (custom)"))
                .unwrap_or_else(|| format!("{:?}", cloud.color_scheme()))
        } else {
            format!("{:?}", cloud.color_scheme())
        };
        Self {
            color,
            scene: scene_name.to_string(),
            charset: charset_preset.to_string(),
            speed: cloud.chars_per_sec,
            density: cloud.droplet_density,
            msg_mode: cfg.msg_mode,
            message: cfg.message.clone(),
            message_border: cfg.message_border,
            msg_fill_style: cfg.msg_fill_style.as_str().to_string(),
            power_dragon: cfg.power_dragon,
            crystal_dragon: cfg.crystal_dragon,
            async_mode: cfg.async_mode,
            intro_color: cfg.intro_color.clone(),
            ambient_snapback_secs: cfg.ambient_snapback_secs,
            ambient_entries: cfg.ambient_schedule.entries.len(),
            crystal_dragon_secs: cfg.crystal_dragon_secs,
            fps: cfg.target_fps,
            glitch_level: format!("{:?}", cloud.glitch_level()),
            bold_mode: format!("{:?}", cfg.bold_mode),
            shading_mode: format!("{:?}", cfg.shading_mode),
            monolith_size: format!("{:?}", cfg.monolith_size),
            color_bg: cfg.default_bg,
            color_tune: color_tune_label(&cfg.color_tune),
        }
    }
}

/// Shared color-tune label used by both snapshot constructors (the
/// startup and exit sections must print the identical shape so the
/// `(was X)` diff is meaningful).
fn color_tune_label(tune: &ColorTune) -> String {
    format!(
        "sat={:.2} bright={:.2} head={:.2} body={:.2} tail={:.2}",
        tune.saturation, tune.brightness, tune.head, tune.body, tune.tail
    )
}

/// Store final runtime state for post-exit verbose summary. Extended over
/// v50/v80 to cover EVERY live-reload-able field: msg family, dragons,
/// async, intro_color, ambient (snapback + entries), fps, glitch, bold,
/// shading, monolith, color_bg, color_tune, and (v80.0.0-alpha.1)
/// crystal_dragon_secs — enums stored as Debug labels for (was X) diffs.
///
/// Takes the snapshot by value: this runs once, at loop exit, and every
/// field moves straight into its OnceLock.
pub(crate) fn set_final_state(state: SessionState) {
    let SessionState {
        color,
        scene,
        charset,
        speed,
        density,
        msg_mode,
        message,
        message_border,
        msg_fill_style,
        power_dragon,
        crystal_dragon,
        async_mode,
        intro_color,
        ambient_snapback_secs,
        ambient_entries,
        crystal_dragon_secs,
        fps,
        glitch_level,
        bold_mode,
        shading_mode,
        monolith_size,
        color_bg,
        color_tune,
    } = state;
    let _ = FINAL_COLOR.set(color);
    let _ = FINAL_SCENE.set(scene);
    let _ = FINAL_CHARSET.set(charset);
    let _ = FINAL_SPEED.set(speed);
    let _ = FINAL_DENSITY.set(density);
    let _ = FINAL_MSG_MODE.set(msg_mode);
    let _ = FINAL_MESSAGE.set(message);
    let _ = FINAL_MESSAGE_BORDER.set(message_border);
    // v80.0.0-beta.1 msg-fill-style: stored as the canonical lowercase label.
    let _ = FINAL_MSG_FILL_STYLE.set(msg_fill_style);
    let _ = FINAL_POWER_DRAGON.set(power_dragon);
    let _ = FINAL_CRYSTAL_DRAGON.set(crystal_dragon);
    let _ = FINAL_ASYNC_MODE.set(async_mode);
    let _ = FINAL_INTRO_COLOR.set(intro_color);
    let _ = FINAL_AMBIENT_SNAPBACK_SECS.set(ambient_snapback_secs);
    let _ = FINAL_AMBIENT_ENTRIES.set(ambient_entries);
    let _ = FINAL_CRYSTAL_DRAGON_SECS.set(crystal_dragon_secs);
    // v80.0.0-beta.2 (S-master-LOGIC-1):
    let _ = FINAL_FPS.set(fps);
    let _ = FINAL_GLITCH_LEVEL.set(glitch_level);
    let _ = FINAL_BOLD_MODE.set(bold_mode);
    let _ = FINAL_SHADING_MODE.set(shading_mode);
    let _ = FINAL_MONOLITH_SIZE.set(monolith_size);
    let _ = FINAL_COLOR_BG.set(color_bg);
    let _ = FINAL_COLOR_TUNE.set(color_tune);
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
    FINAL_GLITCH_LEVEL
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

/// Format an `Option<&str>` for the live-reload change tracker.
///
/// `Some("text")` -> `"text"` (quoted, matches verbose.rs style)
/// `None`         -> `(none)` (matches live_config/mod.rs style)
///
/// Replaces the ambiguous `{:?}` Debug format which produced
/// `Some("...")` / `None` in the verbose output — the Rust wrapper
/// made the live-reload tracker read like a REPL instead of a UX.
pub(crate) fn fmt_opt_str(opt: Option<&str>) -> String {
    match opt {
        Some(s) => format!("\"{s}\""),
        None => "(none)".to_string(),
    }
}

/// v50.0.0-alpha.7: print "final runtime state" section showing live-reload
/// changes between startup and exit. Extracted from main.rs to keep that
/// file under the 800-LOC cap; moved here from `mod.rs` with the rest of
/// the final-state family.
///
/// Diffs the startup [`SessionState`] against the final OnceLock values
/// set by [`set_final_state`]. Only prints fields that actually changed
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
pub(crate) fn print_final_runtime_state(startup: &SessionState, start_time: Instant) {
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

    if final_color != startup.color {
        crate::output::eprintln_verbose(
            "  color_scheme:",
            &format!(" {} (was {})", final_color, startup.color),
        );
    }
    if final_scene != startup.scene {
        crate::output::eprintln_verbose(
            "  scene:",
            &format!(" {} (was {})", final_scene, startup.scene),
        );
    }
    if final_charset != startup.charset {
        crate::output::eprintln_verbose(
            "  charset:",
            &format!(" {} (was {})", final_charset, startup.charset),
        );
    }
    if (final_speed - startup.speed).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  speed:",
            &format!(" {:.1} (was {:.1})", final_speed, startup.speed),
        );
    }
    if (final_density - startup.density).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  density:",
            &format!(" {:.2} (was {:.2})", final_density, startup.density),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): fps + glitch_level join the
    // change-tracked motion fields — both are ambient-owned and
    // config-editable now, so a mid-run change must be verifiable here.
    if (final_fps - startup.fps).abs() >= 0.01 {
        crate::output::eprintln_verbose(
            "  fps:",
            &format!(" {:.1} (was {:.1})", final_fps, startup.fps),
        );
    }
    if final_glitch_level != startup.glitch_level {
        crate::output::eprintln_verbose(
            "  glitch_level:",
            &format!(" {} (was {})", final_glitch_level, startup.glitch_level),
        );
    }
    if final_msg_mode != startup.msg_mode {
        crate::output::eprintln_verbose(
            "  msg_mode:",
            &format!(" {} (was {})", final_msg_mode, startup.msg_mode),
        );
    }
    if final_message != startup.message.as_deref() {
        crate::output::eprintln_verbose(
            "  message:",
            &format!(
                " {} (was {})",
                fmt_opt_str(final_message),
                fmt_opt_str(startup.message.as_deref())
            ),
        );
    }
    if final_message_border != startup.message_border {
        crate::output::eprintln_verbose(
            "  message_border:",
            &format!(" {} (was {})", final_message_border, startup.message_border),
        );
    }
    // v80.0.0-beta.1 msg-fill-style: ALWAYS printed (not change-gated) so users can
    // verify the effective reveal style at session end — same policy as
    // the ambient lines below. The `(was X)` suffix appears only when a
    // live-reload edit changed it.
    let style_was_label = if final_msg_fill_style != startup.msg_fill_style {
        format!(" (was {})", startup.msg_fill_style)
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  msg_fill_style:",
        &format!(" {final_msg_fill_style}{style_was_label}"),
    );
    if final_power_dragon != startup.power_dragon {
        crate::output::eprintln_verbose(
            "  power_dragon:",
            &format!(" {} (was {})", final_power_dragon, startup.power_dragon),
        );
    }
    if final_crystal_dragon != startup.crystal_dragon {
        crate::output::eprintln_verbose(
            "  crystal_dragon:",
            &format!(" {} (was {})", final_crystal_dragon, startup.crystal_dragon),
        );
    }
    if final_async_mode != startup.async_mode {
        crate::output::eprintln_verbose(
            "  async_mode:",
            &format!(" {} (was {})", final_async_mode, startup.async_mode),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): bold / shading — top-level
    // config keys since the scene-custom v2 schema removed the block
    // fields; their live-reload edits are finally verifiable here.
    if final_bold_mode != startup.bold_mode {
        crate::output::eprintln_verbose(
            "  bold:",
            &format!(" {} (was {})", final_bold_mode, startup.bold_mode),
        );
    }
    if final_shading_mode != startup.shading_mode {
        crate::output::eprintln_verbose(
            "  shading:",
            &format!(" {} (was {})", final_shading_mode, startup.shading_mode),
        );
    }
    if final_intro_color != startup.intro_color.as_deref() {
        crate::output::eprintln_verbose(
            "  intro_color:",
            &format!(
                " {} (was {})",
                fmt_opt_str(final_intro_color),
                fmt_opt_str(startup.intro_color.as_deref())
            ),
        );
    }
    // v80.0.0-beta.2 (S-master-LOGIC-1): monolith / color_bg / color_tune
    // close the per-key coverage — every live-reload-able config key now
    // has a final-state line.
    if final_monolith_size != startup.monolith_size {
        crate::output::eprintln_verbose(
            "  monolith:",
            &format!(" {} (was {})", final_monolith_size, startup.monolith_size),
        );
    }
    if final_color_bg != startup.color_bg {
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
                bg_label(startup.color_bg)
            ),
        );
    }
    if final_color_tune != startup.color_tune {
        crate::output::eprintln_verbose(
            "  color_tune:",
            &format!(" {} (was {})", final_color_tune, startup.color_tune),
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
    let snapback_was = startup
        .ambient_snapback_secs
        .unwrap_or(crate::constants::AUTO_SNAPBACK_DELAY_SECS);
    let snapback_src = if final_ambient_snapback_secs.is_some() {
        "config"
    } else {
        "default (unset — 30.0s)"
    };
    let snapback_was_label = if final_ambient_snapback_secs != startup.ambient_snapback_secs {
        format!(" (was {snapback_was:.1}s)")
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  snapback_secs:",
        &format!(" {snapback_now:.1}s ({snapback_src}){snapback_was_label}"),
    );
    let entries_was_label = if final_ambient_entries != startup.ambient_entries {
        format!(" (was {})", startup.ambient_entries)
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
    let cd_secs_was = startup.crystal_dragon_secs.unwrap_or(
        crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS as f64,
    );
    let cd_secs_src = if final_crystal_dragon_secs.is_some() {
        "CLI/config"
    } else {
        "default (unset — 60.0s)"
    };
    let cd_secs_was_label = if final_crystal_dragon_secs != startup.crystal_dragon_secs {
        format!(" (was {cd_secs_was:.1}s)")
    } else {
        String::new()
    };
    crate::output::eprintln_verbose(
        "  cadence_secs:",
        &format!(" {cd_secs_now:.1}s ({cd_secs_src}){cd_secs_was_label}"),
    );

    let diag = super::ambient_diag_summary();
    crate::output::eprintln_verbose_purple(&format!("  {diag}"));
}
