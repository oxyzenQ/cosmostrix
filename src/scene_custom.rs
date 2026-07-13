// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! User-defined custom scene support for `[scene-custom.<name>]` config blocks.
//!
//! Custom scenes are user-authored themes that start from a base scene (or
//! preset) and override individual runtime fields. They replace the legacy
//! `[profile.<name>]` namespace with a clearer name that mirrors `--scene`.
//!
//! ## Backward compatibility
//!
//! When `--scene-custom <name>` is invoked, the loader first looks for
//! `[scene-custom.<name>]` in config. If absent, it falls back to
//! `[profile.<name>]` and emits a deprecation warning guiding migration to
//! the new namespace. This keeps existing user configs working without
//! silent breakage.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::config::Args;
use crate::profile::{
    apply_profile_layer, collect_profiles, is_valid_profile_name, validate_profile_name,
    UserProfile, PROFILE_FIELDS,
};

/// Config namespace prefix for custom scene blocks.
pub const SCENE_CUSTOM_NAMESPACE: &str = "scene-custom";

/// Returns `true` if `key` is a recognized `[scene-custom.<name>.<field>]` key.
///
/// Mirrors [`crate::profile::is_profile_config_key`] but for the
/// `scene-custom` namespace. The accepted `<field>` set is identical to
/// `PROFILE_FIELDS` so users can migrate a profile block to a custom-scene
/// block by renaming the prefix only.
#[must_use]
pub fn is_scene_custom_config_key(key: &str) -> bool {
    let Some((prefix, rest)) = key.split_once('.') else {
        return false;
    };
    if prefix != SCENE_CUSTOM_NAMESPACE {
        return false;
    }
    let Some((name, field)) = rest.rsplit_once('.') else {
        return false;
    };
    is_valid_profile_name(name) && PROFILE_FIELDS.contains(&field)
}

/// Collect all `[scene-custom.<name>]` blocks from a flat config map.
///
/// Returns a `BTreeMap<name, UserProfile>` mirroring
/// [`crate::profile::collect_profiles`] but scoped to the `scene-custom`
/// namespace. Field parsing reuses `PROFILE_FIELDS` so the resulting
/// `UserProfile` is structurally identical to a profile entry.
#[must_use]
pub fn collect_custom_scenes(cfg: &HashMap<String, String>) -> BTreeMap<String, UserProfile> {
    let mut scenes = BTreeMap::new();
    for (key, value) in cfg {
        if !is_scene_custom_config_key(key) {
            continue;
        }
        let (_, rest) = key.split_once('.').expect("scene-custom key has prefix");
        let (name, field) = rest.rsplit_once('.').expect("scene-custom key has field");
        let scene = scenes
            .entry(name.to_string())
            .or_insert_with(UserProfile::default);
        match field {
            "base" | "scene" => scene.base = Some(value.clone()),
            "preset" => scene.preset = Some(value.clone()),
            "color" => scene.color = Some(value.clone()),
            "charset" => scene.charset = Some(value.clone()),
            "fps" => scene.fps = Some(value.clone()),
            "speed" => scene.speed = Some(value.clone()),
            "density" => scene.density = Some(value.clone()),
            "glitch-level" => scene.glitch_level = Some(value.clone()),
            "monolith-size" => scene.monolith_size = Some(value.clone()),
            "color-bg" => scene.color_bg = Some(value.clone()),
            "atmosphere-mode" => scene.atmosphere_mode = Some(value.clone()),
            "atmosphere-regime" => scene.atmosphere_regime = Some(value.clone()),
            _ => {}
        }
    }
    scenes
}

/// Apply a user-defined custom scene by name.
///
/// Lookup order:
/// 1. `[scene-custom.<name>]` in config — applied directly.
/// 2. `[profile.<name>]` in config — applied with a deprecation warning
///    instructing the user to migrate to the `scene-custom` namespace.
/// 3. Neither — returns an error (or warning, depending on `strict_unknown`).
///
/// On success, sets `args.scene_custom = Some(name)` and clears
/// `args.profile` so subsequent profile-application logic does not re-run.
/// The applied field set is returned as `HashSet<&'static str>` for
/// downstream precedence tracking.
pub fn apply_scene_custom_layer(
    matches: &clap::ArgMatches,
    args: &mut Args,
    cfg: &HashMap<String, String>,
    name: &str,
    strict_unknown: bool,
) -> Result<HashSet<&'static str>, String> {
    let custom_scenes = collect_custom_scenes(cfg);

    // 1. Prefer the new [scene-custom.<name>] namespace.
    if custom_scenes.contains_key(name) {
        let modified = apply_profile_layer(matches, args, &custom_scenes, name, strict_unknown)?;
        // apply_profile_layer sets args.profile; redirect to args.scene_custom.
        args.profile = None;
        args.scene_custom = Some(name.to_string());
        return Ok(modified);
    }

    // 2. Fallback: legacy [profile.<name>] with a deprecation warning.
    let profiles = collect_profiles(cfg);
    if profiles.contains_key(name) {
        eprintln!(
            "warning: profile '{name}' is deprecated; migrate to [scene-custom.{name}] in config.toml (rename the prefix only — fields are unchanged)"
        );
        let modified = apply_profile_layer(matches, args, &profiles, name, strict_unknown)?;
        args.profile = None;
        args.scene_custom = Some(name.to_string());
        return Ok(modified);
    }

    // 3. Not found in either namespace.
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

/// Validate a custom-scene name. Shares the same rules as profile names
/// (letters, digits, `-`, `_`) so migration is frictionless.
#[must_use]
pub fn is_valid_custom_scene_name(name: &str) -> bool {
    is_valid_profile_name(name)
}

/// Normalize and validate a custom-scene name. Returns the lowercased name
/// on success or an error message on failure.
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

/// Re-export `validate_profile_name` so callers that need it can reach it
/// through the `scene_custom` namespace as well. Kept as a thin alias to
/// avoid duplicate logic.
#[allow(clippy::module_name_repetitions)]
#[allow(dead_code)] // surfaced for future CLI helpers (Stage 3+)
pub fn validate_scene_custom_name(name: &str) -> Result<String, String> {
    validate_profile_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_custom_keys_are_recognized() {
        assert!(is_scene_custom_config_key("scene-custom.hacker-mode.base"));
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
                "scene-custom.hacker-mode.base".to_string(),
                "storm".to_string(),
            ),
            (
                "scene-custom.hacker-mode.color".to_string(),
                "green".to_string(),
            ),
            ("scene-custom.nightcore.speed".to_string(), "24".to_string()),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes["hacker-mode"].color.as_deref(), Some("green"));
        assert_eq!(scenes["hacker-mode"].base.as_deref(), Some("storm"));
        assert_eq!(scenes["nightcore"].speed.as_deref(), Some("24"));
    }

    #[test]
    fn collect_custom_scenes_ignores_profile_keys() {
        let cfg = HashMap::from([
            ("profile.nightcore.base".to_string(), "monolith".to_string()),
            (
                "scene-custom.nightcore.color".to_string(),
                "purple".to_string(),
            ),
        ]);
        let scenes = collect_custom_scenes(&cfg);
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes["nightcore"].color.as_deref(), Some("purple"));
        assert!(scenes["nightcore"].base.is_none());
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

    #[test]
    fn profile_fields_are_reusable_for_custom_scenes() {
        // Custom scenes accept the same field set as profiles so migration
        // is a pure prefix rename (`profile.` → `scene-custom.`).
        assert!(PROFILE_FIELDS.contains(&"base"));
        assert!(PROFILE_FIELDS.contains(&"color"));
        assert!(PROFILE_FIELDS.contains(&"atmosphere-regime"));
        assert!(!PROFILE_FIELDS.contains(&"nonexistent-field"));
    }
}
