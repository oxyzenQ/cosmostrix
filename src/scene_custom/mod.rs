// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined custom scene support for `[scene-custom.<name>]` config blocks.
//!
//! v80.0.0-beta.2 schema (owner contract, S-master-LOGIC-3): a custom
//! scene is a COMPLETE, self-contained profile — exactly seven
//! conceptual fields, ALL required (NIGHT-research-5: added `rain`):
//!
//! ```toml
//! [scene-custom.example]
//! rain = "lorenz"               # NIGHT-research-5: pick rain style (glyph/monolith/vortex/flux/lorenz/dragon/physarum)
//! color = "aurora"              # built-in color name  OR:
//! # colors-custom = "aurora"    # custom palette block reference
//! charset = "binary"            # built-in charset     OR:
//! # charset-custom = "binary"   # custom charset block reference
//! fps = 90
//! speed = 12
//! density = 0.90
//! glitch-level = "none"
//! ```
//!
//! Each of the seven fields must be present (one of each pair) — an
//! incomplete block is a hard validation error (`--testconf`, startup,
//! and live-reload all reject it). The block owns the same seven
//! scene-family dimensions an ambient entry owns (scene, rain, color,
//! charset, fps, speed, density, glitch-level). Field VALUES are
//! validated too (S-master-HUNT): `colors-custom`/`charset-custom`
//! must reference blocks that exist in the same config (a BUILT-IN
//! name gets a targeted "use the `color`/`charset` field" hint — it is
//! a hard error, never a silent runtime no-op), and the numeric/enum
//! fields carry the same ranges as their top-level keys.
//!
//! REMOVED in v80.0.0-beta.2 (owner mandate): `base-scene` (custom
//! scenes no longer inherit from built-ins — they stand alone),
//! `bold`, `shading-mode`, and `async-mode` (not scene-family
//! dimensions; use the top-level config keys, which stay
//! live-reloadable and are reported in the final runtime state).
//! Legacy configs carrying these keys get a targeted removal hint
//! from `config_hints`.
//!
//! NIGHT-research-5 (owner-approved): added the `rain` field. Custom
//! scenes can now pick any of the seven existing rain styles (glyph,
//! monolith, vortex, flux, lorenz, dragon, physarum) — previously
//! custom scenes always rendered `RainStyle::Glyph`. The `rain` field is validated
//! against [`crate::rain_style::RainStyle::from_label`] (same
//! canonical labels as `--show-scene` / `--list-scenes` output).
//! The user's example: `rain = "lorenz"` selects the lorenz style.
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
///
/// v80.0.0-beta.2 (S-master-LOGIC-3): shrunk to the six scene-family
/// dimensions. `base-scene`, `bold`, `shading-mode`, `async-mode`,
/// `monolith-size`, and `color-bg` are REMOVED — blocks are complete,
/// self-contained profiles now (see [`SCENE_CUSTOM_REQUIRED_FIELDS`]).
/// NIGHT-research-5 (owner-approved): added `rain` — the seventh
/// scene-family dimension. Custom scenes can now pick any existing
/// rain style by name (glyph/monolith/vortex/flux/lorenz/dragon/
/// physarum).
pub(crate) const PROFILE_FIELDS: &[&str] = &[
    "rain",
    "color",
    "colors-custom",
    "charset",
    "charset-custom",
    "fps",
    "speed",
    "density",
    "glitch-level",
];

/// The seven required scene-custom dimensions, in error-message order.
///
/// Each entry is (primary key, alternative key) — the two pair fields
/// (`color`/`colors-custom`, `charset`/`charset-custom`) accept either
/// key; the other five are single mandatory keys. Used by
/// [`missing_scene_custom_fields`] and the completeness validation so a
/// half-filled block reports exactly which dimensions are missing
/// instead of a generic "incomplete" error.
///
/// NIGHT-research-5 (owner-approved): added `rain` as the first entry.
/// The `rain` field picks a rain style by canonical label (glyph,
/// monolith, vortex, flux, lorenz, dragon, physarum). It leads the error-message order so a
/// missing `rain` field is the first thing the user sees when a block
/// is incomplete — rain style is now the headline dimension.
pub(crate) const SCENE_CUSTOM_REQUIRED_FIELDS: &[(&str, Option<&str>)] = &[
    ("rain", None),
    ("color", Some("colors-custom")),
    ("charset", Some("charset-custom")),
    ("fps", None),
    ("speed", None),
    ("density", None),
    ("glitch-level", None),
];

/// The seven required dimensions as a flat human-readable list for error
/// messages and hints: "rain, color|colors-custom, charset|charset-custom,
/// fps, speed, density, glitch-level".
#[must_use]
pub(crate) fn scene_custom_required_fields_hint() -> String {
    SCENE_CUSTOM_REQUIRED_FIELDS
        .iter()
        .map(|(primary, alt)| match alt {
            Some(alt) => format!("{primary}|{alt}"),
            None => (*primary).to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// List the required dimensions a `[scene-custom.<name>]` block is
/// missing (empty = complete).
///
/// v80.0.0-beta.2 owner contract (S-master-LOGIC-3): a block must be
/// COMPLETELY filled — `color` or `colors-custom`, `charset` or
/// `charset-custom`, `fps`, `speed`, `density`, and `glitch-level`.
/// An incomplete block is a hard validation error, not a
/// fall-back-to-defaults (the old partial-block model is retired with
/// `base-scene`). Returns each missing dimension rendered as
/// "primary|alternative" for the pair fields, bare key otherwise.
#[must_use]
pub(crate) fn missing_scene_custom_fields(profile: &UserProfile) -> Vec<String> {
    let has = |primary: &Option<String>, alt: &Option<String>| primary.is_some() || alt.is_some();
    let mut missing = Vec::new();
    if profile.rain.is_none() {
        missing.push("rain".to_string());
    }
    if !has(&profile.color, &profile.colors_custom) {
        missing.push("color|colors-custom".to_string());
    }
    if !has(&profile.charset, &profile.charset_custom) {
        missing.push("charset|charset-custom".to_string());
    }
    if profile.fps.is_none() {
        missing.push("fps".to_string());
    }
    if profile.speed.is_none() {
        missing.push("speed".to_string());
    }
    if profile.density.is_none() {
        missing.push("density".to_string());
    }
    if profile.glitch_level.is_none() {
        missing.push("glitch-level".to_string());
    }
    missing
}

/// Validate that every `[scene-custom.<name>]` block in `cfg` is
/// COMPLETELY filled (owner contract, S-master-LOGIC-3).
///
/// Returns `Err(message)` naming the first incomplete block and its
/// missing dimensions, `Ok(())` when every block is complete. Called
/// from `validate_config_strictly` (startup + live-reload + watcher)
/// and from `--testconf` so all three surfaces reject in lockstep.
pub(crate) fn validate_scene_custom_completeness(
    cfg: &HashMap<String, String>,
) -> Result<(), String> {
    for (name, profile) in collect_custom_scenes(cfg) {
        let missing = missing_scene_custom_fields(&profile);
        if missing.is_empty() {
            continue;
        }
        return Err(format!(
            "scene-custom '{name}' is incomplete: missing {} — a [scene-custom.<name>] block must be COMPLETELY filled ({}); incomplete blocks are rejected",
            missing.join(", "),
            scene_custom_required_fields_hint()
        ));
    }
    Ok(())
}

/// Resolve the fps a scene (built-in OR custom) declares, if any.
///
/// v80.0.0-beta.2 (S-master-LOGIC-3): an ambient entry owns the SAME
/// scene-family dimensions as a config scene switch — including `fps`
/// (previously fps was construction-time only, so an ambient-applied
/// scene-custom `fps = 12` never took effect). Returns:
/// - built-in scene → the scene's declared fps default;
/// - custom scene → the block's `fps` field (parsed, range-checked by
///   strict validation upstream);
/// - unknown scene / no fps declared → `None` (leave current fps).
#[must_use]
pub(crate) fn ambient_scene_fps(scene_name: &str, cfg: &HashMap<String, String>) -> Option<f64> {
    if let Some(scene_info) = crate::scene::get_scene(scene_name) {
        return scene_info.config.fps;
    }
    let normalized = scene_name.trim().to_ascii_lowercase();
    let key = format!("scene-custom.{normalized}.fps");
    cfg.get(&key)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|fps| (1.0..=240.0).contains(fps))
}

/// Lightweight collection of override fields for a scene-custom block.
///
/// Originally `UserProfile` from the inert `profile` module. The name is
/// kept to avoid a massive rename across scene-custom code.
///
/// v80.0.0-beta.2: only the six scene-family dimensions remain —
/// `base_scene`, `bold`, `shading_mode`, `async_mode`, `monolith_size`,
/// and `color_bg` were removed with the schema simplification.
/// NIGHT-research-5: added `rain` — the seventh scene-family dimension
/// (canonical RainStyle label string, parsed via `RainStyle::from_label`).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct UserProfile {
    /// NIGHT-research-5: rain style selection. The string is one of
    /// the canonical `RainStyle::as_str()` labels (glyph, monolith,
    /// vortex, flux, lorenz, dragon, physarum). Parsed by
    /// `RainStyle::from_label` at apply time; invalid values get a
    /// targeted hint with the valid list.
    pub rain: Option<String>,
    pub color: Option<String>,
    pub charset: Option<String>,
    pub fps: Option<String>,
    pub speed: Option<String>,
    pub density: Option<String>,
    pub glitch_level: Option<String>,
    /// Custom palette name referencing a `[colors-custom.<name>]` block.
    pub colors_custom: Option<String>,
    /// Custom charset name referencing a `[charset-custom.<name>]` block.
    pub charset_custom: Option<String>,
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
            "rain" => profile.rain = Some(value.clone()),
            "color" => profile.color = Some(value.clone()),
            "charset" => profile.charset = Some(value.clone()),
            "fps" => profile.fps = Some(value.clone()),
            "speed" => profile.speed = Some(value.clone()),
            "density" => profile.density = Some(value.clone()),
            "glitch-level" => profile.glitch_level = Some(value.clone()),
            "colors-custom" => profile.colors_custom = Some(value.clone()),
            "charset-custom" => profile.charset_custom = Some(value.clone()),
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
/// v80.0.0-beta.2: no `base-scene` inheritance layer — the block is a
/// complete self-contained profile (see the module docs). Missing
/// dimensions are rejected upstream by
/// `validate_scene_custom_completeness`, so this layer only ever sees
/// complete blocks.
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
        crate::output::warn_runtime_or_now(&format!(
            "ignoring unknown profile '{}' (available: {}; see --list-scenes)",
            name,
            profile_name_list(profiles)
        ));
        return Ok(modified);
    };

    apply_profile_overrides(matches, args, &normalized, profile, cfg, &mut modified);
    Ok(modified)
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
/// v80.0.0-beta.2 schema (owner contract, S-master-LOGIC-3): exactly the
/// seven scene-family dimensions. ALLOWED: `rain`, `color`, `colors-custom`,
/// `charset`, `charset-custom`, `fps`, `speed`, `density`,
/// `glitch-level`. Every block must set ALL seven dimensions — see
/// [`SCENE_CUSTOM_REQUIRED_FIELDS`] (incomplete blocks are a hard
/// validation error).
///
/// FORBIDDEN (rejected as unknown key by `is_scene_custom_config_key`):
/// - `base-scene`, `bold`, `shading-mode`, `async-mode` — REMOVED in
///   v80.0.0-beta.2 (custom scenes stand alone; use the top-level keys
///   for bold/shading-mode/async-mode);
/// - `monolith-size`, `color-bg`, `ambient`, `crystal-dragon`,
///   `color.tune`, `intro`, `density-map` (removed in v80.0.0-beta.2 —
///   the per-column monolith density-map burden function was retired;
///   configs still carrying them get a targeted removal hint from
///   `config_hints`).
///
/// NIGHT-research-5 (owner-approved): added `rain` — the seventh
/// scene-family dimension. Custom scenes can now pick any existing
/// rain style by canonical label (glyph, monolith, vortex, flux,
/// lorenz, dragon, physarum).
pub(crate) const SCENE_CUSTOM_FIELDS: &[&str] = &[
    "rain",
    "color",
    "colors-custom",
    "charset",
    "charset-custom",
    "fps",
    "speed",
    "density",
    "glitch-level",
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
            "rain" => scene.rain = Some(value.clone()),
            "color" => scene.color = Some(value.clone()),
            "charset" => scene.charset = Some(value.clone()),
            "fps" => scene.fps = Some(value.clone()),
            "speed" => scene.speed = Some(value.clone()),
            "density" => scene.density = Some(value.clone()),
            "glitch-level" => scene.glitch_level = Some(value.clone()),
            "colors-custom" => scene.colors_custom = Some(value.clone()),
            "charset-custom" => scene.charset_custom = Some(value.clone()),
            // Removed fields (base-scene/bold/shading-mode/async-mode/
            // monolith-size/color-bg) are NOT in SCENE_CUSTOM_FIELDS, so
            // is_scene_custom_config_key already filtered them out.
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
        // custom scene name so verbose output and
        // CloudConfig.scene_name both show `<name>`. v80.0.0-beta.2: the
        // block is a complete self-contained profile (no base-scene
        // inheritance, no fall-back-to-defaults — incomplete blocks are
        // rejected upstream by `validate_scene_custom_completeness`).
        // rain_style for the custom scene is resolved at Cloud
        // construction time — v80.0.0-beta.2: always Glyph (base-scene
        // inheritance removed; see `resolve_rain_style`).
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
    // v80.0.0-beta.1 did-you-mean audit: message built by the testable helper below.
    let message = unknown_custom_scene_error(name, &available);
    if strict_unknown {
        return Err(message);
    }
    crate::output::warn_runtime_or_now(&format!(
        "ignoring unknown custom scene '{}' (available: {}; see --list-scenes)",
        name, list
    ));
    Ok(HashSet::new())
}

/// Resolve the rain_style for any scene name (built-in OR custom).
///
/// Built-in scene → its declared rain_style. Custom scene → the block's
/// `rain` field value, parsed via [`crate::rain_style::RainStyle::from_label`]
/// (NIGHT-research-5 owner-approved). Returns `RainStyle::Glyph` (the
/// default) when the field is missing or unrecognized.
///
/// Resolve the rain style for a scene at Cloud construction time.
///
/// Built-in scene → the scene's declared rain style.
/// Custom scene (`[scene-custom.<name>]`) → the block's `rain` field,
/// parsed via [`crate::rain_style::RainStyle::from_label`]. Falls back
/// to [`crate::rain_style::RainStyle::Glyph`] when the field is missing
/// (the completeness validator rejects this upstream, but the fallback
/// keeps the function total) or when the label is unrecognized (the
/// apply path renders a targeted warning; startup falls back silently
/// to keep the rain rendering while the user fixes the config).
/// Unknown scene / no rain declared → [`crate::rain_style::RainStyle::Glyph`].
///
/// NIGHT-research-5 (owner-approved): the `rain` field lets users pick
/// any existing rain style by canonical label. Previously custom scenes
/// always rendered `RainStyle::Glyph`; now they're flexible.
///
/// Called from `main.rs` at Cloud construction time.
#[must_use]
pub(crate) fn resolve_rain_style(
    name: Option<&str>,
    cfg: &HashMap<String, String>,
) -> crate::rain_style::RainStyle {
    let Some(name) = name else {
        return crate::rain_style::RainStyle::Glyph;
    };
    // Built-in scene → its declared rain style.
    if let Some(style) = crate::scene::rain_style_for_scene(name) {
        return style;
    }
    // Custom scene → consult the block's `rain` field.
    let normalized = name.trim().to_ascii_lowercase();
    let key = format!("scene-custom.{normalized}.rain");
    if let Some(label) = cfg.get(&key) {
        if let Some(style) = crate::rain_style::RainStyle::from_label(label) {
            return style;
        }
    }
    // Fallback: Glyph (the default rain style).
    crate::rain_style::RainStyle::Glyph
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

// v50.0.0-beta.7 LOC refactor: display + name validation extracted to
// display.rs to keep mod.rs under the 800-LOC hard
// cap. Re-exported here so all existing call sites resolve unchanged.
mod display;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use display::{is_valid_custom_scene_name, validate_custom_scene_name};
pub(crate) use display::{list_custom_scenes_text, show_custom_scene_text};

// v50.0.0-beta.7 LOC refactor: parse helpers extracted to helpers.rs.
mod helpers;
mod overrides;
#[allow(unused_imports)]
pub(crate) use helpers::{
    is_explicit, parse_f32_override, parse_f64_override, parse_speed_override, profile_name_list,
    warn_invalid,
};
#[allow(unused_imports)]
pub(crate) use overrides::{
    apply_profile_overrides, apply_scene_custom_field_to_cloud_config,
    apply_scene_custom_to_cloud_config,
};

#[cfg(test)]
#[path = "../../test/scene_custom/tests.rs"]
mod tests;

/// v80.0.0-beta.1 did-you-mean audit: build the "unknown custom scene" error with a
/// closest-match suggestion (edit-distance <= 2, same policy as every
/// other value surface). Separate fn so the format is unit-testable
/// without constructing a full `Args`.
fn unknown_custom_scene_error(name: &str, available: &[String]) -> String {
    let list = if available.is_empty() {
        "<none defined>".to_string()
    } else {
        available.join(", ")
    };
    let tip = crate::cli::suggestion::closest_value_match(
        name,
        &available.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    )
    .map(|s| crate::cli::ux::format_value_suggestion(&s))
    .unwrap_or_default();
    format!(
        "error: unknown custom scene '{name}'{tip}\nexpected one of: {list}\n\n  Use --list-scenes to see built-in and custom scenes."
    )
}

#[cfg(test)]
mod suggestion_tests {
    use super::*;

    #[test]
    fn unknown_custom_scene_suggests_closest() {
        let available = vec!["afternoon".to_string()];
        let msg = unknown_custom_scene_error("afternon", &available);
        assert!(
            msg.contains("tip: a similar value exists: 'afternoon'"),
            "custom scene typo must suggest the closest block, got: {msg}"
        );
    }

    #[test]
    fn unknown_custom_scene_no_close_match_has_no_tip() {
        let available = vec!["afternoon".to_string()];
        let msg = unknown_custom_scene_error("zzzzzzz", &available);
        assert!(!msg.contains("tip: a similar"), "got: {msg}");
        assert!(msg.contains("expected one of: afternoon"));
    }

    #[test]
    fn unknown_custom_scene_empty_list_renders_none_defined() {
        let msg = unknown_custom_scene_error("whatever", &[]);
        assert!(msg.contains("expected one of: <none defined>"));
        assert!(!msg.contains("tip: a similar"));
    }
}
