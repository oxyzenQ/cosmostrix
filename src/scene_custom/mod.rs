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

use crate::config::Args;

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

/// v50.0.0-beta.6 LTS: maximum number of custom scene blocks accepted
/// in a single config.toml. Aligned with colors-custom and charset-custom
/// (all 3 systems use 100). Bounds the BTreeMap size + iteration cost in
/// `collect_custom_scenes`. 100 blocks is far beyond any realistic use
/// case; the cap prevents a config typo from spawning hundreds of blocks.
pub(crate) const SCENE_CUSTOM_MAX_BLOCKS: usize = 100;

/// v50.0.0-beta.6 LTS: maximum length of a custom scene block name.
/// Aligned with colors-custom and charset-custom (all use 64 chars).
/// Bounds BTreeMap key allocation. 64 chars is generous (built-in scene
/// names are ≤16 chars like "cinematic"); longer names are likely typos.
pub(crate) const SCENE_CUSTOM_MAX_NAME_LEN: usize = 64;

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
        // v50.0.0-beta.6 LTS: skip oversized names early (before
        // to_ascii_lowercase allocates). 64 chars is generous.
        if name.len() > SCENE_CUSTOM_MAX_NAME_LEN {
            continue;
        }
        let name_lower = name.to_ascii_lowercase();
        // v50.0.0-beta.6 LTS: skip if we already hit the block cap.
        // Prevents a config with hundreds of [scene-custom.X] blocks
        // from bloating the BTreeMap.
        if scenes.len() >= SCENE_CUSTOM_MAX_BLOCKS && !scenes.contains_key(&name_lower) {
            continue;
        }
        let scene = scenes
            .entry(name_lower)
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
            new.glitch_level = GlitchLevel::None;
            new.glitch_enabled = false;
            new.glitch_low = 300;
            new.glitch_high = 400;
            new.glitch_pct = 0.0;
            new.short_pct = 50.0;
            new.die_early_pct = 33.33333;
        }
        GlitchLevel::Subtle => {
            new.glitch_level = GlitchLevel::Subtle;
            new.glitch_enabled = true;
            new.glitch_low = 200;
            new.glitch_high = 300;
            new.glitch_pct = 3.0;
            new.short_pct = 60.0;
            new.die_early_pct = 45.0;
        }
        GlitchLevel::Default => {
            new.glitch_level = GlitchLevel::Default;
            new.glitch_enabled = true;
            new.glitch_low = 300;
            new.glitch_high = 400;
            new.glitch_pct = 10.0;
            new.short_pct = 50.0;
            new.die_early_pct = 33.33333;
        }
        GlitchLevel::Intense => {
            new.glitch_level = GlitchLevel::Intense;
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
// v50.0.0-beta.7 LOC refactor: display + name validation + density map
// parser extracted to display.rs to keep mod.rs under the 800-LOC hard
// cap. Re-exported here so all existing call sites resolve unchanged.
mod display;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use display::{is_valid_custom_scene_name, validate_custom_scene_name};
pub(crate) use display::{list_custom_scenes_text, parse_density_map, show_custom_scene_text};

// v50.0.0-beta.7 LOC refactor: parse helpers extracted to helpers.rs.
mod helpers;
mod overrides;
#[allow(unused_imports)]
pub(crate) use helpers::{
    is_explicit, parse_bool, parse_color_bg, parse_f32_override, parse_f64_override,
    parse_speed_override, parse_u8_override, profile_name_list, warn_invalid,
};
#[allow(unused_imports)]
pub(crate) use overrides::{apply_profile_overrides, apply_scene_custom_field_to_cloud_config};

#[cfg(test)]
mod tests;
