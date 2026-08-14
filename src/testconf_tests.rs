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
    // (CLI-D-3): removed `mouse` assertions — mouse is no longer in
    // USER_CONFIG_KEYS (caught as unknown_key upstream). The bool validator
    // arm now only covers `auto-color-drift` and `async-mode`.
    assert!(validate_field_value("auto-color-drift", "maybe").is_some());
    assert!(validate_field_value("auto-color-drift", "true").is_none());
    assert!(validate_field_value("auto-color-drift", "yes").is_none());
    assert!(validate_field_value("auto-color-drift", "on").is_none());
    assert!(validate_field_value("auto-color-drift", "1").is_none());
    assert!(validate_field_value("auto-color-drift", "false").is_none());
    assert!(validate_field_value("auto-color-drift", "no").is_none());
    assert!(validate_field_value("auto-color-drift", "off").is_none());
    assert!(validate_field_value("auto-color-drift", "0").is_none());
    assert!(validate_field_value("auto-color-drift", "YES").is_none()); // case-insensitive
    assert!(validate_field_value("async-mode", "maybe").is_some());
    assert!(validate_field_value("async-mode", "true").is_none());
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
