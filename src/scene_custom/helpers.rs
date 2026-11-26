// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Scene-custom parse helpers — extracted from `scene_custom/mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns the pure helper functions used by apply_profile_overrides +
//! apply_scene_custom_field_to_cloud_config:
//! - parse_f32_override / parse_f64_override: range-validated numeric parsing.
//! - parse_speed_override: speed-specific parser with clamp.
//! - profile_name_list: comma-separated profile name list.
//! - is_explicit: clap ValueSource::CommandLine check.
//! - warn_invalid: warning printer for invalid scene-custom values.
//!
//! v80.0.0-beta.2 (S-master-LOGIC-3): parse_u8_override (bold /
//! shading-mode), parse_color_bg (color-bg), and parse_bool (async-mode)
//! were removed together with their fields — scene-custom blocks no
//! longer carry style dimensions.

use std::collections::BTreeMap;

use clap::parser::ValueSource;

use crate::constants::{SPEED_MAX, SPEED_MIN};
use crate::validation::{
    parse_canonical_f32_range, parse_canonical_f64_range, parse_canonical_speed,
};

#[allow(unused_imports)]
use super::UserProfile;

pub(crate) fn parse_f32_override(
    name: &str,
    field: &str,
    value: &str,
    min: f32,
    max: f32,
) -> Option<f32> {
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

pub(crate) fn parse_f64_override(
    name: &str,
    field: &str,
    value: &str,
    min: f64,
    max: f64,
) -> Option<f64> {
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

pub(crate) fn parse_speed_override(name: &str, value: &str) -> Option<f32> {
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

pub(crate) fn profile_name_list(profiles: &BTreeMap<String, UserProfile>) -> String {
    if profiles.is_empty() {
        "<none defined>".to_string()
    } else {
        profiles.keys().cloned().collect::<Vec<_>>().join(", ")
    }
}

pub(crate) fn is_explicit(matches: &clap::ArgMatches, key: &str) -> bool {
    !matches!(
        matches.value_source(key),
        None | Some(ValueSource::DefaultValue)
    )
}

pub(crate) fn warn_invalid(profile: &str, field: &str, value: &str, expected: &str) {
    // v80.0.0-beta.1 defining-features hardening: routed through warn_runtime_or_now.
    // Today warn_invalid fires only on startup paths (pre-alt-screen, prints
    // immediately as before), but the scene-custom field appliers it now
    // serves also run during live reload — if a warning ever fires there it
    // must buffer (AB-10) instead of leaking into the rain matrix.
    crate::output::warn_runtime_or_now(&format!(
        "scene-custom: invalid {field}='{value}' in scene-custom '{profile}' (expected: {expected})"
    ));
}

/// Runtime display helper — the original-case spelling of a custom
/// scene block's name.
///
/// NIGHT-firefly-1 (2026-11-26): the lookup key stays lowercase
/// everywhere (the matching contract: `--scene tron_legacy` and the
/// config value both resolve `[scene-custom.Tron_legacy]`), but the
/// name carried into runtime display state — the HUD `scn:` line, the
/// startup verbose `scene:` line, the session report, the benchmark
/// scene field — must keep the block's original spelling, exactly like
/// the listing surfaces since NIGHT-cafe-1 (`--list-scenes` already
/// shows the original-case header record). Without this, a block
/// spelled `[scene-custom.Tron_legacy]` rendered the startup verbose
/// line as `scene: tron_legacy` although the listing showed
/// `Tron_legacy` — the two surfaces disagreed about one name (the same
/// bug class NIGHT-dragon-fly-2 closed for charsets).
///
/// Mirrors
/// [`crate::scene::charset_custom::original_case_custom_charset_name`]:
/// rides the memoized config load (no extra disk read; the startup
/// caller just resolved the same file). Returns `None` when no
/// `[scene-custom.<normalized>]` block exists (builtin scene, no
/// config, or hand-built values map with no parse record) so the
/// caller keeps its normalized name unchanged.
#[must_use]
pub(crate) fn original_case_custom_scene_name(
    config_path: Option<&std::path::Path>,
    normalized: &str,
) -> Option<String> {
    if normalized.is_empty() {
        return None;
    }
    let key = format!("scene-custom.{normalized}");
    crate::configfile::load_config_file_full(config_path)
        .original_custom_name_map()
        .get(&key)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NIGHT-firefly-1: the runtime display helper returns the block's
    /// original-case spelling for a lowercase lookup key, and `None` for
    /// anything that is not a custom scene block (builtin name, no
    /// match, empty) — the caller then keeps its normalized name, so
    /// builtin scenes and no-config runs pass through unchanged.
    #[test]
    fn original_case_scene_name_returns_block_spelling() {
        let dir = std::env::temp_dir().join(format!(
            "cosmostrix-nf1-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "[scene-custom.Tron_legacy]\n\
             rain = \"glyph\"\n\
             color = \"energy-zen\"\n\
             charset = \"retro\"\n\
             fps = 60\n\
             speed = \"50\"\n\
             density = \"0.65\"\n\
             glitch-level = \"subtle\"\n",
        )
        .expect("write config");
        assert_eq!(
            original_case_custom_scene_name(Some(&path), "tron_legacy").as_deref(),
            Some("Tron_legacy"),
            "lowercase lookup key must resolve to the block's original spelling"
        );
        assert_eq!(
            original_case_custom_scene_name(Some(&path), "cosmos"),
            None,
            "a builtin name has no custom block record"
        );
        assert_eq!(
            original_case_custom_scene_name(Some(&path), ""),
            None,
            "empty lookup never resolves"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
