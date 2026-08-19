// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! scene_custom tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

#[test]
fn scene_custom_keys_are_recognized() {
    // `base-scene` is restored as a recognized scene-custom key.
    // It triggers inheritance from a built-in scene before the custom
    // scene's own overrides are applied. The legacy `preset` field
    // remains removed.
    assert!(is_scene_custom_config_key(
        "scene-custom.hacker-mode.base-scene"
    ));
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.preset"
    ));
    assert!(is_scene_custom_config_key(
        "scene-custom.nightcore.glitch-level"
    ));
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.unknown"
    ));
    assert!(!is_scene_custom_config_key("scene-custom..base"));
    assert!(!is_scene_custom_config_key("profile.nightcore.base"));
}

#[test]
fn collect_custom_scenes_groups_fields_by_name() {
    let cfg = HashMap::from([
        (
            "scene-custom.hacker-mode.color".to_string(),
            "green".to_string(),
        ),
        ("scene-custom.nightcore.speed".to_string(), "24".to_string()),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    assert_eq!(scenes.len(), 2);
    assert_eq!(scenes["hacker-mode"].color.as_deref(), Some("green"));
    assert_eq!(scenes["nightcore"].speed.as_deref(), Some("24"));
}

#[test]
fn collect_custom_scenes_ignores_profile_keys() {
    let cfg = HashMap::from([
        (
            "profile.nightcore.color".to_string(),
            "monolith".to_string(),
        ),
        (
            "scene-custom.nightcore.color".to_string(),
            "purple".to_string(),
        ),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    assert_eq!(scenes.len(), 1);
    assert_eq!(scenes["nightcore"].color.as_deref(), Some("purple"));
}

#[test]
fn validate_custom_scene_name_accepts_valid() {
    assert_eq!(
        validate_custom_scene_name("hacker-mode").unwrap(),
        "hacker-mode"
    );
    assert_eq!(
        validate_custom_scene_name("Nightcore_42").unwrap(),
        "nightcore_42"
    );
}

#[test]
fn validate_custom_scene_name_rejects_invalid() {
    assert!(validate_custom_scene_name("").is_err());
    assert!(validate_custom_scene_name("with space").is_err());
    assert!(validate_custom_scene_name("dot.name").is_err());
}

#[test]
fn scene_custom_namespace_constant_matches_prefix() {
    assert_eq!(SCENE_CUSTOM_NAMESPACE, "scene-custom");
}

// ── rain_style_for_custom_scene ──

#[test]
fn rain_style_for_custom_scene_returns_base_scene_rain_style() {
    // Custom scene with base-scene = monolith → RainStyle::Monolith.
    let cfg = HashMap::from([(
        "scene-custom.afternoon.base-scene".to_string(),
        "monolith".to_string(),
    )]);
    let rs = rain_style_for_custom_scene(&cfg, "afternoon");
    assert_eq!(rs, Some(crate::rain_style::RainStyle::Monolith));
}

#[test]
fn rain_style_for_custom_scene_returns_glyph_for_signal_base() {
    // Custom scene with base-scene = signal → RainStyle::Glyph.
    let cfg = HashMap::from([(
        "scene-custom.afternoon.base-scene".to_string(),
        "signal".to_string(),
    )]);
    let rs = rain_style_for_custom_scene(&cfg, "afternoon");
    assert_eq!(rs, Some(crate::rain_style::RainStyle::Glyph));
}

#[test]
fn rain_style_for_custom_scene_returns_none_when_no_base_scene() {
    // Custom scene with no base-scene → None (caller falls back to Glyph).
    let cfg = HashMap::from([(
        "scene-custom.bare.color".to_string(),
        "neon-green".to_string(),
    )]);
    let rs = rain_style_for_custom_scene(&cfg, "bare");
    assert!(rs.is_none());
}

#[test]
fn rain_style_for_custom_scene_returns_none_for_unknown_custom_name() {
    let cfg = HashMap::new();
    let rs = rain_style_for_custom_scene(&cfg, "nonexistent");
    assert!(rs.is_none());
}

#[test]
fn rain_style_for_custom_scene_returns_none_for_unknown_base_scene() {
    // base-scene = "fake-scene" is not a built-in → None.
    let cfg = HashMap::from([(
        "scene-custom.broken.base-scene".to_string(),
        "fake-scene".to_string(),
    )]);
    let rs = rain_style_for_custom_scene(&cfg, "broken");
    assert!(rs.is_none());
}

#[test]
fn rain_style_for_custom_scene_is_case_insensitive_on_custom_name() {
    // Custom scene names are stored lowercase by collect_custom_scenes;
    // rain_style_for_custom_scene normalizes its input to match.
    let cfg = HashMap::from([(
        "scene-custom.afternoon.base-scene".to_string(),
        "monolith".to_string(),
    )]);
    let rs = rain_style_for_custom_scene(&cfg, "AFTERNOON");
    assert_eq!(rs, Some(crate::rain_style::RainStyle::Monolith));
}

// Note: live-reload path (`apply_base_scene_to_cloud_config`) is exercised
// end-to-end by the `rebuild_cloud_config` integration path. Unit-testing
// it in isolation requires constructing a full CloudConfig (40+ fields),
// which is brittle. The startup apply path (`apply_profile_layer` with
// base-scene) is unit-tested in `config_apply_tests/profiles.rs::profile_base_scene_applies_inherited_defaults`,
// and the runtime apply path (`Cloud::apply_ambient_entry` with a custom
// scene) is unit-tested in `cloud/tests/tests_scene/transitions.rs`.

#[test]
fn profile_fields_are_reusable_for_custom_scenes() {
    // `base-scene` is restored to PROFILE_FIELDS (with cleaner
    // inheritance semantics — see profile.rs). `preset` remains removed.
    assert!(PROFILE_FIELDS.contains(&"base-scene"));
    assert!(!PROFILE_FIELDS.contains(&"preset"));
    assert!(PROFILE_FIELDS.contains(&"color"));
    // Atmosphere engine eliminated — atmosphere-regime is no longer a
    // valid profile field.
    assert!(!PROFILE_FIELDS.contains(&"atmosphere-regime"));
    assert!(!PROFILE_FIELDS.contains(&"atmosphere-mode"));
    assert!(!PROFILE_FIELDS.contains(&"nonexistent-field"));
}

#[test]
fn list_custom_scenes_text_shows_base_annotation_when_set() {
    // when a custom scene sets `base-scene`, the listing
    // annotates it as `name (base: <base-scene>)`. Custom scenes
    // without `base-scene` render as just `name`.
    let cfg = HashMap::from([
        (
            "scene-custom.alpha.base-scene".to_string(),
            "signal".to_string(),
        ),
        ("scene-custom.alpha.color".to_string(), "storm".to_string()),
        ("scene-custom.beta.color".to_string(), "neon".to_string()),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let text = list_custom_scenes_text(&scenes);
    assert!(text.contains("alpha"), "list must include alpha: {text}");
    assert!(
        text.contains("alpha (base: signal)"),
        "alpha should show base annotation: {text}"
    );
    assert!(
        !text.contains("beta (base:"),
        "beta has no base-scene — should NOT show annotation: {text}"
    );
    assert!(text.contains("beta"), "list must include beta: {text}");
}

#[test]
fn show_custom_scene_text_includes_fields_and_usage() {
    let cfg = HashMap::from([
        (
            "scene-custom.hacker-mode.base-scene".to_string(),
            "monolith".to_string(),
        ),
        (
            "scene-custom.hacker-mode.color".to_string(),
            "green".to_string(),
        ),
        (
            "scene-custom.hacker-mode.speed".to_string(),
            "24".to_string(),
        ),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let scene = &scenes["hacker-mode"];
    let text = show_custom_scene_text("hacker-mode", scene);
    assert!(
        text.contains("CUSTOM SCENE: hacker-mode"),
        "header missing: {text}"
    );
    assert!(
        text.contains("base-scene          = monolith"),
        "base-scene field missing: {text}"
    );
    assert!(
        text.contains("color              = green"),
        "color field missing: {text}"
    );
    assert!(
        text.contains("speed              = 24"),
        "speed field missing: {text}"
    );
    assert!(
        text.contains("cosmostrix --scene-custom hacker-mode"),
        "usage hint missing: {text}"
    );
}

#[test]
fn show_custom_scene_text_handles_empty_profile() {
    let scene = UserProfile::default();
    let text = show_custom_scene_text("empty", &scene);
    assert!(
        text.contains("no fields set"),
        "empty profile should mention inheritance: {text}"
    );
}

// ── parse_density_map tests ──

#[test]
fn parse_density_map_valid_csv() {
    let map = parse_density_map("1.0,0.5,0.0,0.8");
    assert!(map.is_some());
    let map = map.unwrap();
    assert_eq!(map.len(), 4);
    assert_eq!(map[0], 1.0);
    assert_eq!(map[1], 0.5);
    assert_eq!(map[2], 0.0);
    assert_eq!(map[3], 0.8);
}

#[test]
fn parse_density_map_clamps_out_of_range() {
    let map = parse_density_map("1.5,-0.3,2.0").unwrap();
    assert_eq!(map[0], 1.0); // 1.5 clamped to 1.0
    assert_eq!(map[1], 0.0); // -0.3 clamped to 0.0
    assert_eq!(map[2], 1.0); // 2.0 clamped to 1.0
}

#[test]
fn parse_density_map_skips_empty_and_whitespace() {
    let map = parse_density_map("1.0, , 0.5 ,, 0.0");
    assert!(map.is_some());
    assert_eq!(map.unwrap().len(), 3);
}

#[test]
fn parse_density_map_empty_string_returns_none() {
    assert!(parse_density_map("").is_none());
    assert!(parse_density_map("   ").is_none());
}

#[test]
fn parse_density_map_invalid_numbers_return_none() {
    assert!(parse_density_map("abc,def").is_none());
    assert!(parse_density_map("not_a_number").is_none());
}

#[test]
fn parse_density_map_single_value() {
    let map = parse_density_map("0.7");
    assert!(map.is_some());
    assert_eq!(map.unwrap(), &[0.7]);
}

#[test]
fn parse_density_map_mixed_valid_invalid() {
    // Valid numbers are kept; invalid entries are skipped.
    let map = parse_density_map("1.0,abc,0.5");
    assert!(map.is_some());
    assert_eq!(map.unwrap(), &[1.0, 0.5]);
}

// v30 fix: quoted CSV strings must work. The configfile parser is a
// custom line-by-line parser that does NOT strip surrounding quotes
// from string values, so the leaf parser must do it. Without this,
// `density-map = "0.05,0.3,1.0"` would parse `"0.05` as the first
// entry (not a float) and silently produce None at runtime while
// also failing --testconf.
#[test]
fn parse_density_map_accepts_double_quoted_csv() {
    let map = parse_density_map("\"0.05,0.3,1.0\"");
    assert!(map.is_some());
    assert_eq!(map.unwrap(), &[0.05, 0.3, 1.0]);
}

#[test]
fn parse_density_map_accepts_single_quoted_csv() {
    let map = parse_density_map("'0.1, 0.2, 0.3'");
    assert!(map.is_some());
    assert_eq!(map.unwrap(), &[0.1, 0.2, 0.3]);
}

#[test]
fn parse_density_map_accepts_quoted_with_whitespace_padding() {
    // User wrote `density-map = " 0.5, 0.5 "` — quotes + outer spaces.
    let map = parse_density_map("  \"0.5,0.5\"  ");
    assert!(map.is_some());
    assert_eq!(map.unwrap(), &[0.5, 0.5]);
}

#[test]
fn parse_density_map_quoted_and_unquoted_share_cache_entry() {
    // Both forms normalize to the same key `"0.5,0.5"` → 0.5,0.5,
    // so the dedup cache should return the same slice pointer.
    let a = parse_density_map("0.5,0.5").unwrap();
    let b = parse_density_map("\"0.5,0.5\"").unwrap();
    assert!(
        std::ptr::eq(a.as_ptr(), b.as_ptr()),
        "quoted and unquoted forms should share the same cached slice"
    );
}

#[test]
fn parse_density_map_quoted_empty_string_returns_none() {
    assert!(parse_density_map("\"\"").is_none());
    assert!(parse_density_map("''").is_none());
}

// ── scene-custom field allowlist / forbidden-field tests ──

#[test]
fn scene_custom_fields_includes_v30_3_additions() {
    // Owner contract: these MUST be accepted in scene-custom blocks.
    for field in &[
        "base-scene",
        "color",
        "charset",
        "bold",
        "colors-custom",
        "charset-custom",
        "shadingmode",
        "glitch-level",
        "fps",
        "speed",
        "density",
        "density-map",
        "async-mode",
    ] {
        assert!(
            SCENE_CUSTOM_FIELDS.contains(field),
            "SCENE_CUSTOM_FIELDS must include '{field}'"
        );
    }
}

#[test]
fn scene_custom_fields_excludes_forbidden_fields() {
    // Owner contract: these MUST NOT be accepted in scene-custom blocks.
    for field in &[
        "monolith-size",
        "color-bg",
        "ambient",
        "crystal-dragon",
        "intro",
    ] {
        assert!(
            !SCENE_CUSTOM_FIELDS.contains(field),
            "SCENE_CUSTOM_FIELDS must NOT include '{field}' (forbidden per owner contract)"
        );
    }
}

#[test]
fn is_scene_custom_config_key_accepts_v30_3_fields() {
    for field in &[
        "bold",
        "colors-custom",
        "charset-custom",
        "shadingmode",
        "async-mode",
    ] {
        let key = format!("scene-custom.test.{field}");
        assert!(
            is_scene_custom_config_key(&key),
            "is_scene_custom_config_key should accept '{key}'"
        );
    }
}

#[test]
fn is_scene_custom_config_key_rejects_forbidden_fields() {
    // monolith-size and color-bg were accepted — they
    // MUST now be rejected per owner contract.
    for field in &[
        "monolith-size",
        "color-bg",
        "ambient",
        "crystal-dragon",
        "intro",
    ] {
        let key = format!("scene-custom.test.{field}");
        assert!(
            !is_scene_custom_config_key(&key),
            "is_scene_custom_config_key must REJECT '{key}' (forbidden)"
        );
    }
}

#[test]
fn collect_custom_scenes_parses_v30_3_fields() {
    let cfg = HashMap::from([
        ("scene-custom.test.bold".to_string(), "1".to_string()),
        (
            "scene-custom.test.colors-custom".to_string(),
            "sunset".to_string(),
        ),
        (
            "scene-custom.test.charset-custom".to_string(),
            "zen".to_string(),
        ),
        ("scene-custom.test.shadingmode".to_string(), "1".to_string()),
        (
            "scene-custom.test.async-mode".to_string(),
            "true".to_string(),
        ),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let scene = &scenes["test"];
    assert_eq!(scene.bold.as_deref(), Some("1"));
    assert_eq!(scene.colors_custom.as_deref(), Some("sunset"));
    assert_eq!(scene.charset_custom.as_deref(), Some("zen"));
    assert_eq!(scene.shading_mode.as_deref(), Some("1"));
    assert_eq!(scene.async_mode.as_deref(), Some("true"));
}

#[test]
fn collect_custom_scenes_silently_drops_forbidden_fields() {
    // monolith-size and color-bg are filtered out by
    // is_scene_custom_config_key, so collect_custom_scenes never sees
    // them. Verify they don't appear in the parsed UserProfile.
    let cfg = HashMap::from([
        ("scene-custom.test.color".to_string(), "green".to_string()),
        (
            "scene-custom.test.monolith-size".to_string(),
            "large".to_string(),
        ),
        (
            "scene-custom.test.color-bg".to_string(),
            "black".to_string(),
        ),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let scene = &scenes["test"];
    assert_eq!(scene.color.as_deref(), Some("green"));
    // monolith_size and color_bg are NOT set (keys were filtered out).
    assert!(scene.monolith_size.is_none());
    assert!(scene.color_bg.is_none());
}
use super::*;
