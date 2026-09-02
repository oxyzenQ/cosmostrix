// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
    // v80.0.0-beta.1 killer-features hardening: routed through warn_runtime_or_now.
    // Today warn_invalid fires only on startup paths (pre-alt-screen, prints
    // immediately as before), but the scene-custom field appliers it now
    // serves also run during live reload — if a warning ever fires there it
    // must buffer (AB-10) instead of leaking into the rain matrix.
    crate::output::warn_runtime_or_now(&format!(
        "scene-custom: invalid {field}='{value}' in scene-custom '{profile}' (expected: {expected})"
    ));
}
