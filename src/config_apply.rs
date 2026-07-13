// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Apply config file values, presets, and low-power defaults to parsed CLI args.
//!
//! Precedence (highest wins):
//! 1. Built-in clap defaults
//! 2. Scene defaults (only for keys NOT set in config — fills the gaps)
//! 3. Config file values (always wins over scene defaults for user-set keys)
//! 4. Config preset
//! 5. Config profile
//! 6. CLI preset
//! 7. CLI scene (still respects config-set keys; only fills unset keys)
//! 8. CLI profile
//! 9. Low-power values for fields not touched by curated layers or explicit CLI
//! 10. Explicit CLI flags
//!
//! Key rule: a value explicitly set in config.toml ALWAYS wins over a scene's
//! hardcoded default. Scenes are templates for *unset* keys, not overrides for
//! user-set keys. This prevents the surprise where `speed = 30` in config gets
//! silently overwritten by a scene's `speed = 8`.

use std::collections::{HashMap, HashSet};

use clap::parser::ValueSource;
use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::config::{Args, ColorBg, GlitchLevel};
use crate::configfile::load_config_file;
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::preset::{get_preset, validate_preset_name};
use crate::profile::{apply_profile_layer, collect_profiles, validate_profile_name};
use crate::runtime::MonolithSize;
use crate::scene::{get_scene, validate_scene_name, DEFAULT_SCENE};
use crate::validation::{
    parse_canonical_f32_range, parse_canonical_f64_range, parse_canonical_speed,
    parse_canonical_u8_range,
};

/// Validate atmosphere-mode config value.
/// Allowed: disabled, controlled-live. Storm is NOT config-safe.
fn parse_atmosphere_mode_config(name: &str, value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "controlled-live" => Some(value.trim().to_ascii_lowercase()),
        _ => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (allowed: disabled, controlled-live)"
            ));
            None
        }
    }
}

/// Validate atmosphere-regime config value.
/// Allowed: calm, pulse, signal, compression, void, monolith-pressure.
/// Storm is unavailable and will be rejected.
fn parse_atmosphere_regime_config(name: &str, value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "calm" | "pulse" | "signal" | "compression" | "void" | "monolith-pressure" => {
            Some(value.trim().to_ascii_lowercase())
        }
        "storm" => {
            crate::output::eprintln_error_labeled(
                "rejecting atmosphere-regime='storm' — storm is unavailable",
            );
            None
        }
        _ => {
            eprintln!(
                "error: invalid {name}='{value}' (allowed: calm, pulse, signal, compression, void, monolith-pressure)"
            );
            None
        }
    }
}

/// Resolve atmosphere mode from the config string value.
/// Returns Disabled (default) if the value is "disabled" or None.
/// Returns ControlledLive if the value is "controlled-live".
#[must_use]
pub(crate) fn resolve_atmosphere_mode(
    mode_str: Option<&str>,
) -> crate::atmosphere_apply::AtmosphereApplicationMode {
    match mode_str {
        Some("controlled-live") => {
            crate::atmosphere_apply::AtmosphereApplicationMode::ControlledLive
        }
        _ => crate::atmosphere_apply::AtmosphereApplicationMode::Disabled,
    }
}

/// Resolve atmosphere regime from the config string value.
/// Returns Calm (default) if the value is "calm" or None.
/// Returns the corresponding AtmosphereRegime for valid values.
/// Storm is never returned — it's rejected at the parsing layer.
#[must_use]
pub(crate) fn resolve_atmosphere_regime(
    regime_str: Option<&str>,
) -> crate::atmosphere::AtmosphereRegime {
    match regime_str {
        Some("pulse") => crate::atmosphere::AtmosphereRegime::Pulse,
        Some("signal") => crate::atmosphere::AtmosphereRegime::Signal,
        Some("compression") => crate::atmosphere::AtmosphereRegime::Compression,
        Some("void") => crate::atmosphere::AtmosphereRegime::Void,
        Some("monolith-pressure") => crate::atmosphere::AtmosphereRegime::MonolithPressure,
        _ => crate::atmosphere::AtmosphereRegime::Calm,
    }
}

pub(crate) fn apply_config_and_runtime_defaults(
    matches: &clap::ArgMatches,
    args: &mut Args,
) -> Result<(), String> {
    let mut config_touched = HashSet::new();

    // Security: validate --config path is in a safe location AND has .toml extension.
    if let Some(ref config_path) = args.config {
        let path_str = config_path.to_string_lossy();
        let safe = crate::is_safe_path(&path_str);
        if args.verbose {
            eprintln!("[verbose] config path: {path_str} (safe: {safe})");
        }
        if !safe {
            return Err(format!(
                "error: --config '{path_str}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, current directory (.), /etc/cosmostrix/, /tmp/"
            ));
        }
        // Strict: only .toml files allowed. Prevents reading arbitrary
        // file types (.c, .txt, .py, .sh, etc.) via --config.
        if !path_str.ends_with(".toml") {
            return Err(format!(
                "error: --config '{path_str}' must have a .toml extension\n  \
                 Only TOML config files are accepted."
            ));
        }
    }

    let cfg = load_config_file(args.config.as_deref());
    if args.verbose {
        let config_path = args
            .config
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                crate::configfile::default_config_file_path()
                    .to_string_lossy()
                    .into_owned()
            });
        eprintln!(
            "[verbose] config loaded from: {config_path} ({} keys)",
            cfg.len()
        );
    }
    let profiles = collect_profiles(&cfg);
    if !cfg.is_empty() {
        apply_config_values(matches, args, &cfg, &mut config_touched);
    }

    let preset_is_cli = is_explicit(matches, "preset");
    let scene_is_cli = is_explicit(matches, "scene");
    let profile_is_cli = is_explicit(matches, "profile");
    let scene_is_default = args.scene.is_none();
    if scene_is_default {
        args.scene = Some(DEFAULT_SCENE.to_string());
        apply_default_scene_values(matches, args, &config_touched)?;
    }

    let mut curated_modified = HashSet::new();
    if !preset_is_cli {
        curated_modified.extend(apply_preset_values(matches, args)?);
    }
    if !scene_is_cli && !scene_is_default {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if !profile_is_cli {
        if let Some(profile_name) = args.profile.clone() {
            curated_modified.extend(apply_profile_layer(
                matches,
                args,
                &profiles,
                &profile_name,
                false,
            )?);
        }
    }
    if preset_is_cli {
        curated_modified.extend(apply_preset_values(matches, args)?);
    }
    if scene_is_cli {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if profile_is_cli {
        if let Some(profile_name) = args.profile.clone() {
            curated_modified.extend(apply_profile_layer(
                matches,
                args,
                &profiles,
                &profile_name,
                true,
            )?);
        }
    }

    apply_low_power_values(matches, args, &curated_modified);
    apply_glitch_level_values(matches, args, &config_touched, &curated_modified);

    Ok(())
}

fn apply_default_scene_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
) -> Result<(), String> {
    let Some(scene) = get_scene(DEFAULT_SCENE) else {
        return Ok(());
    };
    let cfg = scene.config;
    if let Some(color) = cfg.color {
        if !is_explicit(matches, "color") && !config_touched.contains("color") {
            args.color = color.to_string();
        }
    }
    if let Some(charset) = cfg.charset {
        if !is_explicit(matches, "charset") && !config_touched.contains("charset") {
            args.charset = charset.to_string();
        }
    }
    if let Some(fps) = cfg.fps {
        if !is_explicit(matches, "fps") && !config_touched.contains("fps") {
            args.fps = fps;
        }
    }
    if let Some(speed) = cfg.speed {
        if !is_explicit(matches, "speed") && !config_touched.contains("speed") {
            args.speed = speed;
        }
    }
    if let Some(density) = cfg.density {
        if !is_explicit(matches, "density") && !config_touched.contains("density") {
            args.density = density;
        }
    }
    if let Some(glitch_level) = cfg.glitch_level {
        if !is_explicit(matches, "glitch_level") && !config_touched.contains("glitch_level") {
            args.glitch_level = glitch_level;
        }
    }
    Ok(())
}

fn apply_config_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    cfg: &HashMap<String, String>,
    config_touched: &mut HashSet<&'static str>,
) {
    if let Some(v) = config_value(matches, cfg, "preset", "preset") {
        match validate_preset_name(&v) {
            Ok(name) => {
                args.preset = Some(name);
                config_touched.insert("preset");
            }
            Err(e) => crate::output::eprintln_error_labeled(&format!("invalid preset='{v}' ({e})")),
        }
    }

    if let Some(v) = config_value(matches, cfg, "scene", "scene") {
        match validate_scene_name(&v) {
            Ok(name) => {
                args.scene = Some(name);
                config_touched.insert("scene");
            }
            Err(e) => {
                // Strip the "error: " prefix from validate_scene_name's message
                // since eprintln_error_labeled adds its own "error:" label.
                let msg = e.strip_prefix("error: ").unwrap_or(&e);
                crate::output::eprintln_error_labeled(msg);
            }
        }
    }

    if let Some(v) = config_value(matches, cfg, "profile", "profile") {
        match validate_profile_name(&v) {
            Ok(name) => {
                args.profile = Some(name);
                config_touched.insert("profile");
            }
            Err(e) => {
                crate::output::eprintln_error_labeled(&format!(
                    "unknown profile '{v}' ({e}; see --list-profiles)"
                ));
            }
        }
    }

    if let Some(v) = config_value(matches, cfg, "color", "color") {
        if parse_color_scheme(&v).is_ok() {
            args.color = v;
            config_touched.insert("color");
        } else {
            crate::output::eprintln_error_labeled(&format!(
                "invalid color='{v}' (see --list-colors)"
            ));
        }
    }
    if let Some(v) = config_value(matches, cfg, "charset", "charset") {
        if charset_from_str(&v, false).is_ok() {
            args.charset = v;
            config_touched.insert("charset");
        } else {
            crate::output::eprintln_error_labeled(&format!(
                "invalid charset='{v}' (see --list-charsets)"
            ));
        }
    }
    if let Some(v) = config_value(matches, cfg, "fps", "fps") {
        if let Some(f) = parse_f64_config("fps", &v, 1.0, 240.0) {
            args.fps = f;
            config_touched.insert("fps");
        }
    }
    if let Some(v) = config_value(matches, cfg, "speed", "speed") {
        if let Some(f) = parse_speed_config("speed", &v) {
            args.speed = f;
            config_touched.insert("speed");
        }
    }
    if let Some(v) = config_value(matches, cfg, "density", "density") {
        if let Some(f) = parse_f32_config("density", &v, 0.01, DENSITY_CLAMP_MAX) {
            args.density = f;
            config_touched.insert("density");
        }
    }
    if let Some(v) = config_value(matches, cfg, "monolith_size", "monolith-size") {
        match MonolithSize::from_str(&v, true) {
            Ok(size) => {
                args.monolith_size = size;
                config_touched.insert("monolith_size");
            }
            Err(_) => {
                crate::output::eprintln_error_labeled(&format!(
                    "invalid monolith-size='{v}' (allowed: small, normal, large)"
                ));
            }
        }
    }
    if let Some(v) = config_value(matches, cfg, "glitch_level", "glitch-level") {
        match GlitchLevel::from_str(&v, true) {
            Ok(level) => {
                args.glitch_level = level;
                config_touched.insert("glitch_level");
            }
            Err(_) => eprintln!(
                "error: invalid glitch-level='{v}' (allowed: none, subtle, default, intense)"
            ),
        }
    }
    if let Some(v) = config_value(matches, cfg, "bold", "bold") {
        if let Some(n) = parse_u8_config("bold", &v, 0, 2) {
            args.bold = n;
            config_touched.insert("bold");
        }
    }
    if let Some(v) = config_value(matches, cfg, "shading_mode", "shadingmode") {
        if let Some(n) = parse_u8_config("shadingmode", &v, 0, 1) {
            args.shading_mode = n;
            config_touched.insert("shading_mode");
        }
    }
    if let Some(v) = config_value(matches, cfg, "color_bg", "color-bg") {
        if let Some(bg) = parse_color_bg_config(&v) {
            args.color_bg = bg;
            config_touched.insert("color_bg");
        }
    }
    if let Some(v) = config_value(matches, cfg, "low_power", "low-power") {
        if let Some(b) = parse_bool_config("low-power", &v) {
            args.low_power = b;
            config_touched.insert("low_power");
        }
    }
    if let Some(v) = config_value(matches, cfg, "mouse", "mouse") {
        if let Some(b) = parse_bool_config("mouse", &v) {
            args.mouse = b;
            config_touched.insert("mouse");
        }
    }
    if let Some(v) = config_value(matches, cfg, "fullwidth", "fullwidth") {
        if let Some(b) = parse_bool_config("fullwidth", &v) {
            args.fullwidth = b;
            config_touched.insert("fullwidth");
        }
    }
    if let Some(v) = config_value(matches, cfg, "auto_color_drift", "auto-color-drift") {
        if let Some(b) = parse_bool_config("auto-color-drift", &v) {
            args.auto_color_drift = b;
            config_touched.insert("auto_color_drift");
        }
    }
    if let Some(v) = config_value(matches, cfg, "async_mode", "async-mode") {
        if let Some(b) = parse_bool_config("async-mode", &v) {
            args.async_mode = b;
            config_touched.insert("async_mode");
        }
    }
    if let Some(v) = config_value(matches, cfg, "atmosphere_mode_str", "atmosphere-mode") {
        if let Some(valid) = parse_atmosphere_mode_config("atmosphere-mode", &v) {
            args.atmosphere_mode_str = Some(valid);
            config_touched.insert("atmosphere_mode_str");
        }
    }
    if let Some(v) = config_value(matches, cfg, "atmosphere_regime_str", "atmosphere-regime") {
        if let Some(valid) = parse_atmosphere_regime_config("atmosphere-regime", &v) {
            args.atmosphere_regime_str = Some(valid);
            config_touched.insert("atmosphere_regime_str");
        }
    }

    apply_legacy_config(matches, args, cfg, config_touched);
}

fn apply_legacy_config(
    matches: &clap::ArgMatches,
    args: &mut Args,
    cfg: &HashMap<String, String>,
    config_touched: &mut HashSet<&'static str>,
) {
    if let Some(v) = config_value(matches, cfg, "glitch_pct", "glitchpct") {
        if let Some(f) = parse_f32_config("glitchpct", &v, 0.0, 100.0) {
            args.glitch_pct = f;
            config_touched.insert("glitch_pct");
        }
    }
    if let Some(v) = config_value(matches, cfg, "shortpct", "shortpct") {
        if let Some(f) = parse_f32_config("shortpct", &v, 0.0, 100.0) {
            args.shortpct = f;
            config_touched.insert("shortpct");
        }
    }
    if let Some(v) = config_value(matches, cfg, "rippct", "rippct") {
        if let Some(f) = parse_f32_config("rippct", &v, 0.0, 100.0) {
            args.rippct = f;
            config_touched.insert("rippct");
        }
    }
    if let Some(v) = config_value(matches, cfg, "max_droplets_per_column", "maxdpc") {
        if let Some(n) = parse_u8_config("maxdpc", &v, 1, 3) {
            args.max_droplets_per_column = n;
            config_touched.insert("max_droplets_per_column");
        }
    }
}

fn apply_preset_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
) -> Result<HashSet<&'static str>, String> {
    let mut preset_modified = HashSet::new();
    let Some(ref preset_name) = args.preset else {
        return Ok(preset_modified);
    };

    let name = validate_preset_name(preset_name)?;
    args.preset = Some(name.clone());

    if let Some(p) = get_preset(&name) {
        if !is_explicit(matches, "color") {
            args.color = p.color.to_string();
            preset_modified.insert("color");
        }
        if !is_explicit(matches, "charset") {
            args.charset = p.charset.to_string();
            preset_modified.insert("charset");
        }
        if !is_explicit(matches, "fps") {
            args.fps = p.fps;
            preset_modified.insert("fps");
        }
        if !is_explicit(matches, "speed") {
            args.speed = p.speed;
            preset_modified.insert("speed");
        }
        if !is_explicit(matches, "density") {
            args.density = p.density;
            preset_modified.insert("density");
        }
        if !is_explicit(matches, "glitch_level") {
            args.glitch_level = p.glitch_level;
            preset_modified.insert("glitch_level");
        }
    }

    Ok(preset_modified)
}

fn apply_scene_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
) -> Result<HashSet<&'static str>, String> {
    let mut scene_modified = HashSet::new();
    let Some(ref scene_name) = args.scene else {
        return Ok(scene_modified);
    };

    let name = validate_scene_name(scene_name)?;
    args.scene = Some(name.clone());

    if let Some(scene) = get_scene(&name) {
        let cfg = scene.config;
        // Scene defaults only apply to keys NOT explicitly set by the user
        // in config.toml. This mirrors the apply_default_scene_values
        // pattern: config-set keys win over scene defaults. CLI flags
        // still win over both (checked via is_explicit).
        if let Some(color) = cfg.color {
            if !is_explicit(matches, "color") && !config_touched.contains("color") {
                args.color = color.to_string();
                scene_modified.insert("color");
            }
        }
        if let Some(charset) = cfg.charset {
            if !is_explicit(matches, "charset") && !config_touched.contains("charset") {
                args.charset = charset.to_string();
                scene_modified.insert("charset");
            }
        }
        if let Some(fps) = cfg.fps {
            if !is_explicit(matches, "fps") && !config_touched.contains("fps") {
                args.fps = fps;
                scene_modified.insert("fps");
            }
        }
        if let Some(speed) = cfg.speed {
            if !is_explicit(matches, "speed") && !config_touched.contains("speed") {
                args.speed = speed;
                scene_modified.insert("speed");
            }
        }
        if let Some(density) = cfg.density {
            if !is_explicit(matches, "density") && !config_touched.contains("density") {
                args.density = density;
                scene_modified.insert("density");
            }
        }
        if let Some(glitch_level) = cfg.glitch_level {
            if !is_explicit(matches, "glitch_level") && !config_touched.contains("glitch_level") {
                args.glitch_level = glitch_level;
                scene_modified.insert("glitch_level");
            }
        }
    }

    Ok(scene_modified)
}

fn apply_low_power_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    curated_modified: &HashSet<&'static str>,
) {
    if !args.low_power {
        return;
    }

    if !is_explicit(matches, "fps") && !curated_modified.contains("fps") {
        args.fps = 30.0;
    }
    if !is_explicit(matches, "speed") && !curated_modified.contains("speed") {
        args.speed = 5.0;
    }
    if !is_explicit(matches, "density") && !curated_modified.contains("density") {
        args.density = 0.5;
    }
}

fn apply_glitch_level_values(
    matches: &clap::ArgMatches,
    args: &mut Args,
    config_touched: &HashSet<&'static str>,
    curated_modified: &HashSet<&'static str>,
) {
    let high_precedence_glitch_level =
        is_explicit(matches, "glitch_level") || curated_modified.contains("glitch_level");

    let should_skip = |arg_id: &'static str| {
        is_explicit(matches, arg_id)
            || (config_touched.contains(arg_id) && !high_precedence_glitch_level)
    };

    match args.glitch_level {
        GlitchLevel::None => {
            if !should_skip("noglitch") {
                args.noglitch = true;
            }
        }
        GlitchLevel::Subtle => {
            if !should_skip("noglitch") {
                args.noglitch = false;
            }
            if !should_skip("glitch_pct") {
                args.glitch_pct = 3.0;
            }
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 200,
                    high: 300,
                };
            }
            if !should_skip("shortpct") {
                args.shortpct = 60.0;
            }
            if !should_skip("rippct") {
                args.rippct = 45.0;
            }
        }
        GlitchLevel::Default => {
            if !should_skip("noglitch") {
                args.noglitch = false;
            }
            if !should_skip("glitch_pct") {
                args.glitch_pct = 10.0;
            }
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 300,
                    high: 400,
                };
            }
            if !should_skip("shortpct") {
                args.shortpct = 50.0;
            }
            if !should_skip("rippct") {
                args.rippct = 33.33333;
            }
        }
        GlitchLevel::Intense => {
            if !should_skip("noglitch") {
                args.noglitch = false;
            }
            if !should_skip("glitch_pct") {
                args.glitch_pct = 25.0;
            }
            if !should_skip("glitch_ms") {
                args.glitch_ms = crate::config::U16Range {
                    low: 500,
                    high: 800,
                };
            }
            if !should_skip("shortpct") {
                args.shortpct = 30.0;
            }
            if !should_skip("rippct") {
                args.rippct = 20.0;
            }
        }
    }
}

fn config_value(
    matches: &clap::ArgMatches,
    cfg: &HashMap<String, String>,
    arg_id: &str,
    config_key: &str,
) -> Option<String> {
    if is_explicit(matches, arg_id) {
        None
    } else {
        cfg.get(config_key).cloned()
    }
}

#[inline]
fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
    !matches!(
        matches.value_source(key),
        None | Some(ValueSource::DefaultValue)
    )
}

fn parse_f32_config(name: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    match parse_canonical_f32_range(&format!("config {name}"), value, min, max) {
        Ok(f) => Some(f),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_f64_config(name: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    match parse_canonical_f64_range(&format!("config {name}"), value, min, max) {
        Ok(f) => Some(f),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_u8_config(name: &str, value: &str, min: u8, max: u8) -> Option<u8> {
    match parse_canonical_u8_range(&format!("config {name}"), value, min, max) {
        Ok(valid) => Some(valid),
        Err(_) => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected: number in range {min}..={max})"
            ));
            None
        }
    }
}

fn parse_speed_config(name: &str, value: &str) -> Option<f32> {
    match parse_canonical_speed(&format!("config {name}"), value) {
        Ok(valid) => Some(valid),
        Err(_) => {
            eprintln!(
                "error: invalid {name}='{value}' (expected: canonical integer in range {SPEED_MIN}..={SPEED_MAX})"
            );
            None
        }
    }
}

fn parse_bool_config(name: &str, value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid {name}='{value}' (expected true/false)"
            ));
            None
        }
    }
}

fn parse_color_bg_config(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        _ => {
            crate::output::eprintln_error_labeled(&format!(
                "invalid color-bg='{value}' (allowed: black, default-background)"
            ));
            None
        }
    }
}
