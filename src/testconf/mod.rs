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
use crate::output::{eprintln_safe, println_safe};
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

    println_safe!("testconf: checking {}", path.display());

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            crate::output::eprintln_error_labeled(&format!(
                "testconf: cannot read config file: {e}"
            ));
            crate::output::eprintln_suggestion_line(
                "testconf: hint: run `cosmostrix --config-path` to see the expected location",
            );
            crate::output::eprintln_suggestion_line("testconf: hint: cosmostrix --dump-config <config-path>  (writes directly, whitelist-enforced)");
            std::process::exit(2);
        }
    };

    // v50: SHA-512 fingerprint of the config file on disk.
    // Lets the user verify exact config state with `sha512sum`,
    // detect config drift across machines, and prove config identity
    // in bug reports. Uses the same sha2 crate as --dump-config and
    // live-reload change detection (zero new dependencies).
    let file_hash = configfile::sha512_hex(content.as_bytes());
    println_safe!("testconf: file-sha512: {file_hash}");

    // v50 (alpha.2): Extract template-fingerprint from the header (if present)
    // and compare against the current built-in template to detect drift.
    let header_fp = configfile::extract_template_fingerprint(&content);
    let current_template_hash = configfile::sha512_hex(configfile::dump_config_text().as_bytes());
    match &header_fp {
        Some(fp) => {
            println_safe!("testconf: template-fingerprint: {fp}");
            if fp == &current_template_hash {
                println_safe!(
                    "testconf: template drift: none (matches built-in v{} template)",
                    env!("CARGO_PKG_VERSION")
                );
            } else {
                println_safe!("testconf: template drift: detected — header fingerprint differs from built-in template");
                println_safe!("testconf:   built-in hash: {current_template_hash}");
                crate::output::eprintln_suggestion_line(
                    "testconf: hint: run `cosmostrix --dump-config <path> --force` to regenerate the template"
                );
            }
        }
        None => {
            println_safe!("testconf: template-fingerprint: (not found in header — config may be hand-written or older)");
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
        crate::output::eprintln_suggestion_line(
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
                crate::output::eprintln_suggestion_line(&format!("testconf: hint: {hint}"));
            }
            errors += 1;
        }
        eprintln_safe!(
            "testconf: known keys: {}",
            configfile::known_keys().join(", ")
        );
    }

    // Info-only notice for auto-promoted keys (forgiving parser).
    // These are NOT errors — the keys were silently re-homed to root scope.
    // We surface them so users know their TOML structure was off and can
    // optionally fix it for clarity (move the key BEFORE any [section] header).
    if !parsed.promoted_keys.is_empty() {
        println_safe!(
            "testconf: info: {} key(s) auto-promoted to root scope (TOML mis-nesting, fixed):",
            parsed.promoted_keys.len()
        );
        for (from, to) in &parsed.promoted_keys {
            println_safe!("testconf:   {from}  →  {to}");
        }
        println_safe!(
            "testconf: hint: move these keys BEFORE any [section] header to silence this notice"
        );
    }

    // Check scene-custom block keys for correct format AND field value
    // validity. Keys are iterated in SORTED order so --testconf's error
    // output is deterministic across runs (HashMap order is
    // seed-randomized; sorted iteration gives stable reports — same
    // contract as validate_config_strictly).
    let mut block_keys: Vec<_> = parsed
        .values
        .keys()
        .filter(|k| k.starts_with("scene-custom."))
        .collect();
    block_keys.sort();
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
            // Use the canonical SCENE_CUSTOM_FIELDS list so testconf never
            // drifts from the actual config parser. Previously this was a
            // hardcoded copy that drifted when fields were added or removed
            // (the density-map removal in v80.0.0-beta.2 is the latest
            // example). v80.0.0-beta.2 (S-master-LOGIC-3): the list shrunk
            // to the six scene-family dimensions — base-scene/bold/
            // shading-mode/async-mode are removed and land here as unknown
            // block fields (with a targeted migration hint).
            let valid_fields: &[&str] = crate::scene_custom::SCENE_CUSTOM_FIELDS;
            if !valid_fields.contains(&field) {
                crate::output::eprintln_error_labeled(&format!(
                    "testconf: unknown block field '{field}' in '{pk}'"
                ));
                // S-master-HUNT-5: guidance line → suggestion white.
                crate::output::eprintln_suggestion_line(&format!(
                    "testconf: valid block fields: {}",
                    valid_fields.join(", ")
                ));
                errors += 1;
            } else {
                // Field is recognized — now validate the VALUE using the same
                // rules as top-level keys. Block fields accept the same value
                // vocabulary (color, charset, fps, speed, density,
                // glitch-level). The context-aware variant emits a richer
                // hint when the value matches a custom block (e.g.
                // `color = z` where `[colors-custom.z]` exists — points the
                // user to `colors-custom = z`).
                if let Some(msg) = validate_field_value_with_cfg(field, value, &parsed.values) {
                    crate::output::eprintln_error_labeled(&format!(
                        "testconf: {pk} = {value}: {msg}"
                    ));
                    errors += 1;
                }
            }
        }
    }

    // v80.0.0-beta.2 (S-master-LOGIC-3): scene-custom blocks must be
    // COMPLETELY filled — one of each pair (color|colors-custom,
    // charset|charset-custom) plus fps, speed, density, glitch-level.
    // An incomplete block is a hard error (owner mandate: "if user not
    // complete fill under block scene custom value will be error because
    // should complete filled").
    if let Err(msg) = crate::scene_custom::validate_scene_custom_completeness(&parsed.values) {
        crate::output::eprintln_error_labeled(&format!("testconf: {msg}"));
        errors += 1;
    }

    // Validate known value-ranges for top-level (non-block) keys.
    // v14: invalid values are now ERRORS, not warnings — silent PASS for
    // bad values is a bug. Owner requirement: strict value validation.
    // ambient.* keys are validated as a group via
    // validate_ambient_entries (which checks scene-name validity and
    // rejects legacy multi-field format with a migration message).
    let mut ambient_validated = false;
    // Sorted iteration: deterministic error ordering across runs (the
    // underlying HashMap order is seed-randomized per process).
    let mut value_keys: Vec<&String> = parsed.values.keys().collect();
    value_keys.sort();
    for key in value_keys {
        let value = &parsed.values[key];
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
                println_safe!(
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
    println_safe!();
    println_safe!(
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
        println_safe!("testconf: PASS — config is valid");
    }
    Ok(())
}

/// Validate ALL fields in a parsed config HashMap — top-level keys AND
/// custom-block keys (scene-custom.X.field values, colors-custom hex,
/// charset-custom content, ambient.* scene references).
///
/// Returns `Ok(())` if every key has a valid value, or `Err(message)`
/// with a human-readable error for the first invalid field (see the
/// determinism note below on which error surfaces first).
///
/// Used by:
/// - Startup: `apply_config_and_runtime_defaults` rejects invalid config
///   before cosmostrix starts running (exit code 2).
/// - Live reload: watcher rejects invalid config edits (exit code 2).
/// - --testconf: validates and reports errors.
pub(crate) fn validate_config_strictly(
    cfg: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    // v80.0.0-beta.2 (S-master-LOGIC-3): scene-custom blocks must be
    // COMPLETELY filled — one of each pair (color|colors-custom,
    // charset|charset-custom) plus fps, speed, density, glitch-level.
    // Runs BEFORE the per-key loop so the completeness error surfaces
    // first (it names the block, not a single key). Startup, the
    // live-reload watcher, and --testconf all reject through here.
    crate::scene_custom::validate_scene_custom_completeness(cfg)?;

    // v80.0.0-beta.2 (S-master-HUNT-2, owner cp77x bug 2026-09-02):
    // deterministic FULL-COVERAGE validation. The old loop `break`ed
    // after validating the first `ambient.*` key it happened to reach,
    // which terminated the ENTIRE per-key loop — every key not yet
    // iterated was silently blessed. HashMap iteration order is
    // seed-randomized per instance/thread, so coverage depended on
    // where `ambient.*` landed: a config pairing `ambient.12-00` with
    // an invalid `scene-custom.<name>.color` errored at startup on some
    // runs (exit 2) and silently ran the scene's default color on
    // others (measured: 11 reject / 9 silent over 20 runs). Live-reload
    // parses on the watcher thread — a different seed — so the same
    // file also validated differently there, which is exactly why the
    // owner's error only surfaced AFTER a second config touch.
    //
    // Fix: (1) ambient entries are validated ONCE in a dedicated
    // pre-pass; (2) the per-key loop iterates SORTED keys so the first
    // reported error is stable across runs and threads (same contract
    // as `validate_ambient_entries`' own sorted iteration); (3)
    // `ambient.*` keys `continue` in the loop — never `break` — so
    // every key is always checked, no matter the hash order.
    if cfg.keys().any(|k| k.starts_with("ambient.")) {
        crate::crystal_dragon_engine::ambient::validate_ambient_entries(cfg)?;
    }

    let mut keys: Vec<&String> = cfg.keys().collect();
    keys.sort();
    for key in keys {
        let value = &cfg[key];
        if key.starts_with("scene-custom.") {
            // v80.0.0-beta.2 (S-master-HUNT, owner bug 3): block fields are
            // VALUE-validated here too — `--testconf` already checked them
            // (run_testconf's block-field loop), but startup and the
            // live-reload watcher validate through THIS function and used
            // to skip block keys entirely. A block referencing a missing
            // custom palette/charset (`colors-custom = "cosmos"` where
            // cosmos is a BUILTIN color, not a [colors-custom.<name>]
            // block) therefore passed startup silently and was a silent
            // no-op at runtime (the HUD kept the previous color/charset).
            // Now all three surfaces (startup exit 2, live-reload reject,
            // --testconf) reject invalid block values in lockstep — the
            // same uniform-rejection contract as every other key.
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() != 3 {
                return Err(format!(
                    "malformed block key '{key}' (expected scene-custom.<name>.<field>)"
                ));
            }
            let field = parts[2];
            if !crate::scene_custom::SCENE_CUSTOM_FIELDS.contains(&field) {
                // Unknown block fields are also caught upstream as unknown
                // config keys (is_scene_custom_config_key); this is
                // defense-in-depth so the strict validator never silently
                // blesses a drifted field name.
                return Err(format!(
                    "unknown block field '{field}' in '{key}' (valid fields: {})",
                    crate::scene_custom::SCENE_CUSTOM_FIELDS.join(", ")
                ));
            }
            if let Some(msg) = validate_field_value_with_cfg(field, value, cfg) {
                return Err(format!("invalid value '{value}' for '{key}': {msg}"));
            }
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
        // Ambient phase scheduler: `ambient.<HH-MM>` keys. Validated as
        // a group by the pre-pass above (before the loop) — the per-key
        // loop has nothing left to check here. `continue`, never
        // `break`: breaking would skip every key not yet iterated.
        if key.starts_with("ambient.") {
            continue;
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
#[path = "../../test/testconf/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../../test/testconf/tests_validation_order.rs"]
mod tests_validation_order;
