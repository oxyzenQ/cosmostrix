// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined profile support for flat `key = value` config files.
//!
//! Profiles are intentionally lightweight. They reuse existing scenes and
//! presets as foundations, then override only already-supported runtime fields.

use std::collections::{BTreeMap, HashSet};

use clap::parser::ValueSource;
use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::config::{Args, ColorBg, GlitchLevel};
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::preset::{get_preset, validate_preset_name};
use crate::runtime::MonolithSize;
use crate::scene::{get_scene, validate_scene_name, DEFAULT_SCENE};
use crate::validation::{
    parse_canonical_f32_range, parse_canonical_f64_range, parse_canonical_speed,
};

pub const PROFILE_FIELDS: &[&str] = &[
    "base",
    "scene",
    "preset",
    "color",
    "charset",
    "fps",
    "speed",
    "density",
    "glitch-level",
    "monolith-size",
    "color-bg",
    "atmosphere-mode",
    "atmosphere-regime",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct UserProfile {
    pub base: Option<String>,
    pub preset: Option<String>,
    pub color: Option<String>,
    pub charset: Option<String>,
    pub fps: Option<String>,
    pub speed: Option<String>,
    pub density: Option<String>,
    pub glitch_level: Option<String>,
    pub monolith_size: Option<String>,
    pub color_bg: Option<String>,
    pub atmosphere_mode: Option<String>,
    pub atmosphere_regime: Option<String>,
}

#[must_use]
pub fn is_profile_config_key(key: &str) -> bool {
    let Some((prefix, rest)) = key.split_once('.') else {
        return false;
    };
    if prefix != "profile" {
        return false;
    }
    let Some((name, field)) = rest.rsplit_once('.') else {
        return false;
    };
    is_valid_profile_name(name) && PROFILE_FIELDS.contains(&field)
}

#[must_use]
pub fn collect_profiles(
    cfg: &std::collections::HashMap<String, String>,
) -> BTreeMap<String, UserProfile> {
    let mut profiles = BTreeMap::new();
    for (key, value) in cfg {
        if !is_profile_config_key(key) {
            continue;
        }
        let (_, rest) = key.split_once('.').expect("profile key has prefix");
        let (name, field) = rest.rsplit_once('.').expect("profile key has field");
        let profile = profiles
            .entry(name.to_string())
            .or_insert_with(UserProfile::default);
        match field {
            "base" | "scene" => profile.base = Some(value.clone()),
            "preset" => profile.preset = Some(value.clone()),
            "color" => profile.color = Some(value.clone()),
            "charset" => profile.charset = Some(value.clone()),
            "fps" => profile.fps = Some(value.clone()),
            "speed" => profile.speed = Some(value.clone()),
            "density" => profile.density = Some(value.clone()),
            "glitch-level" => profile.glitch_level = Some(value.clone()),
            "monolith-size" => profile.monolith_size = Some(value.clone()),
            "color-bg" => profile.color_bg = Some(value.clone()),
            "atmosphere-mode" => profile.atmosphere_mode = Some(value.clone()),
            "atmosphere-regime" => profile.atmosphere_regime = Some(value.clone()),
            _ => {}
        }
    }
    profiles
}

pub fn validate_profile_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if is_valid_profile_name(&normalized) {
        Ok(normalized)
    } else {
        Err(format!(
            "error: invalid profile: {name}\nexpected: letters, digits, '-' or '_'"
        ))
    }
}

pub fn apply_profile_layer(
    matches: &clap::ArgMatches,
    args: &mut Args,
    profiles: &BTreeMap<String, UserProfile>,
    name: &str,
    strict_unknown: bool,
) -> Result<HashSet<&'static str>, String> {
    let mut modified = HashSet::new();
    let normalized = validate_profile_name(name)?;
    let Some(profile) = profiles.get(&normalized) else {
        let message = format!(
            "error: unknown profile '{name}'\nexpected one of: {}\n\n  Use --list-profiles to see available profiles.",
            profile_name_list(profiles)
        );
        if strict_unknown {
            return Err(message);
        }
        eprintln!(
            "config: ignoring unknown profile '{name}' (available: {}; see --list-profiles)",
            profile_name_list(profiles)
        );
        return Ok(modified);
    };

    args.profile = Some(normalized.clone());

    if let Some(preset) = profile.preset.as_deref() {
        apply_profile_preset(matches, args, preset, &mut modified);
    }

    let base_scene = profile
        .base
        .as_deref()
        .or(args.scene.as_deref())
        .unwrap_or(DEFAULT_SCENE);
    if let Ok(base) = validate_scene_name(base_scene) {
        args.scene = Some(base.clone());
        apply_profile_scene(matches, args, &base, &mut modified);
    } else {
        eprintln!(
            "profile: invalid base='{base_scene}' in profile '{normalized}' (see --list-scenes)"
        );
    }

    apply_profile_overrides(matches, args, &normalized, profile, &mut modified);
    Ok(modified)
}

/// Produce a concise section listing controlled atmosphere presets.
///
/// This is appended to `--list-profiles` output so users discover the
/// available atmosphere profiles without needing to read docs first.
/// No preset is default; all are opt-in only.
fn atmosphere_presets_section() -> String {
    use crate::atmosphere_presets::all_atmosphere_presets;
    let presets = all_atmosphere_presets();
    let mut out = String::from("\nCONTROLLED ATMOSPHERE PRESETS (opt-in only)\n\n");
    out.push_str("  Presets are opt-in. Default remains disabled/protected/identity.\n");
    out.push_str("  Storm preset does not exist. See docs/ATMOSPHERE_PRESETS.md\n");
    out.push_str("  See also: docs/PROFILE_ECOSYSTEM.md\n\n");
    for p in &presets {
        out.push_str(&format!(
            "  {:30} mode={} regime={} shadow={}\n",
            p.name, p.mode, p.regime, p.expected_shadow
        ));
    }
    out
}

#[must_use]
pub fn list_profiles_text(profiles: &BTreeMap<String, UserProfile>) -> String {
    let mut out = if profiles.is_empty() {
        String::from("USER PROFILES\n\n  (none defined)\n")
    } else {
        let mut s = String::from("USER PROFILES\n\n");
        for (name, profile) in profiles {
            let base = profile.base.as_deref().unwrap_or(DEFAULT_SCENE);
            s.push_str(&format!("  {name:12} base={base}\n"));
        }
        s
    };
    out.push_str("\n  See docs/PROFILE_EXAMPLES.md for profile examples.\n");
    out.push_str(&atmosphere_presets_section());
    out
}

pub fn dump_profile_text(
    profiles: &BTreeMap<String, UserProfile>,
    name: &str,
) -> Result<String, String> {
    let normalized = validate_profile_name(name)?;
    let Some(profile) = profiles.get(&normalized) else {
        return Err(format!(
            "error: invalid profile: {name}\nexpected one of: {}\n\n  Use --list-profiles to see available profiles.",
            profile_name_list(profiles)
        ));
    };

    let mut out = String::new();
    push_field(&mut out, &normalized, "base", profile.base.as_deref());
    push_field(&mut out, &normalized, "preset", profile.preset.as_deref());
    push_field(&mut out, &normalized, "color", profile.color.as_deref());
    push_field(&mut out, &normalized, "charset", profile.charset.as_deref());
    push_field(&mut out, &normalized, "fps", profile.fps.as_deref());
    push_field(&mut out, &normalized, "speed", profile.speed.as_deref());
    push_field(&mut out, &normalized, "density", profile.density.as_deref());
    push_field(
        &mut out,
        &normalized,
        "glitch-level",
        profile.glitch_level.as_deref(),
    );
    push_field(
        &mut out,
        &normalized,
        "monolith-size",
        profile.monolith_size.as_deref(),
    );
    push_field(
        &mut out,
        &normalized,
        "color-bg",
        profile.color_bg.as_deref(),
    );
    push_field(
        &mut out,
        &normalized,
        "atmosphere-mode",
        profile.atmosphere_mode.as_deref(),
    );
    push_field(
        &mut out,
        &normalized,
        "atmosphere-regime",
        profile.atmosphere_regime.as_deref(),
    );
    Ok(out)
}

fn apply_profile_preset(
    matches: &clap::ArgMatches,
    args: &mut Args,
    preset: &str,
    modified: &mut HashSet<&'static str>,
) {
    let Ok(name) = validate_preset_name(preset) else {
        eprintln!("profile: invalid preset='{preset}' in profile (see --list-presets)");
        return;
    };
    let Some(p) = get_preset(&name) else {
        return;
    };
    if !is_explicit(matches, "color") {
        args.color = p.color.to_string();
        modified.insert("color");
    }
    if !is_explicit(matches, "charset") {
        args.charset = p.charset.to_string();
        modified.insert("charset");
    }
    if !is_explicit(matches, "fps") {
        args.fps = p.fps;
        modified.insert("fps");
    }
    if !is_explicit(matches, "speed") {
        args.speed = p.speed;
        modified.insert("speed");
    }
    if !is_explicit(matches, "density") {
        args.density = p.density;
        modified.insert("density");
    }
    if !is_explicit(matches, "glitch_level") {
        args.glitch_level = p.glitch_level;
        modified.insert("glitch_level");
    }
}

fn apply_profile_scene(
    matches: &clap::ArgMatches,
    args: &mut Args,
    scene_name: &str,
    modified: &mut HashSet<&'static str>,
) {
    let Some(scene) = get_scene(scene_name) else {
        return;
    };
    let cfg = scene.config;
    if let Some(color) = cfg.color {
        if !is_explicit(matches, "color") {
            args.color = color.to_string();
            modified.insert("color");
        }
    }
    if let Some(charset) = cfg.charset {
        if !is_explicit(matches, "charset") {
            args.charset = charset.to_string();
            modified.insert("charset");
        }
    }
    if let Some(fps) = cfg.fps {
        if !is_explicit(matches, "fps") {
            args.fps = fps;
            modified.insert("fps");
        }
    }
    if let Some(speed) = cfg.speed {
        if !is_explicit(matches, "speed") {
            args.speed = speed;
            modified.insert("speed");
        }
    }
    if let Some(density) = cfg.density {
        if !is_explicit(matches, "density") {
            args.density = density;
            modified.insert("density");
        }
    }
    if let Some(glitch) = cfg.glitch_level {
        if !is_explicit(matches, "glitch_level") {
            args.glitch_level = glitch;
            modified.insert("glitch_level");
        }
    }
}

fn apply_profile_overrides(
    matches: &clap::ArgMatches,
    args: &mut Args,
    name: &str,
    profile: &UserProfile,
    modified: &mut HashSet<&'static str>,
) {
    if let Some(value) = profile
        .color
        .as_deref()
        .filter(|_| !is_explicit(matches, "color"))
    {
        if parse_color_scheme(value).is_ok() {
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
        if charset_from_str(value, false).is_ok() {
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
        if let Some(fps) = parse_f64_profile(name, "fps", value, 1.0, 240.0) {
            args.fps = fps;
            modified.insert("fps");
        }
    }
    if let Some(value) = profile
        .speed
        .as_deref()
        .filter(|_| !is_explicit(matches, "speed"))
    {
        if let Some(speed) = parse_speed_profile(name, value) {
            args.speed = speed;
            modified.insert("speed");
        }
    }
    if let Some(value) = profile
        .density
        .as_deref()
        .filter(|_| !is_explicit(matches, "density"))
    {
        if let Some(density) = parse_f32_profile(name, "density", value, 0.01, DENSITY_CLAMP_MAX) {
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
            None => warn_invalid(
                name,
                "color-bg",
                value,
                "black, default-background, transparent",
            ),
        }
    }
    if let Some(value) = profile
        .atmosphere_mode
        .as_deref()
        .filter(|_| !is_explicit(matches, "atmosphere_mode_str"))
    {
        if let Some(mode) = parse_atmosphere_mode_profile(name, value) {
            args.atmosphere_mode_str = Some(mode);
            modified.insert("atmosphere_mode_str");
        }
    }
    if let Some(value) = profile
        .atmosphere_regime
        .as_deref()
        .filter(|_| !is_explicit(matches, "atmosphere_regime_str"))
    {
        if let Some(regime) = parse_atmosphere_regime_profile(name, value) {
            args.atmosphere_regime_str = Some(regime);
            modified.insert("atmosphere_regime_str");
        }
    }
}

fn parse_f32_profile(name: &str, field: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    parse_canonical_f32_range(&format!("profile.{name}.{field}"), value, min, max)
        .map_err(|_| {
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max}"),
            )
        })
        .ok()
}

fn parse_f64_profile(name: &str, field: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    parse_canonical_f64_range(&format!("profile.{name}.{field}"), value, min, max)
        .map_err(|_| {
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max}"),
            )
        })
        .ok()
}

fn parse_speed_profile(name: &str, value: &str) -> Option<f32> {
    parse_canonical_speed(&format!("profile.{name}.speed"), value)
        .map_err(|_| {
            warn_invalid(
                name,
                "speed",
                value,
                &format!("canonical integer in range {SPEED_MIN}..={SPEED_MAX}"),
            );
        })
        .ok()
}

fn parse_color_bg(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        "transparent" => Some(ColorBg::Transparent),
        _ => None,
    }
}

fn parse_atmosphere_mode_profile(name: &str, value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "controlled-live" => Some(value.trim().to_ascii_lowercase()),
        _ => {
            warn_invalid(name, "atmosphere-mode", value, "disabled, controlled-live");
            None
        }
    }
}

fn parse_atmosphere_regime_profile(name: &str, value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "calm" | "pulse" | "signal" | "compression" | "void" | "monolith-pressure" => {
            Some(value.trim().to_ascii_lowercase())
        }
        "storm" => {
            eprintln!(
                "profile: invalid atmosphere-regime='storm' in profile '{name}' — storm is unavailable"
            );
            None
        }
        _ => {
            warn_invalid(
                name,
                "atmosphere-regime",
                value,
                "calm, pulse, signal, compression, void, monolith-pressure",
            );
            None
        }
    }
}

fn profile_name_list(profiles: &BTreeMap<String, UserProfile>) -> String {
    if profiles.is_empty() {
        "<none defined>".to_string()
    } else {
        profiles.keys().cloned().collect::<Vec<_>>().join(", ")
    }
}

fn is_valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
    !matches!(
        matches.value_source(key),
        None | Some(ValueSource::DefaultValue)
    )
}

fn warn_invalid(profile: &str, field: &str, value: &str, expected: &str) {
    eprintln!("profile: invalid {field}='{value}' in profile '{profile}' (expected: {expected})");
}

fn push_field(out: &mut String, profile: &str, field: &str, value: Option<&str>) {
    if let Some(value) = value {
        out.push_str(&format!("profile.{profile}.{field} = {value}\n"));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn profile_keys_are_recognized() {
        assert!(is_profile_config_key("profile.nightcore.base"));
        assert!(is_profile_config_key("profile.nightcore.glitch-level"));
        assert!(!is_profile_config_key("profile.nightcore.unknown"));
        assert!(!is_profile_config_key("profile..base"));
    }

    #[test]
    fn collect_profiles_groups_fields_by_name() {
        let cfg = HashMap::from([
            ("profile.nightcore.base".to_string(), "monolith".to_string()),
            ("profile.nightcore.color".to_string(), "purple".to_string()),
            ("profile.day.speed".to_string(), "12".to_string()),
        ]);
        let profiles = collect_profiles(&cfg);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles["nightcore"].color.as_deref(), Some("purple"));
        assert_eq!(profiles["day"].speed.as_deref(), Some("12"));
    }

    #[test]
    fn list_profiles_includes_defined_names() {
        let cfg = HashMap::from([("profile.nightcore.base".to_string(), "monolith".to_string())]);
        let text = list_profiles_text(&collect_profiles(&cfg));
        assert!(text.contains("nightcore"));
        assert!(text.contains("base=monolith"));
    }
}
