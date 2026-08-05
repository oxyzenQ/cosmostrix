// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined profile support for flat `key = value` config files.
//!
//! Profiles are intentionally lightweight collections of override fields.
//! They no longer inherit from a `base-scene` — custom scenes stand on
//! their own, and missing fields fall back to global defaults
//! (`DEFAULT_SCENE` = cinematic). The `base-scene` and `preset` fields
//! were removed in v20.1 and are now reported as unknown keys by
//! `--testconf`, prompting migration.

use std::collections::{BTreeMap, HashSet};

use clap::parser::ValueSource;
use clap::ValueEnum;

use crate::charset::charset_from_str;
use crate::cli::parse_color_scheme;
use crate::colors_custom::is_colors_custom_name;
use crate::config::{Args, ColorBg, GlitchLevel};
use crate::constants::{DENSITY_CLAMP_MAX, SPEED_MAX, SPEED_MIN};
use crate::runtime::MonolithSize;
use crate::validation::{
    parse_canonical_f32_range, parse_canonical_f64_range, parse_canonical_speed,
};

pub(crate) const PROFILE_FIELDS: &[&str] = &[
    "color",
    "charset",
    "fps",
    "speed",
    "density",
    "density-map",
    "glitch-level",
    "monolith-size",
    "color-bg",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UserProfile {
    pub color: Option<String>,
    pub charset: Option<String>,
    pub fps: Option<String>,
    pub speed: Option<String>,
    pub density: Option<String>,
    /// Comma-separated f64 weights (0.0..=1.0) for monolith pillar placement.
    /// Parsed into a Vec<f64> and leaked to &'static for Cloud consumption.
    pub density_map: Option<String>,
    pub glitch_level: Option<String>,
    pub monolith_size: Option<String>,
    pub color_bg: Option<String>,
}

#[must_use]
pub(crate) fn is_profile_config_key(key: &str) -> bool {
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

/// Collect all `[profile.<name>.<field>]` entries from `cfg` into a
/// `BTreeMap<String, UserProfile>`.
///
/// **Design note (Phase 4 P4-6 — positive finding, intentional pattern):**
/// This iterates ALL config keys to find profile keys (O(n) where n =
/// total config keys). For a typical 50-key config with 3 profiles (15
/// profile keys), this iterates 50 keys to find 15. ~5μs per call.
/// Called at startup AND on every live reload. The O(n) iteration is
/// kept because:
/// 1. The HashMap is small (50 keys typical) — ~5μs is invisible.
/// 2. Filtering by `is_profile_config_key` is O(1) (prefix + rsplit).
/// 3. Maintaining a separate `profile_keys` subset during config load
///    would add complexity to the load path for no user-visible benefit.
#[must_use]
pub(crate) fn collect_profiles(
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
            .entry(name.to_ascii_lowercase())
            .or_insert_with(UserProfile::default);
        match field {
            "color" => profile.color = Some(value.clone()),
            "charset" => profile.charset = Some(value.clone()),
            "fps" => profile.fps = Some(value.clone()),
            "speed" => profile.speed = Some(value.clone()),
            "density" => profile.density = Some(value.clone()),
            "density-map" => profile.density_map = Some(value.clone()),
            "glitch-level" => profile.glitch_level = Some(value.clone()),
            "monolith-size" => profile.monolith_size = Some(value.clone()),
            "color-bg" => profile.color_bg = Some(value.clone()),
            _ => {}
        }
    }
    profiles
}

pub(crate) fn validate_profile_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if is_valid_profile_name(&normalized) {
        Ok(normalized)
    } else {
        Err(format!(
            "error: invalid profile: {name}\nexpected: letters, digits, '-' or '_'"
        ))
    }
}

pub(crate) fn apply_profile_layer(
    matches: &clap::ArgMatches,
    args: &mut Args,
    profiles: &BTreeMap<String, UserProfile>,
    cfg: &std::collections::HashMap<String, String>,
    name: &str,
    strict_unknown: bool,
) -> Result<HashSet<&'static str>, String> {
    let mut modified = HashSet::new();
    let normalized = validate_profile_name(name)?;
    let Some(profile) = profiles.get(&normalized) else {
        let message = format!(
            "error: unknown profile '{name}'\nexpected one of: {}\n\n  Use --list-scenes to see available scenes.",
            profile_name_list(profiles)
        );
        if strict_unknown {
            return Err(message);
        }
        eprintln!(
            "config: ignoring unknown profile '{name}' (available: {}; see --list-scenes)",
            profile_name_list(profiles)
        );
        return Ok(modified);
    };

    // v20.1: `base-scene` and `preset` are gone — custom scenes stand on
    // their own. Their own fields override args.* directly via
    // apply_profile_overrides, and missing fields fall back to whatever
    // args already has (DEFAULT_SCENE = cinematic's values from
    // apply_default_scene_values). The caller is responsible for setting
    // args.scene to the custom scene name so verbose output shows
    // `scene: <custom_name>` instead of a fallback foundation scene.
    //
    // Phase 5 closure (P1-#5): pass `cfg` through so apply_profile_overrides
    // can resolve custom charset/color names from [charset-custom.*] and
    // [colors-custom.*] blocks — matching the top-level config_apply behavior.
    apply_profile_overrides(matches, args, &normalized, profile, cfg, &mut modified);
    Ok(modified)
}

fn apply_profile_overrides(
    matches: &clap::ArgMatches,
    args: &mut Args,
    name: &str,
    profile: &UserProfile,
    cfg: &std::collections::HashMap<String, String>,
    modified: &mut HashSet<&'static str>,
) {
    if let Some(value) = profile
        .color
        .as_deref()
        .filter(|_| !is_explicit(matches, "color"))
    {
        // Phase 5 closure (P1-#5): resolve custom color names from
        // [colors-custom.*] blocks, matching top-level config_apply behavior.
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
        // Phase 5 closure (P1-#5): resolve custom charset names from
        // [charset-custom.*] blocks, matching top-level config_apply behavior.
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
        if let Some(fps) = parse_f64_profile(name, "fps", value, 1.0, 300.0) {
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
            None => warn_invalid(name, "color-bg", value, "black, default-background"),
        }
    }
}

fn parse_f32_profile(name: &str, field: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    parse_canonical_f32_range(&format!("profile.{name}.{field}"), value, min, max)
        .map_err(|e| {
            // Phase 5 (P3-8): pass the canonical parser's error message
            // through to warn_invalid. Previously the error was discarded
            // and a generic "number in range X..=Y" message was emitted,
            // hiding whether the value was non-canonical (e.g. "1e2") or
            // out of range (e.g. "200"). The canonical message distinguishes.
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max} ({e})"),
            )
        })
        .ok()
}

fn parse_f64_profile(name: &str, field: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    parse_canonical_f64_range(&format!("profile.{name}.{field}"), value, min, max)
        .map_err(|e| {
            // Phase 5 (P3-8): pass the canonical parser's error message through.
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max} ({e})"),
            )
        })
        .ok()
}

fn parse_speed_profile(name: &str, value: &str) -> Option<f32> {
    parse_canonical_speed(&format!("profile.{name}.speed"), value)
        .map_err(|e| {
            // Phase 5 (P3-8): pass the canonical parser's error message through.
            warn_invalid(
                name,
                "speed",
                value,
                &format!("canonical integer in range {SPEED_MIN}..={SPEED_MAX} ({e})"),
            );
        })
        .ok()
}

fn parse_color_bg(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        _ => None,
    }
}

fn profile_name_list(profiles: &BTreeMap<String, UserProfile>) -> String {
    if profiles.is_empty() {
        "<none defined>".to_string()
    } else {
        profiles.keys().cloned().collect::<Vec<_>>().join(", ")
    }
}

pub(crate) fn is_valid_profile_name(name: &str) -> bool {
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
    crate::output::eprintln_warn_labeled(&format!(
        "profile: invalid {field}='{value}' in profile '{profile}' (expected: {expected})"
    ));
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn profile_keys_are_recognized() {
        // v20.1: `base-scene` is gone — it must NOT be recognized as a valid
        // profile key. --testconf will flag it as unknown, prompting migration.
        assert!(!is_profile_config_key("profile.nightcore.base-scene"));
        assert!(!is_profile_config_key("profile.nightcore.preset"));
        assert!(is_profile_config_key("profile.nightcore.glitch-level"));
        assert!(!is_profile_config_key("profile.nightcore.unknown"));
        assert!(!is_profile_config_key("profile..base"));
    }

    #[test]
    fn collect_profiles_groups_fields_by_name() {
        let cfg = HashMap::from([
            ("profile.nightcore.color".to_string(), "purple".to_string()),
            ("profile.day.speed".to_string(), "12".to_string()),
        ]);
        let profiles = collect_profiles(&cfg);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles["nightcore"].color.as_deref(), Some("purple"));
        assert_eq!(profiles["day"].speed.as_deref(), Some("12"));
    }

    // ── Phase 5 closure (P1-#5): custom charset/color name resolution ──

    #[test]
    fn is_colors_custom_name_finds_defined_palette() {
        let cfg = HashMap::from([
            ("colors-custom.sunset.bg".to_string(), "#0a0a12".to_string()),
            (
                "colors-custom.sunset.rain".to_string(),
                "[\"#1a0033\", \"#9933ff\"]".to_string(),
            ),
        ]);
        assert!(is_colors_custom_name(&cfg, "sunset"));
        assert!(is_colors_custom_name(&cfg, "Sunset")); // case-insensitive
        assert!(!is_colors_custom_name(&cfg, "nonexistent"));
    }

    #[test]
    fn is_colors_custom_name_empty_cfg_returns_false() {
        let cfg = HashMap::new();
        assert!(!is_colors_custom_name(&cfg, "anything"));
    }
}
