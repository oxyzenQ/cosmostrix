// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom scene display + name validation helpers — extracted from
//! `scene_custom/mod.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Owns 5 pure functions:
//! - is_valid_custom_scene_name / validate_custom_scene_name: name
//!   validation (charset + length + leading-char rules).
//! - parse_density_map: CSV -> &'static [f64] with leak-cache dedup.
//! - list_custom_scenes_text / show_custom_scene_text: human-readable
//!   formatting for --list-scenes / --show-scene output.
//!
//! Re-exported from `scene_custom/mod.rs` via `pub(crate) use` so all
//! existing call sites resolve unchanged.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

#[cfg(test)]
use super::is_valid_profile_name;
use crate::scene_custom::UserProfile;

/// v51 killer-features hardening: maximum number of entries accepted in a
/// `density-map` value. The map weights monolith pillar placement per
/// column region; real terminals are at most a few hundred columns wide, so
/// 1024 entries is already generous. Without the cap, a pasted 1M-entry CSV
/// would leak ~8 MB into the dedup cache per DISTINCT value (the cache
/// leaks `Box::leak` slices), and a long editing session changing the value
/// repeatedly would grow RSS without bound — the same typo-bloat class the
/// charset (256 chars) / colors (64 stops) / block-count (100) caps close.
pub(crate) const DENSITY_MAP_MAX_ENTRIES: usize = 1024;

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
        let mut weights: Vec<f64> = csv
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
            return None;
        }
        // v51 killer-features hardening: cap the entry count BEFORE the
        // leak (a pasted mega-CSV must not leak megabytes into the cache).
        // warn_runtime_or_now keeps mid-session fires buffered (AB-10);
        // identical truncation notes dedup in the warning log.
        if weights.len() > DENSITY_MAP_MAX_ENTRIES {
            crate::output::warn_runtime_or_now(&format!(
                "density-map has {} entries — truncated to {DENSITY_MAP_MAX_ENTRIES} at runtime",
                weights.len()
            ));
            weights.truncate(DENSITY_MAP_MAX_ENTRIES);
        }
        Some(weights)
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
///
/// v51 killer-features hardening: `monolith-size` and `color-bg` are
/// intentionally NOT displayed — they are forbidden in `[scene-custom.*]`
/// blocks by the owner contract (`SCENE_CUSTOM_FIELDS` excludes them, so
/// `collect_custom_scenes` never sets those fields). The former display
/// arms were unreachable dead code.
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

    if !has_field {
        out.push_str("    (no fields set — using global defaults from cinematic)\n");
    }

    out.push_str("\n  Use: cosmostrix --scene-custom ");
    out.push_str(name);
    out.push('\n');
    out
}
