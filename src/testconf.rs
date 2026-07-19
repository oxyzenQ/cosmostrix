// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config file validation (`--testconf` command).
//!
//! Reads `~/.config/cosmostrix/config` (or `--config PATH`) and reports:
//!   - Unknown keys (likely typos)
//!   - Malformed profile keys
//!   - Out-of-range values for known numeric keys
//!   - Invalid enum values (color, scene, monolith-size, glitch-level)
//!
//! Exit code 0 = PASS, 2 = FAIL (errors found).

use crate::configfile;
use crate::theme;
use crate::Args;

/// Run the `--testconf` validation.
pub fn run(args: &Args) -> std::io::Result<()> {
    let path = args
        .config
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(configfile::default_config_file_path);

    println!("testconf: checking {}", path.display());

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: cannot read config file: {e}"
            ));
            eprintln!(
                "testconf: hint: run `cosmostrix --config-path` to see the expected location"
            );
            eprintln!("testconf: hint: cosmostrix --dump-config <config-path>  (writes directly, whitelist-enforced)");
            std::process::exit(2);
        }
    };

    let parsed = configfile::parse_config_text(&content);
    let mut errors = 0usize;
    let warnings = 0usize;

    // Check for malformed lines (non-empty, non-comment lines without 'key = value')
    if !parsed.malformed_lines.is_empty() {
        for line in &parsed.malformed_lines {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: malformed line '{line}' (expected 'key = value' syntax)"
            ));
            errors += 1;
        }
        eprintln!(
            "testconf: hint: comment lines start with '#', blank lines are ignored, all other lines must be 'key = value'"
        );
    }

    // Check for unknown keys (likely typos)
    if !parsed.unknown_keys.is_empty() {
        for key in &parsed.unknown_keys {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: unknown key '{key}' (likely typo)"
            ));
            errors += 1;
        }
        eprintln!(
            "testconf: known keys: {}",
            configfile::known_keys().join(", ")
        );
    }

    // Check profile keys for correct format AND field value validity
    let profile_keys: Vec<_> = parsed
        .values
        .keys()
        .filter(|k| k.starts_with("profile.") || k.starts_with("scene-custom."))
        .collect();
    for pk in &profile_keys {
        // profile.<name>.<field> or scene-custom.<name>.<field>
        let parts: Vec<&str> = pk.split('.').collect();
        if parts.len() != 3 {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: malformed block key '{pk}' (expected <namespace>.<name>.<field>)"
            ));
            errors += 1;
        } else {
            let field = parts[2];
            let value = parsed.values.get(*pk).map(String::as_str).unwrap_or("");
            // Use the canonical PROFILE_FIELDS list so testconf never drifts
            // from the actual config parser. Previously this was a hardcoded
            // copy that missed 'density-map' when it was added to PROFILE_FIELDS.
            let valid_fields: &[&str] = crate::profile::PROFILE_FIELDS;
            if !valid_fields.contains(&field) {
                crate::output::eprintln_error_labeled(&format!(
                    "testconf: unknown block field '{field}' in '{pk}'"
                ));
                eprintln!("testconf: valid block fields: {}", valid_fields.join(", "));
                errors += 1;
            } else {
                // Field is recognized — now validate the VALUE using the same
                // rules as top-level keys. Block fields accept the same value
                // vocabulary (color, charset, scene, atmosphere-regime, etc.).
                // 'base' and 'scene' are both scene names; 'preset' is treated
                // as a scene name too (v14 deprecated alias).
                let effective_field = match field {
                    "base" | "preset" => "scene",
                    other => other,
                };
                if let Some(msg) = validate_field_value(effective_field, value) {
                    crate::output::eprintln_error_labeled(&format!(
                        "testconf: {pk} = {value}: {msg}"
                    ));
                    errors += 1;
                }
            }
        }
    }

    // Validate known value-ranges for top-level (non-block) keys.
    // v14: invalid values are now ERRORS, not warnings — silent PASS for
    // bad values is a bug. Owner requirement: strict value validation.
    for (key, value) in &parsed.values {
        if key.starts_with("profile.") || key.starts_with("scene-custom.") {
            continue; // block keys validated above
        }
        // adaptive-custom.HH-MM keys: validate via parse_custom_time_map.
        if key.starts_with("adaptive-custom.") {
            let mut single = std::collections::HashMap::new();
            single.insert(key.clone(), value.clone());
            if let Err(e) = crate::atmosphere_custom::parse_custom_time_map(&single) {
                crate::output::eprintln_error_labeled(&format!("testconf: {key} = {value}: {e}"));
                errors += 1;
            }
            continue;
        }
        if let Some(msg) = validate_field_value(key, value) {
            crate::output::eprintln_error_labeled(&format!("testconf: {key} = {value}: {msg}"));
            errors += 1;
        }
    }

    // Summary (to stdout — machine-parseable)
    println!();
    println!(
        "testconf: {} keys parsed, {} errors, {} warnings",
        parsed.values.len(),
        errors,
        warnings
    );
    if errors > 0 {
        crate::output::eprintln_error_labeled(
            "testconf: FAIL — fix the errors above before running cosmostrix",
        );
        std::process::exit(2);
    } else if warnings > 0 {
        println!("testconf: PASS (with warnings) — config is usable but review the warnings");
    } else {
        println!("testconf: PASS — config is valid");
    }
    Ok(())
}

/// Validate ALL top-level fields in a parsed config HashMap.
///
/// Returns `Ok(())` if every top-level key has a valid value, or
/// `Err(message)` with a human-readable error for the first invalid field.
/// Block keys (profile.X.field, scene-custom.X.field) are skipped —
/// they're validated separately by --testconf's block-field check.
///
/// Used by:
/// - Startup: `apply_config_and_runtime_defaults` rejects invalid config
///   before cosmostrix starts running (exit code 2).
/// - Live reload: watcher rejects invalid config edits (exit code 2).
/// - --testconf: validates and reports errors.
pub fn validate_config_strictly(
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    for (key, value) in cfg {
        if key.starts_with("profile.") || key.starts_with("scene-custom.") {
            continue;
        }
        // adaptive-custom.HH-MM keys: validate via parse_custom_time_map.
        // This catches invalid color/scene names, invalid parameters, etc.
        if key.starts_with("adaptive-custom.") {
            let mut single = std::collections::HashMap::new();
            single.insert(key.clone(), value.clone());
            crate::atmosphere_custom::parse_custom_time_map(&single)?;
            continue;
        }
        // colors-custom.<name>.<field> keys: validate hex format.
        // The key pattern is already validated by is_known_key() in
        // configfile.rs, so we only need to check the value is valid hex.
        if key.starts_with("colors-custom.") {
            if let Some(msg) = validate_colors_custom_value(key, value) {
                return Err(format!("invalid value '{value}' for '{key}': {msg}"));
            }
            continue;
        }
        if let Some(msg) = validate_field_value(key, value) {
            return Err(format!("invalid value '{value}' for '{key}': {msg}"));
        }
    }
    Ok(())
}

/// Validate a colors-custom value (hex color or comma-separated hex stops).
///
/// Accepted formats:
/// - `#rrggbb` (standard hex with #)
/// - `rrggbb` (hex without #)
/// - `#rgb` (short hex with #)
/// - `rgb` (short hex without #)
/// - `"#rrggbb"` (quoted — quotes stripped before parsing)
///
/// For `stops` field: comma-separated list of the above.
fn validate_colors_custom_value(key: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some("empty color value".to_string());
    }

    // stops field: comma-separated hex list
    if key.ends_with(".stops") {
        for stop in trimmed.split(',') {
            // Strip quotes from each stop individually (the config parser
            // preserves quotes in values since v16 step 2).
            let s = stop.trim().trim_matches('"').trim();
            if !is_valid_hex_color(s) {
                return Some(format!(
                    "invalid hex color '{s}' in stops (expected #rrggbb or rrggbb)"
                ));
            }
        }
        return None;
    }

    // single color field — strip quotes before validation
    let unquoted = trimmed.trim_matches('"').trim();
    if !is_valid_hex_color(unquoted) {
        return Some(format!(
            "invalid hex color '{unquoted}' (expected #rrggbb or rrggbb)"
        ));
    }
    None
}

/// Check if a string is a valid hex color (#rrggbb, rrggbb, #rgb, or rgb).
fn is_valid_hex_color(s: &str) -> bool {
    let s = s.strip_prefix('#').unwrap_or(s);
    (s.len() == 6 || s.len() == 3) && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Strict value validation for a config key (top-level or block field).
///
/// Returns `Some(message)` if the value is invalid for the given key,
/// `None` if it is acceptable. The message includes the list of valid
/// values (or range) so the user can fix the typo without consulting docs.
///
/// Used for both top-level keys and `profile.<name>.<field>` /
/// `scene-custom.<name>.<field>` block values. The caller is responsible
/// for mapping block-specific field names (e.g. `base` -> `scene`) before
/// calling this function.
pub fn validate_field_value(key: &str, value: &str) -> Option<String> {
    let v = value.trim();
    match key {
        // ── Numeric ranges ──
        "fps" => v.parse::<f64>().ok().and_then(|n| {
            if !(1.0..=240.0).contains(&n) {
                Some(format!("out of range [1, 240], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            // Non-numeric fps is also an error.
            if v.parse::<f64>().is_err() {
                Some(format!("expected number in [1, 240], got '{v}'"))
            } else {
                None
            }
        }),
        "speed" => v.parse::<i64>().ok().and_then(|n| {
            if !(1..=100).contains(&n) {
                Some(format!("out of range [1, 100], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<i64>().is_err() {
                Some(format!("expected integer in [1, 100], got '{v}'"))
            } else {
                None
            }
        }),
        "density" => v.parse::<f64>().ok().and_then(|n| {
            if !(0.01..=5.0).contains(&n) {
                Some(format!("out of range [0.01, 5.0], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<f64>().is_err() {
                Some(format!("expected number in [0.01, 5.0], got '{v}'"))
            } else {
                None
            }
        }),
        "glitchpct" => v.parse::<f64>().ok().and_then(|n| {
            if !(0.0..=100.0).contains(&n) {
                Some(format!("out of range [0.0, 100.0], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<f64>().is_err() {
                Some(format!("expected number in [0.0, 100.0], got '{v}'"))
            } else {
                None
            }
        }),
        "shortpct" => v.parse::<f64>().ok().and_then(|n| {
            if !(0.0..=100.0).contains(&n) {
                Some(format!("out of range [0.0, 100.0], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<f64>().is_err() {
                Some(format!("expected number in [0.0, 100.0], got '{v}'"))
            } else {
                None
            }
        }),
        "rippct" => v.parse::<f64>().ok().and_then(|n| {
            if !(0.0..=100.0).contains(&n) {
                Some(format!("out of range [0.0, 100.0], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<f64>().is_err() {
                Some(format!("expected number in [0.0, 100.0], got '{v}'"))
            } else {
                None
            }
        }),
        "maxdpc" => v.parse::<i64>().ok().and_then(|n| {
            if !(1..=3).contains(&n) {
                Some(format!("out of range [1, 3], got {n}"))
            } else {
                None
            }
        }).or_else(|| {
            if v.parse::<i64>().is_err() {
                Some(format!("expected integer in [1, 3], got '{v}'"))
            } else {
                None
            }
        }),
        "bold" => match v {
            "0" | "1" | "2" => None,
            _ => Some(format!("expected 0, 1, or 2, got '{v}'")),
        },
        "shadingmode" => match v {
            "0" | "1" => None,
            _ => Some(format!("expected 0 or 1, got '{v}'")),
        },

        // ── Enum-like string values ──
        "color" => {
            if theme::canonical_name_for_input(v).is_some() {
                None
            } else {
                Some(format!(
                    "unknown color '{v}' (run `cosmostrix --list-colors` for valid names)"
                ))
            }
        }
        "charset" => {
            // Reuse the production charset parser. false = don't auto-pick
            // ASCII on unknown; we want the parse error.
            if crate::charset::charset_from_str(v, false).is_ok() {
                None
            } else {
                Some(format!(
                    "unknown charset '{v}' (run `cosmostrix --list-charsets` for valid names)"
                ))
            }
        }
        "scene" => {
            if crate::scene::get_scene(v).is_some() {
                None
            } else {
                Some(format!(
                    "unknown scene '{v}' (run `cosmostrix --list-scenes` for valid names)"
                ))
            }
        }
        "atmosphere-regime" => match v {
            "calm" | "pulse" | "signal" | "compression" | "void" | "monolith-pressure"
            | "adaptive" => None,
            "storm" => Some(
                "storm is unavailable and will be rejected".to_string(),
            ),
            _ => Some(format!(
                "unknown regime '{v}'. Available: calm, pulse, signal, compression, void, monolith-pressure, adaptive"
            )),
        },
        "atmosphere-mode" => match v {
            "disabled" | "controlled-live" => None,
            _ => Some(format!(
                "unknown mode '{v}'. Available: disabled, controlled-live"
            )),
        },
        "monolith-size" => match v {
            "small" | "normal" | "large" => None,
            _ => Some(format!("expected small/normal/large, got '{v}'")),
        },
        "glitch-level" => match v {
            "none" | "subtle" | "default" | "intense" => None,
            _ => Some(format!("expected none/subtle/default/intense, got '{v}'")),
        },
        "color-bg" => match v {
            "black" | "default-background" | "default_background" => None,
            _ => Some(format!(
                "expected black/default-background, got '{v}'"
            )),
        },
        "low-power" | "mouse" | "fullwidth" | "auto-color-drift" | "async-mode" => match v {
            "true" | "false" => None,
            _ => Some(format!("expected true/false, got '{v}'")),
        },

        // Keys we don't have a specific validator for — assume OK.
        // Unknown keys are caught earlier by the unknown_keys check.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Bug regression: atmosphere-regime = adaptivee must error ──

    #[test]
    fn atmosphere_regime_typo_is_rejected() {
        let msg = validate_field_value("atmosphere-regime", "adaptivee");
        assert!(
            msg.is_some(),
            "'adaptivee' (typo) must be rejected for atmosphere-regime"
        );
        let msg = msg.expect("checked Some above");
        assert!(
            msg.contains("unknown regime"),
            "error must say 'unknown regime': {msg}"
        );
        assert!(
            msg.contains("adaptive"),
            "error must list 'adaptive' as valid: {msg}"
        );
    }

    #[test]
    fn atmosphere_regime_valid_values_pass() {
        for v in [
            "calm",
            "pulse",
            "signal",
            "compression",
            "void",
            "monolith-pressure",
            "adaptive",
        ] {
            assert!(
                validate_field_value("atmosphere-regime", v).is_none(),
                "'{v}' should be a valid atmosphere-regime"
            );
        }
    }

    #[test]
    fn atmosphere_regime_storm_is_rejected() {
        let msg = validate_field_value("atmosphere-regime", "storm");
        assert!(
            msg.is_some(),
            "'storm' must be rejected for atmosphere-regime"
        );
    }

    // ── Bug regression: charset = hackeres must error ──

    #[test]
    fn charset_typo_is_rejected() {
        let msg = validate_field_value("charset", "hackeres");
        assert!(
            msg.is_some(),
            "'hackeres' (typo) must be rejected for charset"
        );
        let msg = msg.expect("checked Some above");
        assert!(
            msg.contains("unknown charset"),
            "error must say 'unknown charset': {msg}"
        );
        assert!(
            msg.contains("--list-charsets"),
            "error must point to --list-charsets: {msg}"
        );
    }

    #[test]
    fn charset_valid_values_pass() {
        for v in ["binary", "matrix", "katakana", "hacker", "minimal", "retro"] {
            assert!(
                validate_field_value("charset", v).is_none(),
                "'{v}' should be a valid charset"
            );
        }
    }

    // ── Numeric range validation ──

    #[test]
    fn fps_out_of_range_is_rejected() {
        assert!(validate_field_value("fps", "0").is_some());
        assert!(validate_field_value("fps", "241").is_some());
        assert!(validate_field_value("fps", "60").is_none());
    }

    #[test]
    fn fps_non_numeric_is_rejected() {
        let msg = validate_field_value("fps", "fast");
        assert!(msg.is_some(), "'fast' must be rejected for fps");
    }

    #[test]
    fn speed_out_of_range_is_rejected() {
        assert!(validate_field_value("speed", "0").is_some());
        assert!(validate_field_value("speed", "101").is_some());
        assert!(validate_field_value("speed", "30").is_none());
    }

    #[test]
    fn density_out_of_range_is_rejected() {
        assert!(validate_field_value("density", "0.001").is_some());
        assert!(validate_field_value("density", "5.5").is_some());
        assert!(validate_field_value("density", "0.85").is_none());
    }

    // ── Enum value validation ──

    #[test]
    fn color_unknown_is_rejected() {
        let msg = validate_field_value("color", "not-a-color");
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("unknown color"));
    }

    #[test]
    fn scene_unknown_is_rejected() {
        let msg = validate_field_value("scene", "nonexistent");
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("unknown scene"));
    }

    #[test]
    fn monolith_size_invalid_is_rejected() {
        assert!(validate_field_value("monolith-size", "huge").is_some());
        assert!(validate_field_value("monolith-size", "normal").is_none());
    }

    #[test]
    fn glitch_level_invalid_is_rejected() {
        assert!(validate_field_value("glitch-level", "extreme").is_some());
        assert!(validate_field_value("glitch-level", "subtle").is_none());
    }

    #[test]
    fn color_bg_invalid_is_rejected() {
        assert!(validate_field_value("color-bg", "white").is_some());
        assert!(validate_field_value("color-bg", "black").is_none());
        assert!(validate_field_value("color-bg", "default-background").is_none());
    }

    #[test]
    fn atmosphere_mode_invalid_is_rejected() {
        assert!(validate_field_value("atmosphere-mode", "enabled").is_some());
        assert!(validate_field_value("atmosphere-mode", "disabled").is_none());
        assert!(validate_field_value("atmosphere-mode", "controlled-live").is_none());
    }

    #[test]
    fn boolean_keys_reject_non_bool() {
        assert!(validate_field_value("mouse", "yes").is_some());
        assert!(validate_field_value("fullwidth", "1").is_some());
        assert!(validate_field_value("mouse", "true").is_none());
        assert!(validate_field_value("fullwidth", "false").is_none());
    }

    #[test]
    fn block_field_base_uses_scene_validator() {
        // 'base' in profile/scene-custom blocks is validated as a scene name.
        // The caller maps 'base' -> 'scene' before calling validate_field_value.
        assert!(validate_field_value("scene", "nonexistent").is_some());
        assert!(validate_field_value("scene", "monolith").is_none());
    }

    #[test]
    fn unknown_key_returns_none() {
        // Unknown keys are caught by the unknown_keys check, not here.
        assert!(validate_field_value("unknown-key", "anything").is_none());
    }

    // ── v16: colors-custom hex validation ──

    #[test]
    fn hex_color_valid_full_with_hash() {
        assert!(is_valid_hex_color("#ff0000"));
        assert!(is_valid_hex_color("#00ff88"));
        assert!(is_valid_hex_color("#abcdef"));
    }

    #[test]
    fn hex_color_valid_full_without_hash() {
        assert!(is_valid_hex_color("ff0000"));
        assert!(is_valid_hex_color("00ff88"));
    }

    #[test]
    fn hex_color_valid_short_with_hash() {
        assert!(is_valid_hex_color("#f00"));
        assert!(is_valid_hex_color("#abc"));
    }

    #[test]
    fn hex_color_valid_short_without_hash() {
        assert!(is_valid_hex_color("f00"));
        assert!(is_valid_hex_color("abc"));
    }

    #[test]
    fn hex_color_invalid_non_hex_chars() {
        assert!(!is_valid_hex_color("#gg0000"));
        assert!(!is_valid_hex_color("#xyz123"));
        assert!(!is_valid_hex_color("hello!"));
    }

    #[test]
    fn hex_color_invalid_wrong_length() {
        assert!(!is_valid_hex_color("#ff00"));
        assert!(!is_valid_hex_color("#ff000000"));
        assert!(!is_valid_hex_color(""));
    }

    #[test]
    fn colors_custom_value_validates_single_hex() {
        assert!(
            validate_colors_custom_value("colors-custom.mytheme.normal.red", "#ff0000").is_none()
        );
        assert!(
            validate_colors_custom_value("colors-custom.mytheme.normal.red", "\"#ff0000\"")
                .is_none()
        );
    }

    #[test]
    fn colors_custom_value_rejects_invalid_hex() {
        assert!(
            validate_colors_custom_value("colors-custom.mytheme.normal.red", "#gg0000").is_some()
        );
        assert!(
            validate_colors_custom_value("colors-custom.mytheme.normal.red", "notacolor").is_some()
        );
    }

    #[test]
    fn colors_custom_stops_validates_each() {
        assert!(validate_colors_custom_value(
            "colors-custom.mytheme.stops",
            "\"#1a0033\", \"#4d0080\", \"#9933ff\""
        )
        .is_none());
    }

    #[test]
    fn colors_custom_stops_rejects_one_bad() {
        assert!(validate_colors_custom_value(
            "colors-custom.mytheme.stops",
            "\"#1a0033\", \"#gg0080\", \"#9933ff\""
        )
        .is_some());
    }

    #[test]
    fn colors_custom_stops_rejects_empty() {
        assert!(validate_colors_custom_value("colors-custom.mytheme.stops", "").is_some());
    }
}
