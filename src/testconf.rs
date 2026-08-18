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

    // CD-07: surface inert [profile.<name>] blocks. The legacy `profile.*`
    // prefix is accepted by is_known_key (so --testconf PASSES silently on
    // configs containing them), but the blocks are never applied at runtime
    // — they were replaced by `scene-custom.*` . Add a clear warning
    // so users who keep legacy [profile.<name>] blocks know they are inert.
    let profile_only_keys: Vec<_> = parsed
        .values
        .keys()
        .filter(|k| k.starts_with("profile."))
        .collect();
    if !profile_only_keys.is_empty() {
        crate::output::eprintln_verbose_raw(
            "testconf: warning: [profile.<name>] blocks are inert (replaced by [scene-custom.<name>] ). Rename the prefix to apply them at runtime.",
        );
    }

    // Validate known value-ranges for top-level (non-block) keys.
    // v14: invalid values are now ERRORS, not warnings — silent PASS for
    // bad values is a bug. Owner requirement: strict value validation.
    // ambient.* keys are validated as a group via
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
        // (bug #6): color.tune.* fields must be in [0.0, 3.0].
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
        // maxdpc) are REMOVED — they fall into unknown_keys and are rejected
        // by --testconf and at startup. Use --glitch-level instead.
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
        // (CLI-D-3): removed dead validators for `atmosphere-regime` /
        // `atmosphere-mode` — these keys are NOT in USER_CONFIG_KEYS (eliminated
        // at commit 07b44b5), so they fall into `unknown_keys` at parse time
        // and never reach `validate_field_value`. The migration hints now live
        // in `config_hints::suggest_for_unknown_key` (which fires when the
        // unknown_keys check catches them). Same for `low-power`, `mouse`,
        // and `adaptive-custom` below.
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
        // (true/yes/on/1/false/no/off/0, case-insensitive). (CLI-D-3):
        // removed dead `low-power` / `mouse` from this arm (no longer in
        // USER_CONFIG_KEYS — caught as unknown_keys upstream).
        "async-mode" => {
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "true" | "yes" | "on" | "1" | "false" | "no" | "off" | "0" => None,
                _ => Some(format!(
                    "expected true/false (or yes/no, on/off, 1/0), got '{v}'"
                )),
            }
        }
        // (bug #17): intro selector — must match the clap ValueEnum
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
        // (CLI-V-2): scene-custom `async-mode` field validator — now unified
        // with the top-level `async-mode` match arm above (same validation).
        // Previously this was a separate `"async"` arm; renaming to `async-mode`
        // merged the two into one match arm.

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
/// scene-custom block path — which is exactly the design-consistent
/// behavior: inside `[scene-custom.<name>]`, the explicit `charset-custom`
/// field is the canonical way to reference a custom block.
pub(crate) fn validate_field_value_with_cfg(
    key: &str,
    value: &str,
    cfg: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // (CLI-V-2): scene-custom block-reference validators. These need
    // cfg to check whether the referenced [colors-custom.<name>] /
    // [charset-custom.<name>] block exists in this config. Previously fell
    // through to the base catch-all `_ => None` and silently passed
    // --testconf, then failed at runtime with no warning. Run BEFORE the
    // base call so they short-circuit when the reference is broken.
    if key == "colors-custom" {
        let lower = value.trim().to_ascii_lowercase();
        let bg_key = format!("colors-custom.{lower}.bg");
        let rain_key = format!("colors-custom.{lower}.rain");
        let stops_key = format!("colors-custom.{lower}.stops");
        if cfg.contains_key(&bg_key) || cfg.contains_key(&rain_key) || cfg.contains_key(&stops_key)
        {
            return None;
        }
        return Some(format!(
            "unknown colors-custom block '{value}' — define [colors-custom.{value}] in this config (with .bg and .rain/.stops sub-fields)"
        ));
    }
    if key == "charset-custom" {
        let lower = value.trim().to_ascii_lowercase();
        let set_key = format!("charset-custom.{lower}.set");
        if cfg.contains_key(&set_key) {
            return None;
        }
        return Some(format!(
            "unknown charset-custom block '{value}' — define [charset-custom.{value}] in this config (with .set sub-field)"
        ));
    }
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
#[path = "testconf_tests.rs"]
mod testconf_tests;
