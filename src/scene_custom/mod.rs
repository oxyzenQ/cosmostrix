// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined custom scene support for `[scene-custom.<name>]` config blocks.
//!
//! Custom scenes are user-authored themes that stand on their own — they
//! no longer inherit from a `base-scene`. Missing fields fall back to
//! global defaults (`DEFAULT_SCENE` = cinematic), not to another named scene.
//! This makes custom scenes first-class citizens: when invoked via
//! `--scene-custom <name>`, the verbose output shows `scene: <name>` and
//! live reload applies edits to the block immediately.
//!
//! ## changes
//!
//! `base-scene` is RESTORED with cleaner inheritance semantics. When a
//! `[scene-custom.<name>]` block sets `base-scene = <built-in-scene>`, the
//! custom scene inherits ALL scene-managed defaults (color, charset, fps,
//! speed, density, glitch-level, rain_style) from that built-in scene
//! before applying its own overrides. This lets users write:
//!
//! ```toml
//! [scene-custom.afternoon]
//! base-scene = "signal"
//! color = "neon-green"
//! speed = "50"
//! ```
//!
//! ...and get the `signal` rain style + signal's density/glitch, but with
//! neon-green color and speed 50.
//!
//! The legacy `preset` field remains removed (it was a confusing synonym
//! for `base-scene`). Chained inheritance (`base-scene = <custom-name>`)
//! is NOT supported — base-scene must be a built-in scene name. This
//! keeps the apply graph a flat 2-level, avoiding cycles.
//!
//! ## changes (historical)
//!
//! `preset` was removed entirely. Existing configs that still contain
//! `preset = <name>` will have those keys flagged as unknown by
//! `--testconf`, prompting migration. The `[profile.<name>]` fallback was
//! also removed — `--scene-custom` now resolves ONLY `[scene-custom.<name>]`
//! blocks. Users with legacy `[profile.<name>]` blocks must rename the
//! prefix to `scene-custom`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

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

/// Canonical field list for `key=value` override blocks.
///
/// Used by both scene-custom blocks and testconf validation to ensure
/// the recognized field set never drifts between the parser and the
/// validator. Originally lived in `profile` module; moved here when the
/// inert profile system was removed.
pub(crate) const PROFILE_FIELDS: &[&str] = &[
    "base-scene",
    "color",
    "charset",
    "fps",
    "speed",
    "density",
    "density-map",
    "glitch-level",
    "monolith-size",
    "color-bg",
    // scene-custom-only fields.
    "bold",
    "colors-custom",
    "charset-custom",
    "shadingmode",
    "async-mode",
];

/// Lightweight collection of override fields for a scene-custom block.
///
/// Originally `UserProfile` from the inert `profile` module. The name is
/// kept to avoid a massive rename across scene-custom code.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UserProfile {
    /// Optional built-in scene name to inherit defaults from before applying
    /// this block's own overrides.
    pub base_scene: Option<String>,
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
    pub bold: Option<String>,
    /// Custom palette name referencing a `[colors-custom.<name>]` block.
    pub colors_custom: Option<String>,
    /// Custom charset name referencing a `[charset-custom.<name>]` block.
    pub charset_custom: Option<String>,
    /// Shading mode: "0"=Random, "1"=DistanceFromHead.
    pub shading_mode: Option<String>,
    /// Async render toggle: "true"/"false".
    pub async_mode: Option<String>,
}

/// Collect all `[profile.<name>.<field>]` entries from `cfg`.
///
/// Retained for testconf validation — profile.* keys are still parsed as
/// config (stored in values) so `--testconf` can report them as inert and
/// surface them in the "available scenes" list. They are NOT applied at
/// runtime.
#[must_use]
pub(crate) fn collect_profiles(cfg: &HashMap<String, String>) -> BTreeMap<String, UserProfile> {
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
            "base-scene" => profile.base_scene = Some(value.clone()),
            "color" => profile.color = Some(value.clone()),
            "charset" => profile.charset = Some(value.clone()),
            "fps" => profile.fps = Some(value.clone()),
            "speed" => profile.speed = Some(value.clone()),
            "density" => profile.density = Some(value.clone()),
            "density-map" => profile.density_map = Some(value.clone()),
            "glitch-level" => profile.glitch_level = Some(value.clone()),
            "monolith-size" => profile.monolith_size = Some(value.clone()),
            "color-bg" => profile.color_bg = Some(value.clone()),
            "bold" => profile.bold = Some(value.clone()),
            "colors-custom" => profile.colors_custom = Some(value.clone()),
            "charset-custom" => profile.charset_custom = Some(value.clone()),
            "shadingmode" => profile.shading_mode = Some(value.clone()),
            "async-mode" => profile.async_mode = Some(value.clone()),
            _ => {}
        }
    }
    profiles
}

/// Check if `key` matches `profile.<name>.<field>` pattern.
///
/// Retained for configfile.rs `is_known_key` so legacy `profile.*` keys
/// are not flagged as unknown — they are stored but inert.
fn is_profile_config_key(key: &str) -> bool {
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

/// Validate and normalize a profile/scene-custom name.
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

pub(crate) fn is_valid_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Apply a scene-custom (or profile) override layer to `args`.
///
/// When `base-scene` is set, the named built-in scene's defaults are
/// applied first, then the block's own overrides win on top.
pub(crate) fn apply_profile_layer(
    matches: &clap::ArgMatches,
    args: &mut Args,
    profiles: &BTreeMap<String, UserProfile>,
    cfg: &HashMap<String, String>,
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
        crate::output::eprintln_warn_labeled(&format!(
            "ignoring unknown profile '{}' (available: {}; see --list-scenes)",
            name,
            profile_name_list(profiles)
        ));
        return Ok(modified);
    };

    if let Some(base_name) = profile.base_scene.as_deref() {
        apply_base_scene_to_args(
            matches,
            args,
            base_name,
            &normalized,
            strict_unknown,
            &mut modified,
        );
    }
    apply_profile_overrides(matches, args, &normalized, profile, cfg, &mut modified);
    Ok(modified)
}

/// Apply a built-in scene's defaults to `args` as the first inheritance
/// layer. Mirrors `apply_default_scene_values` but reads from a parameter
/// scene instead of `args.scene`.
fn apply_base_scene_to_args(
    matches: &clap::ArgMatches,
    args: &mut Args,
    base_name: &str,
    profile_name: &str,
    strict_unknown: bool,
    modified: &mut HashSet<&'static str>,
) {
    let normalized = base_name.trim().to_ascii_lowercase();
    let Some(scene_info) = crate::scene::get_scene(&normalized) else {
        let message = format!(
            "error: unknown base-scene '{base_name}' in profile '{profile_name}'\n\
             expected one of: {}\n\
             note: base-scene must be a built-in scene name (custom scenes are not allowed)",
            crate::scene::all_scene_names().join(", ")
        );
        if strict_unknown {
            crate::output::eprintln_error_labeled(&message);
        } else {
            crate::output::eprintln_warn_labeled(&message);
        }
        return;
    };
    let cfg = &scene_info.config;

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
        if let Some(n) = parse_u8_override(name, "shadingmode", value, 0, 1) {
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
                args.async_mode = b;
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

fn parse_f32_override(name: &str, field: &str, value: &str, min: f32, max: f32) -> Option<f32> {
    parse_canonical_f32_range(&format!("scene-custom.{name}.{field}"), value, min, max)
        .map_err(|e| {
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max} ({e})"),
            )
        })
        .ok()
}

fn parse_f64_override(name: &str, field: &str, value: &str, min: f64, max: f64) -> Option<f64> {
    parse_canonical_f64_range(&format!("scene-custom.{name}.{field}"), value, min, max)
        .map_err(|e| {
            warn_invalid(
                name,
                field,
                value,
                &format!("number in range {min}..={max} ({e})"),
            )
        })
        .ok()
}

fn parse_speed_override(name: &str, value: &str) -> Option<f32> {
    parse_canonical_speed(&format!("scene-custom.{name}.speed"), value)
        .map_err(|e| {
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

/// Parse a u8 field for scene-custom/profile application.
fn parse_u8_override(name: &str, field: &str, value: &str, min: u8, max: u8) -> Option<u8> {
    let v = value.trim();
    match v.parse::<u8>() {
        Ok(n) if n >= min && n <= max => Some(n),
        _ => {
            warn_invalid(
                name,
                field,
                value,
                &format!("integer in range {min}..={max}"),
            );
            None
        }
    }
}

/// Parse a bool field ("true"/"false", case-insensitive, also accepts "1"/"0").
fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
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

fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
    !matches!(
        matches.value_source(key),
        None | Some(ValueSource::DefaultValue)
    )
}

fn warn_invalid(profile: &str, field: &str, value: &str, expected: &str) {
    crate::output::eprintln_warn_labeled(&format!(
        "scene-custom: invalid {field}='{value}' in scene-custom '{profile}' (expected: {expected})"
    ));
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

    for (key, value) in cfg {
        let Some(field) = key.strip_prefix(&prefix) else {
            continue;
        };
        if field == "base-scene" || field == "preset" {
            continue;
        }
        if apply_scene_custom_field_to_cloud_config(new, cfg, field, value) {
            touched_any = true;
        }
    }

    if touched_any {
        crate::live_config::push_runtime_warning(&format!(
            "[live-reload] scene-custom '{normalized}': re-applied fields from config"
        ));
    }
}

/// Config namespace prefix for custom scene blocks.
pub(crate) const SCENE_CUSTOM_NAMESPACE: &str = "scene-custom";

/// explicit field allowlist for `[scene-custom.<name>]` blocks.
///
/// Owner contract (2026-08-07):
/// - ALLOWED: `base-scene`, `color`, `charset`, `bold`, `colors-custom`,
///   `charset-custom`, `shadingmode`, `glitch-level`, `fps`, `speed`,
///   `density`, `density-map`, `async-mode`.
/// - FORBIDDEN (rejected as unknown key by `is_scene_custom_config_key`):
///   `ambient`, `crystal-dragon`, `color.tune`, `monolith-size`,
///   `intro`, `color-bg`.
///
/// `monolith-size` and `color-bg` were accepted (because the
/// allowlist was `PROFILE_FIELDS`, which included them). They are removed
/// here because they collide with the ambient simplification: a custom
/// scene used by an ambient entry should not own monolith-size or
/// color-bg (those are top-level / scene-managed, not per-block).
///
/// `density-map` is retained because it is tightly coupled to `density`
/// for monolith pillar placement and was already supported.
pub(crate) const SCENE_CUSTOM_FIELDS: &[&str] = &[
    "base-scene",
    "color",
    "charset",
    "bold",
    "colors-custom",
    "charset-custom",
    "shadingmode",
    "glitch-level",
    "fps",
    "speed",
    "density",
    "density-map",
    "async-mode",
];

/// Returns `true` if `key` is a recognized `[scene-custom.<name>.<field>]` key.
///
/// uses [`SCENE_CUSTOM_FIELDS`] (explicit allowlist) instead of
/// `PROFILE_FIELDS`. This rejects `monolith-size` and `color-bg` which
/// were accepted but are forbidden by owner contract.
#[must_use]
pub(crate) fn is_scene_custom_config_key(key: &str) -> bool {
    let Some((prefix, rest)) = key.split_once('.') else {
        return false;
    };
    if prefix != SCENE_CUSTOM_NAMESPACE {
        return false;
    }
    let Some((name, field)) = rest.rsplit_once('.') else {
        return false;
    };
    is_valid_profile_name(name) && SCENE_CUSTOM_FIELDS.contains(&field)
}

/// Collect all `[scene-custom.<name>]` blocks from a flat config map.
///
/// Mirrors [`collect_profiles`] but scoped to the `scene-custom`
/// namespace. only fields in [`SCENE_CUSTOM_FIELDS`] are parsed —
/// `monolith-size` and `color-bg` are silently dropped (the keys are
/// flagged as unknown upstream by `is_scene_custom_config_key`).
#[must_use]
pub(crate) fn collect_custom_scenes(
    cfg: &HashMap<String, String>,
) -> BTreeMap<String, UserProfile> {
    let mut scenes = BTreeMap::new();
    for (key, value) in cfg {
        if !is_scene_custom_config_key(key) {
            continue;
        }
        let (_, rest) = key.split_once('.').expect("scene-custom key has prefix");
        let (name, field) = rest.rsplit_once('.').expect("scene-custom key has field");
        let scene = scenes
            .entry(name.to_ascii_lowercase())
            .or_insert_with(UserProfile::default);
        match field {
            "base-scene" => scene.base_scene = Some(value.clone()),
            "color" => scene.color = Some(value.clone()),
            "charset" => scene.charset = Some(value.clone()),
            "fps" => scene.fps = Some(value.clone()),
            "speed" => scene.speed = Some(value.clone()),
            "density" => scene.density = Some(value.clone()),
            "density-map" => scene.density_map = Some(value.clone()),
            "glitch-level" => scene.glitch_level = Some(value.clone()),
            // new scene-custom fields per owner spec.
            "bold" => scene.bold = Some(value.clone()),
            "colors-custom" => scene.colors_custom = Some(value.clone()),
            "charset-custom" => scene.charset_custom = Some(value.clone()),
            "shadingmode" => scene.shading_mode = Some(value.clone()),
            "async-mode" => scene.async_mode = Some(value.clone()),
            // monolith-size and color-bg are NOT in SCENE_CUSTOM_FIELDS,
            // so is_scene_custom_config_key already filtered them out.
            _ => {}
        }
    }
    scenes
}

/// Apply a user-defined custom scene by name.
///
/// Lookup: `[scene-custom.<name>]` in config only. removed the
/// `[profile.<name>]` fallback — users must rename the prefix to migrate.
///
/// On success, sets `args.scene_custom = Some(name)` and
/// `args.scene = Some(name)`. The applied field set is returned as
/// `HashSet<&'static str>` for downstream precedence tracking.
pub(crate) fn apply_scene_custom_layer(
    matches: &clap::ArgMatches,
    args: &mut Args,
    cfg: &HashMap<String, String>,
    name: &str,
    strict_unknown: bool,
) -> Result<HashSet<&'static str>, String> {
    let custom_scenes = collect_custom_scenes(cfg);
    // Also surface [profile.<name>] entries in the "available" list so
    // the error message is helpful when a user forgot to rename the prefix.
    // We do NOT load from profiles anymore — the lookup is scene-custom only.
    let profiles = collect_profiles(cfg);
    // Normalize the lookup name to lowercase so it matches the lowercase
    // keys stored by collect_custom_scenes. The original `name` is
    // preserved for display in error messages.
    let normalized = name.trim().to_ascii_lowercase();

    if custom_scenes.contains_key(&normalized) {
        let modified = apply_profile_layer(
            matches,
            args,
            &custom_scenes,
            cfg,
            &normalized,
            strict_unknown,
        )?;
        args.scene_custom = Some(normalized.clone());
        // custom scenes are first-class — args.scene reflects the
        // custom scene name (not a base-scene) so verbose output and
        // CloudConfig.scene_name both show `<name>`. Built-in scene defaults
        // are applied via `apply_profile_layer`'s base-scene inheritance
        // (when `base-scene = <name>` is set in the block) BEFORE the custom
        // scene's own overrides. Missing fields retain whatever args already
        // has (DEFAULT_SCENE = cinematic's values from
        // apply_default_scene_values).
        //
        // rain_style for the custom scene is resolved separately at Cloud
        // construction time via `rain_style_for_custom_scene` (looks up the
        // block's `base-scene` field). This keeps args.scene as the custom
        // name while still honoring base-scene's rain_style (Glyph vs Monolith).
        args.scene = Some(normalized);
        return Ok(modified);
    }

    // Not found in the scene-custom namespace.
    let mut available: Vec<String> = custom_scenes
        .keys()
        .cloned()
        .chain(profiles.keys().cloned())
        .collect();
    available.sort();
    available.dedup();
    let list = if available.is_empty() {
        "<none defined>".to_string()
    } else {
        available.join(", ")
    };
    let message = format!(
        "error: unknown custom scene '{name}'\nexpected one of: {list}\n\n  Use --list-scenes to see built-in and custom scenes."
    );
    if strict_unknown {
        return Err(message);
    }
    crate::output::eprintln_warn_labeled(&format!(
        "ignoring unknown custom scene '{}' (available: {}; see --list-scenes)",
        name, list
    ));
    Ok(HashSet::new())
}

/// Resolve the rain_style for a custom scene by looking up its `base-scene`.
///
/// Returns `None` if:
/// - The custom scene block doesn't exist in cfg.
/// - The block has no `base-scene` field.
/// - The `base-scene` value is not a recognized built-in scene name.
///
/// Called from `main.rs` at Cloud construction time and from
/// `Cloud::apply_ambient_entry` at runtime when an ambient entry references
/// a custom scene. The returned `RainStyle` is what the Cloud should use
/// for rain rendering (Glyph vs Monolith).
#[must_use]
pub(crate) fn rain_style_for_custom_scene(
    cfg: &HashMap<String, String>,
    custom_name: &str,
) -> Option<crate::rain_style::RainStyle> {
    let normalized = custom_name.trim().to_ascii_lowercase();
    let key = format!("scene-custom.{normalized}.base-scene");
    let base_name = cfg.get(&key)?.trim();
    crate::scene::rain_style_for_scene(base_name)
}

/// Resolve the rain_style for any scene name (built-in OR custom).
///
/// if `name` is a built-in scene, returns its rain_style. If `name`
/// is a custom scene, looks up its `[scene-custom.<name>]` block in `cfg`
/// and returns the `base-scene`'s rain_style. Returns `RainStyle::Glyph`
/// (the default) if neither resolves.
///
/// Called from `main.rs` at Cloud construction time.
#[must_use]
pub(crate) fn resolve_rain_style(
    name: Option<&str>,
    cfg: &HashMap<String, String>,
) -> crate::rain_style::RainStyle {
    name.and_then(|n| {
        crate::scene::rain_style_for_scene(n).or_else(|| rain_style_for_custom_scene(cfg, n))
    })
    .unwrap_or(crate::rain_style::RainStyle::Glyph)
}

/// Apply a `[scene-custom.<name>]` block's `base-scene` defaults to a
/// CloudConfig in place. Used by live-reload to inherit a built-in scene's
/// managed defaults before applying the custom block's own overrides.
///
/// (Glitch-BUG4): shared preset-derivation helper for the live-reload
/// path. Mirrors `Cloud::apply_glitch_level_runtime` (scene_runtime.rs:426)
/// and `config_apply::apply_glitch_level_values` (startup). All three paths
/// now agree on the 5 preset fields per GlitchLevel variant.
///
/// Called from:
/// - `apply_base_scene_to_cloud_config` when `base_cfg.glitch_level` is Some
/// - `apply_scene_custom_field_to_cloud_config` "glitch-level" arm
/// - (live_config.rs top-level `glitch-level` branch has its own inline match
///   but the values are identical — kept inline there to avoid a circular dep)
pub(crate) fn apply_glitch_level_preset_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    level: crate::config::GlitchLevel,
) {
    use crate::config::GlitchLevel;
    match level {
        GlitchLevel::None => {
            new.glitch_enabled = false;
            new.glitch_low = 300;
            new.glitch_high = 400;
            new.glitch_pct = 0.0;
            new.short_pct = 50.0;
            new.die_early_pct = 33.33333;
        }
        GlitchLevel::Subtle => {
            new.glitch_enabled = true;
            new.glitch_low = 200;
            new.glitch_high = 300;
            new.glitch_pct = 3.0;
            new.short_pct = 60.0;
            new.die_early_pct = 45.0;
        }
        GlitchLevel::Default => {
            new.glitch_enabled = true;
            new.glitch_low = 300;
            new.glitch_high = 400;
            new.glitch_pct = 10.0;
            new.short_pct = 50.0;
            new.die_early_pct = 33.33333;
        }
        GlitchLevel::Intense => {
            new.glitch_enabled = true;
            new.glitch_low = 500;
            new.glitch_high = 800;
            new.glitch_pct = 25.0;
            new.short_pct = 30.0;
            new.die_early_pct = 20.0;
        }
    }
}

/// extracted from `live_config::apply_scene_custom_to_cloud_config`
/// to keep that file under the LOC cap. Returns `true` if a base-scene was
/// found and applied (so the caller can track `touched_any`).
pub(crate) fn apply_base_scene_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    normalized_name: &str,
) -> bool {
    let base_key = format!("scene-custom.{normalized_name}.base-scene");
    let Some(base_name) = cfg.get(&base_key).map(|s| s.trim()) else {
        return false;
    };
    let Some(base_info) = crate::scene::get_scene(base_name) else {
        return false;
    };
    let base_cfg = &base_info.config;
    if let Some(color) = base_cfg.color {
        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
            new.color_scheme = scheme;
        }
    }
    if let Some(charset) = base_cfg.charset {
        if let Ok(cs) = crate::charset::charset_from_str(charset, false) {
            new.charset_preset = charset.to_string();
            new.chars = crate::charset::build_chars(cs, &new.user_ranges, new.def_ascii);
        }
    }
    // (FPS-F4): gate fps with cli_explicit.fps — matches the startup
    // path (apply_profile_layer → apply_base_scene_to_args checks
    // is_explicit(matches, "fps")). Without this gate, `cosmostrix --fps 144
    // --scene-custom my-scene` silently drops to the base-scene's fps on the
    // first config edit (live-reload path was missing the gate).
    if let Some(fps) = base_cfg.fps {
        if !new.cli_explicit.fps {
            new.target_fps = fps;
        }
    }
    if let Some(speed) = base_cfg.speed {
        new.speed = speed;
    }
    if let Some(density) = base_cfg.density {
        new.density = density;
        new.base_density = density;
    }
    // (Glitch-BUG4): use shared preset helper — was only flipping
    // glitch_enabled, leaving glitch_pct/short_pct/die_early_pct stale.
    if let Some(glitch) = base_cfg.glitch_level {
        apply_glitch_level_preset_to_cloud_config(new, glitch);
    }
    true
}

/// Apply a single `[scene-custom.<name>]` field to a CloudConfig.
/// Extracted from `live_config::apply_scene_custom_to_cloud_config` to keep
/// that file under the LOC cap. Returns `true` if the field was recognized
/// and applied (so the caller can track `touched_any`).
///
/// Field allowlist is `SCENE_CUSTOM_FIELDS`. `monolith-size` and `color-bg`
/// are silently dropped (forbidden per owner contract — they should never
/// reach this function because `is_scene_custom_config_key` filters them
/// upstream, but we handle them defensively).
#[must_use]
pub(crate) fn apply_scene_custom_field_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    field: &str,
    value: &str,
) -> bool {
    match field {
        "color" => {
            if let Ok(scheme) = crate::cli::parse_color_scheme(value) {
                new.color_scheme = scheme;
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
            if let Ok(n) = value.trim().parse::<u8>() {
                new.bold_mode = match n {
                    0 => crate::runtime::BoldMode::Off,
                    2 => crate::runtime::BoldMode::All,
                    _ => crate::runtime::BoldMode::Random,
                };
                return true;
            }
            false
        }
        "shadingmode" => {
            if let Ok(n) = value.trim().parse::<u8>() {
                new.shading_mode = match n {
                    1 => crate::runtime::ShadingMode::DistanceFromHead,
                    _ => crate::runtime::ShadingMode::Random,
                };
                return true;
            }
            false
        }
        "async-mode" => {
            new.async_mode = matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            );
            true
        }
        // monolith-size and color-bg are FORBIDDEN in scene-custom.
        "monolith-size" | "color-bg" => false,
        _ => false,
    }
}

/// Validate a custom-scene name. Shares the same rules as profile names
/// (letters, digits, `-`, `_`) so migration is frictionless.
/// Test-only — production validation uses validate_custom_scene_name()
/// which calls is_valid_profile_name() directly.
#[must_use]
#[cfg(test)]
pub fn is_valid_custom_scene_name(name: &str) -> bool {
    is_valid_profile_name(name)
}

/// Normalize and validate a custom-scene name. Returns the lowercased name
/// on success or an error message on failure.
/// Test-only — production code uses validate_profile_name() directly.
#[cfg(test)]
pub fn validate_custom_scene_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if is_valid_custom_scene_name(&normalized) {
        Ok(normalized)
    } else {
        Err(format!(
            "error: invalid custom scene: {name}\nexpected: letters, digits, '-' or '_'"
        ))
    }
}

/// Parse a comma-separated density-map string into a leaked `&'static [f64]`.
///
/// Format: `"1.0,0.5,0.0,0.8,..."` — weights in `[0.0, 1.0]` (out-of-range
/// clamped). Empty/whitespace entries skipped. Returns `None` if no valid
/// numbers. The slice is `'static`. v30: leak is deduplicated by content
/// via a global `OnceLock<HashMap<String, &'static [f64]>>`.
#[must_use]
pub(crate) fn parse_density_map(csv: &str) -> Option<&'static [f64]> {
    // v30 fix: accept BOTH unquoted (`0.05,0.3,1.0`) and quoted
    // (`"0.05,0.3,1.0"`) CSV. The configfile parser is a custom line-by-line
    // parser (not real TOML) and does NOT strip surrounding quotes — quoted
    // silently failed --testconf. Now we strip a single pair of `"` (or `'`)
    // before splitting, matching colors_custom + charset_custom.
    let csv = csv.trim().trim_matches('"').trim_matches('\'').trim();

    // Dedup cache: maps normalized CSV → parsed &'static slice. Keyed on the
    // quote-stripped string so `"0.5,0.5"` and `0.5,0.5` share one entry.
    static DENSITY_MAP_CACHE: OnceLock<std::sync::Mutex<HashMap<String, &'static [f64]>>> =
        OnceLock::new();
    let cache = DENSITY_MAP_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));

    // Shared parse closure — used by both the healthy-lock + poisoned-mutex
    // paths so they stay in sync (no behavior drift between cached/uncached).
    let parse_weights = || -> Option<Vec<f64>> {
        let weights: Vec<f64> = csv
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() {
                    return None;
                }
                s.parse::<f64>().ok().map(|v| v.clamp(0.0, 1.0))
            })
            .collect();
        if weights.is_empty() {
            None
        } else {
            Some(weights)
        }
    };

    // v50 poison-safe lock: never propagate a poisoned mutex as a panic.
    // Matches the `if let Ok(g)` pattern used by every other production lock.
    if let Ok(mut cache) = cache.lock() {
        if let Some(existing) = cache.get(csv) {
            return Some(*existing);
        }
        let weights = parse_weights()?;
        // Leak the Vec → &'static slice. Cache ensures we leak once per
        // distinct CSV string (live-reload no longer grows memory).
        let leaked: &'static [f64] = Box::leak(weights.into_boxed_slice());
        cache.insert(csv.to_string(), leaked);
        Some(leaked)
    } else {
        // Poisoned-mutex recovery: one-shot parse, skip dedup. Only
        // reachable after a panic in another thread holding this lock.
        let weights = parse_weights()?;
        Some(Box::leak(weights.into_boxed_slice()))
    }
}

/// Render a one-line-per-entry listing of custom scenes from config.
///
/// Output is appended under the "CUSTOM SCENES (from config)" heading in
/// `--list-scenes`. Mirrors the column layout of `scene::list_scenes_text`
/// so the two groups visually align.
///
/// when a custom scene sets `base-scene`, the listing annotates it
/// as `name (base: <base-scene>)` so users can see at a glance which
/// built-in scene a custom scene inherits from. Custom scenes without
/// `base-scene` render as just `name` (inherit from cinematic implicitly).
#[must_use]
pub(crate) fn list_custom_scenes_text(scenes: &BTreeMap<String, UserProfile>) -> String {
    let mut out = String::new();
    for (name, scene) in scenes {
        if let Some(base) = scene.base_scene.as_deref() {
            out.push_str(&format!("  {name} (base: {base})\n"));
        } else {
            out.push_str(&format!("  {name}\n"));
        }
    }
    out
}

/// Render a detailed description of a single custom scene.
#[must_use]
pub(crate) fn show_custom_scene_text(name: &str, scene: &UserProfile) -> String {
    let mut out = String::new();
    out.push_str(&format!("CUSTOM SCENE: {name}\n\n"));
    out.push_str("  Configuration:\n");

    let mut has_field = false;
    if let Some(base) = scene.base_scene.as_deref() {
        out.push_str(&format!("    base-scene          = {base}\n"));
        has_field = true;
    }
    if let Some(color) = scene.color.as_deref() {
        out.push_str(&format!("    color              = {color}\n"));
        has_field = true;
    }
    if let Some(charset) = scene.charset.as_deref() {
        out.push_str(&format!("    charset            = {charset}\n"));
        has_field = true;
    }
    if let Some(fps) = scene.fps.as_deref() {
        out.push_str(&format!("    fps                = {fps}\n"));
        has_field = true;
    }
    if let Some(speed) = scene.speed.as_deref() {
        out.push_str(&format!("    speed              = {speed}\n"));
        has_field = true;
    }
    if let Some(density) = scene.density.as_deref() {
        out.push_str(&format!("    density            = {density}\n"));
        has_field = true;
    }
    if let Some(glitch) = scene.glitch_level.as_deref() {
        out.push_str(&format!("    glitch-level       = {glitch}\n"));
        has_field = true;
    }
    if let Some(size) = scene.monolith_size.as_deref() {
        out.push_str(&format!("    monolith-size      = {size}\n"));
        has_field = true;
    }
    if let Some(bg) = scene.color_bg.as_deref() {
        out.push_str(&format!("    color-bg           = {bg}\n"));
        has_field = true;
    }

    if !has_field {
        out.push_str("    (no fields set — using global defaults from cinematic)\n");
    }

    out.push_str("\n  Use: cosmostrix --scene-custom ");
    out.push_str(name);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests;
