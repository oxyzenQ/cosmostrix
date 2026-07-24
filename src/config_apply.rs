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
use crate::config::{Args, ColorBg, GlitchLevel, IntroType};
use crate::configfile::load_config_file;
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::runtime::MonolithSize;
use crate::scene::{get_scene, validate_scene_name, DEFAULT_SCENE};
use crate::scene_custom::apply_scene_custom_layer;
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
/// Allowed: calm, pulse, signal, compression, void, monolith-pressure, adaptive.
/// Storm is unavailable and will be rejected.
fn parse_atmosphere_regime_config(name: &str, value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "calm" | "pulse" | "signal" | "compression" | "void" | "monolith-pressure" | "adaptive" => {
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
                "error: invalid {name}='{value}' (allowed: calm, pulse, signal, compression, void, monolith-pressure, adaptive)"
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
        Some("adaptive") => crate::atmosphere::AtmosphereRegime::Adaptive,
        _ => crate::atmosphere::AtmosphereRegime::Calm,
    }
}

pub(crate) fn apply_config_and_runtime_defaults(
    matches: &clap::ArgMatches,
    args: &mut Args,
) -> Result<(), String> {
    let mut config_touched = HashSet::new();

    // Security: validate --config path is in a safe location AND has .toml extension.
    // Centralized in safepath::validate_config_path so testconf, --show-scene,
    // --colors-custom, and --scene-custom all apply the same check consistently.
    if let Some(ref config_path) = args.config {
        let path_str = config_path.to_string_lossy();
        crate::validate_config_path(&path_str, args.verbose)?;
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
        crate::output::eprintln_verbose_raw(&format!(
            "config loaded from: {config_path} ({} keys)",
            cfg.len()
        ));
        // List the actual keys so the user can see exactly what is set.
        // This is critical for debugging "why is the atmosphere engine
        // running?" — the answer is almost always "atmosphere-mode and
        // atmosphere-regime are uncommented in config.toml". Without this
        // list, the user only sees "(2 keys)" and has to manually re-read
        // the config file to figure out which 2.
        if !cfg.is_empty() {
            let mut keys: Vec<&str> = cfg.keys().map(String::as_str).collect();
            keys.sort();
            crate::output::eprintln_verbose_raw(&format!("config keys: {}", keys.join(", ")));
        }
        // Surface adaptive-custom entries explicitly. These run regardless
        // of atmosphere-mode (defining them is an opt-in), so it's important
        // the user sees that the schedule is active even when the built-in
        // atmosphere engine is Disabled.
        let adaptive_custom_count = cfg
            .keys()
            .filter(|k| k.starts_with("adaptive-custom."))
            .count();
        if adaptive_custom_count > 0 {
            crate::output::eprintln_verbose_raw(
                &format!("adaptive-custom: {adaptive_custom_count} entries (active regardless of atmosphere-mode)")
            );
        }
    }

    // Strict startup validation: if config has ANY error (malformed lines,
    // unknown keys, or invalid values), exit. This matches --testconf
    // behavior: invalid config = exit code 2, not silent fallback.
    //
    // load_config_file() silently drops malformed_lines and unknown_keys
    // (only prints warnings). We re-parse the raw file to catch them.
    //
    // Test bypass: COSMOSTRIX_SKIP_STARTUP_VALIDATION=1 skips this check
    // so existing tests that verify apply/fallback logic with invalid values
    // still work. Production builds never set this env var.
    if !cfg.is_empty() && std::env::var("COSMOSTRIX_SKIP_STARTUP_VALIDATION").is_err() {
        // Re-read raw file to check malformed lines + unknown keys.
        let config_path = args
            .config
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(crate::configfile::default_config_file_path);
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            let parsed = crate::configfile::parse_config_text(&content);

            // Layer 1: malformed lines (stray text without 'key = value')
            if !parsed.malformed_lines.is_empty() {
                let lines: Vec<&str> = parsed
                    .malformed_lines
                    .iter()
                    .take(3)
                    .map(String::as_str)
                    .collect();
                return Err(format!(
                    "error: invalid config — malformed line(s): '{}' (expected 'key = value' syntax)\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
                    lines.join(", ")
                ));
            }

            // Layer 2: unknown keys (typos)
            if !parsed.unknown_keys.is_empty() {
                let keys: Vec<&str> = parsed
                    .unknown_keys
                    .iter()
                    .take(3)
                    .map(String::as_str)
                    .collect();
                return Err(format!(
                    "error: invalid config — unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys)\n\n  Fix the error above, or run 'cosmostrix --testconf' for details.",
                    keys.join(", ")
                ));
            }

            // Layer 3: invalid values (out of range, unknown enum, etc.)
            if let Err(msg) = crate::testconf::validate_config_strictly(&cfg) {
                return Err(format!(
                    "error: invalid config — {msg}\n\n  Fix the error above, or run 'cosmostrix --testconf' for details."
                ));
            }
        }
    }

    if !cfg.is_empty() {
        apply_config_values(matches, args, &cfg, &mut config_touched);
    }

    let scene_is_cli = is_explicit(matches, "scene");
    let scene_custom_is_cli = is_explicit(matches, "scene_custom");
    let scene_is_default = args.scene.is_none();
    if scene_is_default {
        args.scene = Some(DEFAULT_SCENE.to_string());
        apply_default_scene_values(matches, args, &config_touched)?;
    }

    let mut curated_modified = HashSet::new();
    if !scene_is_cli && !scene_is_default {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if !scene_custom_is_cli {
        if let Some(scene_custom_name) = args.scene_custom.clone() {
            curated_modified.extend(apply_scene_custom_layer(
                matches,
                args,
                &cfg,
                &scene_custom_name,
                false,
            )?);
        }
    }
    if scene_is_cli {
        curated_modified.extend(apply_scene_values(matches, args, &config_touched)?);
    }
    if scene_custom_is_cli {
        if let Some(scene_custom_name) = args.scene_custom.clone() {
            curated_modified.extend(apply_scene_custom_layer(
                matches,
                args,
                &cfg,
                &scene_custom_name,
                true,
            )?);
        }
    }

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
    // v17: 'preset' deprecated alias REMOVED. Use 'scene' instead.
    // Existing configs with 'preset = X' will flag it as unknown key
    // via --testconf, prompting migration to 'scene = X'.

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

    // v17: 'profile' deprecated alias REMOVED. Use --scene-custom CLI flag.
    // The [profile.<name>] table format is also REMOVED — use [scene-custom.<name>].

    // v17 mastery: scene-custom selector key REMOVED from config.toml.
    // Use the CLI flag: cosmostrix --scene-custom <name>
    // The [scene-custom.<name>] table definitions are still parsed — only
    // the top-level 'scene-custom = name' selector key is removed.
    // Rationale: having two ways to select a custom scene (CLI flag + config
    // key) was confusing. The CLI flag is the single source of truth.

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
    if let Some(v) = config_value(matches, cfg, "intro", "intro") {
        // Parse the intro type using clap's ValueEnum machinery so the
        // accepted values stay in sync with the --intro CLI flag.
        // Precedence: CLI --intro flag wins over this config key (handled
        // by `config_value` returning None when the flag is explicit).
        match IntroType::from_str(&v, true) {
            Ok(t) => {
                args.intro = Some(t);
                config_touched.insert("intro");
            }
            Err(_) => eprintln!("error: invalid intro='{v}' (allowed: cosmic, logo, none)"),
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
    // v17: 'low-power = true' deprecated alias REMOVED. Use 'scene = low-power'.
    // v17 mastery: --mouse flag deleted. Mouse effects are always-on.
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
    // v17: --async flag removed (always on). Config key 'async-mode' still
    // respected for users who want to disable it via config. No is_explicit
    // check needed since the CLI flag is gone.
    if let Some(v) = cfg.get("async-mode") {
        if let Some(b) = parse_bool_config("async-mode", v) {
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
    _matches: &clap::ArgMatches,
    _args: &mut Args,
    cfg: &HashMap<String, String>,
    _config_touched: &mut HashSet<&'static str>,
) {
    // v17 mastery: legacy advanced config keys (glitchpct, shortpct, rippct,
    // maxdpc) REMOVED. These are now fully controlled by --glitch-level.
    // The old keys are silently ignored if present in config.toml.
    // Use --glitch-level (none|subtle|default|intense) for all glitch tuning.
    let _ = cfg; // suppress unused warning
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
            // v17 mastery: glitch_pct, shortpct, rippct, max_dpc are no longer
            // CLI flags or config keys (removed legacy). Always set from the
            // glitch_level preset — no should_skip needed.
            args.glitch_pct = 3.0;
            args.glitch_ms = crate::config::U16Range {
                low: 200,
                high: 300,
            };
            args.shortpct = 60.0;
            args.rippct = 45.0;
        }
        GlitchLevel::Default => {
            if !should_skip("noglitch") {
                args.noglitch = false;
            }
            args.glitch_pct = 10.0;
            args.glitch_ms = crate::config::U16Range {
                low: 300,
                high: 400,
            };
            args.shortpct = 50.0;
            args.rippct = 33.33333;
        }
        GlitchLevel::Intense => {
            if !should_skip("noglitch") {
                args.noglitch = false;
            }
            args.glitch_pct = 25.0;
            args.glitch_ms = crate::config::U16Range {
                low: 500,
                high: 800,
            };
            args.shortpct = 30.0;
            args.rippct = 20.0;
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
