// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene-custom profile override functions — extracted from
//! `scene_custom/mod.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Owns:
//! - `apply_profile_overrides`: applies [scene-custom.<name>] block
//!   fields to Args during startup (CLI > config > scene priority).
//! - `apply_scene_custom_to_cloud_config`: applies a scene-custom block
//!   field layer to CloudConfig during live reload.
//! - `apply_scene_custom_field_to_cloud_config`: applies a single
//!   scene-custom field to CloudConfig during live reload.
//!
//! v80.0.0-beta.2 (S-master-LOGIC-3) runtime precedence contract:
//!
//! ```text
//! Startup:  CLI flags > config.toml > scene defaults > built-in defaults
//! Runtime:  user shortkeys > ambient scene > config keys (incl.
//!           scene-custom block fields) > CLI locks > scene defaults
//! ```
//!
//! The CLI wins ONLY at startup. A CLI value stays LOCKED underneath as
//! the fallback for when the config key is removed (RestoreLocked), but
//! it never blocks a present config value at runtime — the config save
//! is the most recent user intent. The old `cli_explicit` gates in the
//! live-reload field arms (Z1-1/Z2-1/FPS-F4) encoded the premature
//! "CLI always wins" model the owner rejected; they are gone.
//!
//! v80.0.0-beta.2 (S-master-HUNT) ownership refinement: the scene-custom
//! block layer overrides CLI locks only when RUNTIME CONFIG INTENT
//! selected the block (the config `scene` key, or the ambient scheduler
//! via the runtime-scene sync). A LOCK-owned tracker (startup
//! `--scene <custom>` / `--scene-custom`, or a family just restored
//! after the overlay lifted) applies block fields per-field gated on
//! the CLI locks — the startup snapshot already shadowed CLI flags over
//! block fields, and re-deriving them stomped the lock (owner bug 2:
//! `-c test -C test` never came back). See
//! `apply_scene_custom_to_cloud_config` and
//! `docs/LIVE_RELOAD_BEHAVIOR.md` §16.

use std::collections::{HashMap, HashSet};

use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::colors_custom::is_colors_custom_name;
use crate::config::{Args, GlitchLevel};
use crate::constants::DENSITY_CLAMP_MAX;

use super::helpers::{
    is_explicit, parse_f32_override, parse_f64_override, parse_speed_override, warn_invalid,
};
use super::{apply_glitch_level_preset_to_cloud_config, UserProfile};

/// NIGHT-research-5: parse the scene-custom `rain` field's string value
/// into a `RainStyle`. Returns `Some(style)` on a valid canonical label
/// (glyph, monolith, vortex, flux, lorenz, dragon, physarum), `None` on
/// invalid input (caller renders a targeted hint with the valid list).
///
/// The validation is centralized here so both the startup path
/// (`apply_profile_overrides`) and the live-reload path
/// (`apply_scene_custom_field_to_cloud_config`) agree on the accepted
/// labels. Case-insensitive — matches the existing enum-string
/// convention (`GlitchLevel::from_str(value, true)`).
fn parse_rain_style_value(value: &str) -> Option<crate::rain_style::RainStyle> {
    crate::rain_style::RainStyle::from_label(value)
}

pub(crate) fn apply_profile_overrides(
    matches: &clap::ArgMatches,
    args: &mut Args,
    name: &str,
    profile: &UserProfile,
    cfg: &HashMap<String, String>,
    modified: &mut HashSet<&'static str>,
) {
    // NIGHT-research-5: `rain` field — validate the canonical label here
    // (warn on invalid values with the valid-labels hint). The actual
    // application happens later in `resolve_rain_style` (main.rs) which
    // reads the block's `rain` value directly from cfg — there's no
    // `args.rain_style` field because rain style is resolved at Cloud
    // construction time, not at CLI parse time. The validation here is
    // still useful: it warns the user immediately at startup about a
    // typo'd label, instead of silently falling back to Glyph.
    if let Some(value) = profile.rain.as_deref() {
        if parse_rain_style_value(value).is_none() {
            warn_invalid(
                name,
                "rain",
                value,
                crate::rain_style::RainStyle::valid_labels_hint(),
            );
        }
    }
    if let Some(value) = profile
        .color
        .as_deref()
        .filter(|_| !is_explicit(matches, "color"))
    {
        let is_valid = parse_color_scheme(value).is_ok() || is_colors_custom_name(cfg, value);
        if is_valid {
            args.color = value.to_string();
            modified.insert("color");
        } else {
            warn_invalid(name, "color", value, "see --list-colors");
        }
    }
    if let Some(value) = profile
        .charset
        .as_deref()
        .filter(|_| !is_explicit(matches, "charset"))
    {
        let is_valid = charset_from_str(value, false).is_ok()
            || crate::charset_custom::load_custom_charset_if_matches(cfg, value).is_some();
        if is_valid {
            args.charset = value.to_string();
            modified.insert("charset");
        } else {
            warn_invalid(name, "charset", value, "see --list-charsets");
        }
    }
    if let Some(value) = profile
        .fps
        .as_deref()
        .filter(|_| !is_explicit(matches, "fps"))
    {
        if let Some(fps) = parse_f64_override(name, "fps", value, 1.0, 240.0) {
            args.fps = fps;
            modified.insert("fps");
            // v80.0.0-beta.2 fps-intent: a scene-custom block fps
            // (including exactly 60) is explicit user intent — record it
            // so main.rs's dynamic-default layer (144 on high-perf
            // terminals) cannot stomp it (owner bug: cp77 with fps = 60
            // showed `tgt: 144` on the HUD).
            crate::record_fps_explicit("scene-custom");
        }
    }
    if let Some(value) = profile
        .speed
        .as_deref()
        .filter(|_| !is_explicit(matches, "speed"))
    {
        if let Some(speed) = parse_speed_override(name, value) {
            args.speed = speed;
            modified.insert("speed");
        }
    }
    if let Some(value) = profile
        .density
        .as_deref()
        .filter(|_| !is_explicit(matches, "density"))
    {
        if let Some(density) = parse_f32_override(name, "density", value, 0.01, DENSITY_CLAMP_MAX) {
            args.density = density;
            modified.insert("density");
        }
    }
    if let Some(value) = profile
        .glitch_level
        .as_deref()
        .filter(|_| !is_explicit(matches, "glitch_level"))
    {
        match GlitchLevel::from_str(value, true) {
            Ok(level) => {
                args.glitch_level = level;
                modified.insert("glitch_level");
            }
            Err(_) => warn_invalid(
                name,
                "glitch-level",
                value,
                "none, subtle, default, intense",
            ),
        }
    }
    if let Some(value) = profile.colors_custom.as_deref() {
        // v80.0.0-beta.2 CLI-priority fix (Z1-1 startup parity): an
        // explicit `-c/--color` wins over the block's palette reference.
        // The live-reload path (apply_scene_custom_field_to_cloud_config)
        // already gates on `new.cli_explicit.color` — the Z1-1 comment
        // there documents "same layering as the startup path", but the
        // startup path never actually implemented the gate: `-c cosmos
        // --scene-custom cp77` silently applied the block's colors-custom
        // palette over the CLI color. Now both paths agree.
        if !is_explicit(matches, "colors_custom")
            && !is_explicit(matches, "color")
            && profile.color.is_none()
        {
            if is_colors_custom_name(cfg, value) {
                args.colors_custom = Some(value.to_string());
                modified.insert("colors_custom");
            } else {
                warn_invalid(name, "colors-custom", value, "see [colors-custom.*] blocks");
            }
        }
    }
    if let Some(value) = profile.charset_custom.as_deref() {
        if !is_explicit(matches, "charset") && profile.charset.is_none() {
            if crate::charset_custom::load_custom_charset_if_matches(cfg, value).is_some() {
                args.charset = value.to_string();
                modified.insert("charset");
            } else {
                warn_invalid(
                    name,
                    "charset-custom",
                    value,
                    "see [charset-custom.*] blocks",
                );
            }
        }
    }
}

pub(crate) fn apply_scene_custom_field_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    scene_name: &str,
    field: &str,
    value: &str,
) -> bool {
    // v80.0.0-beta.2 (S-master-LOGIC-3): NO CLI gates. The scene-custom
    // block lives in config.toml — a present block field is the most
    // recent user intent and WINS at runtime over the locked startup
    // CLI value ("cli is always wins just on startup"). The old
    // cli_explicit gates (Z1-1/Z2-1/FPS-F4) encoded the premature
    // "CLI always wins" model and made runtime scene-custom edits
    // silent no-ops (owner bug: `-c test -C test` + live-reload to a
    // scene-custom block kept showing the stale CLI color/charset on
    // the HUD). The CLI lock still exists as the FALLBACK when the
    // config `scene` key is removed (RestoreLocked rolls the whole
    // scene family back to the locked startup snapshot).
    match field {
        "rain" => {
            // NIGHT-research-5: parse the rain style label and apply to
            // CloudConfig.rain_style. Invalid labels return false (caller
            // buffers a runtime warning via live_config::push_runtime_warning).
            if let Some(style) = parse_rain_style_value(value) {
                new.rain_style = style;
                return true;
            }
            false
        }
        "color" => {
            if let Ok(scheme) = crate::cli::parse_color_scheme(value) {
                new.color_scheme = scheme;
                // Startup parity: switching to a builtin clears any
                // active custom palette — create_cloud applies the
                // palette AFTER the scheme, so a lingering palette
                // would silently shadow the scheme the block selected.
                if new.custom_palette.is_some() {
                    new.custom_palette = None;
                    new.custom_palette_name = None;
                }
                return true;
            }
            // v80.0.0-beta.2 custom-reference parity: a block `color`
            // may also name a [colors-custom.<name>] block — same
            // acceptance as the top-level `color` key.
            if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, value) {
                new.custom_palette = Some(palette);
                new.custom_palette_name = Some(value.to_string());
                return true;
            }
            false
        }
        "colors-custom" => {
            if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, value) {
                new.custom_palette = Some(palette);
                new.custom_palette_name = Some(value.to_string());
                return true;
            }
            false
        }
        "charset" => {
            if let Some(custom_chars) =
                crate::charset_custom::load_custom_charset_if_matches(cfg, value)
            {
                new.charset_preset = value.to_string();
                new.chars = custom_chars;
                return true;
            }
            if let Ok(charset) = crate::charset::charset_from_str(value, false) {
                new.charset_preset = value.to_string();
                new.chars = crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                return true;
            }
            false
        }
        "charset-custom" => {
            if let Some(custom_chars) =
                crate::charset_custom::load_custom_charset_if_matches(cfg, value)
            {
                new.charset_preset = value.to_string();
                new.chars = custom_chars;
                return true;
            }
            false
        }
        "fps" => {
            if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", value, 1.0, 240.0) {
                new.target_fps = n;
                return true;
            }
            false
        }
        "speed" => {
            if let Ok(n) = crate::validation::parse_canonical_speed("speed", value) {
                new.speed = n;
                return true;
            }
            false
        }
        "density" => {
            if let Ok(n) = crate::validation::parse_canonical_f32_range("density", value, 0.01, 5.0)
            {
                new.density = n;
                new.base_density = n;
                return true;
            }
            false
        }
        "glitch-level" => {
            // (Glitch-BUG4): shared preset helper — applies all 5 preset
            // fields, not just glitch_enabled.
            use clap::ValueEnum;
            if let Ok(level) = crate::config::GlitchLevel::from_str(value, true) {
                apply_glitch_level_preset_to_cloud_config(new, level);
                return true;
            }
            false
        }
        // Removed in v80.0.0-beta.2 (S-master-LOGIC-3 schema): bold,
        // shading-mode, async-mode (and base-scene, monolith-size,
        // color-bg) are no longer scene-custom fields — the parser
        // rejects the keys upstream, these arms are defense-in-depth.
        _ => {
            let _ = scene_name;
            false
        }
    }
}

/// Apply a scene-custom block to a CloudConfig during live reload.
///
/// v80.0.0-beta.2: no base-scene pre-pass — the block is a complete
/// self-contained profile (see scene_custom/mod.rs). Per-field
/// application is delegated to `apply_scene_custom_field_to_cloud_config`
/// (same module). On any touched field, a runtime warning is buffered via
/// `live_config::push_runtime_warning` so it lands on the main screen
/// post-exit (AB-10 rain-screen cleanliness) instead of leaking into the
/// alt screen mid-rain.
///
/// v80.0.0-beta.2 (S-master-HUNT) field-level CLI locks: when the tracker
/// is LOCK-owned (`config_owned == false` — the startup snapshot selected
/// the scene via `--scene`/`--scene-custom`, or
/// `restore_locked_scene_family` just rolled the family back after the
/// config `scene`/`ambient.*` overlay lifted), each block field is gated
/// on its CLI lock: a field the user pinned with an explicit CLI flag
/// (`-c`, `-C`, `--speed`, ...) keeps the locked startup value; the other
/// block fields still apply, so live EDITS to the block take effect for
/// non-shadowed dimensions (the v20 block-edit feature). When the tracker
/// is CONFIG-owned (`config_owned == true` — the config `scene` key or
/// the ambient scheduler selected the custom scene), no gates fire: the
/// whole block layer is present config content and wins at runtime over
/// the locked CLI values (the S-master-LOGIC-3 contract).
pub(crate) fn apply_scene_custom_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    name: &str,
    config_owned: bool,
) {
    let normalized = name.trim().to_ascii_lowercase();
    let prefix = format!("scene-custom.{normalized}.");
    let mut touched_any = false;

    // (Z1-2): conflict determinism — mirror the startup precedence from
    // `apply_profile_overrides`: inside a block, `color` beats
    // `colors-custom` (the palette field is skipped when `color` is
    // present) and `charset` beats `charset-custom`. The cfg HashMap
    // iteration below is unordered; without this pre-scan the two fields
    // could apply in either order across reloads and diverge from the
    // startup result (startup: `color` wins; reload: whichever field the
    // HashMap yields last wins).
    let has_color_field = cfg.contains_key(&format!("{prefix}color"));
    let has_charset_field = cfg.contains_key(&format!("{prefix}charset"));

    for (key, value) in cfg {
        let Some(field) = key.strip_prefix(&prefix) else {
            continue;
        };
        if field == "preset" {
            continue;
        }
        if field == "colors-custom" && has_color_field {
            continue;
        }
        if field == "charset-custom" && has_charset_field {
            continue;
        }
        // S-master-HUNT: LOCK-owned tracker — respect the per-field CLI
        // locks (see the function doc). CONFIG-owned tracker: no gates.
        if !config_owned {
            let locked = match field {
                "color" | "colors-custom" => {
                    new.cli_explicit.color || new.cli_explicit.colors_custom
                }
                "charset" | "charset-custom" => new.cli_explicit.charset,
                "fps" => new.cli_explicit.fps,
                "speed" => new.cli_explicit.speed,
                "density" => new.cli_explicit.density,
                "glitch-level" => new.cli_explicit.glitch_level,
                _ => false,
            };
            if locked {
                crate::lr_trace!(
                    "scene-custom '{normalized}': field '{field}' skipped — CLI lock owns this dimension (tracker is lock-owned)"
                );
                continue;
            }
        }
        if apply_scene_custom_field_to_cloud_config(new, cfg, &normalized, field, value) {
            touched_any = true;
        }
    }

    if touched_any {
        crate::live_config::push_runtime_warning(&format!(
            "[live-reload] scene-custom '{normalized}': re-applied fields from config"
        ));
    }
}
