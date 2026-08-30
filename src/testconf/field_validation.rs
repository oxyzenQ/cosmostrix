// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Field-value validation — extracted from `testconf/mod.rs` to keep
//! that file under the 800-LOC hard cap.
//!
//! Owns:
//! - `validate_field_value`: validates a single key=value pair (top-level
//!   and scene-custom block values).
//! - `validate_field_value_with_cfg`: same with charset-custom cross-ref
//!   support.
//!
//! Re-exported from `testconf/mod.rs` via `pub(crate) use`.

use crate::theme;

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
        //
        // v50.0.0-beta.6: out-of-range values are now WARNINGS, not errors.
        // Runtime parse_density_map clamps to [0.0, 1.0] (1.5 → 1.0, -0.3 →
        // 0.0), so rejecting at testconf created an ambiguity — testconf
        // failed but runtime would have worked. Now testconf warns about
        // the clamp so the user knows, but does not block the config.
        // Non-numeric entries (typos like "abc") are still hard errors.
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
                        // v50.0.0-beta.6: warn about clamp, don't error.
                        // Runtime will clamp to [0.0, 1.0] — the user's
                        // intent is clear, just the value is out of range.
                        crate::output::eprintln_warn_labeled(&format!(
                            "density-map entry '{entry}' = {n} is out of range [0.0, 1.0] — will be clamped at runtime"
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
        // Bool config keys: accept the same lenient set as parse_bool_config
        // (true/yes/on/1/false/no/off/0, case-insensitive).
        "crystal-dragon" | "power-dragon" => {
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "true" | "yes" | "on" | "1" | "false" | "no" | "off" | "0" => None,
                _ => Some(format!(
                    "expected true/false (or yes/no, on/off, 1/0), got '{v}'"
                )),
            }
        }
        // v51 msg-fill-style: must match the clap ValueEnum accepted by
        // -mfs/--msg-fill-style. Same uniform-rejection contract as
        // `intro` (bug #17): --testconf, startup validation, and
        // live-reload strict validation all reject invalid values here,
        // instead of only being caught by the soft `MsgFillStyle::from_str`
        // error path in config_apply.rs.
        "msg-fill-style" => {
            // Case-insensitive to match the config surface (the CLI flag
            // itself is case-sensitive; the config key is forgiving —
            // same asymmetry as every other enum key).
            let lower = v.trim().to_ascii_lowercase();
            match lower.as_str() {
                "typewriter" | "fade" | "words" | "slide" | "pulse" | "instant" => None,
                _ => Some(format!(
                    "expected typewriter/fade/words/slide/pulse/instant, got '{v}' (run `cosmostrix --help` for valid message fill styles)"
                )),
            }
        }
        // v50.0.0-beta.7: ambient-snapback-secs — float in [0.0, 86400.0].
        // 0.0 = instant snapback, 86400.0 (24h) = effectively disabled.
        // Default 30.0 when unset. Range matches parse_f64_config in
        // config_apply.rs.
        "ambient-snapback-secs" => match v.trim().parse::<f64>() {
            Ok(n) if (0.0..=86400.0).contains(&n) => None,
            Ok(n) => Some(format!("expected 0.0..=86400.0, got {n}")),
            Err(_) => Some(format!("expected a number in 0.0..=86400.0, got '{v}'")),
        },
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
    // intro-color: must be a known builtin theme OR a custom palette
    // defined in [colors-custom.<name>]. Same logic as config_apply.rs.
    if key == "intro-color" {
        let lower = value.trim().to_ascii_lowercase();
        if theme::canonical_name_for_input(&lower).is_some() {
            return None;
        }
        let bg_key = format!("colors-custom.{lower}.bg");
        let rain_key = format!("colors-custom.{lower}.rain");
        let stops_key = format!("colors-custom.{lower}.stops");
        if cfg.contains_key(&bg_key) || cfg.contains_key(&rain_key) || cfg.contains_key(&stops_key)
        {
            return None;
        }
        return Some(format!(
            "unknown intro-color '{value}' — not a builtin theme or custom palette. \
             Use --list-colors to see available themes."
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
