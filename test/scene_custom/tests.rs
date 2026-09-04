// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! scene_custom tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.
//!
//! v80.0.0-beta.2 (S-master-LOGIC-3): the schema is six required
//! dimensions (color|colors-custom, charset|charset-custom, fps, speed,
//! density, glitch-level); base-scene/bold/shading-mode/async-mode are
//! removed. These tests pin the new contract.

#[test]
fn scene_custom_keys_are_recognized() {
    // v80.0.0-beta.2: `base-scene` is REMOVED — custom scenes are complete
    // self-contained profiles with no built-in inheritance. The legacy
    // `preset` field remains removed.
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.base-scene"
    ));
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.preset"
    ));
    assert!(is_scene_custom_config_key(
        "scene-custom.nightcore.glitch-level"
    ));
    assert!(is_scene_custom_config_key(
        "scene-custom.nightcore.colors-custom"
    ));
    assert!(is_scene_custom_config_key(
        "scene-custom.nightcore.charset-custom"
    ));
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.unknown"
    ));
    assert!(!is_scene_custom_config_key("scene-custom..base"));
    assert!(!is_scene_custom_config_key("profile.nightcore.base"));
    // v80.0.0-beta.2 removal lock: density-map is no longer a valid
    // scene-custom field (the burden function was retired).
    assert!(!is_scene_custom_config_key(
        "scene-custom.hacker-mode.density-map"
    ));
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

// ── resolve_rain_style (v80.0.0-beta.2: custom scenes are always Glyph) ──

#[test]
fn resolve_rain_style_builtin_scene_returns_its_rain_style() {
    // Monolith is the canonical Monolith-rain built-in.
    let cfg = HashMap::new();
    let rs = resolve_rain_style(Some("monolith"), &cfg);
    assert_eq!(rs, crate::rain_style::RainStyle::Monolith);
}

#[test]
fn resolve_rain_style_custom_scene_is_always_glyph() {
    // v80.0.0-beta.2: base-scene inheritance removed — a custom scene
    // (even one whose legacy config still carries a base-scene key,
    // which the parser now rejects) resolves to Glyph rain.
    let cfg = HashMap::from([
        (
            "scene-custom.afternoon.color".to_string(),
            "green".to_string(),
        ),
        (
            "scene-custom.afternoon.base-scene".to_string(),
            "monolith".to_string(),
        ),
    ]);
    let rs = resolve_rain_style(Some("afternoon"), &cfg);
    assert_eq!(rs, crate::rain_style::RainStyle::Glyph);
}

#[test]
fn resolve_rain_style_none_name_defaults_to_glyph() {
    let cfg = HashMap::new();
    let rs = resolve_rain_style(None, &cfg);
    assert_eq!(rs, crate::rain_style::RainStyle::Glyph);
}

#[test]
fn resolve_rain_style_unknown_name_defaults_to_glyph() {
    let cfg = HashMap::new();
    let rs = resolve_rain_style(Some("no-such-scene"), &cfg);
    assert_eq!(rs, crate::rain_style::RainStyle::Glyph);
}

#[test]
fn override_fields_match_v80_beta2_schema() {
    // v80.0.0-beta.2 (S-master-LOGIC-3): exactly the six scene-family
    // dimensions. base-scene/bold/shading-mode/async-mode/monolith-size/
    // color-bg are REMOVED; `preset` remains removed.
    for field in &[
        "color",
        "colors-custom",
        "charset",
        "charset-custom",
        "fps",
        "speed",
        "density",
        "glitch-level",
    ] {
        assert!(
            PROFILE_FIELDS.contains(field),
            "PROFILE_FIELDS must include '{field}'"
        );
        assert!(
            SCENE_CUSTOM_FIELDS.contains(field),
            "SCENE_CUSTOM_FIELDS must include '{field}'"
        );
    }
    for removed in &[
        "base-scene",
        "bold",
        "shading-mode",
        "async-mode",
        "monolith-size",
        "color-bg",
        "preset",
        "atmosphere-regime",
        "nonexistent-field",
    ] {
        assert!(
            !PROFILE_FIELDS.contains(removed),
            "PROFILE_FIELDS must NOT include '{removed}' (removed in v80.0.0-beta.2)"
        );
        assert!(
            !SCENE_CUSTOM_FIELDS.contains(removed),
            "SCENE_CUSTOM_FIELDS must NOT include '{removed}'"
        );
    }
}

#[test]
fn list_custom_scenes_text_renders_plain_names() {
    // v80.0.0-beta.2: no base-scene annotation — custom scenes are
    // self-contained. Every entry renders as just `name`.
    let cfg = HashMap::from([
        ("scene-custom.alpha.color".to_string(), "storm".to_string()),
        ("scene-custom.beta.color".to_string(), "neon".to_string()),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let text = list_custom_scenes_text(&scenes);
    assert!(text.contains("alpha"), "list must include alpha: {text}");
    assert!(text.contains("beta"), "list must include beta: {text}");
    assert!(
        !text.contains("(base:"),
        "no base annotation may render post-removal: {text}"
    );
}

#[test]
fn show_custom_scene_text_includes_fields_and_usage() {
    let cfg = HashMap::from([
        (
            "scene-custom.hacker-mode.color".to_string(),
            "green".to_string(),
        ),
        (
            "scene-custom.hacker-mode.charset".to_string(),
            "hacker".to_string(),
        ),
        ("scene-custom.hacker-mode.fps".to_string(), "60".to_string()),
        (
            "scene-custom.hacker-mode.speed".to_string(),
            "24".to_string(),
        ),
        (
            "scene-custom.hacker-mode.density".to_string(),
            "1.2".to_string(),
        ),
        (
            "scene-custom.hacker-mode.glitch-level".to_string(),
            "intense".to_string(),
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
        text.contains("color              = green"),
        "color field missing: {text}"
    );
    assert!(
        text.contains("charset            = hacker"),
        "charset field missing: {text}"
    );
    assert!(
        text.contains("speed              = 24"),
        "speed field missing: {text}"
    );
    assert!(
        text.contains("cosmostrix --scene-custom hacker-mode"),
        "usage hint missing: {text}"
    );
    // Complete block — no incompleteness warning may render.
    assert!(
        !text.contains("incomplete"),
        "complete block must not warn: {text}"
    );
}

#[test]
fn show_custom_scene_text_handles_empty_scene() {
    let scene = UserProfile::default();
    let text = show_custom_scene_text("empty", &scene);
    assert!(
        text.contains("no fields set"),
        "empty scene must say so: {text}"
    );
    // v80.0.0-beta.2: an empty block is INCOMPLETE — the display warns
    // with the missing-dimension list (completeness is a hard contract).
    assert!(
        text.contains("incomplete"),
        "empty scene must warn incomplete: {text}"
    );
}

#[test]
fn show_custom_scene_text_warns_on_partial_block() {
    let cfg = HashMap::from([(
        "scene-custom.partial.color".to_string(),
        "green".to_string(),
    )]);
    let scenes = collect_custom_scenes(&cfg);
    let text = show_custom_scene_text("partial", &scenes["partial"]);
    assert!(
        text.contains("incomplete"),
        "partial block must warn: {text}"
    );
    assert!(
        text.contains("missing charset|charset-custom"),
        "warning must name the missing pair: {text}"
    );
}

// ── scene-custom field allowlist / forbidden-field tests ──

#[test]
fn is_scene_custom_config_key_rejects_removed_style_fields() {
    // v80.0.0-beta.2 (S-master-LOGIC-3): base-scene/bold/shading-mode/
    // async-mode are REMOVED from the schema — the parser rejects the
    // keys (unknown key + config_hints migration hint).
    for field in &[
        "base-scene",
        "bold",
        "shading-mode",
        "async-mode",
        "monolith-size",
        "color-bg",
        "ambient",
        "crystal-dragon",
        "intro",
        "density-map",
    ] {
        let key = format!("scene-custom.test.{field}");
        assert!(
            !is_scene_custom_config_key(&key),
            "is_scene_custom_config_key must REJECT '{key}' (removed/forbidden in v80.0.0-beta.2)"
        );
    }
}

#[test]
fn collect_custom_scenes_silently_drops_removed_fields() {
    // Removed fields are filtered out by is_scene_custom_config_key, so
    // collect_custom_scenes never sees them — the six surviving
    // dimensions parse normally.
    let cfg = HashMap::from([
        ("scene-custom.test.color".to_string(), "green".to_string()),
        ("scene-custom.test.bold".to_string(), "1".to_string()),
        (
            "scene-custom.test.shading-mode".to_string(),
            "1".to_string(),
        ),
        (
            "scene-custom.test.async-mode".to_string(),
            "true".to_string(),
        ),
    ]);
    let scenes = collect_custom_scenes(&cfg);
    let scene = &scenes["test"];
    assert_eq!(scene.color.as_deref(), Some("green"));
    // The removed fields are not representable on UserProfile anymore
    // (compile-time guarantee) — the block counts as having ONLY color.
    let missing = missing_scene_custom_fields(scene);
    assert!(
        missing.contains(&"charset|charset-custom".to_string()),
        "charset pair must be missing: {missing:?}"
    );
}

// ── v80.0.0-beta.2 (S-master-LOGIC-3): completeness validation ──

#[test]
fn completeness_validation_accepts_a_complete_block() {
    let cfg = HashMap::from([
        ("scene-custom.full.color".to_string(), "green".to_string()),
        (
            "scene-custom.full.charset".to_string(),
            "hacker".to_string(),
        ),
        ("scene-custom.full.fps".to_string(), "90".to_string()),
        ("scene-custom.full.speed".to_string(), "12".to_string()),
        ("scene-custom.full.density".to_string(), "0.90".to_string()),
        (
            "scene-custom.full.glitch-level".to_string(),
            "none".to_string(),
        ),
    ]);
    assert!(
        validate_scene_custom_completeness(&cfg).is_ok(),
        "a completely filled block must pass"
    );
}

#[test]
fn completeness_validation_accepts_alternative_pair_keys() {
    // colors-custom / charset-custom satisfy the pair dimensions too.
    let cfg = HashMap::from([
        (
            "scene-custom.full.colors-custom".to_string(),
            "cyberpunk_2077".to_string(),
        ),
        (
            "scene-custom.full.charset-custom".to_string(),
            "cyberpunk_2077".to_string(),
        ),
        ("scene-custom.full.fps".to_string(), "12".to_string()),
        ("scene-custom.full.speed".to_string(), "12".to_string()),
        ("scene-custom.full.density".to_string(), "0.90".to_string()),
        (
            "scene-custom.full.glitch-level".to_string(),
            "none".to_string(),
        ),
    ]);
    assert!(
        validate_scene_custom_completeness(&cfg).is_ok(),
        "pair-alternative keys must satisfy completeness"
    );
}

#[test]
fn completeness_validation_rejects_a_partial_block() {
    // Only color + speed — the other four dimensions are missing.
    let cfg = HashMap::from([
        (
            "scene-custom.partial.color".to_string(),
            "green".to_string(),
        ),
        ("scene-custom.partial.speed".to_string(), "12".to_string()),
    ]);
    let err = validate_scene_custom_completeness(&cfg).unwrap_err();
    assert!(
        err.contains("scene-custom 'partial' is incomplete"),
        "error must name the block: {err}"
    );
    assert!(
        err.contains("missing charset|charset-custom, fps, density, glitch-level"),
        "error must list the missing dimensions: {err}"
    );
}

#[test]
fn completeness_validation_reports_every_missing_dimension() {
    let cfg = HashMap::from([("scene-custom.bare.fps".to_string(), "60".to_string())]);
    let err = validate_scene_custom_completeness(&cfg).unwrap_err();
    for missing in &[
        "color|colors-custom",
        "charset|charset-custom",
        "speed",
        "density",
        "glitch-level",
    ] {
        assert!(err.contains(missing), "error must list '{missing}': {err}");
    }
    assert!(
        !err.contains("missing fps"),
        "fps IS set — must not be reported in the missing list: {err}"
    );
}

#[test]
fn completeness_validation_ok_when_no_blocks_exist() {
    assert!(validate_scene_custom_completeness(&HashMap::new()).is_ok());
}

#[test]
fn required_fields_hint_renders_the_six_dimensions() {
    let hint = scene_custom_required_fields_hint();
    assert!(
        hint.contains("color|colors-custom")
            && hint.contains("charset|charset-custom")
            && hint.contains("fps")
            && hint.contains("speed")
            && hint.contains("density")
            && hint.contains("glitch-level"),
        "hint must render all six dimensions: {hint}"
    );
}

// ── ambient_scene_fps (v80.0.0-beta.2: ambient scene owns fps) ──

#[test]
fn ambient_scene_fps_resolves_builtin_scene_default() {
    // storm declares fps 120 — a built-in ambient entry owns that fps.
    let fps = ambient_scene_fps("storm", &HashMap::new());
    assert_eq!(fps, Some(120.0));
}

#[test]
fn ambient_scene_fps_resolves_custom_block_field() {
    let cfg = HashMap::from([("scene-custom.cp77.fps".to_string(), "12".to_string())]);
    let fps = ambient_scene_fps("cp77", &cfg);
    assert_eq!(fps, Some(12.0));
}

#[test]
fn ambient_scene_fps_none_when_scene_declares_no_fps() {
    // A custom block without fps, or an unknown scene → None (leave
    // the current target untouched).
    let cfg = HashMap::from([("scene-custom.bare.color".to_string(), "green".to_string())]);
    assert_eq!(ambient_scene_fps("bare", &cfg), None);
    assert_eq!(ambient_scene_fps("no-such-scene", &cfg), None);
}

#[test]
fn ambient_scene_fps_rejects_out_of_range_block_fps() {
    // Defense-in-depth: strict validation rejects the config upstream,
    // but the resolver still range-gates so a hand-built map cannot
    // sneak an out-of-range target into the power manager.
    let cfg = HashMap::from([("scene-custom.broken.fps".to_string(), "999".to_string())]);
    assert_eq!(ambient_scene_fps("broken", &cfg), None);
}

#[test]
fn ambient_scene_fps_is_case_insensitive_on_custom_name() {
    let cfg = HashMap::from([("scene-custom.cp77.fps".to_string(), "12".to_string())]);
    assert_eq!(ambient_scene_fps("CP77", &cfg), Some(12.0));
}

// ── v50.0.0-beta.6 LTS: bounds enforcement tests ─────────────────

#[test]
fn collect_custom_scenes_caps_total_blocks_at_max() {
    // A config with >SCENE_CUSTOM_MAX_BLOCKS blocks should only
    // keep the first MAX_BLOCKS entries, not allocate unbounded.
    let mut cfg = HashMap::new();
    for i in 0..(SCENE_CUSTOM_MAX_BLOCKS + 10) {
        cfg.insert(format!("scene-custom.scene{i}.color"), "green".to_string());
    }
    let scenes = collect_custom_scenes(&cfg);
    assert!(
        scenes.len() <= SCENE_CUSTOM_MAX_BLOCKS,
        "total blocks must be capped at {}, got {}",
        SCENE_CUSTOM_MAX_BLOCKS,
        scenes.len()
    );
}

#[test]
fn collect_custom_scenes_skips_oversized_names() {
    // A name longer than SCENE_CUSTOM_MAX_NAME_LEN should be
    // silently skipped (no allocation, no BTreeMap entry).
    let mut cfg = HashMap::new();
    let long_name = "x".repeat(SCENE_CUSTOM_MAX_NAME_LEN + 1);
    cfg.insert(
        format!("scene-custom.{long_name}.color"),
        "green".to_string(),
    );
    let scenes = collect_custom_scenes(&cfg);
    assert!(
        scenes.is_empty(),
        "oversized name must be skipped, got {} entries",
        scenes.len()
    );
}
use super::*;

// ── v80.0.0-beta.2 display hardening ─────────────────────────────────────

#[test]
fn show_custom_scene_text_never_renders_removed_fields() {
    // The removed style fields cannot be represented on UserProfile
    // (compile-time guarantee); the display must not render them even
    // via field values smuggled through the six surviving dimensions.
    let scene = UserProfile {
        color: Some("green".to_string()),
        charset: Some("hacker".to_string()),
        fps: Some("60".to_string()),
        speed: Some("24".to_string()),
        density: Some("1.2".to_string()),
        glitch_level: Some("intense".to_string()),
        colors_custom: None,
        charset_custom: None,
    };
    let text = show_custom_scene_text("test-scene", &scene);
    for banned in &[
        "base-scene",
        "monolith-size",
        "color-bg",
        "bold",
        "shading-mode",
        "async-mode",
    ] {
        assert!(
            !text.contains(banned),
            "removed field '{banned}' must not render: {text}"
        );
    }
}
