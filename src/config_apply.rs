// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Apply config file values, presets, and low-power defaults to parsed CLI args.
//!
//! Precedence:
//! 1. Built-in clap defaults
//! 2. Config file values
//! 3. Config preset
//! 4. Config scene
//! 5. Config profile
//! 6. CLI preset
//! 7. CLI scene
//! 8. CLI profile
//! 9. Low-power values for fields not touched by curated layers or explicit CLI
//! 10. Explicit CLI flags

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

pub(crate) fn apply_config_and_runtime_defaults(
    matches: &clap::ArgMatches,
    args: &mut Args,
) -> Result<(), String> {
    let mut config_touched = HashSet::new();
    let cfg = load_config_file(args.config.as_deref());
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
        curated_modified.extend(apply_scene_values(matches, args)?);
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
        curated_modified.extend(apply_scene_values(matches, args)?);
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
            Err(e) => eprintln!("config: ignoring invalid preset='{v}' ({e})"),
        }
    }

    if let Some(v) = config_value(matches, cfg, "scene", "scene") {
        match validate_scene_name(&v) {
            Ok(name) => {
                args.scene = Some(name);
                config_touched.insert("scene");
            }
            Err(e) => eprintln!("config: ignoring invalid scene='{v}' ({e})"),
        }
    }

    if let Some(v) = config_value(matches, cfg, "profile", "profile") {
        match validate_profile_name(&v) {
            Ok(name) => {
                args.profile = Some(name);
                config_touched.insert("profile");
            }
            Err(e) => eprintln!("config: ignoring invalid profile='{v}' ({e})"),
        }
    }

    if let Some(v) = config_value(matches, cfg, "color", "color") {
        if parse_color_scheme(&v).is_ok() {
            args.color = v;
            config_touched.insert("color");
        } else {
            eprintln!("config: ignoring invalid color='{v}' (see --list-colors)");
        }
    }
    if let Some(v) = config_value(matches, cfg, "charset", "charset") {
        if charset_from_str(&v, false).is_ok() {
            args.charset = v;
            config_touched.insert("charset");
        } else {
            eprintln!("config: ignoring invalid charset='{v}' (see --list-charsets)");
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
            Err(_) => eprintln!(
                "config: ignoring invalid monolith-size='{v}' (allowed: small, normal, large)"
            ),
        }
    }
    if let Some(v) = config_value(matches, cfg, "glitch_level", "glitch-level") {
        match GlitchLevel::from_str(&v, true) {
            Ok(level) => {
                args.glitch_level = level;
                config_touched.insert("glitch_level");
            }
            Err(_) => eprintln!(
                "config: ignoring invalid glitch-level='{v}' (allowed: none, subtle, default, intense)"
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
) -> Result<HashSet<&'static str>, String> {
    let mut scene_modified = HashSet::new();
    let Some(ref scene_name) = args.scene else {
        return Ok(scene_modified);
    };

    let name = validate_scene_name(scene_name)?;
    args.scene = Some(name.clone());

    if let Some(scene) = get_scene(&name) {
        let cfg = scene.config;
        if let Some(color) = cfg.color {
            if !is_explicit(matches, "color") {
                args.color = color.to_string();
                scene_modified.insert("color");
            }
        }
        if let Some(charset) = cfg.charset {
            if !is_explicit(matches, "charset") {
                args.charset = charset.to_string();
                scene_modified.insert("charset");
            }
        }
        if let Some(fps) = cfg.fps {
            if !is_explicit(matches, "fps") {
                args.fps = fps;
                scene_modified.insert("fps");
            }
        }
        if let Some(speed) = cfg.speed {
            if !is_explicit(matches, "speed") {
                args.speed = speed;
                scene_modified.insert("speed");
            }
        }
        if let Some(density) = cfg.density {
            if !is_explicit(matches, "density") {
                args.density = density;
                scene_modified.insert("density");
            }
        }
        if let Some(glitch_level) = cfg.glitch_level {
            if !is_explicit(matches, "glitch_level") {
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
            eprintln!(
                "config: ignoring invalid {name}='{value}' (expected: number in range {min}..={max})"
            );
            None
        }
    }
}

fn parse_f64_config(name: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    match parse_canonical_f64_range(&format!("config {name}"), value, min, max) {
        Ok(f) => Some(f),
        Err(_) => {
            eprintln!(
                "config: ignoring invalid {name}='{value}' (expected: number in range {min}..={max})"
            );
            None
        }
    }
}

fn parse_u8_config(name: &str, value: &str, min: u8, max: u8) -> Option<u8> {
    match parse_canonical_u8_range(&format!("config {name}"), value, min, max) {
        Ok(valid) => Some(valid),
        Err(_) => {
            eprintln!(
                "config: ignoring invalid {name}='{value}' (expected: number in range {min}..={max})"
            );
            None
        }
    }
}

fn parse_speed_config(name: &str, value: &str) -> Option<f32> {
    match parse_canonical_speed(&format!("config {name}"), value) {
        Ok(valid) => Some(valid),
        Err(_) => {
            eprintln!(
                "config: ignoring invalid {name}='{value}' (expected: canonical integer in range {SPEED_MIN}..={SPEED_MAX})"
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
            eprintln!("config: ignoring invalid {name}='{value}' (expected true/false)");
            None
        }
    }
}

fn parse_color_bg_config(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        "transparent" => Some(ColorBg::Transparent),
        _ => {
            eprintln!(
                "config: ignoring invalid color-bg='{value}' (allowed: black, default-background, transparent)"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::{CommandFactory, FromArgMatches};

    use super::*;
    use crate::configfile::dump_config_text;

    fn args_with_config(config: &str, cli: &[&str]) -> Args {
        let mut path = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        path.push(format!(
            "cosmostrix-config-test-{}-{unique}.conf",
            std::process::id(),
        ));
        std::fs::write(&path, config).expect("write temp config");

        let path_string = path.to_string_lossy().into_owned();
        let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
        argv.extend_from_slice(cli);

        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");

        let _ = std::fs::remove_file(path);
        args
    }

    fn args_from_cli(cli: &[&str]) -> Args {
        if cli.contains(&"--config") {
            let mut argv = vec!["cosmostrix"];
            argv.extend_from_slice(cli);
            let cmd = Args::command();
            let matches = cmd.get_matches_from(argv);
            let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
            apply_config_and_runtime_defaults(&matches, &mut args).expect("apply config");
            return args;
        }
        args_with_config("", cli)
    }

    fn args_from_cli_result(cli: &[&str]) -> Result<Args, String> {
        if cli.contains(&"--config") {
            let mut argv = vec!["cosmostrix"];
            argv.extend_from_slice(cli);
            let cmd = Args::command();
            let matches = cmd.get_matches_from(argv);
            let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
            apply_config_and_runtime_defaults(&matches, &mut args)?;
            return Ok(args);
        }

        let mut path = std::env::temp_dir();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_nanos();
        path.push(format!(
            "cosmostrix-empty-config-test-{}-{unique}.conf",
            std::process::id(),
        ));
        std::fs::write(&path, "").expect("write temp config");

        let path_string = path.to_string_lossy().into_owned();
        let mut argv = vec!["cosmostrix", "--config", path_string.as_str()];
        argv.extend_from_slice(cli);
        let cmd = Args::command();
        let matches = cmd.get_matches_from(argv);
        let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
        let result = apply_config_and_runtime_defaults(&matches, &mut args).map(|()| args);

        let _ = std::fs::remove_file(path);
        result
    }

    #[test]
    fn config_glitch_level_subtle_applies() {
        let args = args_with_config("glitch-level = subtle\n", &[]);
        assert_eq!(args.glitch_level, GlitchLevel::Subtle);
        assert_eq!(args.glitch_pct, 3.0);
        assert_eq!(args.shortpct, 60.0);
        assert!(!args.noglitch);
    }

    #[test]
    fn config_preset_calm_applies() {
        let args = args_with_config("preset = calm\n", &[]);
        assert_eq!(args.preset.as_deref(), Some("calm"));
        assert_eq!(args.color, "ocean");
        assert_eq!(args.charset, "minimal");
        assert_eq!(args.speed, 5.0);
        assert!((args.density - 0.65).abs() < f32::EPSILON);
    }

    #[test]
    fn default_scene_is_monolith() {
        let args = args_from_cli(&[]);
        assert_eq!(args.scene.as_deref(), Some("monolith"));
        assert_eq!(args.color, "cosmos");
        assert_eq!(args.charset, "binary");
        assert_eq!(args.speed, 20.0);
        assert_eq!(args.density, 0.75);
        assert_eq!(args.glitch_level, GlitchLevel::Subtle);
    }

    #[test]
    fn explicit_matrix_scene_restores_classic_defaults() {
        let args = args_from_cli(&["--scene", "matrix"]);
        assert_eq!(args.scene.as_deref(), Some("matrix"));
        assert_eq!(args.color, "green");
        assert_eq!(args.charset, "binary");
        assert_eq!(args.speed, 8.0);
        assert_eq!(args.density, 1.0);
        assert_eq!(args.glitch_level, GlitchLevel::Default);
    }

    #[test]
    fn invalid_cli_scene_is_clear_error() {
        let err = args_from_cli_result(&["--scene", "nonexistent"]).unwrap_err();
        assert_eq!(err, "invalid scene: nonexistent (see --list-scenes)");
    }

    #[test]
    fn config_scene_monolith_applies() {
        let args = args_with_config("scene = monolith\n", &[]);
        assert_eq!(args.scene.as_deref(), Some("monolith"));
        assert_eq!(args.color, "cosmos");
        assert_eq!(args.charset, "binary");
        assert_eq!(args.speed, 20.0);
        assert!((args.density - 0.75).abs() < f32::EPSILON);
        assert_eq!(args.glitch_level, GlitchLevel::Subtle);
        assert_eq!(args.glitch_pct, 3.0);
    }

    #[test]
    fn cli_scene_overrides_config_scene() {
        let args = args_with_config("scene = monolith\n", &["--scene", "signal"]);
        assert_eq!(args.scene.as_deref(), Some("signal"));
        assert_eq!(args.color, "cyan");
        assert_eq!(args.charset, "code");
        assert_eq!(args.speed, 10.0);
    }

    #[test]
    fn explicit_cli_flags_override_scene_managed_values() {
        let args = args_from_cli(&["--scene", "signal", "--color", "green", "--fps", "120"]);
        assert_eq!(args.scene.as_deref(), Some("signal"));
        assert_eq!(args.color, "green");
        assert_eq!(args.fps, 120.0);
        assert_eq!(args.charset, "code");
        assert_eq!(args.speed, 10.0);
    }

    #[test]
    fn monolith_scene_respects_explicit_color_override() {
        let args = args_from_cli(&["--scene", "monolith", "--color", "deepspace"]);
        assert_eq!(args.scene.as_deref(), Some("monolith"));
        assert_eq!(args.color, "deepspace");
        assert_eq!(args.charset, "binary");
    }

    #[test]
    fn monolith_scene_respects_explicit_motion_overrides() {
        let args = args_from_cli(&[
            "--scene",
            "monolith",
            "--fps",
            "120",
            "--speed",
            "9",
            "--density",
            "0.25",
        ]);
        assert_eq!(args.scene.as_deref(), Some("monolith"));
        assert_eq!(args.fps, 120.0);
        assert_eq!(args.speed, 9.0);
        assert!((args.density - 0.25).abs() < f32::EPSILON);
        assert_eq!(args.color, "cosmos");
    }

    #[test]
    fn config_speed_outside_safe_range_is_ignored() {
        for value in ["0", "0.5", "100.1", "1000", "100000"] {
            let args = args_with_config(&format!("speed = {value}\n"), &[]);
            assert_eq!(args.speed, 20.0);
        }
    }

    #[test]
    fn monolith_size_cli_values_parse() {
        let small = args_from_cli(&["--scene", "monolith", "--monolith-size", "small"]);
        let normal = args_from_cli(&["--scene", "monolith", "--monolith-size", "normal"]);
        let large = args_from_cli(&["--scene", "monolith", "--monolith-size", "large"]);

        assert_eq!(small.monolith_size, MonolithSize::Small);
        assert_eq!(normal.monolith_size, MonolithSize::Normal);
        assert_eq!(large.monolith_size, MonolithSize::Large);
    }

    #[test]
    fn config_monolith_size_large_applies() {
        let args = args_with_config("monolith-size = large\n", &[]);
        assert_eq!(args.monolith_size, MonolithSize::Large);
    }

    #[test]
    fn cli_scene_overrides_cli_preset_for_overlapping_values() {
        let args = args_from_cli(&["--preset", "calm", "--scene", "signal"]);
        assert_eq!(args.preset.as_deref(), Some("calm"));
        assert_eq!(args.scene.as_deref(), Some("signal"));
        assert_eq!(args.color, "cyan");
        assert_eq!(args.charset, "code");
        assert_eq!(args.speed, 10.0);
        assert!((args.density - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn cli_preset_overrides_config_scene_for_overlapping_values() {
        let args = args_with_config("scene = monolith\n", &["--preset", "storm"]);
        assert_eq!(args.scene.as_deref(), Some("monolith"));
        assert_eq!(args.preset.as_deref(), Some("storm"));
        assert_eq!(args.color, "purple");
        assert_eq!(args.charset, "cyberpunk");
        assert_eq!(args.speed, 24.0);
    }

    #[test]
    fn explicit_cli_overrides_config_value() {
        let args = args_with_config(
            "color = ocean\nfps = 30\n",
            &["--color", "red", "--fps", "60"],
        );
        assert_eq!(args.color, "red");
        assert_eq!(args.fps, 60.0);
    }

    #[test]
    fn explicit_cli_overrides_config_preset() {
        let args = args_with_config("preset = storm\n", &["--fps", "60", "--color", "green"]);
        assert_eq!(args.preset.as_deref(), Some("storm"));
        assert_eq!(args.fps, 60.0);
        assert_eq!(args.color, "green");
        assert_eq!(args.speed, 24.0);
    }

    #[test]
    fn cli_preset_overrides_config_preset() {
        let args = args_with_config("preset = calm\n", &["--preset", "storm"]);
        assert_eq!(args.preset.as_deref(), Some("storm"));
        assert_eq!(args.color, "purple");
        assert_eq!(args.charset, "cyberpunk");
        assert_eq!(args.speed, 24.0);
    }

    #[test]
    fn preset_overrides_config_managed_fields() {
        let args = args_with_config("preset = calm\ncolor = red\nspeed = 20\n", &[]);
        assert_eq!(args.color, "ocean");
        assert_eq!(args.speed, 5.0);
    }

    #[test]
    fn config_low_power_applies_after_config_without_preset() {
        let args = args_with_config(
            "fps = 120\nspeed = 30\ndensity = 2\nlow-power = true\n",
            &[],
        );
        assert_eq!(args.fps, 30.0);
        assert_eq!(args.speed, 5.0);
        assert_eq!(args.density, 0.5);
    }

    #[test]
    fn low_power_does_not_override_preset_values() {
        let args = args_from_cli(&["--preset", "storm", "--low-power"]);
        assert_eq!(args.fps, 120.0);
        assert_eq!(args.speed, 24.0);
        assert!((args.density - 1.35).abs() < f32::EPSILON);
    }

    #[test]
    fn invalid_config_values_are_ignored() {
        let args = args_with_config(
            "color = not-a-color\nfps = 0\nspeed = nope\nlow-power = maybe\npreset = unknown\n",
            &[],
        );
        assert_eq!(args.color, "cosmos");
        assert_eq!(args.fps, 60.0);
        assert_eq!(args.speed, 20.0);
        assert!(!args.low_power);
        assert!(args.preset.is_none());
    }

    #[test]
    fn legacy_keys_still_apply() {
        let args = args_with_config(
            "glitchpct = 7\nshortpct = 22\nrippct = 11\nmaxdpc = 2\n",
            &[],
        );
        assert_eq!(args.glitch_pct, 7.0);
        assert_eq!(args.shortpct, 22.0);
        assert_eq!(args.rippct, 11.0);
        assert_eq!(args.max_droplets_per_column, 2);
    }

    #[test]
    fn config_path_arg_is_stored() {
        let args = args_from_cli(&["--config", "/tmp/cosmostrix.conf"]);
        assert_eq!(args.config, Some(PathBuf::from("/tmp/cosmostrix.conf")));
    }

    #[test]
    fn dump_config_mentions_supported_keys() {
        let dump = dump_config_text();
        for key in [
            "scene",
            "preset",
            "color",
            "charset",
            "fps",
            "speed",
            "density",
            "monolith-size",
            "glitch-level",
            "bold",
            "shadingmode",
            "color-bg",
            "low-power",
            "mouse",
            "fullwidth",
            "glitchpct",
            "shortpct",
            "rippct",
            "maxdpc",
        ] {
            assert!(dump.contains(key), "dump config should contain {key}");
        }
        assert!(dump.contains("scene = monolith"));
        assert!(dump.contains("speed = 20"));
        assert!(dump.contains("density = 0.75"));
        assert!(dump.contains("glitch-level = subtle"));
    }
}
