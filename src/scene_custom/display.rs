// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom scene display + name validation helpers — extracted from
//! `scene_custom/mod.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Owns 4 pure functions:
//! - is_valid_custom_scene_name / validate_custom_scene_name: name
//!   validation (charset + length + leading-char rules).
//! - list_custom_scenes_text / show_custom_scene_text: human-readable
//!   formatting for --list-scenes / --show-scene output.
//!
//! Re-exported from `scene_custom/mod.rs` via `pub(crate) use` so all
//! existing call sites resolve unchanged.
//!
//! v80.0.0-beta.2: `parse_density_map` (CSV -> leaked `&'static [f64]`
//! with a dedup cache) was removed together with the density-map
//! feature — the burden function is gone, so is its parser.

use std::collections::BTreeMap;

#[cfg(test)]
use super::is_valid_profile_name;
use crate::scene_custom::UserProfile;

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

/// Render a one-line-per-entry listing of custom scenes from config.
///
/// Output is appended under the "CUSTOM SCENES (from config)" heading in
/// `--list-scenes`. Mirrors the column layout of `scene::list_scenes_text`
/// so the two groups visually align.
///
/// v80.0.0-beta.2: custom scenes are self-contained profiles (no
/// `base-scene` inheritance) — every entry renders as just `name`.
#[must_use]
pub(crate) fn list_custom_scenes_text(scenes: &BTreeMap<String, UserProfile>) -> String {
    let mut out = String::new();
    for name in scenes.keys() {
        out.push_str(&format!("  {name}\n"));
    }
    out
}

/// Render a detailed description of a single custom scene.
///
/// v80.0.0-beta.1 killer-features hardening: `monolith-size` and `color-bg` are
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
    // NIGHT-research-5: `rain` field — the rain style label (glyph /
    // monolith / vortex / ripple). Rendered first so the user reads
    // the active motion DNA before the other scene-family fields.
    if let Some(rain) = scene.rain.as_deref() {
        out.push_str(&format!("    rain               = {rain}\n"));
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
    if let Some(colors_custom) = scene.colors_custom.as_deref() {
        out.push_str(&format!("    colors-custom      = {colors_custom}\n"));
        has_field = true;
    }
    if let Some(charset_custom) = scene.charset_custom.as_deref() {
        out.push_str(&format!("    charset-custom     = {charset_custom}\n"));
        has_field = true;
    }

    if !has_field {
        out.push_str("    (no fields set — incomplete block; see --testconf)\n");
    }
    let missing = crate::scene_custom::missing_scene_custom_fields(scene);
    if !missing.is_empty() {
        out.push_str(&format!(
            "\n  WARNING: incomplete block — missing {}\n  (a [scene-custom.<name>] block must be COMPLETELY filled)\n",
            missing.join(", ")
        ));
    }

    out.push_str("\n  Use: cosmostrix --scene-custom ");
    out.push_str(name);
    out.push('\n');
    out
}
