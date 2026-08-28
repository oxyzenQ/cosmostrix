// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene-custom parse helpers — extracted from `scene_custom/mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns 9 pure helper functions used by apply_profile_overrides +
//! apply_scene_custom_field_to_cloud_config:
//! - parse_f32_override / parse_f64_override / parse_u8_override: range-validated numeric parsing.
//! - parse_speed_override: speed-specific parser with clamp.
//! - parse_color_bg: ColorBg enum parser (black/default).
//! - parse_bool: boolean parser (true/false/1/0/yes/no).
//! - profile_name_list: comma-separated profile name list.
//! - is_explicit: clap ValueSource::CommandLine check.
//! - warn_invalid: warning printer for invalid scene-custom values.

use std::collections::BTreeMap;

use clap::parser::ValueSource;

use crate::config::ColorBg;
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

pub(crate) fn parse_color_bg(value: &str) -> Option<ColorBg> {
    match value.trim().to_ascii_lowercase().as_str() {
        "black" => Some(ColorBg::Black),
        "default-background" | "default_background" => Some(ColorBg::DefaultBackground),
        _ => None,
    }
}

/// Parse a u8 field for scene-custom/profile application.
pub(crate) fn parse_u8_override(
    name: &str,
    field: &str,
    value: &str,
    min: u8,
    max: u8,
) -> Option<u8> {
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
pub(crate) fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
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
    crate::output::eprintln_warn_labeled(&format!(
        "scene-custom: invalid {field}='{value}' in scene-custom '{profile}' (expected: {expected})"
    ));
}
