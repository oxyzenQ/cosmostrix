// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config file validation (`--testconf` command).
//!
//! Reads `~/.config/cosmostrix/config` (or `--config PATH`) and reports:
//!   - Unknown keys (likely typos)
//!   - Malformed scene-custom keys
//!   - Out-of-range values for known numeric keys
//!   - Invalid enum values (color, scene, monolith-size, glitch-level)
//!
//! Exit code 0 = PASS, 2 = FAIL (errors found).

use crate::configfile;
use crate::Args;

// v50.0.0-beta.7 LOC refactor: validate_field_value +
// validate_field_value_with_cfg extracted to field_validation.rs to keep
// this file under the 800-LOC hard cap. Re-exported here so all existing
// call sites (including tests via `use super::*`) continue to resolve.
mod field_validation;
#[allow(unused_imports)]
pub(crate) use field_validation::{validate_field_value, validate_field_value_with_cfg};

/// Run the `--testconf` validation.
pub(crate) fn run(args: &Args) -> std::io::Result<()> {
    // Security (v16 audit): validate --config path BEFORE reading.
    // Previously testconf::run called std::fs::read_to_string directly
    // without is_safe_path, allowing `cosmostrix --testconf --config /etc/passwd`
    // to parse arbitrary files as TOML and leak their content via
    // malformed-line / unknown-key error messages. Now uses the same
    // validate_config_path helper as apply_config_and_runtime_defaults.
    // Also uses the resolved path for I/O (expands %APPDATA% on Windows).
    let resolved_config: Option<std::path::PathBuf>;
    if let Some(ref config_path) = args.config {
        let path_str = config_path.to_string_lossy();
        match crate::validate_config_path(&path_str, args.verbose) {
            Ok(resolved) => resolved_config = Some(std::path::PathBuf::from(&resolved)),
            Err(e) => {
                crate::output::eprintln_error_labeled(&e);
                std::process::exit(2);
            }
        }
    } else {
        resolved_config = None;
    }

    let path = if let Some(p) = resolved_config.as_ref() {
        p.clone()
    } else {
        // No explicit --config: try default user path first, then fall back
        // to system-wide config (e.g., /etc/cosmostrix/config.toml). This
        // mirrors the fallback in load_config_file_full, so --testconf works
        // after a --system install where only /etc/cosmostrix/config.toml exists.
        let default_path = configfile::default_config_file_path();
        if default_path.exists() {
            default_path
        } else {
            // Try system-wide fallback paths (same candidates as the live-reload
            // watcher uses, minus the default which we already checked).
            let candidates = configfile::config_candidate_paths();
            candidates
                .into_iter()
                .skip(1) // skip default_path (already checked)
                .find(|p| p.exists())
                .unwrap_or(default_path) // none exist → report error for default
        }
    };

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

    // v50: SHA-512 fingerprint of the config file on disk.
    // Lets the user verify exact config state with `sha512sum`,
    // detect config drift across machines, and prove config identity
    // in bug reports. Uses the same sha2 crate as --dump-config and
    // live-reload change detection (zero new dependencies).
    let file_hash = configfile::sha512_hex(content.as_bytes());
    println!("testconf: file-sha512: {file_hash}");

    // v50 (alpha.2): Extract template-fingerprint from the header (if present)
    // and compare against the current built-in template to detect drift.
    let header_fp = configfile::extract_template_fingerprint(&content);
    let current_template_hash = configfile::sha512_hex(configfile::dump_config_text().as_bytes());
    match &header_fp {
        Some(fp) => {
            println!("testconf: template-fingerprint: {fp}");
            if fp == &current_template_hash {
                println!(
                    "testconf: template drift: none (matches built-in v{} template)",
                    env!("CARGO_PKG_VERSION")
                );
            } else {
                println!("testconf: template drift: detected — header fingerprint differs from built-in template");
                println!("testconf:   built-in hash: {current_template_hash}");
                eprintln!(
                    "testconf: hint: run `cosmostrix --dump-config <path> --force` to regenerate the template"
                );
            }
        }
        None => {
            println!("testconf: template-fingerprint: (not found in header — config may be hand-written or older)");
        }
    }

    let parsed = configfile::parse_config_text(&content);
    let mut errors = 0usize;
    let mut warnings = 0usize;

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
            // depth-test fix: targeted hint for structural TOML
            // mistakes (e.g. `bold` under [color.tune], or adaptive-custom
            // nested under [scene-custom.<name>]). Generic typos get no
            // hint — they fall through to the known-keys list below.
            if let Some(hint) = crate::config_hints::suggest_for_unknown_key(key) {
                eprintln!("testconf: hint: {hint}");
            }
            errors += 1;
        }
        eprintln!(
            "testconf: known keys: {}",
            configfile::known_keys().join(", ")
        );
    }

    // Info-only notice for auto-promoted keys (forgiving parser).
    // These are NOT errors — the keys were silently re-homed to root scope.
    // We surface them so users know their TOML structure was off and can
    // optionally fix it for clarity (move the key BEFORE any [section] header).
    if !parsed.promoted_keys.is_empty() {
        println!(
            "testconf: info: {} key(s) auto-promoted to root scope (TOML mis-nesting, fixed):",
            parsed.promoted_keys.len()
        );
        for (from, to) in &parsed.promoted_keys {
            println!("testconf:   {from}  →  {to}");
        }
        println!(
            "testconf: hint: move these keys BEFORE any [section] header to silence this notice"
        );
    }

    // Check scene-custom block keys for correct format AND field value validity
    let block_keys: Vec<_> = parsed
        .values
        .keys()
        .filter(|k| k.starts_with("scene-custom."))
        .collect();
    for pk in &block_keys {
        // scene-custom.<name>.<field>
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
            // copy that drifted when fields were added or removed (the
            // density-map removal in v80.0.0-beta.2 is the latest example).
            let valid_fields: &[&str] = crate::scene_custom::PROFILE_FIELDS;
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
                // The context-aware variant emits a richer hint when the value
                // matches a custom block (e.g. `color = z` where `[colors-custom.z]`
                // exists — points the user to `colors-custom = z`).
                if let Some(msg) = validate_field_value_with_cfg(field, value, &parsed.values) {
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
    // ambient.* keys are validated as a group via
    // validate_ambient_entries (which checks scene-name validity and
    // rejects legacy multi-field format with a migration message).
    let mut ambient_validated = false;
    for (key, value) in &parsed.values {
        if key.starts_with("scene-custom.") {
            continue; // block keys validated above
        }
        // ambient.* — validate all entries as a group, once.
        // validate_ambient_entries validates ALL ambient keys at once
        // (cross-references scene-custom blocks), so we skip after the
        // first ambient key to avoid re-running the same full validation.
        if key.starts_with("ambient.") {
            if !ambient_validated {
                if let Err(msg) =
                    crate::crystal_dragon_engine::ambient::validate_ambient_entries(&parsed.values)
                {
                    crate::output::eprintln_error_labeled(&format!("testconf: {msg}"));
                    errors += 1;
                }
                ambient_validated = true;
            }
            continue;
        }
        // colors-custom.* keys: validate hex format (same as validate_config_strictly).
        // Without this, --testconf passes invalid hex that crashes at startup.
        if key.starts_with("colors-custom.") {
            // (bug #8): deprecation notice for `.stops` (alias for `rain`).
            // The value is still accepted, but users should migrate to `rain`
            // for clarity — `stops` was an undocumented alias that's now
            // explicitly deprecated.
            if key.ends_with(".stops") {
                println!(
                    "testconf: warning: '{key}' uses deprecated field 'stops' — rename to 'rain' (alias removed in a future release)"
                );
                warnings += 1;
            }
            if let Some(msg) = validate_colors_custom_value(key, value) {
                crate::output::eprintln_error_labeled(&format!("testconf: {key} = {value}: {msg}"));
                errors += 1;
            }
            continue;
        }
        // v25: charset-custom.* keys — validate content (length, control
        // chars, wide-char filter). Mirrors the strict-validation path.
        // Cosmic Dragon principle: wide-char rejection is permanent.
        if key.starts_with("charset-custom.") {
            if let Some(msg) = crate::charset_custom::validate_charset_custom_value(value) {
                crate::output::eprintln_error_labeled(&format!("testconf: {key} = {value}: {msg}"));
                errors += 1;
            }
            continue;
        }
        // v25: top-level `charset` may name a custom block — accepted by
        // validate_field_value_with_cfg's custom-reference parity layer
        // (v80.0.0-beta.2, same acceptance as `color` and `scene`). The
        // block's content was validated in the branch above.
        if let Some(msg) = validate_field_value_with_cfg(key, value, &parsed.values) {
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
        crate::output::eprintln_warn_labeled(
            "testconf: PASS (with warnings) — config is usable but review the warnings",
        );
    } else {
        // Success: print to stdout (not stderr) so scripts can capture it.
        println!("testconf: PASS — config is valid");
    }
    Ok(())
}

/// Validate ALL top-level fields in a parsed config HashMap.
///
/// Returns `Ok(())` if every top-level key has a valid value, or
/// `Err(message)` with a human-readable error for the first invalid field.
/// Block keys (scene-custom.X.field) are skipped —
/// they're validated separately by --testconf's block-field check.
///
/// Used by:
/// - Startup: `apply_config_and_runtime_defaults` rejects invalid config
///   before cosmostrix starts running (exit code 2).
/// - Live reload: watcher rejects invalid config edits (exit code 2).
/// - --testconf: validates and reports errors.
pub(crate) fn validate_config_strictly(
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    for (key, value) in cfg {
        if key.starts_with("scene-custom.") {
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
        // v25: charset-custom.<name>.set keys — validate the charset
        // content (length cap, control char rejection, wide-char filter).
        // Same idea as colors-custom: is_known_key() already verified the
        // key pattern, we only need to validate the value.
        // Cosmic Dragon principle: wide-char rejection is permanent.
        if key.starts_with("charset-custom.") {
            if let Some(msg) = crate::charset_custom::validate_charset_custom_value(value) {
                return Err(format!("invalid value for '{key}': {msg}"));
            }
            continue;
        }
        // Ambient phase scheduler: `ambient.<HH-MM>` keys. The key pattern
        // (HH-MM format) is validated by is_known_key(); here we validate
        // the value (positional color/scene + key=value pairs).
        if key.starts_with("ambient.") {
            // validate_ambient_entries validates ALL ambient keys at once
            // (cross-references colors-custom and charset-custom names),
            // so we break after the first ambient key to avoid re-running
            // the same full validation.
            crate::crystal_dragon_engine::ambient::validate_ambient_entries(cfg)?;
            break;
        }
        // v25: the top-level `charset` key may reference a custom charset
        // block (charset-custom.<name>) instead of a built-in preset —
        // accepted by validate_field_value_with_cfg's custom-reference
        // parity layer (v80.0.0-beta.2), which applies the same acceptance
        // to `color` and `scene`. The block's content was already validated
        // above.
        if let Some(msg) = validate_field_value_with_cfg(key, value, cfg) {
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

    // stops/rain field: hex list (array or CSV format).
    if key.ends_with(".stops") || key.ends_with(".rain") {
        // v25: handle TOML array format. If value starts with '[', strip
        // the brackets before splitting by comma. This matches the
        // parse_rain_array logic in colors_custom.rs.
        let inner = if trimmed.starts_with('[') {
            let s = trimmed.strip_prefix('[').unwrap_or(trimmed);
            let s = s.strip_suffix(']').unwrap_or(s);
            s
        } else {
            trimmed
        };
        for stop in inner.split(',') {
            let s = stop.trim().trim_matches('"').trim();
            // Skip empty stops (trailing comma after ] strip).
            if s.is_empty() {
                continue;
            }
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

#[cfg(test)]
mod tests;
