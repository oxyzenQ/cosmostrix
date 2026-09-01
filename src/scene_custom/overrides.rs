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
//!   (base-scene layer + field layer) to CloudConfig during live reload.
//! - `apply_scene_custom_field_to_cloud_config`: applies a single
//!   scene-custom field to CloudConfig during live reload.

use std::collections::{HashMap, HashSet};

use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::colors_custom::is_colors_custom_name;
use crate::config::{Args, GlitchLevel};
use crate::constants::DENSITY_CLAMP_MAX;
use crate::runtime::MonolithSize;

use super::helpers::{
    is_explicit, parse_bool, parse_color_bg, parse_f32_override, parse_f64_override,
    parse_speed_override, parse_u8_override, warn_invalid,
};
use super::{
    apply_base_scene_to_cloud_config, apply_glitch_level_preset_to_cloud_config, parse_density_map,
    UserProfile,
};

pub(crate) fn apply_profile_overrides(
    matches: &clap::ArgMatches,
    args: &mut Args,
    name: &str,
    profile: &UserProfile,
    cfg: &HashMap<String, String>,
    modified: &mut HashSet<&'static str>,
) {
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
    if let Some(value) = profile
        .monolith_size
        .as_deref()
        .filter(|_| !is_explicit(matches, "monolith_size"))
    {
        match MonolithSize::from_str(value, true) {
            Ok(size) => {
                args.monolith_size = size;
                modified.insert("monolith_size");
            }
            Err(_) => warn_invalid(name, "monolith-size", value, "small, normal, large"),
        }
    }
    if let Some(value) = profile
        .color_bg
        .as_deref()
        .filter(|_| !is_explicit(matches, "color_bg"))
    {
        match parse_color_bg(value) {
            Some(bg) => {
                args.color_bg = bg;
                modified.insert("color_bg");
            }
            None => warn_invalid(name, "color-bg", value, "black, default-background"),
        }
    }
    if let Some(value) = profile
        .bold
        .as_deref()
        .filter(|_| !is_explicit(matches, "bold"))
    {
        if let Some(n) = parse_u8_override(name, "bold", value, 0, 2) {
            args.bold = n;
            modified.insert("bold");
        }
    }
    if let Some(value) = profile
        .shading_mode
        .as_deref()
        .filter(|_| !is_explicit(matches, "shading_mode"))
    {
        if let Some(n) = parse_u8_override(name, "shading-mode", value, 0, 1) {
            args.shading_mode = n;
            modified.insert("shading_mode");
        }
    }
    if let Some(value) = profile
        .async_mode
        .as_deref()
        .filter(|_| !is_explicit(matches, "async_mode"))
    {
        match parse_bool(value) {
            Some(b) => {
                args.async_mode = Some(b);
                modified.insert("async_mode");
            }
            None => warn_invalid(name, "async-mode", value, "true, false"),
        }
    }
    if let Some(value) = profile.colors_custom.as_deref() {
        if !is_explicit(matches, "colors_custom") && profile.color.is_none() {
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
    match field {
        "color" => {
            // (Z1-1): CLI gate — mirrors the FPS-F4 contract. `--color red`
            // must survive a live-reload that re-applies the scene-custom
            // block. Without this gate the block's color field silently
            // overrode the CLI flag on every config edit.
            if new.cli_explicit.color {
                return false;
            }
            if let Ok(scheme) = crate::cli::parse_color_scheme(value) {
                new.color_scheme = scheme;
                return true;
            }
            false
        }
        "colors-custom" => {
            // (Z1-1): CLI gate — an explicit `--color` wins over the block's
            // palette reference (same layering as the startup path in
            // apply_profile_overrides, which skips colors-custom when the
            // CLI color flag is explicit).
            // (Z2-1): `--colors-custom` explicit also wins — the CLI-owned
            // palette must not be replaced by the block's palette reference
            // on a live-reload re-apply.
            if new.cli_explicit.color || new.cli_explicit.colors_custom {
                return false;
            }
            if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, value) {
                new.custom_palette = Some(palette);
                new.custom_palette_name = Some(value.to_string());
                return true;
            }
            false
        }
        "charset" => {
            // (Z1-1): CLI gate — `--charset` (or its --charset-custom alias)
            // must survive a live-reload that re-applies this field.
            if new.cli_explicit.charset {
                return false;
            }
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
            // (Z1-1): CLI gate — same contract as the charset arm above.
            if new.cli_explicit.charset {
                return false;
            }
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
            // (FPS-F4): gate with cli_explicit.fps so `--fps 144`
            // survives a live-reload that re-applies the scene-custom block.
            if new.cli_explicit.fps {
                return false;
            }
            if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", value, 1.0, 240.0) {
                new.target_fps = n;
                return true;
            }
            false
        }
        "speed" => {
            // (Z1-1): CLI gate — mirrors FPS-F4 for `--speed`.
            if new.cli_explicit.speed {
                return false;
            }
            if let Ok(n) = crate::validation::parse_canonical_speed("speed", value) {
                new.speed = n;
                return true;
            }
            false
        }
        "density" => {
            // (Z1-1): CLI gate — mirrors FPS-F4 for `--density`.
            if new.cli_explicit.density {
                return false;
            }
            if let Ok(n) = crate::validation::parse_canonical_f32_range("density", value, 0.01, 5.0)
            {
                new.density = n;
                new.base_density = n;
                return true;
            }
            false
        }
        "glitch-level" => {
            // (Z1-1): CLI gate — mirrors FPS-F4 for `--glitch-level`.
            if new.cli_explicit.glitch_level {
                return false;
            }
            // (Glitch-BUG4): use shared preset helper. Was only
            // flipping glitch_enabled, leaving glitch_pct/short_pct/etc
            // stale — diverging from startup apply_custom_scene_runtime.
            use clap::ValueEnum;
            if let Ok(level) = crate::config::GlitchLevel::from_str(value, true) {
                apply_glitch_level_preset_to_cloud_config(new, level);
                return true;
            }
            false
        }
        "density-map" => {
            if let Some(map) = parse_density_map(value) {
                new.monolith_density_map = Some(map);
                return true;
            }
            false
        }
        "bold" => {
            // (Z2-1): CLI gate — `--bold` wins over the block's field
            // (startup parity: apply_profile_overrides checks is_explicit).
            if new.cli_explicit.bold {
                return false;
            }
            // v80.0.0-beta.1 killer-features hardening: enforce the SAME 0..=2 range as
            // the startup path (apply_profile_overrides uses
            // parse_u8_override(.., 0, 2)) and as --testconf validation
            // ("expected 0, 1, or 2"). Previously this arm accepted ANY u8
            // (255 mapped silently to Random), diverging from startup on
            // out-of-range values — unreachable in practice while strict
            // validation rejects the reload first, fixed as defense-in-depth
            // so the two paths can never drift again.
            match value.trim().parse::<u8>() {
                Ok(n @ 0..=2) => {
                    new.bold_mode = match n {
                        0 => crate::runtime::BoldMode::Off,
                        2 => crate::runtime::BoldMode::All,
                        _ => crate::runtime::BoldMode::Random,
                    };
                    true
                }
                _ => {
                    warn_invalid(scene_name, "bold", value, "0, 1, or 2");
                    false
                }
            }
        }
        "shading-mode" => {
            // (Z2-1): CLI gate — `--shading-mode` wins over the block's
            // field (startup parity: apply_profile_overrides checks
            // is_explicit).
            if new.cli_explicit.shading_mode {
                return false;
            }
            // v80.0.0-beta.1 killer-features hardening: enforce the SAME 0..=1 range as
            // startup (parse_u8_override(.., 0, 1)) and --testconf — see the
            // bold arm note. Previously any u8 was accepted.
            match value.trim().parse::<u8>() {
                Ok(n @ 0..=1) => {
                    new.shading_mode = match n {
                        1 => crate::runtime::ShadingMode::DistanceFromHead,
                        _ => crate::runtime::ShadingMode::Random,
                    };
                    true
                }
                _ => {
                    warn_invalid(scene_name, "shading-mode", value, "0 or 1");
                    false
                }
            }
        }
        "async-mode" => {
            // (Z1-1): CLI gate — `--async-mode` wins over the block's
            // field (startup parity).
            if new.cli_explicit.async_mode {
                return false;
            }
            // v80.0.0-beta.1 killer-features hardening: use the shared parse_bool
            // (true/false/1/0/yes/no/on/off — the SAME accepted set as the
            // startup path) instead of treating every non-true string as
            // false. A typo like `async-mode = "banana"` is now rejected
            // with a warning instead of silently switching async mode off.
            match parse_bool(value) {
                Some(b) => {
                    new.async_mode = b;
                    true
                }
                None => {
                    warn_invalid(scene_name, "async-mode", value, "true, false");
                    false
                }
            }
        }
        // monolith-size and color-bg are FORBIDDEN in scene-custom.
        "monolith-size" | "color-bg" => false,
        _ => false,
    }
}

/// Apply a scene-custom block to a CloudConfig during live reload.
///
/// pre-pass — apply base-scene's defaults BEFORE the block's own
/// overrides. This ensures overrides correctly win over base-scene defaults
/// (e.g. `base-scene = "signal", color = "neon-green"` results in neon-green,
/// not signal's aurora).
///
/// per-field application is delegated to `apply_scene_custom_field_to_cloud_config`
/// (same module). On any touched field, a runtime warning is buffered via
/// `live_config::push_runtime_warning` so it lands on the main screen
/// post-exit (AB-10 rain-screen cleanliness) instead of leaking into the
/// alt screen mid-rain.
pub(crate) fn apply_scene_custom_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    name: &str,
) {
    let normalized = name.trim().to_ascii_lowercase();
    let prefix = format!("scene-custom.{normalized}.");
    let mut touched_any = false;

    if apply_base_scene_to_cloud_config(new, cfg, &normalized) {
        touched_any = true;
    }

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
        if field == "base-scene" || field == "preset" {
            continue;
        }
        if field == "colors-custom" && has_color_field {
            continue;
        }
        if field == "charset-custom" && has_charset_field {
            continue;
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
