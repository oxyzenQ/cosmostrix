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

use crate::config::Args;

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

#[cfg(test)]
use crate::profile::PROFILE_FIELDS;
use crate::profile::{apply_profile_layer, collect_profiles, is_valid_profile_name, UserProfile};

/// Config namespace prefix for custom scene blocks.
pub(crate) const SCENE_CUSTOM_NAMESPACE: &str = "scene-custom";

/// explicit field allowlist for `[scene-custom.<name>]` blocks.
///
/// Owner contract (2026-08-07):
/// - ALLOWED: `base-scene`, `color`, `charset`, `bold`, `colors-custom`,
///   `charset-custom`, `shadingmode`, `glitch-level`, `fps`, `speed`,
///   `density`, `density-map`, `async`.
/// - FORBIDDEN (rejected as unknown key by `is_scene_custom_config_key`):
///   `ambient`, `auto-color-drift`, `color.tune`, `monolith-size`,
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
    "async",
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
/// Returns a `BTreeMap<name, UserProfile>` mirroring
/// [`crate::profile::collect_profiles`] but scoped to the `scene-custom`
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
            "async" => scene.async_mode = Some(value.clone()),
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
    eprintln!(
        "config: ignoring unknown custom scene '{name}' (available: {list}; see --list-scenes)"
    );
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
        "async" => {
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
mod tests {
    use super::*;

    #[test]
    fn scene_custom_keys_are_recognized() {
        // `base-scene` is restored as a recognized scene-custom key.
        // It triggers inheritance from a built-in scene before the custom
        // scene's own overrides are applied. The legacy `preset` field
        // remains removed.
        assert!(is_scene_custom_config_key(
            "scene-custom.hacker-mode.base-scene"
        ));
        assert!(!is_scene_custom_config_key(
            "scene-custom.hacker-mode.preset"
        ));
        assert!(is_scene_custom_config_key(
            "scene-custom.nightcore.glitch-level"
        ));
        assert!(!is_scene_custom_config_key(
            "scene-custom.hacker-mode.unknown"
        ));
        assert!(!is_scene_custom_config_key("scene-custom..base"));
        assert!(!is_scene_custom_config_key("profile.nightcore.base"));
    }

    #[test]
    fn collect_custom_scenes_groups_fields_by_name() {
        let cfg = HashMap::from([
            (
                "scene-custom.hacker-mode.color".to_string(),
                "green".to_string(),
            ),
            ("scene-custom.nightcore.speed".to_string(), "24".to_string()),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes["hacker-mode"].color.as_deref(), Some("green"));
        assert_eq!(scenes["nightcore"].speed.as_deref(), Some("24"));
    }

    #[test]
    fn collect_custom_scenes_ignores_profile_keys() {
        let cfg = HashMap::from([
            (
                "profile.nightcore.color".to_string(),
                "monolith".to_string(),
            ),
            (
                "scene-custom.nightcore.color".to_string(),
                "purple".to_string(),
            ),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes["nightcore"].color.as_deref(), Some("purple"));
    }

    #[test]
    fn validate_custom_scene_name_accepts_valid() {
        assert_eq!(
            validate_custom_scene_name("hacker-mode").unwrap(),
            "hacker-mode"
        );
        assert_eq!(
            validate_custom_scene_name("Nightcore_42").unwrap(),
            "nightcore_42"
        );
    }

    #[test]
    fn validate_custom_scene_name_rejects_invalid() {
        assert!(validate_custom_scene_name("").is_err());
        assert!(validate_custom_scene_name("with space").is_err());
        assert!(validate_custom_scene_name("dot.name").is_err());
    }

    #[test]
    fn scene_custom_namespace_constant_matches_prefix() {
        assert_eq!(SCENE_CUSTOM_NAMESPACE, "scene-custom");
    }

    // ── rain_style_for_custom_scene ──

    #[test]
    fn rain_style_for_custom_scene_returns_base_scene_rain_style() {
        // Custom scene with base-scene = monolith → RainStyle::Monolith.
        let cfg = HashMap::from([(
            "scene-custom.afternoon.base-scene".to_string(),
            "monolith".to_string(),
        )]);
        let rs = rain_style_for_custom_scene(&cfg, "afternoon");
        assert_eq!(rs, Some(crate::rain_style::RainStyle::Monolith));
    }

    #[test]
    fn rain_style_for_custom_scene_returns_glyph_for_signal_base() {
        // Custom scene with base-scene = signal → RainStyle::Glyph.
        let cfg = HashMap::from([(
            "scene-custom.afternoon.base-scene".to_string(),
            "signal".to_string(),
        )]);
        let rs = rain_style_for_custom_scene(&cfg, "afternoon");
        assert_eq!(rs, Some(crate::rain_style::RainStyle::Glyph));
    }

    #[test]
    fn rain_style_for_custom_scene_returns_none_when_no_base_scene() {
        // Custom scene with no base-scene → None (caller falls back to Glyph).
        let cfg = HashMap::from([(
            "scene-custom.bare.color".to_string(),
            "neon-green".to_string(),
        )]);
        let rs = rain_style_for_custom_scene(&cfg, "bare");
        assert!(rs.is_none());
    }

    #[test]
    fn rain_style_for_custom_scene_returns_none_for_unknown_custom_name() {
        let cfg = HashMap::new();
        let rs = rain_style_for_custom_scene(&cfg, "nonexistent");
        assert!(rs.is_none());
    }

    #[test]
    fn rain_style_for_custom_scene_returns_none_for_unknown_base_scene() {
        // base-scene = "fake-scene" is not a built-in → None.
        let cfg = HashMap::from([(
            "scene-custom.broken.base-scene".to_string(),
            "fake-scene".to_string(),
        )]);
        let rs = rain_style_for_custom_scene(&cfg, "broken");
        assert!(rs.is_none());
    }

    #[test]
    fn rain_style_for_custom_scene_is_case_insensitive_on_custom_name() {
        // Custom scene names are stored lowercase by collect_custom_scenes;
        // rain_style_for_custom_scene normalizes its input to match.
        let cfg = HashMap::from([(
            "scene-custom.afternoon.base-scene".to_string(),
            "monolith".to_string(),
        )]);
        let rs = rain_style_for_custom_scene(&cfg, "AFTERNOON");
        assert_eq!(rs, Some(crate::rain_style::RainStyle::Monolith));
    }

    // Note: live-reload path (`apply_base_scene_to_cloud_config`) is exercised
    // end-to-end by the `rebuild_cloud_config` integration path. Unit-testing
    // it in isolation requires constructing a full CloudConfig (40+ fields),
    // which is brittle. The startup apply path (`apply_profile_layer` with
    // base-scene) is unit-tested in `config_apply_tests/profiles.rs::profile_base_scene_applies_inherited_defaults`,
    // and the runtime apply path (`Cloud::apply_ambient_entry` with a custom
    // scene) is unit-tested in `cloud/tests/tests_scene/transitions.rs`.

    #[test]
    fn profile_fields_are_reusable_for_custom_scenes() {
        // `base-scene` is restored to PROFILE_FIELDS (with cleaner
        // inheritance semantics — see profile.rs). `preset` remains removed.
        assert!(PROFILE_FIELDS.contains(&"base-scene"));
        assert!(!PROFILE_FIELDS.contains(&"preset"));
        assert!(PROFILE_FIELDS.contains(&"color"));
        // Atmosphere engine eliminated — atmosphere-regime is no longer a
        // valid profile field.
        assert!(!PROFILE_FIELDS.contains(&"atmosphere-regime"));
        assert!(!PROFILE_FIELDS.contains(&"atmosphere-mode"));
        assert!(!PROFILE_FIELDS.contains(&"nonexistent-field"));
    }

    #[test]
    fn list_custom_scenes_text_shows_base_annotation_when_set() {
        // when a custom scene sets `base-scene`, the listing
        // annotates it as `name (base: <base-scene>)`. Custom scenes
        // without `base-scene` render as just `name`.
        let cfg = HashMap::from([
            (
                "scene-custom.alpha.base-scene".to_string(),
                "signal".to_string(),
            ),
            ("scene-custom.alpha.color".to_string(), "storm".to_string()),
            ("scene-custom.beta.color".to_string(), "neon".to_string()),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        let text = list_custom_scenes_text(&scenes);
        assert!(text.contains("alpha"), "list must include alpha: {text}");
        assert!(
            text.contains("alpha (base: signal)"),
            "alpha should show base annotation: {text}"
        );
        assert!(
            !text.contains("beta (base:"),
            "beta has no base-scene — should NOT show annotation: {text}"
        );
        assert!(text.contains("beta"), "list must include beta: {text}");
    }

    #[test]
    fn show_custom_scene_text_includes_fields_and_usage() {
        let cfg = HashMap::from([
            (
                "scene-custom.hacker-mode.base-scene".to_string(),
                "monolith".to_string(),
            ),
            (
                "scene-custom.hacker-mode.color".to_string(),
                "green".to_string(),
            ),
            (
                "scene-custom.hacker-mode.speed".to_string(),
                "24".to_string(),
            ),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        let scene = &scenes["hacker-mode"];
        let text = show_custom_scene_text("hacker-mode", scene);
        assert!(
            text.contains("CUSTOM SCENE: hacker-mode"),
            "header missing: {text}"
        );
        assert!(
            text.contains("base-scene          = monolith"),
            "base-scene field missing: {text}"
        );
        assert!(
            text.contains("color              = green"),
            "color field missing: {text}"
        );
        assert!(
            text.contains("speed              = 24"),
            "speed field missing: {text}"
        );
        assert!(
            text.contains("cosmostrix --scene-custom hacker-mode"),
            "usage hint missing: {text}"
        );
    }

    #[test]
    fn show_custom_scene_text_handles_empty_profile() {
        let scene = UserProfile::default();
        let text = show_custom_scene_text("empty", &scene);
        assert!(
            text.contains("no fields set"),
            "empty profile should mention inheritance: {text}"
        );
    }

    // ── parse_density_map tests ──

    #[test]
    fn parse_density_map_valid_csv() {
        let map = parse_density_map("1.0,0.5,0.0,0.8");
        assert!(map.is_some());
        let map = map.unwrap();
        assert_eq!(map.len(), 4);
        assert_eq!(map[0], 1.0);
        assert_eq!(map[1], 0.5);
        assert_eq!(map[2], 0.0);
        assert_eq!(map[3], 0.8);
    }

    #[test]
    fn parse_density_map_clamps_out_of_range() {
        let map = parse_density_map("1.5,-0.3,2.0").unwrap();
        assert_eq!(map[0], 1.0); // 1.5 clamped to 1.0
        assert_eq!(map[1], 0.0); // -0.3 clamped to 0.0
        assert_eq!(map[2], 1.0); // 2.0 clamped to 1.0
    }

    #[test]
    fn parse_density_map_skips_empty_and_whitespace() {
        let map = parse_density_map("1.0, , 0.5 ,, 0.0");
        assert!(map.is_some());
        assert_eq!(map.unwrap().len(), 3);
    }

    #[test]
    fn parse_density_map_empty_string_returns_none() {
        assert!(parse_density_map("").is_none());
        assert!(parse_density_map("   ").is_none());
    }

    #[test]
    fn parse_density_map_invalid_numbers_return_none() {
        assert!(parse_density_map("abc,def").is_none());
        assert!(parse_density_map("not_a_number").is_none());
    }

    #[test]
    fn parse_density_map_single_value() {
        let map = parse_density_map("0.7");
        assert!(map.is_some());
        assert_eq!(map.unwrap(), &[0.7]);
    }

    #[test]
    fn parse_density_map_mixed_valid_invalid() {
        // Valid numbers are kept; invalid entries are skipped.
        let map = parse_density_map("1.0,abc,0.5");
        assert!(map.is_some());
        assert_eq!(map.unwrap(), &[1.0, 0.5]);
    }

    // v30 fix: quoted CSV strings must work. The configfile parser is a
    // custom line-by-line parser that does NOT strip surrounding quotes
    // from string values, so the leaf parser must do it. Without this,
    // `density-map = "0.05,0.3,1.0"` would parse `"0.05` as the first
    // entry (not a float) and silently produce None at runtime while
    // also failing --testconf.
    #[test]
    fn parse_density_map_accepts_double_quoted_csv() {
        let map = parse_density_map("\"0.05,0.3,1.0\"");
        assert!(map.is_some());
        assert_eq!(map.unwrap(), &[0.05, 0.3, 1.0]);
    }

    #[test]
    fn parse_density_map_accepts_single_quoted_csv() {
        let map = parse_density_map("'0.1, 0.2, 0.3'");
        assert!(map.is_some());
        assert_eq!(map.unwrap(), &[0.1, 0.2, 0.3]);
    }

    #[test]
    fn parse_density_map_accepts_quoted_with_whitespace_padding() {
        // User wrote `density-map = " 0.5, 0.5 "` — quotes + outer spaces.
        let map = parse_density_map("  \"0.5,0.5\"  ");
        assert!(map.is_some());
        assert_eq!(map.unwrap(), &[0.5, 0.5]);
    }

    #[test]
    fn parse_density_map_quoted_and_unquoted_share_cache_entry() {
        // Both forms normalize to the same key `"0.5,0.5"` → 0.5,0.5,
        // so the dedup cache should return the same slice pointer.
        let a = parse_density_map("0.5,0.5").unwrap();
        let b = parse_density_map("\"0.5,0.5\"").unwrap();
        assert!(
            std::ptr::eq(a.as_ptr(), b.as_ptr()),
            "quoted and unquoted forms should share the same cached slice"
        );
    }

    #[test]
    fn parse_density_map_quoted_empty_string_returns_none() {
        assert!(parse_density_map("\"\"").is_none());
        assert!(parse_density_map("''").is_none());
    }

    // ── scene-custom field allowlist / forbidden-field tests ──

    #[test]
    fn scene_custom_fields_includes_v30_3_additions() {
        // Owner contract: these MUST be accepted in scene-custom blocks.
        for field in &[
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
            "async",
        ] {
            assert!(
                SCENE_CUSTOM_FIELDS.contains(field),
                "SCENE_CUSTOM_FIELDS must include '{field}'"
            );
        }
    }

    #[test]
    fn scene_custom_fields_excludes_forbidden_fields() {
        // Owner contract: these MUST NOT be accepted in scene-custom blocks.
        for field in &[
            "monolith-size",
            "color-bg",
            "ambient",
            "auto-color-drift",
            "intro",
        ] {
            assert!(
                !SCENE_CUSTOM_FIELDS.contains(field),
                "SCENE_CUSTOM_FIELDS must NOT include '{field}' (forbidden per owner contract)"
            );
        }
    }

    #[test]
    fn is_scene_custom_config_key_accepts_v30_3_fields() {
        for field in &[
            "bold",
            "colors-custom",
            "charset-custom",
            "shadingmode",
            "async",
        ] {
            let key = format!("scene-custom.test.{field}");
            assert!(
                is_scene_custom_config_key(&key),
                "is_scene_custom_config_key should accept '{key}'"
            );
        }
    }

    #[test]
    fn is_scene_custom_config_key_rejects_forbidden_fields() {
        // monolith-size and color-bg were accepted — they
        // MUST now be rejected per owner contract.
        for field in &[
            "monolith-size",
            "color-bg",
            "ambient",
            "auto-color-drift",
            "intro",
        ] {
            let key = format!("scene-custom.test.{field}");
            assert!(
                !is_scene_custom_config_key(&key),
                "is_scene_custom_config_key must REJECT '{key}' (forbidden)"
            );
        }
    }

    #[test]
    fn collect_custom_scenes_parses_v30_3_fields() {
        let cfg = HashMap::from([
            ("scene-custom.test.bold".to_string(), "1".to_string()),
            (
                "scene-custom.test.colors-custom".to_string(),
                "sunset".to_string(),
            ),
            (
                "scene-custom.test.charset-custom".to_string(),
                "zen".to_string(),
            ),
            ("scene-custom.test.shadingmode".to_string(), "1".to_string()),
            ("scene-custom.test.async".to_string(), "true".to_string()),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        let scene = &scenes["test"];
        assert_eq!(scene.bold.as_deref(), Some("1"));
        assert_eq!(scene.colors_custom.as_deref(), Some("sunset"));
        assert_eq!(scene.charset_custom.as_deref(), Some("zen"));
        assert_eq!(scene.shading_mode.as_deref(), Some("1"));
        assert_eq!(scene.async_mode.as_deref(), Some("true"));
    }

    #[test]
    fn collect_custom_scenes_silently_drops_forbidden_fields() {
        // monolith-size and color-bg are filtered out by
        // is_scene_custom_config_key, so collect_custom_scenes never sees
        // them. Verify they don't appear in the parsed UserProfile.
        let cfg = HashMap::from([
            ("scene-custom.test.color".to_string(), "green".to_string()),
            (
                "scene-custom.test.monolith-size".to_string(),
                "large".to_string(),
            ),
            (
                "scene-custom.test.color-bg".to_string(),
                "black".to_string(),
            ),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        let scene = &scenes["test"];
        assert_eq!(scene.color.as_deref(), Some("green"));
        // monolith_size and color_bg are NOT set (keys were filtered out).
        assert!(scene.monolith_size.is_none());
        assert!(scene.color_bg.is_none());
    }
}
