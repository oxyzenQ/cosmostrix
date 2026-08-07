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

    let path = resolved_config.unwrap_or_else(configfile::default_config_file_path);

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
            // v25.6 depth-test fix: targeted hint for structural TOML
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

    // v25.7: Info-only notice for auto-promoted keys (forgiving parser).
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
    // v30.2: ambient.* keys are validated as a group via
    // validate_ambient_entries (which checks scene-name validity and
    // rejects legacy multi-field format with a migration message).
    let mut ambient_validated = false;
    for (key, value) in &parsed.values {
        if key.starts_with("profile.") || key.starts_with("scene-custom.") {
            continue; // block keys validated above
        }
        // ambient.* — validate all entries as a group, once.
        // validate_ambient_entries validates ALL ambient keys at once
        // (cross-references scene-custom blocks), so we skip after the
        // first ambient key to avoid re-running the same full validation.
        if key.starts_with("ambient.") {
            if !ambient_validated {
                if let Err(msg) = crate::ambient::validate_ambient_entries(&parsed.values) {
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
            // v25.10 (bug #8): deprecation notice for `.stops` (alias for `rain`).
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
        // v25: top-level `charset` may name a custom block. Accept if the
        // block exists in this same config — the block's content was
        // validated in the branch above.
        if key == "charset" {
            let normalized = value.trim().to_ascii_lowercase();
            let custom_key = format!("charset-custom.{normalized}.set");
            if parsed.values.contains_key(&custom_key) {
                continue;
            }
        }
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
pub(crate) fn validate_config_strictly(
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    for (key, value) in cfg {
        if key.starts_with("profile.") || key.starts_with("scene-custom.") {
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
            crate::ambient::validate_ambient_entries(cfg)?;
            break;
        }
        // v25: the top-level `charset` key may reference a custom charset
        // block (charset-custom.<name>) instead of a built-in preset.
        // Accept the value if it matches a defined custom block — the
        // block's content was already validated above.
        if key == "charset" {
            let normalized = value.trim().to_ascii_lowercase();
            let custom_key = format!("charset-custom.{normalized}.set");
            if cfg.contains_key(&custom_key) {
                continue;
            }
        }
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
pub(crate) fn validate_field_value(key: &str, value: &str) -> Option<String> {
    let v = value.trim();
    match key {
        // ── Numeric ranges ──
        "fps" => v
            .parse::<f64>()
            .ok()
            .and_then(|n| {
                if !(1.0..=240.0).contains(&n) {
                    Some(format!("out of range [1, 240], got {n}"))
                } else {
                    None
                }
            })
            .or_else(|| {
                // Non-numeric fps is also an error.
                if v.parse::<f64>().is_err() {
                    Some(format!("expected number in [1, 240], got '{v}'"))
                } else {
                    None
                }
            }),
        "speed" => v
            .parse::<i64>()
            .ok()
            .and_then(|n| {
                if !(1..=100).contains(&n) {
                    Some(format!("out of range [1, 100], got {n}"))
                } else {
                    None
                }
            })
            .or_else(|| {
                if v.parse::<i64>().is_err() {
                    Some(format!("expected integer in [1, 100], got '{v}'"))
                } else {
                    None
                }
            }),
        "density" => v
            .parse::<f64>()
            .ok()
            .and_then(|n| {
                if !(0.01..=5.0).contains(&n) {
                    Some(format!("out of range [0.01, 5.0], got {n}"))
                } else {
                    None
                }
            })
            .or_else(|| {
                if v.parse::<f64>().is_err() {
                    Some(format!("expected number in [0.01, 5.0], got '{v}'"))
                } else {
                    None
                }
            }),
        // v25.8 (bug #6): color.tune.* fields must be in [0.0, 3.0].
        // Previously these were silently accepted by --testconf and silently
        // defaulted to 1.0 at runtime (see color_tune_from_config's filter).
        // Now they fail loudly, matching the v14 strictness for fps/speed/density.
        "color.tune.brightness"
        | "color.tune.saturation"
        | "color.tune.head"
        | "color.tune.body"
        | "color.tune.tail" => {
            if v.trim().is_empty() {
                return Some(format!("expected number in [0.0, 3.0], got '{v}'"));
            }
            v.parse::<f64>()
                .ok()
                .and_then(|n| {
                    if !(0.0..=3.0).contains(&n) {
                        Some(format!("out of range [0.0, 3.0], got {n}"))
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if v.parse::<f64>().is_err() {
                        Some(format!("expected number in [0.0, 3.0], got '{v}'"))
                    } else {
                        None
                    }
                })
        }
        // v17 mastery: legacy advanced keys (glitchpct, shortpct, rippct,
        // maxdpc) REMOVED from --testconf validation. These are now fully
        // controlled by --glitch-level. If present in config.toml, they are
        // silently ignored (not validated, not flagged as unknown).
        "bold" => match v {
            "0" | "1" | "2" => None,
            _ => Some(format!("expected 0, 1, or 2, got '{v}'")),
        },
        "shadingmode" => match v {
            "0" | "1" => None,
            _ => Some(format!("expected 0 or 1, got '{v}'")),
        },
        // v30 simplify: validate density-map CSV at --testconf time.
        // Previously, `density-map = "abc,def"` passed --testconf cleanly
        // but silently became None at runtime (parse_density_map filters
        // non-numeric entries). Now --testconf fails loudly so users see
        // the typo before running. Format: comma-separated floats in
        // [0.0, 1.0], at least one non-empty entry.
        //
        // v30 fix: also strip a single pair of surrounding `"` or `'`
        // before splitting. The configfile parser does NOT strip quotes
        // from string values, so `density-map = "0.05,0.3,1.0"` would
        // otherwise split into `"0.05`, `0.3`, `1.0"` and the first/last
        // entries would fail the f64 parse with a confusing error message.
        // This mirrors the same quote-stripping logic in parse_density_map.
        "density-map" => {
            let v = v.trim().trim_matches('"').trim_matches('\'').trim();
            let entries: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
            let non_empty: Vec<&str> = entries.iter().copied().filter(|s| !s.is_empty()).collect();
            if non_empty.is_empty() {
                return Some(format!(
                    "expected at least one comma-separated float in [0.0, 1.0], got '{v}'"
                ));
            }
            for entry in &non_empty {
                match entry.parse::<f64>() {
                    Ok(n) if !(0.0..=1.0).contains(&n) => {
                        return Some(format!(
                            "out of range [0.0, 1.0] for entry '{entry}', got {n}"
                        ));
                    }
                    Err(_) => {
                        return Some(format!(
                            "expected float in [0.0, 1.0] for entry '{entry}', got '{entry}'"
                        ));
                    }
                    Ok(_) => {}
                }
            }
            None
        }

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
        "atmosphere-regime" | "atmosphere-mode" => Some(
            "atmosphere-regime and atmosphere-mode config keys have been removed — \
             the atmosphere engine subsystem was eliminated. Remove these keys \
             from your config.toml."
                .to_string(),
        ),
        "monolith-size" => {
            // Phase 5 closure (P1-#4 + P2-6): case-insensitive to match CLI
            // clap ValueEnum. Previously strict-lowercase only, which created
            // an asymmetry where `--monolith-size Large` worked on CLI but
            // `monolith-size = "Large"` was rejected by --testconf. Now all
            // 3 enum paths (CLI, testconf, runtime) agree.
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "small" | "normal" | "large" => None,
                _ => Some(format!("expected small/normal/large, got '{v}'")),
            }
        }
        "glitch-level" => {
            // Phase 5 closure (P1-#4 + P2-6): case-insensitive to match CLI.
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "none" | "subtle" | "default" | "intense" => None,
                _ => Some(format!("expected none/subtle/default/intense, got '{v}'")),
            }
        }
        "color-bg" => {
            // Phase 5 closure (P2-6): case-insensitive to match CLI.
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "black" | "default-background" | "default_background" => None,
                _ => Some(format!("expected black/default-background, got '{v}'")),
            }
        }
        // Phase D Bug #1 fix: accept the same lenient set as parse_bool_config
        // (true/yes/on/1/false/no/off/0, case-insensitive). Previously
        // testconf only accepted lowercase "true"/"false" — stricter than
        // the runtime parser, so a config with `auto-color-drift = yes`
        // would FAIL --testconf but WORK at runtime. Now all 3 paths agree.
        "low-power" | "mouse" | "auto-color-drift" | "async-mode" => {
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "true" | "yes" | "on" | "1" | "false" | "no" | "off" | "0" => None,
                _ => Some(format!(
                    "expected true/false (or yes/no, on/off, 1/0), got '{v}'"
                )),
            }
        }
        // v25.14 (bug #17): intro selector — must match the clap ValueEnum
        // accepted by `--intro`. Previously this fell through to the
        // catch-all `_ => None` arm, so `intro = "blah"` passed strict
        // validation silently and only got caught at runtime by
        // `IntroType::from_str` in config_apply.rs (which prints an error
        // to stderr but does NOT reject the config). Now strict validation
        // catches it the same way as `monolith-size` / `glitch-level` /
        // `color-bg` — startup, --testconf, and live-reload all reject
        // uniformly.
        "intro" => {
            // Phase 5 closure (P1-#4 + P2-6): case-insensitive to match CLI
            // clap ValueEnum (which is case-insensitive by default). Previously
            // strict-lowercase only — `--intro Logo` worked on CLI but
            // `intro = "Logo"` was rejected by --testconf. Now all 3 paths agree.
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "cosmic" | "logo" | "none" => None,
                _ => Some(format!(
                    "expected cosmic/logo/none, got '{v}' (run `cosmostrix --help` for valid intro types)"
                )),
            }
        }
        // v25.14 (bug #17) + v30 (2026-08-05 atmosphere elimination): the
        // bare `adaptive-custom` key is rejected with a clear migration
        // message. The entire atmosphere engine subsystem was eliminated at
        // commit 07b44b5 — both the bare key (caught here) and the
        // `adaptive-custom.HH-MM` form (caught as unknown key by is_known_key)
        // are rejected. Users should remove these keys from config.toml.
        // Historical design spec: docs/archive/specs/ATMOSPHERE_ENGINE.md.
        // Elimination record: docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md.
        "adaptive-custom" => Some(
            "adaptive-custom.* keys have been removed — the atmosphere engine \
             subsystem was eliminated. Remove these keys from your config.toml."
                .to_string(),
        ),

        // Keys we don't have a specific validator for — assume OK.
        // Unknown keys are caught earlier by the unknown_keys check.
        _ => None,
    }
}

/// Context-aware wrapper around [`validate_field_value`].
///
/// Accepts the parsed config map so it can emit targeted hints when a value
/// that failed base validation happens to match a custom block defined
/// elsewhere in the same config. This closes the duplicate-usage confusion
/// between paired fields:
///   - `color` (built-in names only) vs `colors-custom` (references a
///     `[colors-custom.<name>]` block).
///   - `charset` (built-in presets only) vs `charset-custom` (references a
///     `[charset-custom.<name>]` block).
///
/// When `color = <name>` fails because `<name>` is not a built-in color, but
/// the config DOES define a `[colors-custom.<name>]` block, the returned
/// error message points the user to `colors-custom = <name>` instead of just
/// saying "unknown color". Symmetric hint for `charset` → `charset-custom`.
/// The hint is only emitted when a matching custom block exists — otherwise
/// the plain base error is returned unchanged.
///
/// Callers that have the parsed config map available should prefer this over
/// the bare `validate_field_value`. The base function remains available for
/// contexts (e.g. unit tests, CLI arg parsing) where no surrounding config
/// exists.
///
/// Note on the top-level `charset` carve-out: the `run()` and
/// `validate_config_strictly()` callers each have a pre-check that silently
/// accepts `charset = <custom-name>` at the TOP LEVEL (legacy v25 behavior
/// that predates the explicit `charset-custom` field). That pre-check runs
/// BEFORE this wrapper, so the charset hint here only fires for the
/// scene-custom block path — which is exactly the v30.3-design-consistent
/// behavior: inside `[scene-custom.<name>]`, the explicit `charset-custom`
/// field is the canonical way to reference a custom block.
pub(crate) fn validate_field_value_with_cfg(
    key: &str,
    value: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let base = validate_field_value(key, value)?;
    // Base validation FAILED — `base` holds the plain error message. Try to
    // enrich it with a context-aware hint before returning.
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(base);
    }
    let lower = trimmed.to_ascii_lowercase();
    if key == "color" {
        // A `[colors-custom.<name>]` block is recognized by any of its
        // declared sub-fields. We check all three historical spellings
        // (.bg, .rain, .stops) so a partially-migrated config still trips
        // the hint.
        let bg_key = format!("colors-custom.{lower}.bg");
        let rain_key = format!("colors-custom.{lower}.rain");
        let stops_key = format!("colors-custom.{lower}.stops");
        if cfg.contains_key(&bg_key) || cfg.contains_key(&rain_key) || cfg.contains_key(&stops_key)
        {
            return Some(format!(
                "unknown color '{value}' — '{value}' is a custom palette name. \
                 Use `colors-custom = {value}` instead (the `color` field only \
                 accepts built-in names; run `cosmostrix --list-colors` to see them)."
            ));
        }
    }
    if key == "charset" {
        // A `[charset-custom.<name>]` block is recognized by its `.set`
        // sub-field. If the user wrote `charset = <name>` inside a
        // scene-custom block (where the top-level carve-out does NOT apply),
        // point them at the explicit `charset-custom` field.
        let set_key = format!("charset-custom.{lower}.set");
        if cfg.contains_key(&set_key) {
            return Some(format!(
                "unknown charset '{value}' — '{value}' is a custom charset name. \
                 Use `charset-custom = {value}` instead (the `charset` field only \
                 accepts built-in names; run `cosmostrix --list-charsets` to see them)."
            ));
        }
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for v in [
            "binary", "matrix", "katakana", "hacker", "minimal", "retro", "zen",
        ] {
            assert!(
                validate_field_value("charset", v).is_none(),
                "'{v}' should be a valid charset"
            );
        }
    }

    // ── v25.14 (bug #17): intro selector validation ──

    #[test]
    fn intro_typo_is_rejected() {
        let msg = validate_field_value("intro", "logoo");
        assert!(msg.is_some(), "'logoo' (typo) must be rejected for intro");
        let msg = msg.expect("checked Some above");
        assert!(
            msg.contains("cosmic/logo/none"),
            "error must list valid intro types: {msg}"
        );
        assert!(
            msg.contains("--help"),
            "error must point to --help for discovery: {msg}"
        );
    }

    #[test]
    fn intro_valid_values_pass() {
        for v in ["cosmic", "logo", "none"] {
            assert!(
                validate_field_value("intro", v).is_none(),
                "'{v}' should be a valid intro"
            );
        }
    }

    #[test]
    fn intro_case_insensitive_matches_cli_valueenum() {
        // Phase 5 closure (P1-#4 + P2-6): all 3 enum paths (CLI clap
        // ValueEnum, --testconf, runtime from_str) are now case-insensitive.
        // Previously --testconf was strict-lowercase while CLI was lenient,
        // creating a confusing asymmetry. Now `intro = "Logo"` in config.toml
        // is accepted by --testconf (matching `--intro Logo` on CLI).
        for v in [
            "cosmic", "Cosmic", "COSMIC", "logo", "Logo", "LOGO", "none", "None", "NONE",
        ] {
            assert!(
                validate_field_value("intro", v).is_none(),
                "'{v}' should be accepted (case-insensitive, matching CLI)"
            );
        }
    }

    #[test]
    fn intro_empty_value_is_rejected() {
        assert!(
            validate_field_value("intro", "").is_some(),
            "empty intro must be rejected"
        );
        assert!(
            validate_field_value("intro", "   ").is_some(),
            "whitespace-only intro must be rejected"
        );
    }

    // ── Numeric range validation ──

    #[test]
    fn fps_out_of_range_is_rejected() {
        assert!(validate_field_value("fps", "0").is_some());
        // v30.3: cap reverted 300 -> 240. 241 is the new reject edge; 240
        // is the highest valid value. Rationale: 240 matches the most
        // common high-refresh monitor rate, aligns with the project's own
        // stated terminal ceiling (README.md:142: "typically 60-240 FPS on
        // Alacritty/kitty"), and matches the README CLI help text
        // (README.md:329: "--fps <1-240>"). The 300 cap (commit 12629eb)
        // matched no monitor refresh rate and exceeded the project's own
        // stated terminal ceiling.
        assert!(validate_field_value("fps", "241").is_some());
        assert!(validate_field_value("fps", "60").is_none());
        assert!(validate_field_value("fps", "240").is_none());
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

    // v30 simplify: density-map validation at --testconf time.
    #[test]
    fn density_map_valid_csv_passes() {
        assert!(validate_field_value("density-map", "1.0,0.5,0.0,0.8").is_none());
        assert!(validate_field_value("density-map", "0.85").is_none()); // single entry
        assert!(validate_field_value("density-map", "  0.1 , 0.2 , 0.3  ").is_none()); // whitespace
        assert!(validate_field_value("density-map", "1.0,,0.5,,").is_none()); // empty entries skipped
    }

    #[test]
    fn density_map_non_numeric_is_rejected() {
        let err = validate_field_value("density-map", "abc,def").expect("non-numeric should fail");
        assert!(err.contains("expected float"), "got: {err}");
        assert!(err.contains("abc"), "got: {err}");
    }

    #[test]
    fn density_map_out_of_range_is_rejected() {
        let err = validate_field_value("density-map", "0.5,1.5,0.0").expect("oob should fail");
        assert!(err.contains("out of range"), "got: {err}");
        assert!(err.contains("1.5"), "got: {err}");
    }

    #[test]
    fn density_map_empty_is_rejected() {
        let err = validate_field_value("density-map", ",,,").expect("empty should fail");
        assert!(err.contains("at least one"), "got: {err}");
    }

    // v30 fix: quoted CSV strings must pass --testconf. The configfile
    // parser does not strip surrounding quotes, so the validator must.
    #[test]
    fn density_map_quoted_csv_passes() {
        // Double-quoted form (most common user mistake).
        assert!(
            validate_field_value("density-map", "\"0.05,0.3,1.0\"").is_none(),
            "double-quoted CSV should pass --testconf"
        );
        // Single-quoted form.
        assert!(
            validate_field_value("density-map", "'0.1, 0.2, 0.3'").is_none(),
            "single-quoted CSV should pass --testconf"
        );
        // Quoted + outer whitespace.
        assert!(
            validate_field_value("density-map", "  \"0.5,0.5\"  ").is_none(),
            "quoted CSV with whitespace padding should pass --testconf"
        );
    }

    #[test]
    fn density_map_quoted_empty_is_rejected() {
        assert!(
            validate_field_value("density-map", "\"\"").is_some(),
            "quoted empty string should fail --testconf"
        );
        assert!(
            validate_field_value("density-map", "''").is_some(),
            "single-quoted empty string should fail --testconf"
        );
    }

    #[test]
    fn density_map_quoted_non_numeric_is_rejected() {
        // The error message should refer to the *unquoted* entry, not `"abc`.
        let err = validate_field_value("density-map", "\"abc,def\"").expect("should fail");
        assert!(err.contains("expected float"), "got: {err}");
        assert!(err.contains("abc"), "got: {err}");
        // Make sure the error does NOT include a stray quote character.
        assert!(
            !err.contains("\"abc"),
            "error message should reference the stripped entry 'abc', not '\"abc': {err}"
        );
    }

    // ── Enum value validation ──

    #[test]
    fn color_unknown_is_rejected() {
        let msg = validate_field_value("color", "not-a-color");
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("unknown color"));
    }

    // ── Context-aware hints (validate_field_value_with_cfg) ──
    // Closes the duplicate-usage confusion between `color` (built-in only)
    // and `colors-custom` (references a [colors-custom.<name>] block).

    #[test]
    fn color_matching_custom_palette_gets_colors_custom_hint() {
        // User wrote `color = z` inside a [scene-custom.<name>] block, but `z`
        // is the name of a [colors-custom.z] block — not a built-in color.
        // The error must point them at the `colors-custom` field.
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
        cfg.insert(
            "colors-custom.z.rain".to_string(),
            "#111111,#1ee460".to_string(),
        );
        let msg = validate_field_value_with_cfg("color", "z", &cfg)
            .expect("should still error — z is not a built-in color");
        assert!(
            msg.contains("custom palette"),
            "error must explain the value is a custom palette: {msg}"
        );
        assert!(
            msg.contains("colors-custom = z"),
            "error must suggest the `colors-custom = z` field: {msg}"
        );
        assert!(
            msg.contains("--list-colors"),
            "error must still mention --list-colors for built-in names: {msg}"
        );
    }

    #[test]
    fn color_matching_custom_palette_only_bg_field_still_hinted() {
        // A partially-declared [colors-custom.<name>] block (only `bg`, no
        // `rain`) still counts as a custom palette for hint purposes.
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("colors-custom.sunset.bg".to_string(), "#1a0033".to_string());
        let msg = validate_field_value_with_cfg("color", "sunset", &cfg)
            .expect("should error — sunset is not a built-in color");
        assert!(
            msg.contains("colors-custom = sunset"),
            "hint must fire even with only .bg declared: {msg}"
        );
    }

    #[test]
    fn color_matching_custom_palette_via_legacy_stops_field_still_hinted() {
        // Older configs may use the deprecated `.stops` alias for `.rain`.
        // The hint must still fire so users on legacy configs are guided to
        // the right field.
        let mut cfg = std::collections::HashMap::new();
        cfg.insert(
            "colors-custom.legacy.stops".to_string(),
            "#ff0000,#00ff00".to_string(),
        );
        let msg = validate_field_value_with_cfg("color", "legacy", &cfg)
            .expect("should error — legacy is not a built-in color");
        assert!(
            msg.contains("colors-custom = legacy"),
            "hint must fire via legacy .stops field: {msg}"
        );
    }

    #[test]
    fn color_unknown_with_no_matching_palette_keeps_plain_error() {
        // No [colors-custom.<name>] block exists for this value — the hint
        // must NOT fire. The plain "unknown color" error is returned.
        let cfg = std::collections::HashMap::new();
        let msg = validate_field_value_with_cfg("color", "not-a-color", &cfg)
            .expect("should error — not-a-color is unknown");
        assert!(
            msg.contains("unknown color"),
            "plain error must be preserved: {msg}"
        );
        assert!(
            !msg.contains("colors-custom ="),
            "hint must NOT fire when no matching palette exists: {msg}"
        );
    }

    #[test]
    fn color_matching_palette_is_case_insensitive() {
        // Built-in color names are case-insensitive at runtime; the hint
        // matching should also be case-insensitive so `color = Z` matches a
        // declared `[colors-custom.z]` block.
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
        let msg = validate_field_value_with_cfg("color", "Z", &cfg)
            .expect("should error — Z is not a built-in color");
        assert!(
            msg.contains("colors-custom = Z"),
            "hint must fire case-insensitively and preserve original casing: {msg}"
        );
    }

    #[test]
    fn color_valid_built_in_passes_with_cfg_unchanged() {
        // A valid built-in color name must still pass — the wrapper must not
        // turn a passing validation into a failure.
        let cfg = std::collections::HashMap::new();
        assert!(validate_field_value_with_cfg("color", "green", &cfg).is_none());
        assert!(validate_field_value_with_cfg("color", "neon-purple", &cfg).is_none());
    }

    #[test]
    fn validate_field_value_with_cfg_preserves_other_field_errors() {
        // The wrapper must NOT alter errors for non-color fields. Validate
        // that an out-of-range fps error passes through unchanged.
        let cfg = std::collections::HashMap::new();
        let plain = validate_field_value("fps", "9999");
        let wrapped = validate_field_value_with_cfg("fps", "9999", &cfg);
        assert_eq!(
            plain, wrapped,
            "wrapper must be transparent for non-color fields"
        );
    }

    // ── charset → charset-custom hint (parity with color hint) ──

    #[test]
    fn charset_matching_custom_block_gets_charset_custom_hint() {
        // User wrote `charset = pipes` inside a [scene-custom.<name>] block,
        // but `pipes` is the name of a [charset-custom.pipes] block — not a
        // built-in charset preset. The error must point them at the
        // `charset-custom` field. (Note: `pipes` is chosen because it is
        // NOT in the built-in charset list — see src/charset.rs.)
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("charset-custom.pipes.set".to_string(), "|".to_string());
        let msg = validate_field_value_with_cfg("charset", "pipes", &cfg)
            .expect("should still error — pipes is not a built-in charset");
        assert!(
            msg.contains("custom charset"),
            "error must explain the value is a custom charset: {msg}"
        );
        assert!(
            msg.contains("charset-custom = pipes"),
            "error must suggest the `charset-custom = pipes` field: {msg}"
        );
        assert!(
            msg.contains("--list-charsets"),
            "error must still mention --list-charsets for built-in names: {msg}"
        );
    }

    #[test]
    fn charset_matching_custom_block_is_case_insensitive() {
        // Charset name matching is case-insensitive at runtime; the hint
        // matching should also be case-insensitive so `charset = PIPES`
        // matches a declared `[charset-custom.pipes]` block.
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("charset-custom.pipes.set".to_string(), "|".to_string());
        let msg = validate_field_value_with_cfg("charset", "PIPES", &cfg)
            .expect("should error — PIPES is not a built-in charset");
        assert!(
            msg.contains("charset-custom = PIPES"),
            "hint must fire case-insensitively and preserve original casing: {msg}"
        );
    }

    #[test]
    fn charset_unknown_with_no_matching_block_keeps_plain_error() {
        // No [charset-custom.<name>] block exists for this value — the hint
        // must NOT fire. The plain "unknown charset" error is returned.
        let cfg = std::collections::HashMap::new();
        let msg = validate_field_value_with_cfg("charset", "not-a-charset", &cfg)
            .expect("should error — not-a-charset is unknown");
        assert!(
            msg.contains("unknown charset"),
            "plain error must be preserved: {msg}"
        );
        assert!(
            !msg.contains("charset-custom ="),
            "hint must NOT fire when no matching block exists: {msg}"
        );
    }

    #[test]
    fn charset_valid_built_in_passes_with_cfg_unchanged() {
        // A valid built-in charset name must still pass — the wrapper must
        // not turn a passing validation into a failure.
        let cfg = std::collections::HashMap::new();
        assert!(validate_field_value_with_cfg("charset", "matrix", &cfg).is_none());
        assert!(validate_field_value_with_cfg("charset", "hacker", &cfg).is_none());
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
    fn monolith_size_case_insensitive_matches_cli() {
        // Phase 5 closure (P1-#4 + P2-6)
        for v in ["Small", "SMALL", "Normal", "NORMAL", "Large", "LARGE"] {
            assert!(
                validate_field_value("monolith-size", v).is_none(),
                "'{v}' should be accepted (case-insensitive)"
            );
        }
    }

    #[test]
    fn glitch_level_invalid_is_rejected() {
        assert!(validate_field_value("glitch-level", "extreme").is_some());
        assert!(validate_field_value("glitch-level", "subtle").is_none());
    }

    #[test]
    fn glitch_level_case_insensitive_matches_cli() {
        // Phase 5 closure (P1-#4 + P2-6)
        for v in [
            "None", "NONE", "Subtle", "SUBTLE", "Default", "DEFAULT", "Intense", "INTENSE",
        ] {
            assert!(
                validate_field_value("glitch-level", v).is_none(),
                "'{v}' should be accepted (case-insensitive)"
            );
        }
    }

    #[test]
    fn color_bg_invalid_is_rejected() {
        assert!(validate_field_value("color-bg", "white").is_some());
        assert!(validate_field_value("color-bg", "black").is_none());
        assert!(validate_field_value("color-bg", "default-background").is_none());
    }

    #[test]
    fn color_bg_case_insensitive_matches_cli() {
        // Phase 5 closure (P2-6)
        for v in ["Black", "BLACK", "Default-Background", "DEFAULT-BACKGROUND"] {
            assert!(
                validate_field_value("color-bg", v).is_none(),
                "'{v}' should be accepted (case-insensitive)"
            );
        }
    }

    #[test]
    fn boolean_keys_reject_non_bool() {
        // Phase D Bug #1 fix: "yes"/"on"/"1"/"no"/"off"/"0" are now accepted
        // (matching parse_bool_config). Only truly invalid values are rejected.
        assert!(validate_field_value("mouse", "maybe").is_some());
        assert!(validate_field_value("mouse", "true").is_none());
        assert!(validate_field_value("mouse", "yes").is_none());
        assert!(validate_field_value("mouse", "on").is_none());
        assert!(validate_field_value("mouse", "1").is_none());
        assert!(validate_field_value("mouse", "false").is_none());
        assert!(validate_field_value("mouse", "no").is_none());
        assert!(validate_field_value("mouse", "off").is_none());
        assert!(validate_field_value("mouse", "0").is_none());
        assert!(validate_field_value("auto-color-drift", "false").is_none());
        assert!(validate_field_value("auto-color-drift", "YES").is_none()); // case-insensitive
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

    // ── v25.8 (bug #6): color.tune.* range validation ──
    //
    // Previously, `color.tune.brightness = 999` was silently accepted by
    // --testconf (PASS) and silently defaulted to 1.0 at runtime — the user
    // got zero feedback that their value was out of range. This mirrors the
    // v14 fix that made fps/speed/density strict. Now all five color.tune
    // fields reject values outside [0.0, 3.0] (matching TUNE_MIN/TUNE_MAX
    // in color_tune.rs).

    #[test]
    fn color_tune_brightness_out_of_range_is_rejected() {
        assert!(validate_field_value("color.tune.brightness", "3.1").is_some());
        assert!(validate_field_value("color.tune.brightness", "-0.1").is_some());
        assert!(validate_field_value("color.tune.brightness", "999").is_some());
        assert!(validate_field_value("color.tune.brightness", "1.5").is_none());
        assert!(validate_field_value("color.tune.brightness", "0.0").is_none());
        assert!(validate_field_value("color.tune.brightness", "3.0").is_none());
    }

    #[test]
    fn color_tune_saturation_out_of_range_is_rejected() {
        assert!(validate_field_value("color.tune.saturation", "3.5").is_some());
        assert!(validate_field_value("color.tune.saturation", "-1.0").is_some());
        assert!(validate_field_value("color.tune.saturation", "1.0").is_none());
    }

    #[test]
    fn color_tune_head_body_tail_out_of_range_is_rejected() {
        for field in &["head", "body", "tail"] {
            let key = format!("color.tune.{field}");
            assert!(
                validate_field_value(&key, "5.0").is_some(),
                "{key} = 5.0 should be rejected"
            );
            assert!(
                validate_field_value(&key, "-0.01").is_some(),
                "{key} = -0.01 should be rejected"
            );
            assert!(
                validate_field_value(&key, "2.0").is_none(),
                "{key} = 2.0 should be accepted"
            );
        }
    }

    #[test]
    fn color_tune_non_numeric_is_rejected() {
        let msg = validate_field_value("color.tune.brightness", "bright");
        assert!(
            msg.is_some(),
            "'bright' must be rejected for color.tune.brightness"
        );
        assert!(msg.unwrap().contains("expected number"));
    }

    #[test]
    fn color_tune_empty_value_is_rejected() {
        assert!(validate_field_value("color.tune.brightness", "").is_some());
        assert!(validate_field_value("color.tune.brightness", "   ").is_some());
    }

    #[test]
    fn color_tune_end_to_end_via_validate_config_strictly() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("color.tune.brightness".to_string(), "999".to_string());
        let result = validate_config_strictly(&cfg);
        assert!(
            result.is_err(),
            "validate_config_strictly must reject color.tune.brightness=999"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("out of range"),
            "error must mention range, got: {err}"
        );

        // Valid value passes.
        let mut cfg2 = std::collections::HashMap::new();
        cfg2.insert("color.tune.brightness".to_string(), "1.5".to_string());
        assert!(validate_config_strictly(&cfg2).is_ok());
    }

    /// v25.14 (bug #17): end-to-end check that `validate_config_strictly`
    /// rejects an invalid `intro` value the same way it rejects an OOR
    /// `color.tune.brightness`. Before the fix, this passed silently and
    /// the user only saw a stderr warning at runtime (which doesn't stop
    /// startup or live-reload).
    #[test]
    fn intro_end_to_end_via_validate_config_strictly() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("intro".to_string(), "splash".to_string());
        let result = validate_config_strictly(&cfg);
        assert!(
            result.is_err(),
            "validate_config_strictly must reject intro=splash"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("cosmic/logo/none"),
            "error must list valid intro types, got: {err}"
        );
        assert!(
            err.contains("splash"),
            "error must echo the bad value, got: {err}"
        );

        // Each valid value passes end-to-end.
        for v in ["cosmic", "logo", "none"] {
            let mut cfg2 = std::collections::HashMap::new();
            cfg2.insert("intro".to_string(), v.to_string());
            assert!(
                validate_config_strictly(&cfg2).is_ok(),
                "intro={v} must pass strict validation"
            );
        }
    }
}
