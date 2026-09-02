// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

// ── (bug #17): intro selector validation ──

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
    // cap reverted 300 -> 240. 241 is the new reject edge; 240
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
// ── Enum value validation ──

#[test]
fn color_unknown_is_rejected() {
    let msg = validate_field_value("color", "not-a-color");
    assert!(msg.is_some());
    assert!(msg.unwrap().contains("unknown color"));
}

// ── Context-aware custom-reference acceptance (validate_field_value_with_cfg) ──
// v80.0.0-beta.2 custom-reference parity (owner bug fix 2026-09-02):
// `color`, `charset`, and `scene` all accept custom-block references
// (mirroring the runtime resolution paths). Previously only charset had
// a caller-side carve-out; color/scene rejected valid custom names with
// misleading "use X-custom instead" hints while the runtime applied
// them — the fatal-startup inconsistency the owner reported.

#[test]
fn color_matching_custom_palette_is_accepted() {
    // User wrote `color = z` (top-level or inside a scene-custom block),
    // and `z` is the name of a [colors-custom.z] block — accepted,
    // exactly like the runtime path (apply_config_values + main.rs
    // unified resolution + scene_runtime.rs custom.color fallback).
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.z.rain".to_string(),
        "#111111,#1ee460".to_string(),
    );
    assert!(
        validate_field_value_with_cfg("color", "z", &cfg).is_none(),
        "color = <custom palette name> must pass (runtime parity)"
    );
}

#[test]
fn color_matching_custom_palette_only_bg_field_still_accepted() {
    // A partially-declared [colors-custom.<name>] block (only `bg`, no
    // `rain`) still counts as a custom palette reference.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("colors-custom.sunset.bg".to_string(), "#1a0033".to_string());
    assert!(
        validate_field_value_with_cfg("color", "sunset", &cfg).is_none(),
        "acceptance must fire even with only .bg declared: matches runtime is_colors_custom_name"
    );
}

#[test]
fn color_matching_custom_palette_via_legacy_stops_field_still_accepted() {
    // Older configs may use the deprecated `.stops` alias for `.rain`.
    // The acceptance must still fire so legacy configs validate.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert(
        "colors-custom.legacy.stops".to_string(),
        "#ff0000,#00ff00".to_string(),
    );
    assert!(
        validate_field_value_with_cfg("color", "legacy", &cfg).is_none(),
        "acceptance must fire via legacy .stops field"
    );
}

#[test]
fn color_unknown_with_no_matching_palette_keeps_plain_error() {
    // No [colors-custom.<name>] block exists for this value — the plain
    // "unknown color" error is returned.
    let cfg = std::collections::HashMap::new();
    let msg = validate_field_value_with_cfg("color", "not-a-color", &cfg)
        .expect("should error — not-a-color is unknown");
    assert!(
        msg.contains("unknown color"),
        "plain error must be preserved: {msg}"
    );
    assert!(
        !msg.contains("colors-custom ="),
        "no stale hint when no matching palette exists: {msg}"
    );
}

#[test]
fn color_matching_palette_is_case_insensitive() {
    // Built-in color names are case-insensitive at runtime; custom
    // reference matching must be too so `color = Z` matches a declared
    // `[colors-custom.z]` block.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("colors-custom.z.bg".to_string(), "#0a0a0a".to_string());
    assert!(
        validate_field_value_with_cfg("color", "Z", &cfg).is_none(),
        "acceptance must fire case-insensitively (runtime parity)"
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

// ── charset custom-reference acceptance (parity with color) ──

#[test]
fn charset_matching_custom_block_is_accepted() {
    // User wrote `charset = pipes` (top-level or inside a scene-custom
    // block), and `pipes` is the name of a [charset-custom.pipes] block —
    // accepted, exactly like the runtime path (main.rs charset
    // resolution + scene_runtime.rs custom.charset arm both resolve
    // custom blocks first). (Note: `pipes` is chosen because it is NOT
    // in the built-in charset list — see src/scene/charset.rs.)
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("charset-custom.pipes.set".to_string(), "|".to_string());
    assert!(
        validate_field_value_with_cfg("charset", "pipes", &cfg).is_none(),
        "charset = <custom block name> must pass (runtime parity)"
    );
}

#[test]
fn charset_matching_custom_block_is_case_insensitive() {
    // Charset name matching is case-insensitive at runtime; custom
    // reference acceptance must be too so `charset = PIPES` matches a
    // declared `[charset-custom.pipes]` block.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("charset-custom.pipes.set".to_string(), "|".to_string());
    assert!(
        validate_field_value_with_cfg("charset", "PIPES", &cfg).is_none(),
        "acceptance must fire case-insensitively (runtime parity)"
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
fn scene_matching_custom_block_is_accepted() {
    // v80.0.0-beta.2 custom-reference parity (owner fatal bug: a config
    // with `scene = hacker-mode` + a matching [scene-custom.hacker-mode]
    // block was rejected by every validation layer — testconf, startup
    // Layer 3, live-reload watcher — while the runtime resolution path
    // accepted it, blocking ALL launches including CLI overrides).
    let mut cfg = std::collections::HashMap::new();
    cfg.insert(
        "scene-custom.hacker-mode.base-scene".to_string(),
        "matrix".to_string(),
    );
    cfg.insert(
        "scene-custom.hacker-mode.speed".to_string(),
        "20".to_string(),
    );
    assert!(
        validate_field_value_with_cfg("scene", "hacker-mode", &cfg).is_none(),
        "scene = <custom scene name> must pass when the block exists (runtime parity)"
    );
    // Case-insensitive, and recognized via ANY declared block field.
    assert!(
        validate_field_value_with_cfg("scene", "HACKER-MODE", &cfg).is_none(),
        "custom scene acceptance must be case-insensitive"
    );
}

#[test]
fn scene_custom_name_without_block_still_rejected() {
    // The name matches no built-in AND no [scene-custom.<name>] block —
    // the plain unknown-scene error must fire (typo protection).
    let mut cfg = std::collections::HashMap::new();
    cfg.insert(
        "scene-custom.other-scene.base-scene".to_string(),
        "matrix".to_string(),
    );
    let msg = validate_field_value_with_cfg("scene", "hacker-mode", &cfg)
        .expect("should error — no matching block or builtin");
    assert!(
        msg.contains("unknown scene"),
        "plain error must be preserved when the block is absent: {msg}"
    );
}

#[test]
fn scene_custom_block_detected_via_any_field() {
    // A block is recognized by any of its declared fields, not just
    // base-scene (mirrors collect_custom_scenes which parses all
    // SCENE_CUSTOM_FIELDS).
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("scene-custom.cp77.density".to_string(), "0.9".to_string());
    assert!(
        validate_field_value_with_cfg("scene", "cp77", &cfg).is_none(),
        "block detected via a non-base-scene field"
    );
}

#[test]
fn base_scene_key_is_rejected_at_parse_level() {
    // v80.0.0-beta.2 (S-master-LOGIC-3): `base-scene` is REMOVED from
    // scene-custom blocks — custom scenes are complete self-contained
    // profiles with no built-in inheritance. The key is now rejected by
    // the parser (unknown key) with a config_hints migration hint; no
    // value validator exists (the key never reaches validation).
    let parsed = crate::configfile::parse_config_text(
        "[scene-custom.hacker-mode]\nbase-scene = \"matrix\"\n",
    );
    assert!(
        parsed
            .unknown_keys
            .iter()
            .any(|k| k == "scene-custom.hacker-mode.base-scene"),
        "base-scene must land in unknown_keys: {:?}",
        parsed.unknown_keys
    );
    // The removed key carries a targeted migration hint.
    let hint = crate::config_hints::suggest_for_unknown_key("scene-custom.hacker-mode.base-scene")
        .expect("base-scene must have a removal hint");
    assert!(
        hint.contains("base-scene field was removed"),
        "hint must explain the removal: {hint}"
    );
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
    // (CLI-D-3): removed `mouse` assertions — mouse is no longer in
    // USER_CONFIG_KEYS (caught as unknown_key upstream). The bool validator
    // arm now only covers `async-mode`.
    assert!(validate_field_value("async-mode", "maybe").is_some());
    assert!(validate_field_value("async-mode", "true").is_none());
}

#[test]
fn block_field_base_uses_scene_validator() {
    // 'base' in scene-custom blocks is validated as a scene name.
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
    assert!(validate_colors_custom_value("colors-custom.mytheme.normal.red", "#ff0000").is_none());
    assert!(
        validate_colors_custom_value("colors-custom.mytheme.normal.red", "\"#ff0000\"").is_none()
    );
}

#[test]
fn colors_custom_value_rejects_invalid_hex() {
    assert!(validate_colors_custom_value("colors-custom.mytheme.normal.red", "#gg0000").is_some());
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

// ── (bug #6): color.tune.* range validation ──
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

/// (bug #17): end-to-end check that `validate_config_strictly`
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

// ── v80.0.0-beta.2 end-to-end custom-name acceptance via the exact
// startup/live-reload/watcher entry point (validate_config_strictly) ──
// These lock the owner's fatal bug at the integration level: the same
// function gates startup Layer 3 and the live-reload watcher, so a pass
// here means `scene = <custom>` / `color = <custom>` configs no longer
// block launches or live edits.

#[test]
fn strict_validation_accepts_scene_referencing_custom_block() {
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("scene".to_string(), "hacker-mode".to_string());
    cfg.insert(
        "scene-custom.hacker-mode.base-scene".to_string(),
        "matrix".to_string(),
    );
    assert!(
        validate_config_strictly(&cfg).is_ok(),
        "scene = <custom scene name> must pass the startup/live-reload gate"
    );
}

#[test]
fn strict_validation_accepts_color_referencing_custom_palette() {
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("color".to_string(), "test".to_string());
    cfg.insert("colors-custom.test.bg".to_string(), "#0a0a0a".to_string());
    cfg.insert(
        "colors-custom.test.rain".to_string(),
        "#1a0033,#ffffff".to_string(),
    );
    assert!(
        validate_config_strictly(&cfg).is_ok(),
        "color = <custom palette name> must pass (charset parity — owner mandate)"
    );
}

#[test]
fn strict_validation_accepts_scene_custom_block_color_custom_palette() {
    // Inside a scene-custom block, `color = <custom palette>` must also
    // pass — the runtime (scene_runtime.rs) resolves it through the
    // custom palette path. v80.0.0-beta.2: the block must be COMPLETE
    // (all six dimensions) — completeness runs first.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert(
        "scene-custom.cp77.color".to_string(),
        "cyberpunk_2077".to_string(),
    );
    cfg.insert(
        "scene-custom.cp77.charset".to_string(),
        "binary".to_string(),
    );
    cfg.insert("scene-custom.cp77.fps".to_string(), "12".to_string());
    cfg.insert("scene-custom.cp77.speed".to_string(), "12".to_string());
    cfg.insert("scene-custom.cp77.density".to_string(), "0.90".to_string());
    cfg.insert(
        "scene-custom.cp77.glitch-level".to_string(),
        "none".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.bg".to_string(),
        "#0a0a12".to_string(),
    );
    cfg.insert(
        "colors-custom.cyberpunk_2077.rain".to_string(),
        "#00fff7,#ff003c".to_string(),
    );
    assert!(
        validate_config_strictly(&cfg).is_ok(),
        "scene-custom block color field must accept custom palette references"
    );
}

#[test]
fn strict_validation_rejects_scene_referencing_missing_block() {
    // Typo protection: no builtin, no matching block → strict rejection.
    let mut cfg = std::collections::HashMap::new();
    cfg.insert("scene".to_string(), "hacker-mdoe".to_string());
    cfg.insert(
        "scene-custom.hacker-mode.base-scene".to_string(),
        "matrix".to_string(),
    );
    let err = validate_config_strictly(&cfg).expect_err("typo'd scene name must fail");
    assert!(
        err.contains("unknown scene"),
        "error must be the plain unknown-scene rejection: {err}"
    );
}
