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
        "shading-mode" => match v {
            "0" | "1" => None,
            _ => Some(format!("expected 0 or 1, got '{v}'")),
        },
        // ── Enum-like string values ──
        //
        // v80.0.0-beta.2 custom-reference parity: `color`, `charset`, and
        // `scene` may each name a custom block (`[colors-custom.<name>]`,
        // `[charset-custom.<name>]`, `[scene-custom.<name>]`) — the runtime
        // resolution paths (config_apply.rs, main.rs, scene_runtime.rs)
        // accept both builtins and custom blocks for all three fields.
        // The BASE validators here check builtins only; the cfg-aware
        // wrapper `validate_field_value_with_cfg` adds the custom-block
        // acceptance on top, keeping testconf/startup/live-reload in lock
        // step with runtime.
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
        // `base-scene` was REMOVED from [scene-custom.<name>] blocks in
        // v80.0.0-beta.2 (S-master-LOGIC-3) — custom scenes are complete
        // self-contained profiles with no built-in inheritance. The key
        // is rejected upstream as an unknown key (is_known_key ->
        // SCENE_CUSTOM_FIELDS) with a targeted config_hints migration
        // hint, so no value validator is needed here.
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
        // v80.0.0-beta.1 msg-fill-style: must match the clap ValueEnum accepted by
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
                "typewriter" | "fade" | "words" | "slide" | "instant"
                | "engrave" | "hologram" | "glitch" | "scorch" | "cascade" | "radar" | "tide" | "dissolve" => None,
                _ => Some(format!(
                    "expected typewriter/fade/words/slide/instant/engrave/hologram/glitch/scorch/cascade/radar/tide/dissolve, got '{v}' (run `cosmostrix --help` for valid message fill styles)"
                )),
            }
        }
        // v50.0.0-beta.7: ambient-snapback-secs — float in [0.0, 86400.0].
        // 0.0 = instant snapback, 86400.0 (24h) = effectively disabled.
        // Default 30.0 when unset. Range matches parse_secs_config in
        // config_apply.rs.
        // v80.0.0-beta.2 usage note (verified 2026-09-02): the snapback
        // timer fires at ANY value in range — including >= 60s. v80.0.0-alpha.1:
        // the guidance is now RELATIVE — keep ambient-snapback-secs below
        // the effective crystal-dragon-secs (default 60, <= polling-10 for
        // margin) when combining ambient with crystal-dragon: a long window
        // freezes new drifts and holds the ambient palette for the whole
        // window (86400 ≈ 24h). See docs/AMBIENT_SCHEDULER.md "Edge case:
        // snapback >= polling".
        // v80.0.0-alpha.2: human-duration forms accepted (30, 30s, 1m,
        // 1h30m) — one vocabulary with the CLI flags (shared
        // cli::cli_parse::parse_secs_f64 grammar + unit table).
        "ambient-snapback-secs" => match crate::cli::cli_parse::parse_secs_f64(v.trim()) {
            Ok(n) if (0.0..=86400.0).contains(&n) => None,
            Ok(n) => Some(format!("expected 0.0..=86400.0 seconds, got {n}")),
            Err(_) => Some(format!(
                "expected a duration in 0.0..=86400.0 seconds (e.g. 30, 30s, 1m), got '{v}'"
            )),
        },
        // v80.0.0-alpha.1: crystal-dragon-secs — float in [0.0, 86400.0].
        // Same range contract as ambient-snapback-secs (the two harmony
        // knobs share one timeline). Default 60.0 when unset.
        // v80.0.0-alpha.1 (S-master-HUNT-3): the min-dwell anti-flicker floor is
        // min(60s, cadence) — an explicit faster cadence is REAL (the
        // floor yields); at/slow defaults the 60s floor applies.
        // v80.0.0-alpha.2: human-duration forms accepted (60, 60s, 1m,
        // 1h30m) — one vocabulary with the CLI flags.
        "crystal-dragon-secs" => match crate::cli::cli_parse::parse_secs_f64(v.trim()) {
            Ok(n) if (0.0..=86400.0).contains(&n) => None,
            Ok(n) => Some(format!("expected 0.0..=86400.0 seconds, got {n}")),
            Err(_) => Some(format!(
                "expected a duration in 0.0..=86400.0 seconds (e.g. 60, 60s, 1m), got '{v}'"
            )),
        },
        // Keys we don't have a specific validator for — assume OK.
        // Unknown keys are caught earlier by the unknown_keys check.
        _ => None,
    }
}

/// Context-aware wrapper around [`validate_field_value`].
///
/// Accepts the parsed config map so it can resolve custom-block
/// references. v80.0.0-beta.2 custom-reference parity (owner bug fix
/// 2026-09-02): the three name-bearing fields accept BOTH built-ins and
/// custom blocks, exactly mirroring the runtime resolution paths:
///   - `scene = <name>` — built-in scene OR `[scene-custom.<name>]` block
///     (runtime: config_apply.rs accepts both; scene_runtime.rs applies
///     the custom block Cloud-side).
///   - `color = <name>` — built-in theme OR `[colors-custom.<name>]` block
///     (runtime: apply_config_values + main.rs unified resolution accept
///     both; scene_runtime.rs resolves a block's `color` field through
///     the custom palette path when the name is not a built-in).
///   - `charset = <name>` — built-in preset OR `[charset-custom.<name>]`
///     block (runtime: main.rs charset resolution and scene_runtime.rs
///     both try the custom block first).
///
/// The acceptance applies to BOTH top-level keys and
/// `[scene-custom.<name>]` block fields — the runtime treats both
/// surfaces identically (a block's `color`/`charset` field resolves
/// custom names in scene_runtime.rs). Previously only `charset` had a
/// caller-side top-level carve-out and `color`/`scene` rejected custom
/// names with misleading "use X-custom instead" hints even though the
/// runtime happily applied them — the exact inconsistency behind the
/// owner's fatal-startup bug report (a config with `scene = <custom>`
/// passed no validation layer at all, blocking every launch).
///
/// Callers that have the parsed config map available should prefer this over
/// the bare `validate_field_value`. The base function remains available for
/// contexts (e.g. unit tests, CLI arg parsing) where no surrounding config
/// exists.
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
        // v80.0.0-beta.2 (S-master-HUNT, owner bug 3): a BUILTIN color name
        // in `colors-custom` is the classic mistake (the field only accepts
        // [colors-custom.<name>] blocks — built-ins belong in the `color`
        // field). Point the user at the right field instead of a bare
        // "unknown block".
        let builtin_hint = if theme::canonical_name_for_input(&lower).is_some() {
            format!(
                " — '{value}' is a BUILT-IN color name; use the block's 'color' field for built-ins"
            )
        } else {
            String::new()
        };
        return Some(format!(
            "unknown colors-custom block '{value}'{builtin_hint} — define [colors-custom.{value}] in this config (with .bg and .rain/.stops sub-fields)"
        ));
    }
    if key == "charset-custom" {
        let lower = value.trim().to_ascii_lowercase();
        let set_key = format!("charset-custom.{lower}.set");
        if cfg.contains_key(&set_key) {
            return None;
        }
        let builtin_hint = if crate::charset::charset_from_str(&lower, false).is_ok() {
            format!(
                " — '{value}' is a BUILT-IN charset name; use the block's 'charset' field for built-ins"
            )
        } else {
            String::new()
        };
        return Some(format!(
            "unknown charset-custom block '{value}'{builtin_hint} — define [charset-custom.{value}] in this config (with .set sub-field)"
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
    // v80.0.0-beta.2 custom-reference parity (owner mandate: "if charset
    // can custom but why not for colors?" — now all three accept custom
    // names uniformly, matching runtime): accept `scene`/`color`/
    // `charset` values that reference a custom block defined elsewhere
    // in this SAME config. The block's own content is validated by the
    // colors-custom/charset-custom branches above (and scene-custom
    // blocks by the caller's block-field loop), so a resolved reference
    // never masks invalid block data.
    let lower = value.trim().to_ascii_lowercase();
    if !lower.is_empty() {
        match key {
            "scene" => {
                // A [scene-custom.<name>] block is recognized by ANY of its
                // declared fields (color/colors-custom/charset/
                // charset-custom/fps/speed/density/glitch-level).
                let has_block = cfg.keys().any(|k| {
                    k.starts_with("scene-custom.") && k.split('.').nth(1) == Some(lower.as_str())
                });
                if has_block {
                    return None;
                }
            }
            "color" => {
                let bg_key = format!("colors-custom.{lower}.bg");
                let rain_key = format!("colors-custom.{lower}.rain");
                let stops_key = format!("colors-custom.{lower}.stops");
                if cfg.contains_key(&bg_key)
                    || cfg.contains_key(&rain_key)
                    || cfg.contains_key(&stops_key)
                {
                    return None;
                }
            }
            "charset" => {
                let set_key = format!("charset-custom.{lower}.set");
                if cfg.contains_key(&set_key) {
                    return None;
                }
            }
            _ => {}
        }
    }
    validate_field_value(key, value)
}
