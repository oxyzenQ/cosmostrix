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
            eprintln!("testconf: hint: run `cosmostrix --config-path` to see the expected location");
            eprintln!("testconf: hint: cosmostrix --dump-config > <config-path>  (create parent dir first)");
            std::process::exit(2);
        }
    };

    let parsed = configfile::parse_config_text(&content);
    let mut errors = 0usize;
    let mut warnings = 0usize;

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

    // Check profile keys for correct format
    let profile_keys: Vec<_> = parsed
        .values
        .keys()
        .filter(|k| k.starts_with("profile."))
        .collect();
    for pk in &profile_keys {
        // profile.<name>.<field>
        let parts: Vec<&str> = pk.split('.').collect();
        if parts.len() != 3 {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: malformed profile key '{pk}' (expected profile.<name>.<field>)"
            ));
            errors += 1;
        } else {
            let field = parts[2];
            let valid_fields = [
                "base",
                "scene",
                "preset",
                "color",
                "charset",
                "fps",
                "speed",
                "density",
                "glitch-level",
                "monolith-size",
                "color-bg",
                "atmosphere-mode",
                "atmosphere-regime",
            ];
            if !valid_fields.contains(&field) {
                crate::output::eprintln_error_labeled(&format!(
                    "testconf: unknown profile field '{field}' in '{pk}'"
                ));
                eprintln!(
                    "testconf: valid profile fields: {}",
                    valid_fields.join(", ")
                );
                errors += 1;
            }
        }
    }

    // Validate known value-ranges for common keys
    for (key, value) in &parsed.values {
        if key.starts_with("profile.") {
            continue; // profile keys validated above
        }
        if let Some(msg) = validate_config_value(key, value) {
            crate::output::eprintln_warn_labeled(&format!(
                "testconf: {key} = {value}: {msg}"
            ));
            warnings += 1;
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
        crate::output::eprintln_error_labeled("testconf: FAIL — fix the errors above before running cosmostrix");
        std::process::exit(2);
    } else if warnings > 0 {
        println!("testconf: PASS (with warnings) — config is usable but review the warnings");
    } else {
        println!("testconf: PASS — config is valid");
    }
    Ok(())
}

/// Basic value-range validation for known config keys.
/// Returns Some(message) if the value looks suspicious, None if OK.
fn validate_config_value(key: &str, value: &str) -> Option<String> {
    let v = value.trim();
    match key {
        "fps" => v.parse::<f64>().ok().and_then(|n| {
            if !(1.0..=240.0).contains(&n) {
                Some(format!("out of range [1, 240], got {n}"))
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
        }),
        "density" => v.parse::<f64>().ok().and_then(|n| {
            if !(0.0..=2.0).contains(&n) {
                Some(format!("out of range [0.0, 2.0], got {n}"))
            } else {
                None
            }
        }),
        "bold" => match v {
            "0" | "1" | "2" => None,
            _ => Some(format!("expected 0, 1, or 2, got '{v}'")),
        },
        "color" => {
            let valid = theme::canonical_name_for_input(v).is_some();
            if valid {
                None
            } else {
                Some(format!(
                    "unknown color '{v}' (run `cosmostrix --list-colors` for valid names)"
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
        "monolith-size" => match v {
            "small" | "normal" | "large" => None,
            _ => Some(format!("expected small/normal/large, got '{v}'")),
        },
        "glitch-level" => match v {
            "none" | "subtle" | "default" | "intense" => None,
            _ => Some(format!("expected none/subtle/default/intense, got '{v}'")),
        },
        "low-power" | "mouse" | "fullwidth" | "auto-color-drift" => match v {
            "true" | "false" => None,
            _ => Some(format!("expected true/false, got '{v}'")),
        },
        _ => None, // unknown keys handled separately
    }
}
